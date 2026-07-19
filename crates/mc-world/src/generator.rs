//! 地形生成器模块系统
//!
//! ## 架构
//!
//! ```text
//! GeneratorRegistry
//!   ├── "flat"    → FlatGenerator        (超平坦: bedrock + stone + dirt + grass)
//!   ├── "noise"   → NoiseGenerator       (噪声地形: hash-based 起伏)
//!   ├── "empty"   → EmptyGenerator       (虚空: 仅 platform)
//!   ├── "custom"  → CustomGenerator      (配置驱动: TOML 定义层/参数)
//!   └── "compose" → LayerComposer        (组合: 多个生成器叠加)
//! ```
//!
//! ## 添加新生成器的方式
//!
//! ### 方式 1: 编译时注册 (Rust 代码)
//! ```rust,ignore
//! struct MyGenerator;
//! impl TerrainGenerator for MyGenerator { ... }
//! registry.register(MyGenerator::new());
//! ```
//!
//! ### 方式 2: 配置驱动 (零代码)
//! ```toml
//! [world]
//! generator = "custom"
//! [world.generator_options]
//! mode = "layered"                # or "heightmap"
//! surface_height = -59
//! layers = [
//!     { block = "bedrock", thickness = 1 },      # y=-64
//!     { block = "stone",   thickness = 2 },      # y=-63..-61
//!     { block = "dirt",    thickness = 3 },      # y=-61..-58
//!     { block = "grass_block", thickness = 1 },  # y=-58
//! ]
//! ```
//!
//! ### 方式 3: 组合生成器
//! ```toml
//! [world]
//! generator = "compose"
//! [world.generator_options]
//! generators = ["noise", "flat"]
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use mc_core::block::BlockState;
use mc_core::position::ChunkPos;
use crate::chunk::Chunk;
use tracing::info;

/// 常用方块 ID 映射 (protocol 776 官方 ID)
pub mod blocks {
    use mc_core::block::BlockState;
    pub const AIR: BlockState = BlockState::new(0);
    pub const STONE: BlockState = BlockState::new(1);
    pub const GRANITE: BlockState = BlockState::new(2);
    pub const DIRT: BlockState = BlockState::new(9);
    pub const COARSE_DIRT: BlockState = BlockState::new(10);
    pub const GRASS_BLOCK: BlockState = BlockState::new(8);
    pub const BEDROCK: BlockState = BlockState::new(266);
    pub const SAND: BlockState = BlockState::new(24);
    pub const GRAVEL: BlockState = BlockState::new(26);
    pub const OAK_LOG: BlockState = BlockState::new(34);
    pub const OAK_LEAVES: BlockState = BlockState::new(56);
    pub const SPRUCE_LOG: BlockState = BlockState::new(35);
    pub const SPRUCE_LEAVES: BlockState = BlockState::new(57);
    pub const BIRCH_LOG: BlockState = BlockState::new(36);
    pub const BIRCH_LEAVES: BlockState = BlockState::new(58);
    pub const JUNGLE_LOG: BlockState = BlockState::new(37);
    pub const JUNGLE_LEAVES: BlockState = BlockState::new(59);
    pub const ACACIA_LOG: BlockState = BlockState::new(38);
    pub const ACACIA_LEAVES: BlockState = BlockState::new(60);
    pub const DARK_OAK_LOG: BlockState = BlockState::new(40);
    pub const DARK_OAK_LEAVES: BlockState = BlockState::new(61);
    pub const CHERRY_LOG: BlockState = BlockState::new(39);
    pub const CHERRY_LEAVES: BlockState = BlockState::new(62);
    pub const MANGROVE_LOG: BlockState = BlockState::new(41);
    pub const MANGROVE_LEAVES: BlockState = BlockState::new(63);
    // Pale oak (1.21.5): similar to dark oak shape, pale-colored leaves
    pub const PALE_OAK_LOG: BlockState = BlockState::new(40); // reuse dark_oak_log
    pub const PALE_OAK_LEAVES: BlockState = BlockState::new(56); // oak_leaves as proxy (client renders pale in PaleGarden biome)
    pub const SHORT_GRASS: BlockState = BlockState::new(79);
    pub const FERN: BlockState = BlockState::new(80);
    pub const DEAD_BUSH: BlockState = BlockState::new(81);
    pub const DANDELION: BlockState = BlockState::new(201);
    pub const POPPY: BlockState = BlockState::new(202);
    pub const BLUE_ORCHID: BlockState = BlockState::new(203);
    pub const ALLIUM: BlockState = BlockState::new(204);
    pub const AZURE_BLUET: BlockState = BlockState::new(205);
    pub const RED_TULIP: BlockState = BlockState::new(206);
    pub const ORANGE_TULIP: BlockState = BlockState::new(207);
    pub const PINK_TULIP: BlockState = BlockState::new(208);
    pub const WHITE_TULIP: BlockState = BlockState::new(209);
    pub const OXEYE_DAISY: BlockState = BlockState::new(210);
    pub const CORNFLOWER: BlockState = BlockState::new(211);
    pub const LILAC: BlockState = BlockState::new(213);
    pub const ROSE_BUSH: BlockState = BlockState::new(214);
    pub const PEONY: BlockState = BlockState::new(215);
    pub const SUNFLOWER: BlockState = BlockState::new(212);
    pub const TALL_GRASS: BlockState = BlockState::new(216);
    pub const WATER: BlockState = BlockState::new(267);
    pub const CACTUS: BlockState = BlockState::new(81);
    pub const MELON: BlockState = BlockState::new(103);
    pub const SUGAR_CANE: BlockState = BlockState::new(83);

    /// 按名称查找方块 (用于 TOML 配置解析)
    pub fn by_name(name: &str) -> Option<BlockState> {
        match name.to_lowercase().as_str() {
            "air" => Some(AIR),
            "stone" => Some(STONE),
            "granite" => Some(GRANITE),
            "dirt" => Some(DIRT),
            "coarse_dirt" => Some(COARSE_DIRT),
            "grass_block" | "grass" => Some(GRASS_BLOCK),
            "bedrock" => Some(BEDROCK),
            "sand" => Some(SAND),
            "gravel" => Some(GRAVEL),
            "oak_log" | "log" => Some(OAK_LOG),
            "oak_leaves" | "leaves" => Some(OAK_LEAVES),
            "water" => Some(WATER),
            _ => None,
        }
    }
}

// ═══════════════════════════════════════════════════════
// Core trait
// ═══════════════════════════════════════════════════════

/// 地形生成器 trait
pub trait TerrainGenerator: Send + Sync {
    fn name(&self) -> &str;
    fn generate_chunk(&self, pos: ChunkPos, seed: u64) -> Chunk;
    fn init(&mut self, _seed: u64) {}
    fn options_schema(&self) -> HashMap<String, String> { HashMap::new() }
}

// ═══════════════════════════════════════════════════════
// Built-in generators
// ═══════════════════════════════════════════════════════

pub struct EmptyGenerator;
impl EmptyGenerator {
    pub fn new() -> Self { Self }
}
impl Default for EmptyGenerator {
    fn default() -> Self { Self }
}
impl TerrainGenerator for EmptyGenerator {
    fn name(&self) -> &str { "empty" }
    fn generate_chunk(&self, pos: ChunkPos, _seed: u64) -> Chunk { Chunk::new(pos) }
}

/// 超平坦生成器
#[derive(Clone)]
pub struct FlatGenerator {
    pub layers: Vec<(BlockState, i32)>,
}

impl FlatGenerator {
    pub fn new() -> Self {
        Self {
            layers: vec![
                (blocks::BEDROCK, 1),      // y=-64
                (blocks::STONE, 2),        // y=-63..-61
                (blocks::DIRT, 2),         // y=-61..-59
                (blocks::GRASS_BLOCK, 1),  // y=-59
            ],
        }
    }

    pub fn with_layers(mut self, layers: Vec<(BlockState, i32)>) -> Self {
        self.layers = layers;
        self
    }

    fn apply_layers(&self, chunk: &mut Chunk) {
        for x in 0..16usize {
            for z in 0..16usize {
                let mut current_y = -64i32;
                for (block, thickness) in &self.layers {
                    for dy in 0..*thickness {
                        chunk.set_block(x, current_y + dy, z, *block);
                    }
                    current_y += thickness;
                }
            }
        }
    }
}

impl Default for FlatGenerator { fn default() -> Self { Self::new() } }

impl TerrainGenerator for FlatGenerator {
    fn name(&self) -> &str { "flat" }
    fn generate_chunk(&self, pos: ChunkPos, _seed: u64) -> Chunk {
        let mut chunk = Chunk::new(pos);
        self.apply_layers(&mut chunk);
        crate::lighting::init_chunk_lighting(&mut chunk);
        chunk.dirty = false;
        chunk
    }
}

// ── Biome sampling ──

/// Sample a biome at world coordinates using temperature/humidity 2D noise (48 biomes)
pub fn sample_biome(world_x: i32, world_z: i32, seed: u64) -> mc_core::biome::BiomeId {
    use mc_core::biome::BiomeId;
    let h = (world_x as u64).wrapping_mul(1619)
        ^ (world_z as u64).wrapping_mul(31337)
        ^ seed.wrapping_mul(1013);
    // Temperature (0.0-1.0) and Humidity (0.0-1.0)
    let temp = ((h >> 16) as u32 % 100) as f32 / 100.0;
    let hum = ((h >> 32) as u32 % 100) as f32 / 100.0;
    let elev = sample_elevation(world_x, world_z, seed);

    // Ocean check: if elevation < 0.35
    if elev < 0.35 {
        if temp < 0.1 { return BiomeId::FrozenOcean; }
        if temp < 0.3 { return BiomeId::ColdOcean; }
        if temp > 0.7 { return BiomeId::WarmOcean; }
        if elev < 0.2 { return BiomeId::DeepOcean; }
        return BiomeId::Ocean;
    }
    // Beach check
    if elev < 0.42 {
        if temp < 0.1 { return BiomeId::SnowyBeach; }
        return BiomeId::Beach;
    }
    // River check (flat area, low elevation)
    if elev < 0.48 {
        if temp < 0.1 { return BiomeId::FrozenRiver; }
        return BiomeId::River;
    }

    // Mountains
    if elev > 0.7 {
        if temp < 0.15 { return BiomeId::FrozenPeaks; }
        if temp < 0.35 { return BiomeId::SnowySlopes; }
        if temp < 0.5 { return BiomeId::JaggedPeaks; }
        if temp < 0.65 { return BiomeId::StonyPeaks; }
        if hum < 0.3 { return BiomeId::WindsweptHills; }
        if hum < 0.6 { return BiomeId::Grove; }
        return BiomeId::Meadow;
    }

    // Cherry Grove (special, rare)
    if hum > 0.7 && temp > 0.4 && temp < 0.6 && elev > 0.5 {
        return BiomeId::CherryGrove;
    }

    // Arid
    if temp > 0.65 {
        if hum < 0.2 { return BiomeId::Desert; }
        if hum < 0.4 {
            if elev > 0.65 { return BiomeId::Badlands; }
            if elev > 0.55 { return BiomeId::ErodedBadlands; }
            return BiomeId::WoodedBadlands;
        }
        if elev > 0.6 { return BiomeId::SavannaPlateau; }
        return BiomeId::Savanna;
    }

    // Cold
    if temp < 0.25 {
        if hum > 0.6 { return BiomeId::SnowyTaiga; }
        if hum > 0.3 { return BiomeId::Taiga; }
        if hum > 0.15 { return BiomeId::OldGrowthSpruceTaiga; }
        if elev > 0.55 { return BiomeId::OldGrowthPineTaiga; }
        return BiomeId::SnowyPlains;
    }

    // Temperate forests
    if hum > 0.7 {
        if temp > 0.5 { return BiomeId::Jungle; }
        if temp > 0.4 { return BiomeId::BambooJungle; }
        if temp > 0.3 { return BiomeId::SparseJungle; }
        return BiomeId::DarkForest;
    }
    if hum > 0.55 {
        if temp > 0.35 { return BiomeId::BirchForest; }
        if temp > 0.3 { return BiomeId::OldGrowthBirchForest; }
        return BiomeId::FlowerForest;
    }
    if hum > 0.4 {
        if elev < 0.45 { return BiomeId::Swamp; }
        return BiomeId::Forest;
    }

    // Plains variants
    if hum < 0.2 { return BiomeId::SunflowerPlains; }
    if hum < 0.35 { return BiomeId::WindsweptForest; }
    if elev > 0.5 { return BiomeId::WindsweptGravellyHills; }

    BiomeId::Plains
}

/// Simple elevation noise (0.0-1.0)
fn sample_elevation(wx: i32, wz: i32, seed: u64) -> f32 {
    let h = (wx as u64).wrapping_mul(7331)
        ^ (wz as u64).wrapping_mul(5779)
        ^ seed.wrapping_mul(4242);
    let n = ((h >> 8) as u32) % 10000;
    n as f32 / 10000.0
}

/// Sample biome with Y-axis awareness for underground biomes (DeepDark, LushCaves, DripstoneCaves)
pub fn sample_biome_at_y(wx: i32, wz: i32, y: i32, seed: u64) -> mc_core::biome::BiomeId {
    use mc_core::biome::BiomeId;
    let surface = sample_biome(wx, wz, seed);

    // Underground biome override: only below Y=0
    if y >= 0 { return surface; }

    // 3D noise for cave biome selection
    let cave_hash = (wx as u64).wrapping_mul(6364136223846793005)
        ^ (wz as u64).wrapping_mul(1442695040888963407)
        ^ (y as u64).wrapping_mul(3862890488820784953)
        ^ seed.wrapping_mul(2368427849236549237);
    let cave_noise = ((cave_hash >> 16) as u32 % 100) as f32 / 100.0;
    let humidity_3d = ((cave_hash >> 32) as u32 % 100) as f32 / 100.0;

    // DeepDark: very deep (Y < -30), dark and eerie — rare chance
    if y < -30 && cave_noise > 0.85 {
        return BiomeId::DeepDark;
    }
    // LushCaves: moderate depth, high humidity
    if y < 0 && y > -30 && humidity_3d > 0.7 {
        return BiomeId::LushCaves;
    }
    // DripstoneCaves: moderate depth, low humidity, dry
    if y < 0 && y > -30 && humidity_3d < 0.3 && cave_noise > 0.5 {
        return BiomeId::DripstoneCaves;
    }

    surface
}

/// Fill a section's 4x4x4 biome palette container — optimized: 4×4 grid × 64 blocks, 1024 set calls vs 4096
pub fn fill_section_biomes(biomes: &mut crate::paletted::PalettedContainer, section_y: i32, chunk_x: i32, chunk_z: i32, seed: u64) {
    // Pre-sample biomes for the 4×4 grid using Y-aware sampling
    let base_y = section_y * 16;
    let mut biome_grid = [[BlockState::new(0); 4]; 4];
    for (bx, row) in biome_grid.iter_mut().enumerate() {
        for (bz, cell) in row.iter_mut().enumerate() {
            let wx = chunk_x * 16 + bx as i32 * 4 + 2;
            let wz = chunk_z * 16 + bz as i32 * 4 + 2;
            let wy = base_y + 8; // sample at section mid-point
            *cell = BlockState::new(sample_biome_at_y(wx, wz, wy, seed).id());
        }
    }
    // Bulk set: iterate inner positions, using pre-sampled biome
    for (bx, row) in biome_grid.iter().enumerate() {
        for (bz, &block) in row.iter().enumerate() {
            for by in 0..4usize {
                for dx in 0..4usize {
                    for dy in 0..4usize {
                        for dz in 0..4usize {
                            biomes.set(bx*4+dx, by*4+dy, bz*4+dz, block);
                        }
                    }
                }
            }
        }
    }
}

// ── Cave carving ──

#[allow(dead_code)]
fn carve_caves(chunk: &mut Chunk, seed: u64) {
    for x in 0..16usize {
        for z in 0..16usize {
            for y in -60..50i32 {
                let wx = chunk.position.x * 16 + x as i32;
                let wz = chunk.position.z * 16 + z as i32;
                let h = (wx as u64).wrapping_mul(374761393)
                    ^ (y as u64).wrapping_mul(668265263)
                    ^ (wz as u64).wrapping_mul(1274126177)
                    ^ seed.wrapping_mul(4242);
                if (h as i32).wrapping_abs() as u32 % 10000 < 800 {
                    chunk.set_block(x, y, z, blocks::AIR);
                }
            }
        }
    }
}

// ── Ore placement ──

const DEEPSLATE: u32 = 338;
const COAL_ORE: u32 = 31;
const IRON_ORE: u32 = 29;
const GOLD_ORE: u32 = 27;
const DIAMOND_ORE: u32 = 47;
const COPPER_ORE: u32 = 300;
const REDSTONE_ORE: u32 = 117;
const LAPIS_ORE: u32 = 59;
const EMERALD_ORE: u32 = 303;
const DEEPSLATE_IRON_ORE: u32 = 337;
const DEEPSLATE_GOLD_ORE: u32 = 338;
const DEEPSLATE_DIAMOND_ORE: u32 = 339;
const DEEPSLATE_COPPER_ORE: u32 = 340;
const DEEPSLATE_REDSTONE_ORE: u32 = 341;
const DEEPSLATE_LAPIS_ORE: u32 = 342;
const DEEPSLATE_EMERALD_ORE: u32 = 343;

fn place_ores(chunk: &mut Chunk, _seed: u64) {
    // Coal: y=0..128, ~20 veins
    for _ in 0..20 { place_vein(chunk, COAL_ORE, COAL_ORE, 0, 128, 17); }
    // Iron: y=-64..64, ~15 veins
    for _ in 0..15 { place_vein(chunk, IRON_ORE, DEEPSLATE_IRON_ORE, -64, 64, 9); }
    // Gold: y=-64..32, ~8 veins
    for _ in 0..8 { place_vein(chunk, GOLD_ORE, DEEPSLATE_GOLD_ORE, -64, 32, 9); }
    // Diamond: y=-64..16, ~1 vein
    place_vein(chunk, DIAMOND_ORE, DEEPSLATE_DIAMOND_ORE, -64, 16, 6);
    // Copper: y=-16..112, ~16 veins (common)
    for _ in 0..16 { place_vein(chunk, COPPER_ORE, DEEPSLATE_COPPER_ORE, -16, 112, 12); }
    // Redstone: y=-64..16, ~8 veins
    for _ in 0..8 { place_vein(chunk, REDSTONE_ORE, DEEPSLATE_REDSTONE_ORE, -64, 16, 8); }
    // Lapis Lazuli: y=-64..64, ~4 veins
    for _ in 0..4 { place_vein(chunk, LAPIS_ORE, DEEPSLATE_LAPIS_ORE, -64, 64, 7); }
    // Emerald: y=-16..320, only in Mountain biomes (rare single blocks)
    for _ in 0..3 { place_vein(chunk, EMERALD_ORE, DEEPSLATE_EMERALD_ORE, -16, 320, 3); }
}

fn place_vein(chunk: &mut Chunk, ore: u32, deep_ore: u32, y_min: i32, y_max: i32, size: u32) {
    let ox = fastrand::usize(0..16);
    let oz = fastrand::usize(0..16);
    let range = (y_max - y_min) as u32;
    let oy = y_min + (fastrand::u32(..) % range) as i32;
    for _ in 0..size {
        let dx = fastrand::i32(-1..2) as isize;
        let dy = fastrand::i32(-1..2) as isize;
        let dz = fastrand::i32(-1..2) as isize;
        let px = (ox as isize + dx).clamp(0, 15) as usize;
        let py = oy + dy as i32;
        let pz = (oz as isize + dz).clamp(0, 15) as usize;
        if (-64..=319).contains(&py) {
            let existing = chunk.get_block(px, py, pz);
            if existing == blocks::STONE || existing == BlockState::new(338) {
                // Use deepslate variant if below y=0
                let block = if py < 0 { deep_ore } else { ore };
                chunk.set_block(px, py, pz, BlockState::new(block));
            }
        }
    }
}



// ── Tree placement ──

fn place_trees(chunk: &mut Chunk, pos: ChunkPos, seed: u64, height_fn: &dyn Fn(i32, i32) -> i32) {
    for x in 4..12usize {
        for z in 4..12usize {
            let wx = pos.x * 16 + x as i32;
            let wz = pos.z * 16 + z as i32;
            let biome = sample_biome(wx, wz, seed);
            match biome {
                mc_core::biome::BiomeId::Desert | mc_core::biome::BiomeId::Ocean
                | mc_core::biome::BiomeId::SnowyPlains
                | mc_core::biome::BiomeId::WindsweptHills => continue,
                _ => {}
            }
            let h = (wx as u64).wrapping_mul(1619)
                ^ (wz as u64).wrapping_mul(31337)
                ^ seed.wrapping_mul(7919);
            let tree_chance = match biome {
                mc_core::biome::BiomeId::Forest | mc_core::biome::BiomeId::Jungle => 15, // denser forest
                mc_core::biome::BiomeId::Taiga => 10,
                _ => 5,
            };
            if (h as i32).wrapping_abs() as u32 % 100 >= tree_chance { continue; }

            let surface_y = height_fn(wx, wz);
            if !(-58..=300).contains(&surface_y) { continue; }

            match biome {
                mc_core::biome::BiomeId::Taiga => {
                    // Spruce tree: tall, conical
                    let trunk_h = 5 + (h >> 16) as i32 % 4; // 5-8 tall
                    for dy in 0..trunk_h {
                        chunk.set_block(x, surface_y + 1 + dy, z, blocks::SPRUCE_LOG);
                    }
                    // Conical leaves: shrinking layers
                    for ly in 0..4i32 {
                        let radius = 2 - (ly / 2);
                        let top_y = surface_y + trunk_h - 1 - ly;
                        for lx in -radius..=radius { for lz in -radius..=radius {
                            let sx = x as i32 + lx; let sz = z as i32 + lz;
                            if (0..16).contains(&sx) && (0..16).contains(&sz) {
                                chunk.set_block(sx as usize, top_y, sz as usize, blocks::SPRUCE_LEAVES);
                            }
                        }}
                    }
                    // Top spike
                    chunk.set_block(x, surface_y + trunk_h, z, blocks::SPRUCE_LEAVES);
                }
                mc_core::biome::BiomeId::Forest => {
                    // Birch tree: tall and thin with distinct bark
                    let trunk_h = 5 + (h >> 16) as i32 % 3; // 5-7
                    for dy in 0..trunk_h {
                        chunk.set_block(x, surface_y + 1 + dy, z, blocks::BIRCH_LOG);
                    }
                    let top = surface_y + trunk_h;
                    // Small canopy
                    for lx in -1i32..=1 { for lz in -1i32..=1 {
                        let sx = x as i32 + lx; let sz = z as i32 + lz;
                        if (0..16).contains(&sx) && (0..16).contains(&sz) {
                            chunk.set_block(sx as usize, top, sz as usize, blocks::BIRCH_LEAVES);
                            chunk.set_block(sx as usize, top - 1, sz as usize, blocks::BIRCH_LEAVES);
                        }
                    }}
                    for lx in -2i32..=2 { for lz in -2i32..=2 {
                        let sx = x as i32 + lx; let sz = z as i32 + lz;
                        if (lx.abs() == 2 || lz.abs() == 2) && (0..16).contains(&sx) && (0..16).contains(&sz) {
                            chunk.set_block(sx as usize, top - 1, sz as usize, blocks::BIRCH_LEAVES);
                        }
                    }}
                }
                mc_core::biome::BiomeId::Jungle => {
                    // Jungle tree: tall, 2x2 trunk possible, vines
                    let trunk_h = 8 + (h >> 16) as i32 % 5; // 8-12 tall
                    let base_y = surface_y + 1;
                    // Thick trunk (2x2) if wide enough
                    let wide = ((h >> 8) as u32).is_multiple_of(3) && x < 15 && z < 15;
                    if wide {
                        for dy in 0..trunk_h {
                            chunk.set_block(x, base_y + dy, z, blocks::JUNGLE_LOG);
                            chunk.set_block(x+1, base_y + dy, z, blocks::JUNGLE_LOG);
                            chunk.set_block(x, base_y + dy, z+1, blocks::JUNGLE_LOG);
                            chunk.set_block(x+1, base_y + dy, z+1, blocks::JUNGLE_LOG);
                        }
                    } else {
                        for dy in 0..trunk_h { chunk.set_block(x, base_y + dy, z, blocks::JUNGLE_LOG); }
                    }
                    let top = base_y + trunk_h;
                    for lx in -2..=2i32 { for lz in -2..=2i32 {
                        if lx.abs() + lz.abs() <= 3 {
                            let sx = x as i32 + lx; let sz = z as i32 + lz;
                            if (0..16).contains(&sx) && (0..16).contains(&sz) {
                                for ly in 0..2 { chunk.set_block(sx as usize, top + ly, sz as usize, blocks::JUNGLE_LEAVES); }
                            }
                        }
                    }}
                }
                mc_core::biome::BiomeId::Swamp => {
                    // Acacia-style: flat top, diagonal trunk
                    let trunk_h = 5 + (h >> 16) as i32 % 3;
                    for dy in 0..trunk_h { chunk.set_block(x, surface_y + 1 + dy, z, blocks::ACACIA_LOG); }
                    let top = surface_y + trunk_h;
                    // Flat wide canopy
                    for lx in -2..=2i32 { for lz in -2..=2i32 {
                        if lx.abs() + lz.abs() <= 3 {
                            let sx = x as i32 + lx; let sz = z as i32 + lz;
                            if (0..16).contains(&sx) && (0..16).contains(&sz) {
                                chunk.set_block(sx as usize, top, sz as usize, blocks::ACACIA_LEAVES);
                            }
                        }
                    }}
                }
                mc_core::biome::BiomeId::DarkForest => {
                    // Dark oak: 2×2 thick trunk, dense canopy
                    let trunk_h = 5 + (h >> 16) as i32 % 3; // 5-7 tall
                    let base_y = surface_y + 1;
                    let thick = x < 15 && z < 15; // 2×2 trunk
                    if thick {
                        for dy in 0..trunk_h {
                            chunk.set_block(x, base_y + dy, z, blocks::DARK_OAK_LOG);
                            chunk.set_block(x+1, base_y + dy, z, blocks::DARK_OAK_LOG);
                            chunk.set_block(x, base_y + dy, z+1, blocks::DARK_OAK_LOG);
                            chunk.set_block(x+1, base_y + dy, z+1, blocks::DARK_OAK_LOG);
                        }
                    } else {
                        for dy in 0..trunk_h { chunk.set_block(x, base_y + dy, z, blocks::DARK_OAK_LOG); }
                    }
                    let top = base_y + trunk_h;
                    for lx in -2..=2i32 { for lz in -2..=2i32 {
                        if lx.abs() <= 2 && lz.abs() <= 2 {
                            let sx = x as i32 + lx; let sz = z as i32 + lz;
                            if (0..16).contains(&sx) && (0..16).contains(&sz) {
                                for ly in 0..3 { chunk.set_block(sx as usize, top + ly, sz as usize, blocks::DARK_OAK_LEAVES); }
                            }
                        }
                    }}
                }
                mc_core::biome::BiomeId::MangroveSwamp => {
                    // Mangrove: tall with aerial roots + wide canopy
                    let trunk_h = 6 + (h >> 16) as i32 % 4; // 6-9 tall
                    let base_y = surface_y + 1;
                    for dy in 0..trunk_h { chunk.set_block(x, base_y + dy, z, blocks::MANGROVE_LOG); }
                    // Aerial roots extending outward from base
                    for dy in 0..3i32 {
                        for (dx, dz) in &[(1,0), (-1,0), (0,1), (0,-1)] {
                            let rx = x as i32 + dx; let rz = z as i32 + dz;
                            if (0..16).contains(&rx) && (0..16).contains(&rz) {
                                chunk.set_block(rx as usize, base_y + dy, rz as usize, blocks::MANGROVE_LOG);
                            }
                        }
                    }
                    let top = base_y + trunk_h;
                    for lx in -2..=2i32 { for lz in -2..=2i32 {
                        let sx = x as i32 + lx; let sz = z as i32 + lz;
                        if (0..16).contains(&sx) && (0..16).contains(&sz) {
                            chunk.set_block(sx as usize, top, sz as usize, blocks::MANGROVE_LEAVES);
                            chunk.set_block(sx as usize, top - 1, sz as usize, blocks::MANGROVE_LEAVES);
                        }
                    }}
                    for lx in -2..=1i32 { for lz in -2..=1i32 {
                        let sx = x as i32 + lx; let sz = z as i32 + lz;
                        if (0..16).contains(&sx) && (0..16).contains(&sz) {
                            chunk.set_block(sx as usize, top + 1, sz as usize, blocks::MANGROVE_LEAVES);
                        }
                    }}
                }
                mc_core::biome::BiomeId::CherryGrove => {
                    // Cherry: rounded pink canopy, medium height
                    let trunk_h = 4 + (h >> 16) as i32 % 4; // 4-7 tall
                    let base_y = surface_y + 1;
                    for dy in 0..trunk_h { chunk.set_block(x, base_y + dy, z, blocks::CHERRY_LOG); }
                    let top = base_y + trunk_h;
                    // Rounded canopy (3 layers)
                    for ly in -1..=1i32 {
                        let radius = 2 - ly.abs();
                        for lx in -radius..=radius { for lz in -radius..=radius {
                            let sx = x as i32 + lx; let sz = z as i32 + lz;
                            if (0..16).contains(&sx) && (0..16).contains(&sz) {
                                chunk.set_block(sx as usize, top + ly + 1, sz as usize, blocks::CHERRY_LEAVES);
                            }
                        }}
                    }
                }
                mc_core::biome::BiomeId::PaleGarden => {
                    // Pale oak: dark oak shape with pale leaves, hanging moss effect
                    let trunk_h = 5 + (h >> 16) as i32 % 3; // 5-7 tall
                    let base_y = surface_y + 1;
                    let thick = x < 15 && z < 15 && ((h >> 8) as u32).is_multiple_of(2);
                    if thick {
                        for dy in 0..trunk_h {
                            chunk.set_block(x, base_y + dy, z, blocks::PALE_OAK_LOG);
                            chunk.set_block(x+1, base_y + dy, z, blocks::PALE_OAK_LOG);
                            chunk.set_block(x, base_y + dy, z+1, blocks::PALE_OAK_LOG);
                            chunk.set_block(x+1, base_y + dy, z+1, blocks::PALE_OAK_LOG);
                        }
                    } else {
                        for dy in 0..trunk_h { chunk.set_block(x, base_y + dy, z, blocks::PALE_OAK_LOG); }
                    }
                    let top = base_y + trunk_h;
                    for lx in -2..=2i32 { for lz in -2..=2i32 {
                        let sx = x as i32 + lx; let sz = z as i32 + lz;
                        if (0..16).contains(&sx) && (0..16).contains(&sz) {
                            for ly in 0..2 { chunk.set_block(sx as usize, top + ly, sz as usize, blocks::PALE_OAK_LEAVES); }
                        }
                    }}
                }
                _ => {
                    // Oak tree (default)
                    let trunk_h = 4 + (h >> 16) as i32 % 3;
                    for dy in 0..trunk_h {
                        chunk.set_block(x, surface_y + 1 + dy, z, blocks::OAK_LOG);
                    }
                    let top = surface_y + trunk_h;
                    for lx in -1i32..=1 { for lz in -1i32..=1 {
                        let sx = x as i32 + lx; let sz = z as i32 + lz;
                        if (0..16).contains(&sx) && (0..16).contains(&sz) {
                            chunk.set_block(sx as usize, top, sz as usize, blocks::OAK_LEAVES);
                            chunk.set_block(sx as usize, top + 1, sz as usize, blocks::OAK_LEAVES);
                        }
                    }}
                    let top2 = top + 1;
                    for lx in -2i32..=2 { for lz in -2i32..=2 {
                        let sx = x as i32 + lx; let sz = z as i32 + lz;
                        if (0..16).contains(&sx) && (0..16).contains(&sz) {
                            chunk.set_block(sx as usize, top2, sz as usize, blocks::OAK_LEAVES);
                        }
                    }}
                }
            }
        }
    }
}

// ── Surface vegetation ──

fn place_vegetation(chunk: &mut Chunk, pos: ChunkPos, seed: u64, height_fn: &dyn Fn(i32, i32) -> i32) {
    for x in 0..16usize {
        for z in 0..16usize {
            let wx = pos.x * 16 + x as i32;
            let wz = pos.z * 16 + z as i32;
            let biome = sample_biome(wx, wz, seed);
            let h = (wx as u64).wrapping_mul(31397)
                ^ (wz as u64).wrapping_mul(21419)
                ^ seed.wrapping_mul(18181);

            // ~30% chance per position to place vegetation
            if (h as i32).wrapping_abs() as u32 % 100 >= 30 { continue; }

            let surface_y = height_fn(wx, wz);
            if !(-60..=300).contains(&surface_y) { continue; }

            match biome {
                mc_core::biome::BiomeId::Plains => {
                    // Tall grass + flowers
                    let pick = (h >> 4) as u32 % 12;
                    let block = match pick {
                        0..=4 => blocks::SHORT_GRASS,
                        5 => blocks::DANDELION,
                        6 => blocks::POPPY,
                        7 => blocks::AZURE_BLUET,
                        8 => blocks::OXEYE_DAISY,
                        9 => blocks::CORNFLOWER,
                        10 => blocks::ALLIUM,
                        _ => blocks::TALL_GRASS,
                    };
                    chunk.set_block(x, surface_y + 1, z, block);
                }
                mc_core::biome::BiomeId::Forest => {
                    let pick = (h >> 4) as u32 % 8;
                    let block = match pick {
                        0..=3 => blocks::SHORT_GRASS,
                        4 => blocks::FERN,
                        5 => blocks::LILAC,
                        6 => blocks::PEONY,
                        _ => blocks::ROSE_BUSH,
                    };
                    chunk.set_block(x, surface_y + 1, z, block);
                }
                mc_core::biome::BiomeId::Taiga => {
                    // Mostly ferns
                    let block = if h.is_multiple_of(4) { blocks::FERN } else { blocks::SHORT_GRASS };
                    chunk.set_block(x, surface_y + 1, z, block);
                }
                mc_core::biome::BiomeId::Swamp => {
                    let block = match (h >> 4) as u32 % 6 {
                        0..=3 => blocks::SHORT_GRASS,
                        4 => blocks::BLUE_ORCHID,
                        _ => blocks::FERN,
                    };
                    chunk.set_block(x, surface_y + 1, z, block);
                }
                mc_core::biome::BiomeId::Jungle => {
                    let block = if h.is_multiple_of(3) { blocks::FERN } else { blocks::SHORT_GRASS };
                    chunk.set_block(x, surface_y + 1, z, block);
                    // Melon patch (~2% chance per position)
                    if h.is_multiple_of(50) {
                        for dx in -1i32..=1 { for dz in -1i32..=1 {
                            if (dx.abs() + dz.abs()) <= 1 {
                                chunk.set_block((x as i32 + dx) as usize, surface_y, (z as i32 + dz) as usize, blocks::MELON);
                            }
                        }}
                    }
                }
                mc_core::biome::BiomeId::Desert => {
                    if h.is_multiple_of(10) { chunk.set_block(x, surface_y + 1, z, blocks::DEAD_BUSH); }
                    // Cactus: 1-3 tall on sand
                    if h.is_multiple_of(25) {
                        let surface = chunk.get_block(x, surface_y, z);
                        if surface.id == 12 { // sand
                            let height = 1 + (h as u32 % 3);
                            for dy in 1..=height {
                                chunk.set_block(x, surface_y + dy as i32, z, blocks::CACTUS);
                            }
                        }
                    }
                }
                mc_core::biome::BiomeId::Badlands | mc_core::biome::BiomeId::ErodedBadlands
                    if h.is_multiple_of(8) => { chunk.set_block(x, surface_y + 1, z, blocks::DEAD_BUSH); }
                _ => {}
            }

            // ── Sugar cane near water (any biome, ~5% chance) ──
            if h.is_multiple_of(20) {
                let surface = chunk.get_block(x, surface_y, z);
                if surface.id == 12 || surface.id == 13 || surface.id == 2 { // sand/dirt/grass near water
                    // Check for adjacent water
                    let mut near_water = false;
                    for (dx, dz) in &[(1,0),(-1,0),(0,1),(0,-1)] {
                        let nx = (x as i32 + dx) as usize;
                        let nz = (z as i32 + dz) as usize;
                        if nx < 16 && nz < 16 {
                            let neighbor = chunk.get_block(nx, surface_y, nz);
                            if neighbor.id == 267 { near_water = true; break; } // water
                        }
                    }
                    if near_water {
                        let cane_height = 2 + (h as u32 % 3);
                        for dy in 1..=cane_height {
                            chunk.set_block(x, surface_y + dy as i32, z, blocks::SUGAR_CANE);
                        }
                    }
                }
            }
        }
    }
}

// ── Village placement ──

fn place_village(chunk: &mut Chunk, pos: ChunkPos, seed: u64, height_fn: &dyn Fn(i32, i32) -> i32) {
    // ~1 in 256 chunks gets a village
    let h = (pos.x as u64).wrapping_mul(131)
        ^ (pos.z as u64).wrapping_mul(137)
        ^ seed.wrapping_mul(7919);
    if !((h as i32).wrapping_abs() as u32).is_multiple_of(256) { return; }

    let biome = sample_biome(pos.x * 16 + 8, pos.z * 16 + 8, seed);
    if !matches!(biome, mc_core::biome::BiomeId::Plains | mc_core::biome::BiomeId::Desert) { return; }

    let cx = 8i32; let cz = 8i32;
    let base_y = height_fn(pos.x * 16 + cx, pos.z * 16 + cz);
    if base_y < -50 { return; }

    // Enhanced village: central well + 3 small houses (librarian, butcher, farm)
    let floor = BlockState::new(13);
    let wall = BlockState::new(34);
    let _glass = BlockState::new(66);
    let door = BlockState::new(852);
    let path = BlockState::new(199); // grass_path
    let water = BlockState::new(267);

    // Central well (3x3 water)
    let (wx, wz) = (cx as usize, cz as usize);
    if (1..=14).contains(&wx) && (2..=13).contains(&wz) {
        chunk.set_block(wx - 1, base_y, wz + 1, water);
        chunk.set_block(wx, base_y, wz + 1, water);
        chunk.set_block(wx + 1, base_y, wz + 1, water);
    }
    // Surround well with stone bricks
    for dx in -1i32..=2 { for dz in 0i32..=2 {
        let sx = (cx + dx) as usize; let sz = (cz + 1 + dz - 1) as usize;
        if sx < 16 && sz < 16 { chunk.set_block(sx, base_y + 1, sz, BlockState::new(98)); }
    }}

    // 3 small houses around the well
    let houses: [(i32, i32, i32, i32); 3] = [
        (-5, -4, 4, 4),  // west house
        (3, -2, 5, 4),   // east house
        (-2, 4, 4, 4),   // south house
    ];
    for (hx, hz, w, d) in houses {
        // Floor
        for fx in 0..w { for fz in 0..d {
            let sx = (cx + hx + fx) as usize; let sz = (cz + hz + fz) as usize;
            if sx < 16 && sz < 16 { chunk.set_block(sx, base_y, sz, floor); }
        }}
        // Log walls (corners + edges)
        for fy in 1..4 {
            let (wsx, wsz) = ((cx + hx) as usize, (cz + hz) as usize);
            let (wex, wez) = ((cx + hx + w - 1) as usize, (cz + hz + d - 1) as usize);
            if wsx < 16 && wsz < 16 { chunk.set_block(wsx, base_y + fy, wsz, wall); }
            if wex < 16 && wez < 16 { chunk.set_block(wex, base_y + fy, wez, wall); }
            if wsx < 16 && wez < 16 { chunk.set_block(wsx, base_y + fy, wez, wall); }
            if wex < 16 && wsz < 16 { chunk.set_block(wex, base_y + fy, wsz, wall); }
        }
        // Door (center of front face)
        let dx = (cx + hx + w/2) as usize; let dz = (cz + hz) as usize;
        if dx < 16 && dz < 16 { chunk.set_block(dx, base_y + 1, dz, door); }
    }
    // Dirt paths connecting houses to well
    for dx in -5i32..=6 { let sx = (cx + dx) as usize;
        if sx < 16 { chunk.set_block(sx, base_y, cz as usize, path); }
    }
    for dz in -4i32..=4 { let sz = (cz + dz) as usize;
        if sz < 16 { chunk.set_block(cx as usize, base_y, sz, path); }
    }

    // Walls and glass (existing code adapted)
    for wx in 0..12i32 {
        for wy in 1..4i32 {
            if let (sx @ 0..=15, sz @ 0..=15) = ((cx + wx - 5) as usize, (cz - 5) as usize)
                && (wx == 0 || wx == 11 || wy == 3) {
                    chunk.set_block(sx, base_y + wy, sz, wall);
                }
        }
    }
}

// ── Nether Generator ──

pub struct NetherGenerator;

fn sample_nether_biome(wx: i32, wz: i32, seed: u64) -> mc_core::biome::BiomeId {
    let h = (wx as u64).wrapping_mul(0x9E3779B9) ^ (wz as u64).wrapping_mul(0x9E3779B9) ^ seed.wrapping_mul(0x9E3779B9);
    match ((h as i32).wrapping_abs() as u32) % 5 {
        0 => mc_core::biome::BiomeId::NetherWastes,
        1 => mc_core::biome::BiomeId::SoulSandValley,
        2 => mc_core::biome::BiomeId::CrimsonForest,
        3 => mc_core::biome::BiomeId::WarpedForest,
        _ => mc_core::biome::BiomeId::BasaltDeltas,
    }
}

fn sample_end_biome(wx: i32, wz: i32) -> mc_core::biome::BiomeId {
    let dist = ((wx as f64).powi(2) + (wz as f64).powi(2)).sqrt();
    if dist < 80.0 { return mc_core::biome::BiomeId::TheEnd; }
    if dist < 500.0 { return mc_core::biome::BiomeId::EndMidlands; }
    if dist < 1000.0 { return mc_core::biome::BiomeId::EndBarrens; }
    if (wx/32 + wz/32) % 3 == 0 { mc_core::biome::BiomeId::EndHighlands }
    else if (wx/32 + wz/32) % 3 == 1 { mc_core::biome::BiomeId::SmallEndIslands }
    else { mc_core::biome::BiomeId::EndBarrens }
}

impl TerrainGenerator for NetherGenerator {
    fn name(&self) -> &str { "nether" }
    fn generate_chunk(&self, pos: ChunkPos, seed: u64) -> Chunk {
        let mut chunk = Chunk::new(pos);
        for x in 0..16usize {
            for z in 0..16usize {
                let wx = pos.x * 16 + x as i32;
                let wz = pos.z * 16 + z as i32;
                let biome = sample_nether_biome(wx, wz, seed);
                chunk.set_block(x, 0, z, blocks::BEDROCK);
                chunk.set_block(x, 127, z, blocks::BEDROCK);
                let (surface, _sub, _deep) = biome.surface_blocks();
                for y in 1..127i32 {
                    let b = match biome {
                        mc_core::biome::BiomeId::SoulSandValley => BlockState::new(88),
                        mc_core::biome::BiomeId::BasaltDeltas => BlockState::new(87),
                        _ => surface, // NetherWastes/Crimson/Warped → netherrack
                    };
                    chunk.set_block(x, y, z, b);
                }
                // Soul sand valley: soul_soil patches
                if biome == mc_core::biome::BiomeId::SoulSandValley && (x+z) % 3 == 0 {
                    for y in 32..38i32 { chunk.set_block(x, y, z, BlockState::new(88)); }
                }
                // Basalt deltas: lava pockets
                if biome == mc_core::biome::BiomeId::BasaltDeltas && (x+z) % 5 == 0 {
                    for y in 28..34i32 { chunk.set_block(x, y, z, BlockState::new(268)); }
                }
                // Glowstone clusters
                if (wx.wrapping_mul(37i32)) % 11 == 0 {
                    chunk.set_block(x, 118 + (z % 5) as i32, z, BlockState::new(89));
                }
            }
        }
        place_nether_fortress(&mut chunk, pos, seed);
        crate::lighting::init_chunk_lighting(&mut chunk);
        chunk.dirty = false;
        chunk
    }
}

// ── End Generator ──

pub struct EndGenerator;

impl TerrainGenerator for EndGenerator {
    fn name(&self) -> &str { "end" }
    fn generate_chunk(&self, pos: ChunkPos, seed: u64) -> Chunk {
        let mut chunk = Chunk::new(pos);
        for x in 0..16usize {
            for z in 0..16usize {
                let wx = pos.x * 16 + x as i32;
                let wz = pos.z * 16 + z as i32;
                let biome = sample_end_biome(wx, wz);
                let dist = ((wx as f64).powi(2) + (wz as f64).powi(2)).sqrt();
                if dist < 80.0 {
                    for y in 60..65i32 { chunk.set_block(x, y, z, BlockState::new(121)); }
                    if dist > 20.0 && dist < 60.0 && (wx + wz) % 20 == 0 {
                        for y in 65..75i32 { chunk.set_block(x, y, z, BlockState::new(71)); }
                    }
                } else if dist >= 1000.0 && biome == mc_core::biome::BiomeId::EndHighlands {
                    let h = ((wx as f64 * 0.05).sin() * (wz as f64 * 0.07).cos() * 20.0) as i32 + 60;
                    for y in h..h+5i32 {
                        if (0..=255).contains(&y) { chunk.set_block(x, y, z, BlockState::new(121)); }
                    }
                    if (wx + wz) % 13 == 0 {
                        for y in h+5..h+10i32 { chunk.set_block(x, y, z, BlockState::new(199)); }
                        chunk.set_block(x, h+10, z, BlockState::new(200));
                    }
                }
            }
        }
        place_end_city(&mut chunk, pos, seed);
        crate::lighting::init_chunk_lighting(&mut chunk);
        chunk.dirty = false;
        chunk
    }
}

// ── Structure generators ──

fn place_desert_temple(chunk: &mut Chunk, pos: ChunkPos, seed: u64) {
    let h = (pos.x as u64).wrapping_mul(131) ^ (pos.z as u64).wrapping_mul(139) ^ seed.wrapping_mul(8801);
    if !h.is_multiple_of(512) { return; }
    if !matches!(sample_biome(pos.x*16+8, pos.z*16+8, seed), mc_core::biome::BiomeId::Desert) { return; }
    let base_y = -56i32; let cx = 8usize; let cz = 8usize;
    let sandstone = BlockState::new(71); let terracotta = BlockState::new(179);
    for fx in 0..8usize { for fz in 0..8usize {
        if fx < 8 && fz < 8 { chunk.set_block(cx-4+fx, base_y, cz-4+fz, sandstone); }
    }}
    // Walls
    for wy in 1..5i32 {
        for wx in 0..8usize {
            chunk.set_block(cx-4+wx, base_y+wy, cz-4, sandstone);
            chunk.set_block(cx-4+wx, base_y+wy, cz+3, sandstone);
        }
    }
    // Secret chamber below
    chunk.set_block(cx, base_y-3, cz, terracotta);
    chunk.set_block(cx, base_y-3, cz, BlockState::new(46)); // TNT trap
}

fn place_swamp_hut(chunk: &mut Chunk, pos: ChunkPos, seed: u64) {
    let h = (pos.x as u64).wrapping_mul(173) ^ (pos.z as u64).wrapping_mul(179) ^ seed.wrapping_mul(9109);
    if !h.is_multiple_of(400) { return; }
    if !matches!(sample_biome(pos.x*16+8, pos.z*16+8, seed), mc_core::biome::BiomeId::Swamp) { return; }
    let base_y = -56i32; let cx = 8usize; let cz = 8usize;
    let spruce = BlockState::new(35); let planks = BlockState::new(14);
    for fx in 0..6usize { for fz in 0..6usize {
        chunk.set_block(cx-3+fx, base_y, cz-3+fz, planks);
    }}
    for wy in 1..4i32 {
        for wx in 0..6usize {
            if wx == 0 || wx == 5 || wy == 3 {
                chunk.set_block(cx-3+wx, base_y+wy, cz-3, spruce);
                chunk.set_block(cx-3+wx, base_y+wy, cz+2, spruce);
            }
        }
    }
}

fn place_igloo(chunk: &mut Chunk, pos: ChunkPos, seed: u64) {
    let h = (pos.x as u64).wrapping_mul(191) ^ (pos.z as u64).wrapping_mul(193) ^ seed.wrapping_mul(9901);
    if !h.is_multiple_of(450) { return; }
    if !matches!(sample_biome(pos.x*16+8, pos.z*16+8, seed), mc_core::biome::BiomeId::SnowyPlains) { return; }
    let base_y = -57i32; let cx = 8usize; let cz = 8usize;
    let snow = BlockState::new(80); let ice = BlockState::new(119);
    for fx in 0..5usize { for fz in 0..5usize {
        chunk.set_block(cx-2+fx, base_y, cz-2+fz, snow);
    }}
    for wy in 1..4i32 { chunk.set_block(cx, base_y+wy, cz, snow); chunk.set_block(cx+1, base_y+wy, cz, snow); }
    chunk.set_block(cx+2, base_y+1, cz+1, ice); // window
}

#[allow(dead_code)]
fn place_stronghold(chunk: &mut Chunk, _pos: ChunkPos, seed: u64) {
    let h = (chunk.position.x as u64).wrapping_mul(0x9E3779B9) ^ (chunk.position.z as u64).wrapping_mul(0x9E3779B9) ^ seed.wrapping_mul(0x9E3779B9);
    if !h.is_multiple_of(2048) { return; }
    let base_y = 30;
    let stone_brick = BlockState::new(98); // stone_bricks
    let mossy = BlockState::new(99);       // mossy_stone_bricks
    let air = BlockState::new(0);
    for x in 0..14i32 { for z in 0..10i32 { for y in 0..5i32 {
        let bx = ((x + 1) as usize).min(15);
        let bz = ((z + 3) as usize).min(15);
        if x == 0 || x == 13 || z == 0 || z == 9 || y == 0 || y == 4 { chunk.set_block(bx, base_y+y, bz, stone_brick); }
        else { chunk.set_block(bx, base_y+y, bz, air); }
    }}}
    // Library room with mossy accents
    for lx in 2..5i32 { for lz in 2..5i32 { chunk.set_block(lx as usize + 1, base_y+1, (lz+5).min(14) as usize, mossy); }}
    // End portal frame blocks
    if h.is_multiple_of(5) {
        for pex in 5..9usize { for pez in 4..7usize { chunk.set_block(pex, base_y, pez, BlockState::new(120)); }}
    }
}

fn place_nether_fortress(chunk: &mut Chunk, pos: ChunkPos, seed: u64) {
    let h = (pos.x as u64).wrapping_mul(0x7FEB352D) ^ (pos.z as u64).wrapping_mul(0x7FEB352D) ^ seed.wrapping_mul(0x7FEB352D);
    if !h.is_multiple_of(800) { return; }
    let biome = sample_nether_biome(pos.x * 16 + 8, pos.z * 16 + 8, seed);
    if !matches!(biome, mc_core::biome::BiomeId::NetherWastes | mc_core::biome::BiomeId::SoulSandValley) { return; }
    let base_y = 60;
    let brick = BlockState::new(112);  // nether_bricks
    let fence = BlockState::new(113);  // nether_brick_fence
    for x in 0..12i32 { for z in 0..12i32 {
        let bx = ((x + 2) as usize).min(15);
        let bz = ((z + 2) as usize).min(15);
        for y in 0..6i32 {
            if x == 0 || x == 11 || z == 0 || z == 11 || y == 0 || y == 5 { chunk.set_block(bx, base_y+y, bz, brick); }
        }
        // Fence pillars
        if x % 4 == 0 && z % 4 == 0 { for y in 0..4i32 { chunk.set_block(bx, base_y+y, bz, fence); }}
    }}
}

fn place_end_city(chunk: &mut Chunk, pos: ChunkPos, _seed: u64) {
    let h = (pos.x as u64).wrapping_mul(0x1D1D1D1D) ^ (pos.z as u64).wrapping_mul(0x1D1D1D1D) ^ (pos.x as u64).wrapping_mul(pos.z as u64);
    if !h.is_multiple_of(600) { return; }
    let biome = sample_end_biome(pos.x * 16 + 8, pos.z * 16 + 8);
    if biome != mc_core::biome::BiomeId::EndHighlands { return; }
    let base_y = 65;
    let purpur = BlockState::new(201); // purpur_block
    let pillar = BlockState::new(202); // purpur_pillar
    let rod = BlockState::new(203);    // end_rod
    for x in 0..8i32 { for z in 0..8i32 {
        let bx = ((x + 4) as usize).min(15);
        let bz = ((z + 4) as usize).min(15);
        for y in 0..8i32 {
            if x == 0 || x == 7 || z == 0 || z == 7 || y == 0 || y == 7 { chunk.set_block(bx, base_y+y, bz, purpur); }
        }
        // Pillar accents
        if (x == 2 || x == 5) && (z == 2 || z == 5) { for y in 0..6i32 { chunk.set_block(bx, base_y+y, bz, pillar); }}
    }}
    chunk.set_block(8, base_y+6, 8, rod);
}

fn place_jungle_temple(chunk: &mut Chunk, pos: ChunkPos, seed: u64) {
    let h = (pos.x as u64).wrapping_mul(0x5DEECE66D) ^ (pos.z as u64).wrapping_mul(0x5DEECE66D) ^ seed.wrapping_mul(0x5DEECE66D);
    if !h.is_multiple_of(512) { return; }
    let biome = sample_biome(pos.x * 16 + 8, pos.z * 16 + 8, seed);
    if biome != mc_core::biome::BiomeId::Jungle { return; } // Jungle only

    let base_y = 64;
    let cobble = BlockState::new(12); // cobblestone
    let mossy = BlockState::new(13);   // mossy_cobblestone (approximate)
    let chiseled = BlockState::new(14); // chiseled stone (approximate)
    let air = BlockState::new(0);

    for x in 0..12i32 { for z in 0..10i32 { for y in 0..6i32 {
        let bx = (x as usize).min(15);
        let bz = (z as usize).min(15);
        let by = base_y + y;
        if x == 0 || x == 11 || z == 0 || z == 9 || y == 0 || y == 5 {
            chunk.set_block(bx, by, bz, cobble);
        } else {
            chunk.set_block(bx, by, bz, air);
        }
    }}}
    // Decorative chiseled stone blocks
    chunk.set_block(2, base_y + 2, 1, chiseled);
    chunk.set_block(9, base_y + 2, 1, chiseled);
    // Mossy accents
    chunk.set_block(5, base_y, 6, mossy);
    chunk.set_block(5, base_y, 3, mossy);
}

fn place_shipwreck(chunk: &mut Chunk, pos: ChunkPos, seed: u64, height_fn: &dyn Fn(i32, i32) -> i32) {
    let h = (pos.x as u64).wrapping_mul(0x5DEECE66D) ^ (pos.z as u64).wrapping_mul(0x5DEECE66D) ^ seed.wrapping_mul(0x5DEECE66D);
    if !h.is_multiple_of(800) { return; }
    let biome = sample_biome(pos.x * 16 + 8, pos.z * 16 + 8, seed);
    if !matches!(biome, mc_core::biome::BiomeId::Ocean) { return; }
    let base_y = height_fn(pos.x * 16 + 8, pos.z * 16 + 8);
    // Simple wooden ship: planks + logs
    let plank = BlockState::new(13); let log = BlockState::new(34);
    for dx in 4..12i32 { for dz in 4..8i32 {
        if (0..16).contains(&dx) && (0..16).contains(&dz) {
            chunk.set_block(dx as usize, base_y, dz as usize, plank);
            if dz == 5 && dx % 3 == 0 { chunk.set_block(dx as usize, base_y + 1, dz as usize, log); }
        }
    }}
}

fn place_ruined_portal(chunk: &mut Chunk, pos: ChunkPos, seed: u64, height_fn: &dyn Fn(i32, i32) -> i32) {
    let h = (pos.x as u64).wrapping_mul(0x9E3779B9) ^ (pos.z as u64).wrapping_mul(0x9E3779B9) ^ seed.wrapping_mul(0x9E3779B9);
    if !h.is_multiple_of(600) { return; }
    let base_y = height_fn(pos.x * 16 + 8, pos.z * 16 + 8);
    if base_y < 50 { return; }
    // Ruined nether portal: obsidian frame + crying obsidian + gold block
    let obsidian = BlockState::new(49); let crying = BlockState::new(310);
    let gold_blk = BlockState::new(101);
    let cx = 8i32; let cz = 8i32;
    for dx in -1..=2i32 { for dy in 0..4i32 {
        for &(sx, sz) in &[(cx + dx, cz - 1), (cx + dx, cz + 3)] {
            if (0..16).contains(&sx) && (0..16).contains(&sz) {
                let block = if dx == 0 && dy == 1 { crying } else { obsidian };
                chunk.set_block(sx as usize, base_y + dy, sz as usize, block);
            }
        }
    }}
    chunk.set_block((cx-1) as usize, base_y + 1, (cz+1) as usize, gold_blk);
}

fn place_ocean_monument(chunk: &mut Chunk, pos: ChunkPos, seed: u64) {
    let h = (pos.x as u64).wrapping_mul(0x6C078965) ^ (pos.z as u64).wrapping_mul(0x6C078965) ^ seed.wrapping_mul(0x6C078965);
    if !h.is_multiple_of(1024) { return; }
    let biome = sample_biome(pos.x * 16 + 8, pos.z * 16 + 8, seed);
    if biome != mc_core::biome::BiomeId::Ocean { return; }

    let base_y = 45;
    let prismarine = BlockState::new(168);   // prismarine
    let sea_lantern = BlockState::new(169);  // sea_lantern
    let sponge = BlockState::new(19);        // sponge

    for x in 0..12i32 { for z in 0..12i32 { for y in 0..5i32 {
        let bx = (x as usize + 2).min(15);
        let bz = (z as usize + 2).min(15);
        let by = base_y + y;
        if x == 0 || x == 11 || z == 0 || z == 11 || y == 0 || y == 4 {
            chunk.set_block(bx, by, bz, prismarine);
        }
    }}}
    // Sea lantern accents
    chunk.set_block(4, base_y + 3, 4, sea_lantern);
    chunk.set_block(8, base_y + 3, 8, sea_lantern);
    // Sponge room
    chunk.set_block(6, base_y + 1, 6, sponge);
    chunk.set_block(6, base_y + 1, 7, sponge);
}

fn place_mineshaft(chunk: &mut Chunk, pos: ChunkPos, seed: u64) {
    let h = (pos.x as u64).wrapping_mul(211) ^ (pos.z as u64).wrapping_mul(223) ^ seed.wrapping_mul(10103);
    if !h.is_multiple_of(200) { return; }
    let planks = BlockState::new(13); let fence = BlockState::new(853); let rail = BlockState::new(854);
    let y = 30i32; let cx = 8usize;
    // Horizontal tunnel 3x3
    for dy in 0..3i32 { for dx in 0..16usize {
        let existing = chunk.get_block(dx, y+dy, cx);
        if existing == BlockState::new(1) || existing == BlockState::new(338) {
            chunk.set_block(dx, y+dy, cx, BlockState::AIR);
        }
    }}
    // Support beams every 5 blocks
    for dx in (0..16usize).step_by(5) {
        chunk.set_block(dx, y, cx, planks);
        chunk.set_block(dx, y+1, cx, fence);
        chunk.set_block(dx, y+2, cx, fence);
    }
    chunk.set_block(8, y, cx, rail); // track
}

/// 试炼密室结构 (1.21)
fn place_trial_chambers(chunk: &mut Chunk, pos: ChunkPos, seed: u64) {
    let h = (pos.x as u64).wrapping_mul(313) ^ (pos.z as u64).wrapping_mul(317) ^ seed.wrapping_mul(20241);
    if !h.is_multiple_of(350) { return; }
    let copper = BlockState::new(165); // copper_block
    let tuff = BlockState::new(162);   // tuff
    let tuff_bricks = BlockState::new(163); // tuff_bricks
    let copper_grate = BlockState::new(166); // copper_grate
    let y = -20i32;
    let cx = 7usize; let cz = 7usize;
    // Main chamber 10x10x6
    for dx in 0..10usize { for dz in 0..10usize {
        for dy in 0..6i32 {
            let by = y + dy;
            let is_wall = dx == 0 || dx == 9 || dz == 0 || dz == 9 || dy == 0 || dy == 5;
            let block = if is_wall {
                match (dx + dz) % 6 {
                    0 | 3 => copper,
                    1 | 4 => tuff_bricks,
                    _ => tuff,
                }
            } else { BlockState::new(0) }; // air
            if !block.is_air() {
                chunk.set_block(cx - 5 + dx, by, cz - 5 + dz, block);
            }
        }
    }}
    // Trial spawner (copper grate floor decoration)
    chunk.set_block(cx, y + 1, cz - 4, copper_grate);
    chunk.set_block(cx, y + 1, cz + 4, copper_grate);
    chunk.set_block(cx - 4, y + 1, cz, copper_grate);
    chunk.set_block(cx + 4, y + 1, cz, copper_grate);
}

/// 远古城市结构 (1.19 Deep Dark)
fn place_ancient_city(chunk: &mut Chunk, pos: ChunkPos, seed: u64) {
    let h = (pos.x as u64).wrapping_mul(419) ^ (pos.z as u64).wrapping_mul(421) ^ seed.wrapping_mul(31337);
    if !h.is_multiple_of(500) { return; }
    let deepslate = BlockState::new(338);
    let reinforced = BlockState::new(339);  // reinforced_deepslate
    let sculk = BlockState::new(340);       // sculk
    let y = -51i32;
    let cx = 7usize; let cz = 7usize;
    // Large chamber
    for dx in 0..14usize { for dz in 0..14usize {
        for dy in 0..5i32 {
            let by = y + dy;
            let is_wall = dx == 0 || dx == 13 || dz == 0 || dz == 13 || dy == 0;
            let block = if is_wall {
                if (dx + dz) % 5 == 0 { reinforced } else { deepslate }
            } else { BlockState::new(0) };
            chunk.set_block(cx - 7 + dx, by, cz - 7 + dz, block);
        }
    }}
    // Sculk patches
    for _ in 0..8 {
        let sx = cx - 5 + (fastrand::usize(0..10));
        let sz = cz - 5 + (fastrand::usize(0..10));
        chunk.set_block(sx, y + 1, sz, sculk);
    }
}

// PermutationTable 线程局部缓存 — 每线程每 seed 只创建一次
thread_local! {
    static TERRAIN_TABLE: std::cell::RefCell<Option<(u64, noise::permutationtable::PermutationTable)>> = const { std::cell::RefCell::new(None) };
    static CAVE_TABLE: std::cell::RefCell<Option<(u64, noise::permutationtable::PermutationTable)>> = const { std::cell::RefCell::new(None) };
}

fn get_terrain_hasher(seed: u64) -> noise::permutationtable::PermutationTable {
    TERRAIN_TABLE.with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache.as_ref().is_none_or(|(s, _)| *s != seed) {
            *cache = Some((seed, noise::permutationtable::PermutationTable::new(seed as u32)));
        }
        cache.as_ref().unwrap().1
    })
}

fn get_cave_hasher(seed: u64) -> noise::permutationtable::PermutationTable {
    CAVE_TABLE.with(|cell| {
        let mut cache = cell.borrow_mut();
        let cave_seed = seed ^ 0xCAFE;
        if cache.as_ref().is_none_or(|(s, _)| *s != cave_seed) {
            *cache = Some((cave_seed, noise::permutationtable::PermutationTable::new(cave_seed as u32)));
        }
        cache.as_ref().unwrap().1
    })
}

/// Perlin 噪声地形生成器 (使用 noise crate — 3D Perlin + 分形)
/// PermutationTable 通过线程局部缓存复用
pub struct NoiseGenerator {
    pub base_height: i32,
    pub amplitude: i32,
    pub surface_block: BlockState,
    pub subsurface_block: BlockState,
    pub deep_block: BlockState,
}

impl NoiseGenerator {
    pub fn new() -> Self {
        Self {
            base_height: -59,
            amplitude: 20,
            surface_block: blocks::GRASS_BLOCK,
            subsurface_block: blocks::DIRT,
            deep_block: blocks::STONE,
        }
    }

    fn height_at(&self, world_x: i32, world_z: i32, terrain: &noise::permutationtable::PermutationTable) -> i32 {
        let x = world_x as f64 * 0.005;
        let z = world_z as f64 * 0.005;
        let mut val = 0.0;
        let mut freq = 1.0;
        let mut amp = 1.0;
        let mut max_val = 0.0;
        for _ in 0..4 {
            val += noise::core::perlin::perlin_2d(noise::Vector2::new(x * freq, z * freq), terrain) * amp;
            max_val += amp;
            freq *= 2.0;
            amp *= 0.5;
        }
        val /= max_val;
        let noise_val = (val * self.amplitude as f64) as i32;
        self.base_height + noise_val
    }

    fn is_cave(&self, wx: f64, wy: f64, wz: f64, cave: &noise::permutationtable::PermutationTable) -> bool {
        let val = noise::core::perlin::perlin_3d(
            noise::Vector3::new(wx * 0.05, wy * 0.05, wz * 0.05), cave);
        let val2 = noise::core::perlin::perlin_3d(
            noise::Vector3::new(wx * 0.1 + 100.0, wy * 0.1, wz * 0.1 + 100.0), cave) * 0.5;
        (val + val2).abs() < 0.3
    }
}

impl Default for NoiseGenerator { fn default() -> Self { Self::new() } }

impl TerrainGenerator for NoiseGenerator {
    fn name(&self) -> &str { "noise" }
    fn generate_chunk(&self, pos: ChunkPos, seed: u64) -> Chunk {
        // Use thread-locally cached PermutationTables
        let terrain_hasher = get_terrain_hasher(seed);
        let cave_hasher = get_cave_hasher(seed);
        let mut chunk = Chunk::new(pos);
        let origin_x = pos.x * 16;
        let origin_z = pos.z * 16;
        for x in 0..16i32 {
            for z in 0..16i32 {
                let wx = origin_x + x;
                let wz = origin_z + z;
                let biome = sample_biome(wx, wz, seed);
                let (surface_block, subsurface_block, deep_block) = biome.surface_blocks();
                let surface = self.height_at(wx, wz, &terrain_hasher).clamp(-64, 255);
                chunk.set_block(x as usize, -64, z as usize, blocks::BEDROCK);
                for y in -63..surface {
                    let b = if y >= surface - 4 { subsurface_block }
                            else { deep_block };
                    let b = if b.id == blocks::STONE.id && y < 0 { BlockState::new(DEEPSLATE) } else { b };
                    chunk.set_block(x as usize, y, z as usize, b);
                }
                if surface >= -59 {
                    chunk.set_block(x as usize, surface, z as usize, surface_block);
                }
            }
        }

        // 3D 洞穴 (替代旧的 hash 洞穴)
        for x in 0..16usize {
            for z in 0..16usize {
                let wx = (origin_x + x as i32) as f64;
                let wz = (origin_z + z as i32) as f64;
                for y in -60..50i32 {
                    if self.is_cave(wx, y as f64, wz, &cave_hasher) {
                        chunk.set_block(x, y, z, blocks::AIR);
                    }
                }
            }
        }

        place_ores(&mut chunk, seed);
        place_trees(&mut chunk, pos, seed, &|wx, wz| self.height_at(wx, wz, &terrain_hasher));
        place_vegetation(&mut chunk, pos, seed, &|wx, wz| self.height_at(wx, wz, &terrain_hasher));
        place_village(&mut chunk, pos, seed, &|wx, wz| self.height_at(wx, wz, &terrain_hasher));
        place_shipwreck(&mut chunk, pos, seed, &|wx, wz| self.height_at(wx, wz, &terrain_hasher));
        place_ruined_portal(&mut chunk, pos, seed, &|wx, wz| self.height_at(wx, wz, &terrain_hasher));
        place_desert_temple(&mut chunk, pos, seed);
        place_swamp_hut(&mut chunk, pos, seed);
        place_igloo(&mut chunk, pos, seed);
        place_mineshaft(&mut chunk, pos, seed);
        place_jungle_temple(&mut chunk, pos, seed);
        place_ocean_monument(&mut chunk, pos, seed);
        place_trial_chambers(&mut chunk, pos, seed);
        place_ancient_city(&mut chunk, pos, seed);

        for section in chunk.sections.iter_mut().flatten() {
            fill_section_biomes(&mut section.biomes, section.position.y, pos.x, pos.z, seed);
        }

        crate::lighting::init_chunk_lighting(&mut chunk);
        chunk.dirty = false;
        chunk
    }

    fn options_schema(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("base_height".into(), "base terrain height (default: -59)".into());
        m.insert("amplitude".into(), "height variation (default: 20)".into());
        m.insert("surface_block".into(), "top block name".into());
        m.insert("subsurface_block".into(), "subsurface block name".into());
        m.insert("deep_block".into(), "deep stone block name".into());
        m
    }
}

// ═══════════════════════════════════════════════════════
// 配置驱动自定义生成器
// ═══════════════════════════════════════════════════════

/// 层定义 (用于 flat/layered 模式)
#[derive(Debug, Clone)]
pub struct LayerDef {
    pub block: BlockState,
    pub thickness: i32,
}

/// 自定义生成器 — 从 HashMap<String, String> 读取参数
pub struct CustomGenerator {
    options: HashMap<String, String>,
    cached_layers: Option<Vec<LayerDef>>,
    cached_noise: Option<NoiseGenerator>,
}

impl CustomGenerator {
    pub fn new(options: HashMap<String, String>) -> Self {
        Self {
            options,
            cached_layers: None,
            cached_noise: None,
        }
    }

    fn mode(&self) -> &str {
        self.options.get("mode").map(|s| s.as_str()).unwrap_or("layered")
    }

    fn build_layers(&mut self) -> &[LayerDef] {
        if self.cached_layers.is_none() {
            let mut layers = Vec::new();

            // Parse "layers" key: comma-separated "block:thickness" pairs
            if let Some(layer_str) = self.options.get("layers") {
                for part in layer_str.split(',') {
                    let part = part.trim();
                    if let Some((block_name, thickness_str)) = part.split_once(':')
                        && let Some(block) = blocks::by_name(block_name.trim())
                            && let Ok(t) = thickness_str.trim().parse::<i32>() {
                                layers.push(LayerDef { block, thickness: t });
                            }
                }
            }

            // Fallback default layers
            if layers.is_empty() {
                layers = vec![
                    LayerDef { block: blocks::BEDROCK, thickness: 1 },
                    LayerDef { block: blocks::STONE, thickness: 2 },
                    LayerDef { block: blocks::DIRT, thickness: 2 },
                    LayerDef { block: blocks::GRASS_BLOCK, thickness: 1 },
                ];
            }

            self.cached_layers = Some(layers);
        }
        self.cached_layers.as_ref().unwrap()
    }

    fn build_noise(&mut self) -> &NoiseGenerator {
        if self.cached_noise.is_none() {
            let base = self.options.get("base_height")
                .and_then(|s| s.parse::<i32>().ok()).unwrap_or(-59);
            let amp = self.options.get("amplitude")
                .and_then(|s| s.parse::<i32>().ok()).unwrap_or(20);
            let surface = self.options.get("surface_block")
                .and_then(|s| blocks::by_name(s)).unwrap_or(blocks::GRASS_BLOCK);
            let sub = self.options.get("subsurface_block")
                .and_then(|s| blocks::by_name(s)).unwrap_or(blocks::DIRT);
            let deep = self.options.get("deep_block")
                .and_then(|s| blocks::by_name(s)).unwrap_or(blocks::STONE);

            self.cached_noise = Some(NoiseGenerator {
                base_height: base,
                amplitude: amp,
                surface_block: surface,
                subsurface_block: sub,
                deep_block: deep,
            });
        }
        self.cached_noise.as_ref().unwrap()
    }
}

impl TerrainGenerator for CustomGenerator {
    fn name(&self) -> &str { "custom" }

    fn generate_chunk(&self, pos: ChunkPos, seed: u64) -> Chunk {
        // Need &mut self for caching — use interior mutability via unsafe or
        // regenerate each time. For simplicity, we create fresh instances.
        let mut tmp = Self {
            options: self.options.clone(),
            cached_layers: None,
            cached_noise: None,
        };

        match tmp.mode() {
            "heightmap" | "noise" => {
                let noise = tmp.build_noise();
                noise.generate_chunk(pos, seed)
            }
            _ => {
                // "layered" / "flat" — build a layer-based flat terrain
                let layers = tmp.build_layers();
                let mut chunk = Chunk::new(pos);
                for x in 0..16usize {
                    for z in 0..16usize {
                        let mut cy = -64i32;
                        for layer in layers {
                            for dy in 0..layer.thickness {
                                chunk.set_block(x, cy + dy, z, layer.block);
                            }
                            cy += layer.thickness;
                        }
                    }
                }
                chunk.dirty = false;
                chunk
            }
        }
    }

    fn options_schema(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("mode".into(), "layered | heightmap".into());
        m.insert("layers".into(), "block:thickness,block:thickness,...".into());
        m.insert("base_height".into(), "noise base height".into());
        m.insert("amplitude".into(), "noise variation".into());
        m.insert("surface_block".into(), "noise top block".into());
        m
    }
}

// ═══════════════════════════════════════════════════════
// Layer Composer — 组合多个生成器
// ═══════════════════════════════════════════════════════

/// 组合多个生成器，后一个叠加在前一个之上
pub struct LayerComposer {
    generators: Vec<Box<dyn TerrainGenerator>>,
}

impl LayerComposer {
    pub fn new(generators: Vec<Box<dyn TerrainGenerator>>) -> Self {
        Self { generators }
    }
}

impl TerrainGenerator for LayerComposer {
    fn name(&self) -> &str { "compose" }

    fn generate_chunk(&self, pos: ChunkPos, seed: u64) -> Chunk {
        // Start with first generator's output, then overlay subsequent ones
        let mut chunk = if let Some(first) = self.generators.first() {
            first.generate_chunk(pos, seed)
        } else {
            return Chunk::new(pos);
        };

        // Apply additional generators as overlays
        for generator in &self.generators[1..] {
            let overlay = generator.generate_chunk(pos, seed);
            // Copy non-air blocks from overlay onto base chunk
            for x in 0..16usize {
                for z in 0..16usize {
                    for y in -64..320 {
                        let block = overlay.get_block(x, y, z);
                        if !block.is_air() {
                            chunk.set_block(x, y, z, block);
                        }
                    }
                }
            }
        }
        chunk
    }
}

// ═══════════════════════════════════════════════════════
// Generator Registry
// ═══════════════════════════════════════════════════════

pub struct GeneratorRegistry {
    generators: HashMap<String, Arc<dyn TerrainGenerator>>,
    active: String,
    /// 从 config 传入的生成器选项 (供 custom/compose 使用)
    custom_options: HashMap<String, String>,
}

impl GeneratorRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            generators: HashMap::new(),
            active: "flat".into(),
            custom_options: HashMap::new(),
        };
        registry.register(FlatGenerator::new());
        registry.register(NoiseGenerator::new());
        registry.register(EmptyGenerator::new());
        registry.register(NetherGenerator);
        registry.register(EndGenerator);
        registry
    }

    /// 从配置创建注册表，支持 custom 和 compose 生成器
    pub fn with_config(active_gen: &str, options: HashMap<String, String>) -> Self {
        let mut registry = Self::new();
        registry.custom_options = options.clone();

        // Register custom if configured
        if active_gen == "custom" || options.contains_key("mode") {
            registry.generators.insert(
                "custom".into(),
                Arc::new(CustomGenerator::new(options.clone())),
            );
        }

        // Register compose if configured
        if active_gen == "compose"
            && let Some(gen_list) = options.get("generators") {
                let names: Vec<&str> = gen_list.split(',').map(|s| s.trim()).collect();
                let mut gens: Vec<Box<dyn TerrainGenerator>> = Vec::new();
                for name in names {
                    match name {
                        "flat" => gens.push(Box::new(FlatGenerator::new())),
                        "noise" => gens.push(Box::new(NoiseGenerator::new())),
                        "empty" => gens.push(Box::new(EmptyGenerator::new())),
                        _ => {} // skip unknown
                    }
                }
                if !gens.is_empty() {
                    registry.generators.insert(
                        "compose".into(),
                        Arc::new(LayerComposer::new(gens)),
                    );
                }
            }

        // Set active generator (this was missing — always used "flat" before)
        let _ = registry.set_active(active_gen);

        registry
    }

    pub fn register(&mut self, generator: impl TerrainGenerator + 'static) {
        let name = generator.name().to_string();
        info!("Registered terrain generator: '{}'", name);
        self.generators.insert(name, Arc::new(generator));
    }

    pub fn set_active(&mut self, name: &str) -> Result<(), String> {
        if self.generators.contains_key(name) {
            self.active = name.to_string();
            info!("Active terrain generator: '{}'", name);
            Ok(())
        } else {
            let available: Vec<_> = self.list_names().iter().map(|s| s.to_string()).collect();
            Err(format!("Unknown generator: '{}'. Available: {:?}", name, available))
        }
    }

    pub fn active(&self) -> &Arc<dyn TerrainGenerator> {
        self.generators.get(&self.active)
            .unwrap_or_else(|| self.generators.get("flat").expect("flat generator must exist"))
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn TerrainGenerator>> {
        self.generators.get(name)
    }

    pub fn list_names(&self) -> Vec<&str> {
        self.generators.keys().map(|s| s.as_str()).collect()
    }

    pub fn generate_chunk(&self, pos: ChunkPos, seed: u64) -> Chunk {
        self.active().generate_chunk(pos, seed)
    }
}

impl Default for GeneratorRegistry {
    fn default() -> Self { Self::new() }
}
