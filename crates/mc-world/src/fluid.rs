//! 流体引擎 — 水/熔岩流动、传播、物理
//!
//! Features:
//! - BFS fluid propagation (gravity + 4 horizontal)
//! - Proper infinite water source rules (2+ adjacent sources + solid below)
//! - Waterlogging support (slabs/stairs/fences underwater)
//! - Bubble column detection (soul sand = up, magma block = down)
//! - Lava+water mixing (stone/cobblestone/obsidian/basalt)

use mc_core::block::BlockState;
use mc_core::position::ChunkPos;
use crate::chunk_store::ChunkStore;

const WATER_ID: u32 = 267;
const LAVA_ID: u32 = 268;
const SOUL_SAND_ID: u32 = 85;
const MAGMA_BLOCK_ID: u32 = 213;

/// Blocks that can be waterlogged (hold water in the same block space)
fn is_waterloggable(id: u32) -> bool {
    matches!(id,
        // Slabs (various types)
        44 | 126 | 127 | 128 | 129 | 130 | 131 | 182 | 183 | 184 | 185 | 186 | 187 |
        // Stairs (various types)
        53 | 67 | 108 | 109 | 110 | 111 | 112 | 134 | 135 | 136 |
        // Fences
        85 | 188 | 189 | 190 | 191 | 192 |
        // Other: signs, trapdoors, coral, etc.
        63 | 68 | 96 | 147 | 148 | 355 | 356 | 357 | 358 | 359 |
        25 | 26 // rails
    )
}

/// 流体引擎 — interior-mutable queue for lock-free tick
pub struct FluidEngine {
    flow_queue: parking_lot::Mutex<Vec<(i32, i32, i32, u32, u8)>>,
}

impl Default for FluidEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl FluidEngine {
    pub fn new() -> Self {
        Self { flow_queue: parking_lot::Mutex::new(Vec::new()) }
    }

    /// 放置流体源方块时调用 (lock-free — called from connection handlers)
    pub fn on_place(&self, x: i32, y: i32, z: i32, fluid_id: u32) {
        self.flow_queue.lock().push((x, y, z, fluid_id, 7));
    }

    /// 每 5 tick 运行一次：传播流体 + 无限水源检测 + 气泡柱
    pub fn tick(&self, chunk_store: &ChunkStore) {
        let mut queue = self.flow_queue.lock();
        let batch: Vec<_> = queue.drain(..).collect();
        drop(queue);

        if batch.is_empty() { return; }

        // Infinite water source check: 2+ adjacent water sources → fill empty space with solid floor
        for (x, y, z, fluid_id, _level) in &batch {
            if *fluid_id != WATER_ID { continue; }
            check_infinite_water_at(chunk_store, *x, *y, *z);
        }

        for (x, y, z, fluid_id, level) in batch {
            if level == 0 { continue; }

            // Try flow down first (gravity)
            let below = (x, y - 1, z);
            if let Some(block) = get_block(chunk_store, below.0, below.1, below.2) {
                if block.is_air() {
                    set_block(chunk_store, below.0, below.1, below.2, BlockState::new(fluid_id));
                    self.flow_queue.lock().push((below.0, below.1, below.2, fluid_id, 7));
                } else if is_waterloggable(block.id) && fluid_id == WATER_ID {
                    // Waterlog the existing block instead of replacing it
                    let mut waterlogged = block;
                    waterlogged.set_waterlogged(true);
                    set_block(chunk_store, below.0, below.1, below.2, waterlogged);
                    self.flow_queue.lock().push((below.0, below.1, below.2, fluid_id, 7));
                }
            }

            // Horizontal spread: reduce level by 1 each step
            let next_level = level - 1;
            if next_level > 0 {
                let max_spread = if fluid_id == LAVA_ID { 3 } else { 7 };
                let remaining = next_level.min(max_spread);
                for (dx, dz) in &[(1,0), (-1,0), (0,1), (0,-1)] {
                    let nx = x + dx; let nz = z + dz;
                    if let Some(block) = get_block(chunk_store, nx, y, nz) {
                        if block.is_air() {
                            set_block(chunk_store, nx, y, nz, BlockState::new(fluid_id));
                            self.flow_queue.lock().push((nx, y, nz, fluid_id, remaining));
                        } else if is_waterloggable(block.id) && fluid_id == WATER_ID {
                            // Waterlog the existing block
                            let mut waterlogged = block;
                            waterlogged.set_waterlogged(true);
                            set_block(chunk_store, nx, y, nz, waterlogged);
                            self.flow_queue.lock().push((nx, y, nz, fluid_id, remaining));
                        }
                        // Lava + water → obsidian / stone / cobblestone / basalt
                        if fluid_id == LAVA_ID && block.id == WATER_ID {
                            set_block(chunk_store, nx, y, nz, BlockState::new(1)); // stone
                        }
                        if fluid_id == WATER_ID && block.id == LAVA_ID {
                            let result = if level >= 5 { 49 } else { 1 }; // obsidian or stone
                            set_block(chunk_store, nx, y, nz, BlockState::new(result));
                        }
                        // Blue ice + lava → basalt
                        if fluid_id == LAVA_ID && block.id == 174 {
                            set_block(chunk_store, nx, y, nz, BlockState::new(213)); // basalt approximation
                        }
                    }
                }
            }
        }
    }

    /// 获取气泡柱类型: Some(true)=上升(灵魂沙), Some(false)=下降(岩浆块)
    pub fn get_bubble_column(chunk_store: &ChunkStore, x: f64, y: f64, z: f64) -> Option<bool> {
        let bx = x as i32; let by = y as i32; let bz = z as i32;
        // Check block below feet for bubble column source
        let below = get_block(chunk_store, bx, by - 1, bz)?;
        if below.id == WATER_ID {
            // Check the block under the water
            let floor = get_block(chunk_store, bx, by - 2, bz)?;
            if floor.id == SOUL_SAND_ID { return Some(true); }  // upward
            if floor.id == MAGMA_BLOCK_ID { return Some(false); } // downward
        }
        None
    }
}

fn get_block(chunk_store: &ChunkStore, x: i32, y: i32, z: i32) -> Option<BlockState> {
    if !(-64..=319).contains(&y) { return None; }
    let cp = ChunkPos::new(x >> 4, z >> 4);
    chunk_store.get(&cp).map(|c| c.get_block((x & 0xF) as usize, y, (z & 0xF) as usize))
}

fn set_block(chunk_store: &ChunkStore, x: i32, y: i32, z: i32, block: BlockState) {
    if !(-64..=319).contains(&y) { return; }
    let cp = ChunkPos::new(x >> 4, z >> 4);
    if let Some(mut chunk) = chunk_store.get_mut(&cp) {
        chunk.set_block((x & 0xF) as usize, y, (z & 0xF) as usize, block);
    }
}

/// 无限水源检测 — 2+ 相邻水源 + 下方为固体 → 中间填充水源
fn check_infinite_water_at(chunk_store: &ChunkStore, x: i32, y: i32, z: i32) {
    for (dx, dz) in &[(1,0), (-1,0), (0,1), (0,-1)] {
        let nx = x + dx; let nz = z + dz;
        let block = get_block(chunk_store, nx, y, nz).unwrap_or(BlockState::AIR);
        if block.is_air() || is_waterloggable(block.id) {
            // Check if this space has 2+ adjacent water sources and solid floor
            let mut source_count = 0u8;
            for (sx, sz) in &[(1,0), (-1,0), (0,1), (0,-1)] {
                let nb = get_block(chunk_store, nx + sx, y, nz + sz).unwrap_or(BlockState::AIR);
                if nb.id == WATER_ID { source_count += 1; }
            }
            let floor = get_block(chunk_store, nx, y - 1, nz).unwrap_or(BlockState::AIR);
            let has_solid_floor = !floor.is_air() && !is_waterloggable(floor.id) && floor.id != WATER_ID;
            if source_count >= 2 && has_solid_floor {
                if block.is_air() {
                    set_block(chunk_store, nx, y, nz, BlockState::new(WATER_ID));
                } else {
                    // Waterlog the existing block
                    let mut waterlogged = block;
                    waterlogged.set_waterlogged(true);
                    set_block(chunk_store, nx, y, nz, waterlogged);
                }
            }
        }
    }
}

/// 检测玩家是否在水中 (游泳状态, 含水方块也算)
pub fn is_in_water(chunk_store: &ChunkStore, x: f64, y: f64, z: f64) -> bool {
    let bx = x as i32; let by = y as i32; let bz = z as i32;
    get_block(chunk_store, bx, by, bz)
        .map(|b| b.id == WATER_ID || (is_waterloggable(b.id) && b.waterlogged()))
        .unwrap_or(false)
}

/// 检测玩家是否在熔岩中
pub fn is_in_lava(chunk_store: &ChunkStore, x: f64, y: f64, z: f64) -> bool {
    let bx = x as i32; let by = y as i32; let bz = z as i32;
    get_block(chunk_store, bx, by, bz).map(|b| b.id == LAVA_ID).unwrap_or(false)
}
