//! Anvil (Region) 格式读写
//!
//! 读取和写入 Minecraft .mca 文件。
//! 用于 Terra 预生成世界的加载，以及开发端离线生成。

use crate::chunk::{Chunk, Section};
use crate::paletted::PalettedContainer;
use mc_core::block::BlockState;
use mc_core::position::{ChunkPos, SECTIONS_PER_CHUNK};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use tracing::{debug, info, warn};

#[derive(Debug)]
pub enum AnvilError {
    Io(std::io::Error),
    Format(String),
    Nbt(String),
}

impl From<std::io::Error> for AnvilError {
    fn from(e: std::io::Error) -> Self { AnvilError::Io(e) }
}
impl std::fmt::Display for AnvilError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnvilError::Io(e) => write!(f, "IO: {}", e),
            AnvilError::Format(s) => write!(f, "Format: {}", s),
            AnvilError::Nbt(s) => write!(f, "NBT: {}", s),
        }
    }
}
impl std::error::Error for AnvilError {}

/// Anvil 读取器
pub struct AnvilReader {
    block_registry: HashMap<String, BlockState>,
    /// 用于分配未知方块的临时 ID (从高位开始避免冲突)
    next_unknown_id: u32,
}

impl AnvilReader {
    pub fn new() -> Self {
        let mut r = HashMap::new();
        for &(name, id) in STANDARD_BLOCKS {
            r.insert(name.into(), BlockState::new(id));
        }
        Self { block_registry: r, next_unknown_id: 10000 }
    }

    fn resolve_block(&mut self, name: &str) -> BlockState {
        let base = name.split('[').next().unwrap_or(name);
        if let Some(&b) = self.block_registry.get(base) {
            return b;
        }
        // Assign a unique ID from the high range to avoid collision with known blocks
        let id = self.next_unknown_id;
        self.next_unknown_id += 1;
        let block = BlockState::new(id);
        self.block_registry.insert(base.to_string(), block);
        warn!("Unknown block: {} → assigned id {} (will not survive restart)", name, id);
        block
    }

    pub fn load_chunk(&mut self, region_dir: &Path, chunk_x: i32, chunk_z: i32) -> Result<Option<Chunk>, AnvilError> {
        let local_x = chunk_x.rem_euclid(32) as usize;
        let local_z = chunk_z.rem_euclid(32) as usize;
        let filename = format!("r.{}.{}.mca", chunk_x.div_euclid(32), chunk_z.div_euclid(32));
        let path = region_dir.join(&filename);
        if !path.exists() { return Ok(None); }

        let _file_size = fs::metadata(&path)?.len();
        let entry_idx = local_x + local_z * 32;

        let mut file = File::open(&path)?;
        let mut header = vec![0u8; 4096];
        file.read_exact(&mut header)?;

        let entry = &header[entry_idx * 4..entry_idx * 4 + 4];
        let offset = u32::from_be_bytes([0, entry[0], entry[1], entry[2]]) as u64;
        let _sectors = entry[3] as u64;
        if offset < 2 { return Ok(None); }

        file.seek(SeekFrom::Start(offset * 4096))?;
        let mut len_buf = [0u8; 4];
        file.read_exact(&mut len_buf)?;
        let data_len = u32::from_be_bytes(len_buf) as usize;
        let mut comp_buf = [0u8; 1];
        file.read_exact(&mut comp_buf)?;
        let compression = comp_buf[0];
        if data_len == 0 || data_len > 10_000_000 { return Err(AnvilError::Format("invalid size".into())); }

        let mut compressed = vec![0u8; data_len];
        file.read_exact(&mut compressed)?;
        let decompressed = decompress(compression, &compressed)?;
        let nbt: fastnbt::Value = fastnbt::from_bytes(&decompressed).map_err(|e| AnvilError::Nbt(format!("{}", e)))?;
        self.parse_chunk_nbt(chunk_x, chunk_z, &nbt)
    }

    fn parse_chunk_nbt(&mut self, cx: i32, cz: i32, nbt: &fastnbt::Value) -> Result<Option<Chunk>, AnvilError> {
        // 1.18+: root compound IS the level data (no nested "Level" key)
        // Pre-1.18: level data is nested under "Level" key
        let level = match nbt {
            fastnbt::Value::Compound(c) => {
                // Check for pre-1.18 format first
                c.get("Level").or({
                    // 1.18+ format: root compound is the level data
                    // Return a reference to the compound itself (we use "" as sentinel)
                    // Actually just use the root compound directly
                    None
                })
            }
            _ => return Err(AnvilError::Nbt("expected compound".into())),
        };

        // If no "Level" key found, the root IS the level (1.18+)
        let level_c = match level {
            Some(fastnbt::Value::Compound(c)) => c,
            Some(_) => return Err(AnvilError::Nbt("Level is not a compound".into())),
            None => {
                // 1.18+ format: root = level
                match nbt {
                    fastnbt::Value::Compound(c) => c,
                    _ => return Err(AnvilError::Nbt("missing level data".into())),
                }
            }
        };

        let pos = ChunkPos::new(cx, cz);
        let mut chunk = Chunk::new(pos);

        if let Some(fastnbt::Value::List(sections)) = level_c.get("Sections") {
            for sec in sections {
                if let fastnbt::Value::Compound(s) = sec {
                    let sy = match s.get("Y") { Some(fastnbt::Value::Byte(y)) => *y as i32, _ => continue };
                    let idx = crate::chunk::section_index_from_section_y(sy);
                    if idx >= SECTIONS_PER_CHUNK { continue; }

                    let mut blocks = PalettedContainer::new();
                    if let Some(fastnbt::Value::Compound(bs)) = s.get("block_states")
                        && let Some(fastnbt::Value::List(pal)) = bs.get("palette") {
                            let entries: Vec<BlockState> = pal.iter().map(|e| {
                                if let fastnbt::Value::Compound(c) = e {
                                    c.get("Name").and_then(|n| {
                                        if let fastnbt::Value::String(s) = n { Some(self.resolve_block(s)) } else { None }
                                    }).unwrap_or(BlockState::AIR)
                                } else { BlockState::AIR }
                            }).collect();

                            if entries.len() == 1 {
                                blocks = PalettedContainer::filled(entries[0]);
                            } else if let Some(fastnbt::Value::LongArray(data)) = bs.get("data") {
                                let bits = std::cmp::max(1, (usize::BITS as usize - (entries.len() - 1).leading_zeros() as usize) as u8);
                                for y in 0..16usize { for z in 0..16usize { for x in 0..16usize {
                                    let i = y * 256 + z * 16 + x;
                                    let wi = i * bits as usize / 64;
                                    let bo = (i * bits as usize) % 64;
                                    if wi < data.len() {
                                        let raw = data[wi] as u64;
                                        let val = (raw >> bo) & ((1u64 << bits as u64) - 1);
                                        if let Some(&b) = entries.get(val as usize) { blocks.set(x, y, z, b); }
                                    }
                                }}}
                            }
                        }
                    chunk.sections[idx] = Some(Section {
                        position: mc_core::position::SectionPos::new(pos, sy),
                        blocks,
                        biomes: PalettedContainer::filled(BlockState::new(0)),
                        sky_light: Box::new([0xFFu8; 2048]),
                        block_light: Box::new([0u8; 2048]),
                    });
                }
            }
        }
        chunk.dirty = false;
        Ok(Some(chunk))
    }
}

fn decompress(compression: u8, data: &[u8]) -> Result<Vec<u8>, AnvilError> {
    match compression {
        1 => { let mut d = flate2::read::GzDecoder::new(data); let mut o = Vec::new(); d.read_to_end(&mut o)?; Ok(o) }
        2 => { let mut d = flate2::read::ZlibDecoder::new(data); let mut o = Vec::new(); d.read_to_end(&mut o)?; Ok(o) }
        _ => Err(AnvilError::Format(format!("unknown compression: {}", compression)))
    }
}

// ═══════════════════════════════════════════════════════
// Anvil Writer — 将 Chunk 序列化为 .mca 文件
// ═══════════════════════════════════════════════════════

use serde::Serialize;

/// 常用方块 (名称, ID)
const STANDARD_BLOCKS: &[(&str, u32)] = &[
    ("minecraft:air", 0),
    ("minecraft:stone", 1),
    ("minecraft:granite", 2),
    ("minecraft:grass_block", 9),
    ("minecraft:dirt", 10),
    ("minecraft:coarse_dirt", 11),
    ("minecraft:sand", 12),
    ("minecraft:gravel", 13),
    ("minecraft:oak_log", 17),
    ("minecraft:oak_leaves", 18),
    ("minecraft:bedrock", 33),
    ("minecraft:water", 34),
];

/// Anvil 写入器
pub struct AnvilWriter;

impl AnvilWriter {
    pub fn new() -> Self { Self }

    /// 将一批 chunk 写入 Anvil 格式
    pub fn write_chunks(&self, region_dir: &Path, chunks: &[Chunk]) -> Result<usize, AnvilError> {
        fs::create_dir_all(region_dir)?;

        // Group chunks by region
        let mut regions: HashMap<(i32, i32), Vec<&Chunk>> = HashMap::new();
        for chunk in chunks {
            let rx = chunk.position.x.div_euclid(32);
            let rz = chunk.position.z.div_euclid(32);
            regions.entry((rx, rz)).or_default().push(chunk);
        }

        let mut total = 0;
        for ((rx, rz), region_chunks) in &regions {
            self.write_region(region_dir, *rx, *rz, region_chunks)?;
            total += region_chunks.len();
        }
        info!("Wrote {} chunks to {} region files", total, regions.len());
        Ok(total)
    }

    fn write_region(&self, dir: &Path, rx: i32, rz: i32, chunks: &[&Chunk]) -> Result<(), AnvilError> {
        let path = dir.join(format!("r.{}.{}.mca", rx, rz));

        // Build compressed data for each chunk
        let mut entries: Vec<(usize, Vec<u8>)> = Vec::new(); // (local_index, compressed_data)

        for chunk in chunks {
            let lx = chunk.position.x.rem_euclid(32) as usize;
            let lz = chunk.position.z.rem_euclid(32) as usize;
            let local_idx = lx + lz * 32;

            let nbt = serialize_chunk_nbt(chunk)?;
            let compressed = {
                let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(&nbt)?;
                e.finish()?
            };
            entries.push((local_idx, compressed));
        }

        entries.sort_by_key(|(idx, _)| *idx);

        // Header: 4KB
        let mut header = vec![0u8; 4096];
        // Sector 0: header (8 sectors for 4KB header)
        // Sector 2: first data sector (sector 1 is padding after header)
        let mut current_sector: u32 = 2;

        // Write each chunk, tracking position
        let mut chunk_data_blocks: Vec<(usize, Vec<u8>)> = Vec::new();

        for (local_idx, compressed) in &entries {
            let data_size = 5 + compressed.len(); // 4 bytes length + 1 byte compression + data
            let sectors_needed = (data_size as u32).div_ceil(4096);

            // Write header entry: 3 bytes offset (big-endian), 1 byte sector count
            let off = local_idx * 4;
            header[off] = ((current_sector >> 16) & 0xFF) as u8;
            header[off + 1] = ((current_sector >> 8) & 0xFF) as u8;
            header[off + 2] = (current_sector & 0xFF) as u8;
            header[off + 3] = sectors_needed as u8;

            // Build sector data
            let mut sector_data = Vec::new();
            sector_data.extend_from_slice(&(compressed.len() as u32).to_be_bytes());
            sector_data.push(2u8); // zlib compression
            sector_data.extend_from_slice(compressed);
            // Pad to sectors_needed * 4096
            let pad = (sectors_needed as usize * 4096).saturating_sub(sector_data.len());
            sector_data.extend(std::iter::repeat_n(0u8, pad));

            chunk_data_blocks.push((*local_idx, sector_data));
            current_sector += sectors_needed;
        }

        // Write file
        let mut file = File::create(&path)?;
        file.write_all(&header)?;
        // Sector 1: all zeros (padding)
        file.write_all(&vec![0u8; 4096])?;
        // Write each chunk's data
        for (_idx, data) in &chunk_data_blocks {
            file.write_all(data)?;
        }

        debug!("Wrote region r.{}.{}.mca ({} chunks)", rx, rz, entries.len());
        Ok(())
    }
}

/// 将 Chunk 序列化为 NBT
#[allow(non_snake_case)]
fn serialize_chunk_nbt(chunk: &Chunk) -> Result<Vec<u8>, AnvilError> {
    #[derive(Serialize)]
    struct ChunkNbt {
        #[serde(rename = "DataVersion")]
        data_version: i32,
        Level: LevelNbt,
    }
    #[derive(Serialize)]
    struct LevelNbt {
        #[serde(rename = "xPos")]
        x_pos: i32,
        #[serde(rename = "zPos")]
        z_pos: i32,
        Sections: Vec<SectionNbt>,
        #[serde(rename = "isLightOn")]
        is_light_on: i8,
    }
    #[derive(Serialize)]
    struct SectionNbt {
        #[serde(rename = "Y")]
        y: i8,
        block_states: BlockStatesNbt,
        biomes: BiomesNbt,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "SkyLight")]
        sky_light: Option<Vec<u8>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "BlockLight")]
        block_light: Option<Vec<u8>>,
    }
    #[derive(Serialize)]
    struct BlockStatesNbt {
        palette: Vec<PaletteEntryNbt>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        data: Vec<i64>,
    }
    #[derive(Serialize)]
    struct PaletteEntryNbt {
        #[serde(rename = "Name")]
        name: String,
    }
    #[derive(Serialize)]
    struct BiomesNbt {
        palette: Vec<String>,
    }
    let mut sections = Vec::new();
    for (i, sec_opt) in chunk.sections.iter().enumerate() {
        if let Some(sec) = sec_opt {
            let sy = i as i32 + mc_core::position::MIN_SECTION_Y;
            let blocks = sec.blocks.iter_blocks();
            let unique: std::collections::BTreeSet<u32> = blocks.iter().map(|b| b.id).collect();
            let palette: Vec<_> = unique.iter().map(|id| PaletteEntryNbt {
                name: block_id_to_name(*id).into(),
            }).collect();

            let data = if palette.len() > 1 {
                let bits = std::cmp::max(1, (usize::BITS as usize - (palette.len() - 1).leading_zeros() as usize) as u8);
                let total_bits = 4096 * bits as usize;
                let word_count = total_bits.div_ceil(64);
                let mut words = vec![0i64; word_count];
                for (i, b) in blocks.iter().enumerate() {
                    let idx = palette.iter().position(|e| e.name == block_id_to_name(b.id)).unwrap_or(0);
                    let bit_off = i * bits as usize;
                    let word_idx = bit_off / 64;
                    let bit_pos = bit_off % 64;
                    words[word_idx] |= (idx as i64) << bit_pos;
                }
                words
            } else {
                Vec::new() // single palette entry → no data needed
            };

            sections.push(SectionNbt {
                y: sy as i8,
                block_states: BlockStatesNbt { palette, data },
                biomes: BiomesNbt { palette: vec!["minecraft:plains".into()] },
                sky_light: Some(sec.sky_light.to_vec()),
                block_light: Some(sec.block_light.to_vec()),
            });
        }
    }

    let nbt = ChunkNbt {
        data_version: 3700,
        Level: LevelNbt {
            x_pos: chunk.position.x,
            z_pos: chunk.position.z,
            Sections: sections,
            is_light_on: 1,
        },
    };

    fastnbt::to_bytes(&nbt).map_err(|e| AnvilError::Nbt(format!("serialize: {}", e)))
}

fn block_id_to_name(id: u32) -> &'static str {
    match id {
        0 => "minecraft:air",
        1 => "minecraft:stone",
        2 => "minecraft:granite",
        9 => "minecraft:grass_block",
        10 => "minecraft:dirt",
        11 => "minecraft:coarse_dirt",
        12 => "minecraft:sand",
        13 => "minecraft:gravel",
        17 => "minecraft:oak_log",
        18 => "minecraft:oak_leaves",
        33 => "minecraft:bedrock",
        34 => "minecraft:water",
        79 => "minecraft:ice",
        80 => "minecraft:snow",
        81 => "minecraft:snow_block",
        82 => "minecraft:clay",
        _ => "minecraft:stone", // NOTE: unknown IDs preserved as stone — full registry needed for fidelity
    }
}

impl Default for AnvilReader { fn default() -> Self { Self::new() } }
impl Default for AnvilWriter { fn default() -> Self { Self::new() } }
