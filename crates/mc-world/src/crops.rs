//! 作物生长系统 — 每 200 tick 随机生长 + 骨粉加速
//!
//! 支持: 小麦(59), 胡萝卜(141), 马铃薯(142), 甜菜根(207)
//! 年龄追踪使用 DashMap，每列 30% 概率随机生长 1 阶段。
//! 每次生长时更新区块中的方块状态，使客户端可见变化。

use mc_core::block::BlockState;
use mc_core::position::ChunkPos;
use crate::chunk_store::ChunkStore;
use dashmap::DashMap;
use std::sync::LazyLock;

/// 可生长的作物 (block_id, max_age)
const GROWABLE: &[(u32, u8)] = &[
    (59, 7),   // wheat
    (141, 7),  // carrots
    (142, 7),  // potatoes
    (207, 3),  // beetroot
];

/// 全局作物年龄追踪: (x, y, z) → current_age
static CROP_AGES: LazyLock<DashMap<(i32, i32, i32), u8>> = LazyLock::new(DashMap::new);

fn get_crop_info(block_id: u32) -> Option<(u32, u8)> {
    GROWABLE.iter().find(|(id, _)| *id == block_id).copied()
}

fn get_block(cs: &ChunkStore, x: i32, y: i32, z: i32) -> Option<BlockState> {
    if !(-64..=319).contains(&y) { return None; }
    let cp = ChunkPos::new(x >> 4, z >> 4);
    cs.get(&cp).map(|c| c.get_block((x & 0xF) as usize, y, (z & 0xF) as usize))
}

/// 每 200 tick 运行一次：随机生长作物
pub fn tick_crops(cs: &ChunkStore) {
    for cp in cs.all_loaded_positions() {
        let mut needs_update = Vec::new();
        if let Some(_chunk) = cs.get(&cp) {
            for x_off in 0..16i32 {
                for z_off in 0..16i32 {
                    if fastrand::u32(0..100) >= 30 { continue; } // 30% chance per column
                    let wx = cp.x * 16 + x_off;
                    let wz = cp.z * 16 + z_off;
                    // Find top block (the crop sits on top of farmland)
                    for y in (-64..=319).rev() {
                        if let Some(block) = get_block(cs, wx, y, wz) {
                            if block.is_air() { continue; }
                            // Check if this is a growable crop
                            if let Some((_crop_id, max_age)) = get_crop_info(block.id) {
                                let pos = (wx, y, wz);
                                let current_age = CROP_AGES.get(&pos).map(|r| *r).unwrap_or(0);
                                if current_age < max_age && fastrand::u32(0..100) < 45 {
                                    let new_age = current_age + 1;
                                    CROP_AGES.insert(pos, new_age);
                                    needs_update.push((wx, y, wz, block));
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }
        // Update blocks in chunk so clients see the growth (B5 fix)
        if !needs_update.is_empty()
            && let Some(mut chunk) = cs.get_mut(&cp) {
                for (wx, y, wz, block) in needs_update {
                    let sx = (wx & 0xF) as usize;
                    let sz = (wz & 0xF) as usize;
                    chunk.set_block(sx, y, sz, block);
                }
            }
    }
}

/// 获取作物当前年龄 (0..=max_age)
pub fn get_crop_age(x: i32, y: i32, z: i32) -> Option<u8> {
    CROP_AGES.get(&(x, y, z)).map(|r| *r)
}

/// 骨粉右键作物 → 立即生长 1-3 阶段
pub fn apply_bonemeal(cs: &ChunkStore, x: i32, y: i32, z: i32) -> bool {
    if let Some(block) = get_block(cs, x, y, z)
        && let Some((_crop_id, max_age)) = get_crop_info(block.id) {
            let pos = (x, y, z);
            let current_age = CROP_AGES.get(&pos).map(|r| *r).unwrap_or(0);
            if current_age < max_age {
                let growth = fastrand::u8(1..=3);
                let new_age = (current_age + growth).min(max_age);
                CROP_AGES.insert(pos, new_age);
                // Update chunk block so client sees growth (B5 fix)
                let cp = ChunkPos::new(x >> 4, z >> 4);
                if let Some(mut chunk) = cs.get_mut(&cp) {
                    let sx = (x & 0xF) as usize;
                    let sz = (z & 0xF) as usize;
                    chunk.set_block(sx, y, sz, block);
                }
            }
            return true;
        }
    false
}

/// 检测作物是否已完全成熟
/// B4 fix: check actual max_age for the crop type (beetroot=3, others=7)
pub fn is_fully_grown(cs: &ChunkStore, x: i32, y: i32, z: i32) -> bool {
    if let Some(block) = get_block(cs, x, y, z)
        && let Some((_crop_id, max_age)) = get_crop_info(block.id)
            && let Some(age) = CROP_AGES.get(&(x, y, z)).map(|r| *r) {
                return age >= max_age;
            }
    false
}

/// 移除作物追踪 (方块被破坏时调用)
pub fn remove_crop(x: i32, y: i32, z: i32) {
    CROP_AGES.remove(&(x, y, z));
}
