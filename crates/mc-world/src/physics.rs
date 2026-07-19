//! 方块物理引擎 — 沙/砂砾坠落、火蔓延、草扩散、冰融化
//!
//! 每 20 tick 运行一次。坠落方块使用索引避免全量扫描。

use mc_core::block::BlockState;
use mc_core::position::ChunkPos;
use crate::chunk_store::ChunkStore;
use dashmap::DashSet;
use std::sync::LazyLock;

/// 可坠落的方块 (重力影响)
const FALLING_BLOCKS: &[u32] = &[
    12,  // sand
    13,  // gravel
    582, 583, 584, 585, 586, 587, 588, 589, // concrete powder (white→light_gray)
    590, 591, 592, 593, 594, 595, 596, 597, // concrete powder (cyan→black)
];

/// 可燃方块 (火可以蔓延到)
const FLAMMABLE_BLOCKS: &[u32] = &[
    2, 3,                      // grass, dirt
    13, 14, 15, 16, 17, 18, 19, 20, 21, 22, // planks (all types)
    34, 35, 36, 37, 38, 39, 40, // logs
    56, 57, 58, 59, 60, 61,    // leaves
    113,                        // crafting table
    47,                         // bookshelf
];

// ═══════════════════════════════════════════════════════════════
// Falling block index — 避免每 tick O(all_chunks×256) 全表扫描
// ═══════════════════════════════════════════════════════════════

/// 全局坠落方块位置索引: (x, y, z) → present
static FALLING_INDEX: LazyLock<DashSet<(i32, i32, i32)>> = LazyLock::new(DashSet::new);

/// 注册一个受重力影响的方块 (放置时调用)
pub fn register_falling_block(x: i32, y: i32, z: i32) {
    FALLING_INDEX.insert((x, y, z));
}

/// 移除坠落方块追踪 (方块被破坏/移除时调用)
pub fn unregister_falling_block(x: i32, y: i32, z: i32) {
    FALLING_INDEX.remove(&(x, y, z));
}

fn is_falling_block(id: u32) -> bool {
    FALLING_BLOCKS.contains(&id)
}

fn is_flammable(id: u32) -> bool {
    FLAMMABLE_BLOCKS.contains(&id)
}

fn get_block(cs: &ChunkStore, x: i32, y: i32, z: i32) -> Option<BlockState> {
    if !(-64..=319).contains(&y) { return None; }
    let cp = ChunkPos::new(x >> 4, z >> 4);
    cs.get(&cp).map(|c| c.get_block((x & 0xF) as usize, y, (z & 0xF) as usize))
}

fn set_block(cs: &ChunkStore, x: i32, y: i32, z: i32, block: BlockState) {
    if !(-64..=319).contains(&y) { return; }
    let cp = ChunkPos::new(x >> 4, z >> 4);
    if let Some(mut chunk) = cs.get_mut(&cp) {
        chunk.set_block((x & 0xF) as usize, y, (z & 0xF) as usize, block);
    }
}

/// 每 20 tick 运行一次：处理所有方块物理
pub fn tick_physics(cs: &ChunkStore) {
    let mut updates: Vec<(i32, i32, i32, BlockState)> = Vec::new();
    let mut resolved: Vec<(i32, i32, i32)> = Vec::new();

    // ── Phase 1: 仅处理注册的坠落方块 (O(N_falling) 替代 O(all_blocks)) ──
    for pos in FALLING_INDEX.iter() {
        let (x, y, z) = *pos;
        if let Some(block) = get_block(cs, x, y, z) {
            if block.is_air() {
                resolved.push((x, y, z));
                continue;
            }
            if is_falling_block(block.id) {
                let below = get_block(cs, x, y - 1, z);
                if below.map(|b| b.is_air()).unwrap_or(false) {
                    updates.push((x, y - 1, z, block));
                    updates.push((x, y, z, BlockState::AIR));
                    resolved.push((x, y, z));
                }
            } else {
                // No longer a falling block — stop tracking
                resolved.push((x, y, z));
            }
        } else {
            resolved.push((x, y, z));
        }
    }

    // Remove resolved positions from index
    for pos in &resolved {
        FALLING_INDEX.remove(pos);
    }

    // ── Phase 2: 扫描已加载区块处理火蔓延/草扩散/冰融化 ──
    for entry in cs.all_loaded_positions() {
        let cp = entry;
        if let Some(_chunk) = cs.get(&cp) {
            for x in 0..16i32 {
                for z in 0..16i32 {
                    let wx = cp.x * 16 + x;
                    let wz = cp.z * 16 + z;

                    for y in (-64..=319).rev() {
                        if let Some(block) = get_block(cs, wx, y, wz) {
                            if block.is_air() { continue; }

                            // Register new falling blocks into the index for Phase 1
                            if is_falling_block(block.id) {
                                FALLING_INDEX.insert((wx, y, wz));
                                break; // falling blocks handled by Phase 1, stop scan at this column
                            }

                            // ── Fire spread ──
                            if block.id == 51 {
                                for (dx, dz) in &[(1,0),(-1,0),(0,1),(0,-1)] {
                                    let (nx, nz) = (wx + dx, wz + dz);
                                    if let Some(neighbor) = get_block(cs, nx, y, nz)
                                        && is_flammable(neighbor.id) && fastrand::u32(0..100) < 30 {
                                            updates.push((nx, y, nz, BlockState::new(51)));
                                        }
                                }
                                if fastrand::u32(0..100) < 15 {
                                    updates.push((wx, y, wz, BlockState::AIR));
                                }
                            }

                            // ── Grass spread ──
                            if block.id == 2 {
                                for (dx, dz) in &[(1,0),(-1,0),(0,1),(0,-1)] {
                                    let (nx, nz) = (wx + dx, wz + dz);
                                    if let Some(neighbor) = get_block(cs, nx, y, nz)
                                        && neighbor.id == 3
                                            && let Some(above) = get_block(cs, nx, y + 1, nz)
                                                && above.is_air() && fastrand::u32(0..100) < 10 {
                                                    updates.push((nx, y, nz, BlockState::new(2)));
                                                }
                                }
                            }

                            // ── Ice melt ──
                            if block.id == 79
                                && fastrand::u32(0..100) < 5 {
                                    updates.push((wx, y, wz, BlockState::new(267)));
                                }

                            break; // only process topmost block per column
                        }
                    }
                }
            }
        }
    }

    // Apply updates and register new falling blocks
    for (x, y, z, block) in updates {
        if is_falling_block(block.id) {
            FALLING_INDEX.insert((x, y, z));
        }
        set_block(cs, x, y, z, block);
    }
}
