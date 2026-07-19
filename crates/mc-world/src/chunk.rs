//! 区块数据结构
//!
//! Chunk (16×384×16) = 24 Sections × (16×16×16) blocks

use crate::paletted::PalettedContainer;
use mc_core::block::{BlockEntity, BlockState};
use mc_core::position::{ChunkPos, SectionPos, SECTIONS_PER_CHUNK, MIN_SECTION_Y};

/// 一个 16×384×16 的区块
#[derive(Debug, Clone)]
pub struct Chunk {
    pub position: ChunkPos,
    pub sections: [Option<Section>; SECTIONS_PER_CHUNK],
    pub block_entities: Vec<BlockEntity>,
    /// 是否已修改（需要保存）
    pub dirty: bool,
    /// LRU 驱逐顺序 (插入时的全局计数器值)
    pub lru_order: u64,
    /// 缓存的序列化 ChunkData 字节 (Arc 用于零拷贝广播)
    pub cached_packet: Option<std::sync::Arc<Vec<u8>>>,
}

impl Chunk {
    /// 使缓存失效 (方块修改时调用)
    pub fn invalidate_cache(&mut self) {
        self.cached_packet = None;
    }

    /// 获取缓存的序列化 ChunkData，避免重新编码
    pub fn cached_chunk_bytes(&self) -> Option<std::sync::Arc<Vec<u8>>> {
        self.cached_packet.clone()
    }

    /// 设置缓存的序列化 ChunkData
    pub fn set_cached_bytes(&mut self, data: std::sync::Arc<Vec<u8>>) {
        self.cached_packet = Some(data);
    }
}

/// 全局 LRU 计数器
static LRU_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl Chunk {
    pub fn new(pos: ChunkPos) -> Self {
        Self {
            position: pos,
            sections: std::array::from_fn(|_| None),
            block_entities: Vec::new(),
            dirty: true,
            lru_order: LRU_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            cached_packet: None,
        }
    }

    /// 读取指定坐标的方块
    pub fn get_block(&self, x: usize, y: i32, z: usize) -> BlockState {
        let section_idx = section_index(y);
        self.sections
            .get(section_idx)
            .and_then(|s| s.as_ref())
            .map(|s| s.get_block(x, y.rem_euclid(16) as usize, z))
            .unwrap_or(BlockState::AIR)
    }

    /// Update LRU order — called on every access to keep hot chunks in cache
    pub fn touch(&mut self) {
        self.lru_order = LRU_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 设置指定坐标的方块
    pub fn set_block(&mut self, x: usize, y: i32, z: usize, block: BlockState) {
        self.touch();
        self.invalidate_cache();
        let section_idx = section_index(y);
        let section = self.sections[section_idx].get_or_insert_with(|| {
            Section::new(SectionPos::new(
                self.position,
                MIN_SECTION_Y + section_idx as i32,
            ))
        });
        section.set_block(x, y.rem_euclid(16) as usize, z, block);
        self.dirty = true;
    }

    /// 获取或创建指定 section (参数为 section Y 坐标, 如 -4..19)
    pub fn get_or_create_section(&mut self, section_y: i32) -> &mut Section {
        let idx = section_index_from_section_y(section_y);
        self.sections[idx].get_or_insert_with(|| {
            Section::new(mc_core::position::SectionPos::new(self.position, section_y))
        })
    }

    /// 生成 ChunkData 数据包负载 (含光照数据)
    pub fn to_chunk_data(&self) -> mc_protocol::packets::play::ChunkData {
        use mc_protocol::packets::play::{ChunkData, ChunkSectionData};

        let mut sections = Vec::new();
        let mut sky_light_mask: Vec<i64> = Vec::new();
        let mut block_light_mask: Vec<i64> = Vec::new();
        let mut sky_light_arrays: Vec<Vec<u8>> = Vec::new();
        let mut block_light_arrays: Vec<Vec<u8>> = Vec::new();

        // Build a single BitSet word (i64) — 24 sections fit in 1 u64
        let mut sky_mask_word: i64 = 0;
        let mut block_mask_word: i64 = 0;

        for (i, section_opt) in self.sections.iter().enumerate() {
            if let Some(section) = section_opt {
                let non_air = section.count_non_air();
                sections.push(ChunkSectionData {
                    block_count: non_air as i16,
                    blocks: section.blocks.encode_network(),
                    biomes: section.biomes.encode_network(),
                });

                // Set bit i in the mask word
                sky_mask_word |= 1i64 << (i as i64);
                block_mask_word |= 1i64 << (i as i64);
                sky_light_arrays.push(section.sky_light.to_vec());
                block_light_arrays.push(section.block_light.to_vec());
            }
        }

        // Each mask is a BitSet: [u64 count as VarInt][u64 words...]
        // 24 sections → 1 u64 word with bits 0-23 set
        if sky_mask_word != 0 {
            sky_light_mask.push(sky_mask_word);
            block_light_mask.push(block_mask_word);
        }

        // 高度图 (简化为空 NBT compound)
        let heightmaps = vec![0x00]; // TAG_End

        let empty_mask = Vec::new();

        ChunkData {
            chunk_x: self.position.x,
            chunk_z: self.position.z,
            heightmaps,
            sections,
            block_entities: Vec::new(),
            sky_light_mask,
            block_light_mask,
            empty_sky_light_mask: empty_mask.clone(),
            empty_block_light_mask: empty_mask,
            sky_light_arrays,
            block_light_arrays,
        }
    }
}

/// Section — 16×16×16 方块子区域
#[derive(Debug, Clone)]
pub struct Section {
    pub position: SectionPos,
    /// 方块调色板容器
    pub blocks: PalettedContainer,
    /// 生物群系调色板容器（简化：全平原）
    pub biomes: PalettedContainer,
    /// 天光 (0-15, 每方块 4 bits → 2048 bytes)
    pub sky_light: Box<[u8; 2048]>,
    /// 方块光 (0-15, 每方块 4 bits → 2048 bytes)
    pub block_light: Box<[u8; 2048]>,
}

/// 在 2048-byte 数组中读写 4-bit nibble (Minecraft YZX 顺序)
pub fn light_index(x: usize, y: usize, z: usize) -> usize {
    (y << 8) | (z << 4) | x
}

pub fn get_light_nibble(light: &[u8; 2048], x: usize, y: usize, z: usize) -> u8 {
    let idx = light_index(x, y, z);
    if idx.is_multiple_of(2) {
        light[idx / 2] & 0x0F
    } else {
        light[idx / 2] >> 4
    }
}

pub fn set_light_nibble(light: &mut [u8; 2048], x: usize, y: usize, z: usize, value: u8) {
    let idx = light_index(x, y, z);
    let v = value & 0x0F;
    if idx.is_multiple_of(2) {
        light[idx / 2] = (light[idx / 2] & 0xF0) | v;
    } else {
        light[idx / 2] = (light[idx / 2] & 0x0F) | (v << 4);
    }
}

impl Section {
    pub fn new(pos: SectionPos) -> Self {
        Self {
            position: pos,
            blocks: PalettedContainer::new(),
            biomes: PalettedContainer::filled(BlockState::new(0)),
            sky_light: Box::new([0xFFu8; 2048]),    // all 15 = fully lit by sky
            block_light: Box::new([0u8; 2048]),     // all 0 = no block light
        }
    }

    pub fn get_block(&self, x: usize, y: usize, z: usize) -> BlockState {
        self.blocks.get(x, y, z)
    }

    pub fn set_block(&mut self, x: usize, y: usize, z: usize, block: BlockState) {
        self.blocks.set(x, y, z, block);
    }

    pub fn get_sky_light(&self, x: usize, y: usize, z: usize) -> u8 {
        get_light_nibble(&self.sky_light, x, y, z)
    }

    pub fn set_sky_light(&mut self, x: usize, y: usize, z: usize, value: u8) {
        set_light_nibble(&mut self.sky_light, x, y, z, value)
    }

    pub fn get_block_light(&self, x: usize, y: usize, z: usize) -> u8 {
        get_light_nibble(&self.block_light, x, y, z)
    }

    pub fn set_block_light(&mut self, x: usize, y: usize, z: usize, value: u8) {
        set_light_nibble(&mut self.block_light, x, y, z, value)
    }

    /// 填充整个 section 的天光 (生成时使用)
    pub fn fill_sky_light(&mut self, value: u8) {
        let v = value & 0x0F;
        *self.sky_light = [v | (v << 4); 2048];
    }

    /// 计数非空气方块
    pub fn count_non_air(&self) -> usize {
        let mut count = 0;
        for x in 0..16usize {
            for y in 0..16usize {
                for z in 0..16usize {
                    if !self.blocks.get(x, y, z).is_air() {
                        count += 1;
                    }
                }
            }
        }
        count
    }
}

/// 将世界 Y 坐标 (block Y) 映射到 section 数组索引
/// 例如: y=-64 → 0, y=-1 → 3, y=0 → 4, y=319 → 23
pub fn section_index(y: i32) -> usize {
    (y.div_euclid(16) - MIN_SECTION_Y) as usize
}

/// 将 section Y 坐标映射到 section 数组索引
/// 例如: sy=-4 → 0, sy=0 → 4, sy=19 → 23
pub fn section_index_from_section_y(section_y: i32) -> usize {
    (section_y - MIN_SECTION_Y) as usize
}

impl Chunk {
    /// 获取指定 XZ 柱的最高非空气方块 Y 坐标 (用于生物生成)
    pub fn height_at(&self, x: usize, z: usize) -> i32 {
        // Scan from top down
        for y in (0..384i32).rev() {
            let world_y = MIN_SECTION_Y * 16 + y;
            if !self.get_block(x, world_y, z).is_air() {
                return world_y + 1; // spawn on top of block
            }
        }
        MIN_SECTION_Y * 16 // minimum if empty
    }

    /// 获取指定位置的合并光照值 (天光+方块光, max of each)
    pub fn combined_light(&self, x: usize, y: i32, z: usize) -> u8 {
        let section_idx = section_index(y);
        if let Some(Some(section)) = self.sections.get(section_idx) {
            let ly = y.rem_euclid(16) as usize;
            let sky = section.get_sky_light(x, ly, z);
            let block = section.get_block_light(x, ly, z);
            sky.max(block)
        } else {
            0
        }
    }

    /// 检查指定位置是否为可生成表面 (非透明、非流体、非半砖)
    pub fn is_spawn_surface(&self, x: usize, y: i32, z: usize) -> bool {
        let block = self.get_block(x, y, z);
        if block.is_air() { return false; }
        // Exclude fluids, glass, slabs, etc.
        matches!(block.id,
            8 | 9 | 10 | 11 | 24 | 25 | 26 | // grass,dirt,coarse,podzol,sand,red_sand,gravel
            1 | 12 | 87 | 121 | 88 // stone,cobblestone,netherrack,end_stone,soul_sand
        )
    }
}
