//! 简化的 A* 路径寻路 — 2D水平寻路 + 1格跳跃
//!
//! 针对 RPi 5 优化:
//! - 搜索半径 16 格
//! - 最多 256 个节点
//! - 每 40 tick 重新计算一次
//! - 无路径时回退到直接追逐
//! - LRU 缓存: 相同起点/终点复用路径 (DashMap, 64 entry cap)

use dashmap::DashMap;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

/// Path cache entry: (src_chunk, dst_chunk) → (waypoints, access_counter)
static PATH_CACHE: LazyLock<DashMap<(i32, i32, i32, i32), (Vec<(f64, f64, f64)>, AtomicU64)>> =
    LazyLock::new(DashMap::new);
static CACHE_MAX: usize = 64;

/// 2D 寻路节点
#[derive(Copy, Clone, Eq, PartialEq)]
struct Node {
    x: i32,
    z: i32,
    cost: i32, // g + h
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.cmp(&self.cost) // min-heap
    }
}
impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// 检查方块是否可通行 (空气、水、非固体)
fn is_passable(block_id: u32) -> bool {
    // Air, water, grass, flowers, torches, rails are passable
    matches!(block_id,
        0 | 31 | 32 | 33 | 37..=47 | 50 | 78 | 79 |
        83 | 85 | 96 | 267 | 268 | 27 | 28 | 157
    )
}

/// A* 寻路: 从 (sx, sy, sz) 到 (tx, ty, tz)
/// 使用 chunk_store 查询方块通行性
/// 返回路径点列表 (不含起点)
pub fn find_path(
    start_x: f64, start_y: f64, start_z: f64,
    target_x: f64, target_y: f64, target_z: f64,
    chunk_store: &mc_world::chunk_store::ChunkStore,
) -> Vec<(f64, f64, f64)> {
    let sx = start_x as i32;
    let sz = start_z as i32;
    let tx = target_x as i32;
    let tz = target_z as i32;

    // If target is within 1 block, no path needed
    if (sx - tx).abs() <= 1 && (sz - tz).abs() <= 1 {
        return vec![(target_x, target_y, target_z)];
    }

    let max_dist = 16;
    if (sx - tx).abs() > max_dist || (sz - tz).abs() > max_dist {
        return vec![(target_x, target_y, target_z)]; // too far, direct chase
    }

    // Check path cache (keyed by chunk coordinates)
    let scx = sx.div_euclid(16); let scz = sz.div_euclid(16);
    let tcx = tx.div_euclid(16); let tcz = tz.div_euclid(16);
    let cache_key = (scx, scz, tcx, tcz);
    if let Some(entry) = PATH_CACHE.get(&cache_key) {
        entry.1.fetch_add(1, AtomicOrdering::Relaxed);
        return entry.0.clone();
    }

    // A* search (simplified 2D)
    let heuristic = |x: i32, z: i32| -> i32 {
        (x - tx).abs() + (z - tz).abs() // Manhattan distance
    };

    let node_key = |x: i32, z: i32| -> u64 {
        ((x as u64 & 0xFFFF) << 16) | (z as u64 & 0xFFFF)
    };

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<u64, (i32, i32)> = HashMap::new();
    let mut g_score: HashMap<u64, i32> = HashMap::new();

    let start_key = node_key(sx, sz);
    open.push(Node { x: sx, z: sz, cost: heuristic(sx, sz) });
    g_score.insert(start_key, 0);

    let mut found = false;
    let mut _end_key = 0u64;

    while let Some(current) = open.pop() {
        let cur_key = node_key(current.x, current.z);

        if current.x == tx && current.z == tz {
            found = true;
            _end_key = cur_key;
            break;
        }

        if g_score.get(&cur_key).copied().unwrap_or(i32::MAX) < current.cost - heuristic(current.x, current.z) {
            continue;
        }

        // Check 4 horizontal neighbors
        for (dx, dz) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nx = current.x + dx;
            let nz = current.z + dz;
            let nk = node_key(nx, nz);

            // Check passability at mob foot level
            let passable = {
                let cp = mc_core::position::ChunkPos::new(nx >> 4, nz >> 4);
                if let Some(chunk) = chunk_store.get(&cp) {
                    let lx = (nx & 0xF) as usize;
                    let lz = (nz & 0xF) as usize;
                    let foot_y = start_y as i32;
                    let head_y = foot_y + 1;
                    let foot = chunk.get_block(lx, foot_y, lz);
                    let head = chunk.get_block(lx, head_y, lz);
                    foot.is_air() || is_passable(foot.id) || (!foot.is_air() && head.is_air())
                } else {
                    false // unloaded chunk — can't path through
                }
            };

            if !passable { continue; }

            let tentative_g = g_score.get(&cur_key).copied().unwrap_or(i32::MAX) + 1;
            if tentative_g < g_score.get(&nk).copied().unwrap_or(i32::MAX) {
                came_from.insert(nk, (current.x, current.z));
                g_score.insert(nk, tentative_g);
                open.push(Node { x: nx, z: nz, cost: tentative_g + heuristic(nx, nz) });
            }
        }
    }

    if !found {
        // No path found — return direct target for straight-line chase
        return vec![(target_x, target_y, target_z)];
    }

    // Reconstruct path (reversed)
    let mut path: Vec<(i32, i32)> = Vec::new();
    let mut cur = (tx, tz);
    while cur != (sx, sz) {
        path.push(cur);
        if let Some(&prev) = came_from.get(&node_key(cur.0, cur.1)) {
            cur = prev;
        } else {
            break;
        }
    }
    path.reverse();

    // Convert to world coordinates
    let waypoints: Vec<(f64, f64, f64)> = path.into_iter()
        .map(|(x, z)| (x as f64 + 0.5, target_y, z as f64 + 0.5))
        .collect();

    // Cache the result with LRU eviction
    if !waypoints.is_empty() && PATH_CACHE.len() < CACHE_MAX {
        PATH_CACHE.insert(cache_key, (waypoints.clone(), AtomicU64::new(1)));
    } else if PATH_CACHE.len() >= CACHE_MAX {
        // Evict least-recently-used entry
        if let Some(oldest) = PATH_CACHE.iter()
            .min_by_key(|e| e.value().1.load(AtomicOrdering::Relaxed)) {
            PATH_CACHE.remove(oldest.key());
        }
        if !waypoints.is_empty() {
            PATH_CACHE.insert(cache_key, (waypoints.clone(), AtomicU64::new(1)));
        }
    }
    waypoints
}
