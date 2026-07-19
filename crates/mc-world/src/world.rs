//! 世界 — 顶层世界状态容器

use crate::chunk_store::ChunkStore;
use crate::generator::GeneratorRegistry;
use mc_core::position::{BlockPos, ChunkPos};
use mc_core::types::Dimension;

/// Minecraft 世界
pub struct World {
    pub level_name: String,
    pub seed: u64,
    pub spawn_position: BlockPos,
    pub time: u64,
    pub dimension: Dimension,
    pub view_distance: u8,
    /// 已加载区块
    pub chunks: ChunkStore,
    /// 地形生成器注册表
    pub generators: GeneratorRegistry,
}

impl World {
    pub fn new(name: String, seed: u64, view_distance: u8) -> Self {
        let chunks = ChunkStore::new();
        let generators = GeneratorRegistry::new();

        Self {
            level_name: name,
            seed,
            spawn_position: BlockPos::ZERO,
            time: 0,
            dimension: Dimension::Overworld,
            view_distance,
            chunks,
            generators,
        }
    }

    /// 使用自定义生成器选项创建世界
    pub fn with_generator_options(
        name: String,
        seed: u64,
        view_distance: u8,
        active_gen: &str,
        gen_options: std::collections::HashMap<String, String>,
    ) -> Self {
        let chunks = ChunkStore::new();
        let generators = crate::generator::GeneratorRegistry::with_config(active_gen, gen_options);

        Self {
            level_name: name,
            seed,
            spawn_position: BlockPos::ZERO,
            time: 0,
            dimension: Dimension::Overworld,
            view_distance,
            chunks,
            generators,
        }
    }

    /// 使用注册表中激活的生成器动态生成区块
    pub fn generate_chunk(&self, pos: ChunkPos) -> crate::chunk::Chunk {
        self.generators.generate_chunk(pos, self.seed)
    }

    /// 获取玩家视距内的区块坐标列表
    pub fn visible_chunks(&self, center: ChunkPos) -> Vec<ChunkPos> {
        let r = self.view_distance as i32;
        let mut chunks = Vec::with_capacity(((2 * r + 1) * (2 * r + 1)) as usize);
        for dx in -r..=r {
            for dz in -r..=r {
                chunks.push(ChunkPos::new(center.x + dx, center.z + dz));
            }
        }
        chunks
    }

    /// 增加世界时间（每 tick 调用）
    pub fn tick_time(&mut self) {
        self.time = self.time.wrapping_add(1);
    }
}
