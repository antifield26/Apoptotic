//! 世界保存 — 使用 Anvil 格式保存区块

use mc_world::chunk::Chunk;
use mc_world::world::World;
use std::fs;
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use tracing::{debug, info, warn};

type MetadataResult = (String, u64, i64, (i32, i32, i32));

/// 世界保存器
pub struct WorldSaver;

impl WorldSaver {
    pub fn new() -> Self {
        Self
    }

    /// 保存世界中所有的 dirty 区块为 Anvil (.mca) 格式
    /// 返回保存的区块数量
    pub fn save_world(&self, world: &World, base_path: &str) -> Result<usize, String> {
        let path = Path::new(base_path);
        fs::create_dir_all(path).map_err(|e| format!("mkdir: {}", e))?;

        // 保存世界元数据
        self.save_metadata(world, &path.join("level.dat"))?;

        // 保存所有 dirty 区块为 Anvil 格式
        let region_dir = path.join("region");
        fs::create_dir_all(&region_dir).ok();

        let dirty: Vec<(mc_core::position::ChunkPos, Chunk)> = world.chunks.dirty_chunks();
        if dirty.is_empty() {
            return Ok(0);
        }

        let chunks: Vec<Chunk> = dirty.into_iter().map(|(_, c)| c).collect();
        let writer = mc_world::anvil::AnvilWriter::new();
        match writer.write_chunks(&region_dir, &chunks) {
            Ok(n) => {
                info!("Saved {} chunks to {}", n, region_dir.display());
                Ok(n)
            }
            Err(e) => Err(format!("Anvil write error: {}", e)),
        }
    }

    /// 保存世界元数据 (标准 NBT 格式, GZip 压缩)
    fn save_metadata(&self, world: &World, path: &Path) -> Result<(), String> {
        use std::io::Write;

        // Build NBT as a Value tree (matches vanilla level.dat structure)
        let mut data = fastnbt::Value::Compound(std::collections::HashMap::new());
        if let fastnbt::Value::Compound(ref mut map) = data {
            map.insert("LevelName".into(), fastnbt::Value::String(world.level_name.clone()));
            map.insert("RandomSeed".into(), fastnbt::Value::Long(world.seed as i64));
            map.insert("Time".into(), fastnbt::Value::Long(world.time as i64));
            map.insert("SpawnX".into(), fastnbt::Value::Int(world.spawn_position.x));
            map.insert("SpawnY".into(), fastnbt::Value::Int(world.spawn_position.y));
            map.insert("SpawnZ".into(), fastnbt::Value::Int(world.spawn_position.z));
            map.insert("GameType".into(), fastnbt::Value::Int(0));
            map.insert("Difficulty".into(), fastnbt::Value::Byte(1));
            map.insert("version".into(), fastnbt::Value::Int(19133)); // 1.21 DataVersion
        }

        // Wrap in named root "Data" compound (vanilla level.dat root structure)
        let mut root = std::collections::HashMap::new();
        root.insert("Data".into(), data);
        let root_compound = fastnbt::Value::Compound(root);

        // Serialize to uncompressed NBT bytes
        let mut nbt_buf = Vec::new();
        fastnbt::to_writer(&mut nbt_buf, &root_compound)
            .map_err(|e| format!("NBT serialize: {}", e))?;

        // GZip compress (Minecraft level.dat is gzip'd NBT)
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&nbt_buf).map_err(|e| format!("gzip: {}", e))?;
        let compressed = encoder.finish().map_err(|e| format!("gzip finish: {}", e))?;

        fs::write(path, compressed).map_err(|e| format!("write metadata: {}", e))?;
        debug!("Saved world metadata (NBT) to {}", path.display());
        Ok(())
    }

    /// 加载世界元数据 (标准 NBT 格式, GZip 压缩)
    /// Falls back to legacy plaintext format for backward compatibility
    #[allow(dead_code)]
    fn load_metadata(&self, path: &Path) -> Result<MetadataResult, String> {
        use flate2::read::GzDecoder;
        use serde::Deserialize;
        use std::io::Read;

        let data = fs::read(path).map_err(|e| format!("read metadata: {}", e))?;
        if data.is_empty() { return Err("empty metadata file".into()); }

        // Try NBT format first (data starts with gzip magic 0x1F 0x8B)
        if data.len() >= 2 && data[0] == 0x1F && data[1] == 0x8B {
            #[derive(Deserialize)]
            struct LevelDataInner {
                #[serde(rename = "LevelName")]
                level_name: String,
                #[serde(rename = "RandomSeed")]
                random_seed: i64,
                #[serde(rename = "Time")]
                time: i64,
                #[serde(rename = "SpawnX")]
                spawn_x: i32,
                #[serde(rename = "SpawnY")]
                spawn_y: i32,
                #[serde(rename = "SpawnZ")]
                spawn_z: i32,
            }
            #[derive(Deserialize)]
            struct LevelData { #[serde(rename = "Data")] data: LevelDataInner }

            let mut decoder = GzDecoder::new(&data[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed).map_err(|e| format!("gunzip: {}", e))?;

            let root: LevelData = fastnbt::from_bytes(&decompressed)
                .map_err(|e| format!("NBT deserialize: {}", e))?;
            return Ok((root.data.level_name, root.data.random_seed as u64,
                root.data.time, (root.data.spawn_x, root.data.spawn_y, root.data.spawn_z)));
        }

        // Fallback: legacy plaintext format
        let text = String::from_utf8_lossy(&data);
        let mut name = String::from("world");
        let mut seed = 0u64;
        let mut time = 0i64;
        let mut spawn = (0i32, 64i32, 0i32);
        for line in text.lines() {
            if let Some((k, v)) = line.split_once(':') {
                match k {
                    "LevelName" => name = v.to_string(),
                    "Seed" => seed = v.parse().unwrap_or(0),
                    "Time" => time = v.parse().unwrap_or(0),
                    "SpawnX" => spawn.0 = v.parse().unwrap_or(0),
                    "SpawnY" => spawn.1 = v.parse().unwrap_or(64),
                    "SpawnZ" => spawn.2 = v.parse().unwrap_or(0),
                    _ => {}
                }
            }
        }
        Ok((name, seed, time, spawn))
    }

    /// 保存单个区块 (二进制 .chunk 格式, 备用于 Anvil 格式)
    #[allow(dead_code)]
    fn save_chunk(&self, chunk: &Chunk, region_dir: &Path) -> Result<(), String> {
        let filename = format!("c.{}.{}.chunk", chunk.position.x, chunk.position.z);
        let path = region_dir.join(&filename);

        let file = fs::File::create(&path).map_err(|e| format!("create file: {}", e))?;
        let mut writer = BufWriter::new(file);

        // Header
        writer
            .write_all(&chunk.position.x.to_be_bytes())
            .map_err(|e| format!("write: {}", e))?;
        writer
            .write_all(&chunk.position.z.to_be_bytes())
            .map_err(|e| format!("write: {}", e))?;

        // Count non-empty sections
        let non_empty: Vec<_> = chunk
            .sections
            .iter()
            .enumerate()
            .filter(|(_, s)| s.is_some())
            .collect();

        writer
            .write_all(&(non_empty.len() as u32).to_be_bytes())
            .map_err(|e| format!("write: {}", e))?;

        for (idx, section) in &non_empty {
            let section = section.as_ref().unwrap();
            let section_y =
                *idx as i32 + mc_core::position::MIN_SECTION_Y;

            writer
                .write_all(&section_y.to_be_bytes())
                .map_err(|e| format!("write: {}", e))?;

            // Block palette data
            let blocks = section.blocks.encode_network();
            writer
                .write_all(&(blocks.len() as u32).to_be_bytes())
                .map_err(|e| format!("write: {}", e))?;
            writer.write_all(&blocks).map_err(|e| format!("write: {}", e))?;

            // Biome palette data
            let biomes = section.biomes.encode_network();
            writer
                .write_all(&(biomes.len() as u32).to_be_bytes())
                .map_err(|e| format!("write: {}", e))?;
            writer.write_all(&biomes).map_err(|e| format!("write: {}", e))?;
        }

        writer.flush().map_err(|e| format!("flush: {}", e))?;
        Ok(())
    }

    /// 从磁盘加载区块
    pub fn load_chunk(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        region_dir: &Path,
    ) -> Result<Option<Chunk>, String> {
        let filename = format!("c.{}.{}.chunk", chunk_x, chunk_z);
        let path = region_dir.join(&filename);

        if !path.exists() {
            return Ok(None);
        }

        let mut file = fs::File::open(&path).map_err(|e| format!("open: {}", e))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| format!("read: {}", e))?;

        if data.len() < 12 {
            return Err("chunk file too short".into());
        }

        // Read header
        let cx = i32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let cz = i32::from_be_bytes([data[4], data[5], data[6], data[7]]);

        if cx != chunk_x || cz != chunk_z {
            return Err(format!(
                "chunk position mismatch: expected ({},{}), got ({},{})",
                chunk_x, chunk_z, cx, cz
            ));
        }

        let section_count =
            u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;
        let mut offset = 12;

        let pos = mc_core::position::ChunkPos::new(chunk_x, chunk_z);
        let mut chunk = Chunk::new(pos);

        for _ in 0..section_count {
            if offset + 8 > data.len() {
                break;
            }
            let section_y = i32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;

            let block_len =
                u32::from_be_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]) as usize;
            offset += 4;
            let block_data = &data[offset..offset + block_len];
            offset += block_len;

            let biome_len =
                u32::from_be_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]) as usize;
            offset += 4;
            let biome_data = &data[offset..offset + biome_len];
            offset += biome_len;

            // Decode palette data and populate section
            let section = chunk.get_or_create_section(section_y);
            if let Err(e) = section.blocks.decode_network(block_data) {
                warn!("Failed to decode blocks for section {}: {}", section_y, e);
            }
            if let Err(e) = section.biomes.decode_network(biome_data) {
                warn!("Failed to decode biomes for section {}: {}", section_y, e);
            }
        }

        chunk.dirty = false;
        debug!("Loaded chunk ({}, {}) with {} sections", chunk_x, chunk_z, section_count);
        Ok(Some(chunk))
    }
}

impl Default for WorldSaver {
    fn default() -> Self {
        Self::new()
    }
}
