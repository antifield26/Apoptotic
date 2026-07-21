//! 光照引擎 — 天光计算 + 方块光 BFS 传播
//!
//! Minecraft 光照: 每方块 4 bits (0-15), sky_light + block_light 各自独立。
//! - 天光: 从世界顶部 (y=319) 向下传播，不透明方块遮挡
//! - 方块光: 从光源方块 BFS 扩散，每步衰减 1

use crate::chunk::Chunk;
use mc_core::block::BlockState;
use std::collections::VecDeque;
use std::cell::RefCell;

// 线程局部光照 BFS 复用缓冲区 (避免每次分配 96KB)
thread_local! {
    static LIGHT_VISITED: RefCell<Vec<u64>> = RefCell::new(vec![0u64; 1536]); // 98304 bits / 64 = 1536 words (12KB)
    static LIGHT_QUEUE: RefCell<VecDeque<LightNode>> = RefCell::new(VecDeque::with_capacity(4096));
}

/// 最大光照值
const MAX_LIGHT: u8 = 15;

/// 方块是否透光 (天光可通过) — 覆盖 ~60+ 常见透明方块
#[allow(unreachable_patterns)]
pub fn is_transparent(block: BlockState) -> bool {
    if block.waterlogged() { return false; }
    if block.is_air() { return true; }
    matches!(block.id,
        // Liquids
        267 | 268 |
        // Glass family
        66 | 67 | 68 | 69 | 70 |
        // Leaves
        56 | 57 | 58 | 59 | 60 | 61 |
        // Ice, snow
        79 | 80 | 119 | 120 | 174 | 250 |
        // Slabs
        44 | 45 | 46 | 47 | 48 | 126 | 127 |
        // Fences
        85 | 86 | 87 | 88 | 89 | 113 | 188 | 189 | 190 | 191 | 192 |
        // Doors + trapdoors
        64 | 65 | 71 | 96 | 97 | 98 | 99 | 100 | 101 | 102 | 103 | 104 |
        193 | 194 | 195 | 196 | 197 | 198 | 199 |
        // Plants, flowers, grass, crops
        31 | 32 | 37 | 38 | 39 | 40 | 51 | 59 | 83 |
        105 | 106 | 107 | 175 | 176 |
        // Redstone components
        55 | 75 | 76 | 93 | 94 | 131 | 132 | 143 | 144 |
        148 | 149 | 150 | 151 | 152 | 178 |
        // Buttons, pressure plates
        57 | 58 | 59 | 60 | 61 | 62 | 63 |
        // Beds, banners, signs
        26 | 27 | 28 | 29 | 30 | 169 | 170 | 171 | 172 | 173 |
        // Misc: dispenser, piston, sticky_piston, observer
        23 | 33 | 34 | 218 | 317
    )
}

/// 方块是否完全不透光
pub fn is_opaque(block: BlockState) -> bool {
    !is_transparent(block)
}

/// 方块的发光等级 (0-15) — 覆盖 30+ 光源方块
pub fn light_emission(block: BlockState) -> u8 {
    match block.id {
        // Full brightness (15)
        130 => 15,  // glowstone
        131 => 15,  // jack_o_lantern
        268 => 15,  // lava
        176 => 15,  // sea_lantern
        177 => 15,  // beacon
        180 => 15,  // conduit
        152 => 15,  // redstone_lamp (lit)
        50  => 15,  // fire
        91  => 15,  // end_rod
        // High brightness (14)
        108 => 14,  // torch
        169 => 14,  // end_portal_frame (filled)
        // Medium brightness (13)
        90  => 13,  // portal
        // Torch-level (12-10)
        92  => 12,  // soul_torch
        93  => 12,  // soul_lantern
        94  => 12,  // soul_campfire
        // Low brightness (9-7)
        167 => 7,   // beacon block (inactive)
        178 => 7,   // brewing_stand
        179 => 7,   // enchanting_table
        181 => 7,   // ender_chest
        38  => 7,   // redstone_torch (lit)
        37  => 7,   // furnace (lit)
        // Very low (4-1)
        71  => 4,   // cave_vines (berries)
        72  => 3,   // magma_block
        73  => 1,   // redstone_ore (lit)
        74  => 1,   // sculk_sensor (active)
        _ => 0,
    }
}

// ═══════════════════════════════════════════════════
// Sky light: top-down per-column propagation
// ═══════════════════════════════════════════════════

/// 计算整个 chunk 的初始天光
pub fn compute_sky_light(chunk: &mut Chunk) {
    for x in 0..16usize {
        for z in 0..16usize {
            let mut light: u8 = MAX_LIGHT;
            // Iterate sections top-down (23 → 0), blocks top-down within each (15 → 0)
            for si in (0..24usize).rev() {
                if let Some(section) = &mut chunk.sections[si] {
                    let wy_base = section.position.y * 16;
                    for by in (0..16usize).rev() {
                        let wy = wy_base + by as i32;
                        if !(-64..=319).contains(&wy) { continue; }
                        let block = section.get_block(x, by, z);
                        section.set_sky_light(x, by, z, light);
                        if is_opaque(block) {
                            light = light.saturating_sub(1);
                        }
                    }
                }
            }
        }
    }
}

/// 方块被移除后: 为该列重新计算天光
pub fn recalc_sky_light_on_remove(chunk: &mut Chunk, x: usize, y: i32, z: usize) {
    // Find sky light value above the removed block
    let mut light = MAX_LIGHT;
    for check_y in (y..=319).rev() {
        if check_y == y { continue; } // skip the just-removed block
        let si = crate::chunk::section_index(check_y);
        let by = check_y.rem_euclid(16) as usize;
        if si >= 24 { continue; }
        if let Some(sec) = &chunk.sections[si] {
            let block = sec.get_block(x, by, z);
            if is_opaque(block) {
                light = sec.get_sky_light(x, by, z).saturating_sub(1);
                break;
            }
        }
    }

    // Propagate down from the removed position
    for check_y in y..320 {
        let si = crate::chunk::section_index(check_y);
        let by = check_y.rem_euclid(16) as usize;
        if si >= 24 { break; }
        if let Some(sec) = &mut chunk.sections[si] {
            sec.set_sky_light(x, by, z, light);
            let block = sec.get_block(x, by, z);
            if is_opaque(block) {
                light = light.saturating_sub(1);
                if light == 0 { break; }
            }
        }
    }
}

/// 不透明方块被放置后: 遮挡该列下方的天光
pub fn recalc_sky_light_on_place(chunk: &mut Chunk, x: usize, y: i32, z: usize) {
    let mut light: u8 = 0; // opaque block blocks sky
    for check_y in y..320 {
        let si = crate::chunk::section_index(check_y);
        let by = check_y.rem_euclid(16) as usize;
        if si >= 24 { break; }
        if let Some(sec) = &mut chunk.sections[si] {
            if check_y == y {
                sec.set_sky_light(x, by, z, 0);
                continue;
            }
            sec.set_sky_light(x, by, z, light);
            let block = sec.get_block(x, by, z);
            if is_opaque(block) {
                light = light.saturating_sub(1);
                if light == 0 { break; }
            }
        }
    }
}

// ═══════════════════════════════════════════════════
// Block light: BFS flood fill
// ═══════════════════════════════════════════════════

#[derive(Clone, Copy)]
struct LightNode {
    x: usize, y: i32, z: usize, level: u8,
}

/// BFS 传播方块光 (在 chunk 内) — 使用线程局部缓冲区避免分配
pub fn propagate_block_light(chunk: &mut Chunk) {
    LIGHT_QUEUE.with(|q_cell| {
        LIGHT_VISITED.with(|v_cell| {
            let mut queue = q_cell.borrow_mut();
            let mut visited = v_cell.borrow_mut();
            queue.clear();
            visited.fill(0u64);

            // Seed queue with light-emitting blocks
            for x in 0..16usize {
                for z in 0..16usize {
                    for sy in -4..20i32 {
                        let si = crate::chunk::section_index_from_section_y(sy);
                        if let Some(sec) = &chunk.sections[si] {
                            let wy_base = sy * 16;
                            for by in 0..16usize {
                                let wy = wy_base + by as i32;
                                let block = sec.get_block(x, by, z);
                                let emission = light_emission(block);
                                if emission > 0 {
                                    queue.push_back(LightNode { x, y: wy, z, level: emission });
                                    let bit_idx = (x + z * 16 + ((wy + 64) as usize) * 256) as u32;
                                    visited[(bit_idx / 64) as usize] |= 1u64 << (bit_idx % 64);
                                }
                            }
                        }
                    }
                }
            }

            while let Some(node) = queue.pop_front() {
        if node.level <= 1 { continue; }
        let next = node.level - 1;

        let neighbors: [(isize, i32, isize); 6] = [
            (node.x as isize - 1, node.y, node.z as isize),
            (node.x as isize + 1, node.y, node.z as isize),
            (node.x as isize, node.y - 1, node.z as isize),
            (node.x as isize, node.y + 1, node.z as isize),
            (node.x as isize, node.y, node.z as isize - 1),
            (node.x as isize, node.y, node.z as isize + 1),
        ];

        for (nx, ny, nz) in &neighbors {
            if *nx < 0 || *nx >= 16 || *nz < 0 || *nz >= 16 || *ny < -64 || *ny > 319 { continue; }
            let bit_idx = (*nx as usize + *nz as usize * 16 + ((*ny + 64) as usize) * 256) as u32;
            let word = (bit_idx / 64) as usize;
            let mask = 1u64 << (bit_idx % 64);
            if word >= visited.len() || (visited[word] & mask) != 0 { continue; }
            visited[word] |= mask;

            let si = crate::chunk::section_index(*ny);
            let by = ny.rem_euclid(16) as usize;
            if let Some(sec) = &chunk.sections[si] {
                let block = sec.get_block(*nx as usize, by, *nz as usize);
                if is_opaque(block) { continue; }
                let current = sec.get_block_light(*nx as usize, by, *nz as usize);
                if next > current {
                    if let Some(sec_mut) = &mut chunk.sections[si] {
                        sec_mut.set_block_light(*nx as usize, by, *nz as usize, next);
                    }
                    queue.push_back(LightNode { x: *nx as usize, y: *ny, z: *nz as usize, level: next });
                }
            }
        }
    }
        }); // close LIGHT_VISITED.with
    }); // close LIGHT_QUEUE.with
}

/// 初始化新生成区块的完整光照
pub fn init_chunk_lighting(chunk: &mut Chunk) {
    for section in chunk.sections.iter_mut().flatten() {
        section.fill_sky_light(MAX_LIGHT);
        *section.block_light = [0u8; 2048];
    }
    compute_sky_light(chunk);
    propagate_block_light(chunk);
}

/// 在方块变更后传播光照到相邻区块（处理区块边界）
pub fn propagate_lighting_cross_chunk(
    chunk_store: &crate::chunk_store::ChunkStore,
    cp: &mc_core::position::ChunkPos,
    x: usize, y: i32, z: usize,
    is_remove: bool,
) {
    // Get the chunk at the boundary and propagate lighting
    let needs_neighbor_x = x == 0 || x == 15;
    let needs_neighbor_z = z == 0 || z == 15;

    if !needs_neighbor_x && !needs_neighbor_z {
        return;
    }

    if x == 0 {
        let neighbor_cp = mc_core::position::ChunkPos::new(cp.x - 1, cp.z);
        if let Some(mut neighbor) = chunk_store.get_mut(&neighbor_cp) {
            let nx = 15;
            if is_remove {
                recalc_sky_light_on_remove(&mut neighbor, nx, y, z);
            } else {
                recalc_sky_light_on_place(&mut neighbor, nx, y, z);
            }
            propagate_block_light(&mut neighbor);
        }
    }
    if x == 15 {
        let neighbor_cp = mc_core::position::ChunkPos::new(cp.x + 1, cp.z);
        if let Some(mut neighbor) = chunk_store.get_mut(&neighbor_cp) {
            let nx = 0;
            if is_remove {
                recalc_sky_light_on_remove(&mut neighbor, nx, y, z);
            } else {
                recalc_sky_light_on_place(&mut neighbor, nx, y, z);
            }
            propagate_block_light(&mut neighbor);
        }
    }
    if z == 0 {
        let neighbor_cp = mc_core::position::ChunkPos::new(cp.x, cp.z - 1);
        if let Some(mut neighbor) = chunk_store.get_mut(&neighbor_cp) {
            let nz = 15;
            if is_remove {
                recalc_sky_light_on_remove(&mut neighbor, x, y, nz);
            } else {
                recalc_sky_light_on_place(&mut neighbor, x, y, nz);
            }
            propagate_block_light(&mut neighbor);
        }
    }
    if z == 15 {
        let neighbor_cp = mc_core::position::ChunkPos::new(cp.x, cp.z + 1);
        if let Some(mut neighbor) = chunk_store.get_mut(&neighbor_cp) {
            let nz = 0;
            if is_remove {
                recalc_sky_light_on_remove(&mut neighbor, x, y, nz);
            } else {
                recalc_sky_light_on_place(&mut neighbor, x, y, nz);
            }
            propagate_block_light(&mut neighbor);
        }
    }
    // Also handle corners (x=0,z=0 etc.) — covered by the individual checks above
}

// ═══ C1: Multi-chunk light flooding ═══
/// Propagate block light across chunk boundaries using BFS.
/// Queues neighbor chunk positions when light reaches a chunk edge.
/// Handles up to 3 levels of cross-chunk propagation (enough for most light sources).
pub fn propagate_lighting_across_chunks(
    chunk_store: &crate::chunk_store::ChunkStore,
    start_cp: mc_core::position::ChunkPos,
) {
    use mc_core::position::ChunkPos;
    let mut pending_chunks: VecDeque<(ChunkPos, u8)> = VecDeque::with_capacity(16);
    let mut visited_chunks: std::collections::HashSet<ChunkPos> = std::collections::HashSet::with_capacity(16);

    pending_chunks.push_back((start_cp, 0));
    visited_chunks.insert(start_cp);

    while let Some((cp, depth)) = pending_chunks.pop_front() {
        if depth >= 3 { continue; }

        // Phase 1: propagate within this chunk (drop borrow before neighbor access)
        if let Some(mut chunk) = chunk_store.get_mut(&cp) {
            propagate_block_light(&mut chunk);
        }

        // Phase 2: collect edge light levels (immutable borrow, separate scope)
        let neighbor_info: Vec<(ChunkPos, usize)> = {
            let neighbors: [(i32, i32, usize, usize); 4] = [
                (-1, 0, 0, 8), (1, 0, 15, 8), (0, -1, 8, 0), (0, 1, 8, 15),
            ];
            let mut result = Vec::with_capacity(4);
            if let Some(chunk) = chunk_store.get(&cp) {
                for (_dcx, _dcz, edge_x, edge_z) in &neighbors {
                    let ncp = ChunkPos::new(cp.x + _dcx, cp.z + _dcz);
                    if visited_chunks.contains(&ncp) { continue; }
                    // Check if light at this edge is strong enough to propagate
                    let mut has_light = false;
                    for sy in 0..24 {
                        if let Some(sec) = &chunk.sections[sy] {
                            let bl = sec.block_light[*edge_z * 16 + *edge_x] & 0x0F;
                            let bl2 = sec.block_light[*edge_z * 16 + *edge_x] >> 4;
                            if bl > 3 || bl2 > 3 { has_light = true; break; }
                        }
                    }
                    if has_light {
                        result.push((ncp, *edge_z * 16 + *edge_x)); // store edge light position
                    }
                }
            }
            result
        }; // immutable borrow dropped here

        // Phase 3: enqueue and propagate into neighbors (mutable borrows OK now)
        for (ncp, _edge_pos) in neighbor_info {
            visited_chunks.insert(ncp);
            pending_chunks.push_back((ncp, depth + 1));
            if let Some(mut neighbor) = chunk_store.get_mut(&ncp) {
                propagate_block_light(&mut neighbor);
            }
        }
    }
}
