//! 红石引擎 — 信号传播、比较器、观察者、发射器、音符盒
//!
//! Concurrency model:
//! - signal_map: DashMap (lock-free reads for BFS queries from any thread)
//! - observer_states, note_block_notes: DashMap (lock-free access)
//! - BFS state (pending_updates, processed): Mutex (single writer per tick)
//! - TNT/explosions: Mutex (separate from BFS state to reduce contention)

use dashmap::{DashMap, DashSet};
use mc_core::block::BlockState;
use mc_core::position::ChunkPos;
use std::sync::{Arc, LazyLock};
use parking_lot::Mutex;
use std::collections::{HashSet, VecDeque};

/// 红石组件类型判断
pub fn is_redstone_component(id: u32) -> bool {
    matches!(id, 993 | 994 | 152 | 852 | 853 | 995 | 996 | 997 | 998 | 70 | 71 | 46 | 84 | 137 | 138 | 149 | 218 | 317 | 23 | 158 | 74 | 25
        | 146 | 364 | 319 | 151 | 354 | 355
        | 296 | 298 | 299 | 300 | 301 | 302 | 303 | 304 | 305 | 306 | 307 // wood pressure plates
        | 308 | 309 | 310 | 311 | 312 | 313 | 314 | 315 | 316 | 317 // wood buttons
        | 318 | 287 | 356 // stone_button, tripwire_hook, sculk_shrieker
        | 1184 // B7: crafter (redstone pulse → craft one item)
        | 1260 | 1261 | 1262 | 1263 // B7: copper bulb (oxidized variants, unpowered)
        | 1264 | 1265 | 1266 | 1267 // B7: copper bulb (oxidized variants, powered)
    )
}

/// Check if a block is a target block (outputs signal based on projectile accuracy)
pub fn is_target_block(id: u32) -> bool {
    id == TARGET_BLOCK_ID
}

/// Check if a block is a sculk shrieker (summons warden)
pub fn is_sculk_shrieker(id: u32) -> bool {
    id == SCULK_SHRIEKER_ID
}

// ══════════════════════════════════════════════════════════
// Pressure plate & Tripwire hook constants
// ══════════════════════════════════════════════════════════

/// Stone pressure plate — activated by players/mobs
pub const STONE_PRESSURE_PLATE_ID: u32 = 296;
/// Oak pressure plate (first of 10 wood variants: 298-307)
pub const OAK_PRESSURE_PLATE_ID: u32 = 298;
/// Light weighted pressure plate (gold)
pub const LIGHT_WEIGHTED_PRESSURE_PLATE_ID: u32 = 1043;
/// Heavy weighted pressure plate (iron)
pub const HEAVY_WEIGHTED_PRESSURE_PLATE_ID: u32 = 1044;
/// Tripwire hook ID
pub const TRIPWIRE_HOOK_ID: u32 = 287;
/// String / Tripwire ID
pub const TRIPWIRE_ID: u32 = 286;
/// Target block ID — outputs signal based on projectile accuracy
pub const TARGET_BLOCK_ID: u32 = 318;
/// Sculk shrieker ID — triggers warden spawning
pub const SCULK_SHRIEKER_ID: u32 = 356;

/// Check if a block is a pressure plate (any type)
pub fn is_pressure_plate(id: u32) -> bool {
    id == STONE_PRESSURE_PLATE_ID
        || (OAK_PRESSURE_PLATE_ID..=307).contains(&id)
        || id == LIGHT_WEIGHTED_PRESSURE_PLATE_ID
        || id == HEAVY_WEIGHTED_PRESSURE_PLATE_ID
}

/// Check if a block is a wooden button (10 variants: 308-317) or stone button (318)
pub fn is_button(id: u32) -> bool {
    (308..=318).contains(&id)
}

/// Check if a block is a tripwire-related component
pub fn is_tripwire_component(id: u32) -> bool {
    id == TRIPWIRE_HOOK_ID || id == TRIPWIRE_ID
}

/// Trapped chest ID — outputs redstone based on viewers
pub const TRAPPED_CHEST_ID: u32 = 146;
/// Lightning rod ID — outputs 15 when struck
pub const LIGHTNING_ROD_ID: u32 = 319;
/// Daylight detector ID — signal based on time
pub const DAYLIGHT_DETECTOR_ID: u32 = 151;
/// Sculk sensor ID — vibration detection
pub const SCULK_SENSOR_ID: u32 = 354;
/// Calibrated sculk sensor ID — frequency-filtered vibration
pub const CALIBRATED_SCULK_SENSOR_ID: u32 = 355;

/// Get passive redstone signal from non-powered sources
/// 26.2 Sculk Sensor vibrations: (x, y, z, frequency, age_ticks)
static VIBRATION_EVENTS: LazyLock<DashSet<(i32, i32, i32, u8)>> = LazyLock::new(DashSet::new);
const SCULK_SENSOR_RANGE: i32 = 8;

/// Register a vibration event near a position (block place/break, entity step, projectile)
pub fn register_vibration(x: i32, y: i32, z: i32, frequency: u8) {
    VIBRATION_EVENTS.insert((x, y, z, frequency));
}

/// Process vibration events: Sculk Sensors within range detect and output signal
pub fn tick_sculk_sensors(chunk_store: &crate::chunk_store::ChunkStore) {
    let events: Vec<_> = VIBRATION_EVENTS.iter().map(|e| *e.key()).collect();
    VIBRATION_EVENTS.clear();
    if events.is_empty() { return; }
    // Check each loaded chunk for Sculk Sensors
    for (cp, chunk) in &chunk_store.all_chunks() {
        let cx = cp.x; let cz = cp.z;
        for y in -64..320 {
            for lx in 0..16 {
                for lz in 0..16 {
                    let bid = chunk.get_block(lx, y, lz).id;
                    if bid != SCULK_SENSOR_ID && bid != CALIBRATED_SCULK_SENSOR_ID {
                        continue;
                    }
                    let wx = cx * 16 + lx as i32;
                    let wz = cz * 16 + lz as i32;
                    // Check if any vibration event is within range
                    for (ex, ey, ez, _freq) in &events {
                        let dx = (wx - ex).abs();
                        let dy = (y - ey).abs();
                        let dz = (wz - ez).abs();
                        if dx <= SCULK_SENSOR_RANGE && dy <= SCULK_SENSOR_RANGE && dz <= SCULK_SENSOR_RANGE {
                            // Simple: always output max signal when triggered
                            // TODO: signal strength = 15 - distance/2, frequency-based filtering for calibrated
                            // Signal propagation handled by redstone BFS
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// Set of daylight detector positions that are in inverted mode (26.2 toggle mechanic)
pub static INVERTED_DETECTORS: LazyLock<DashSet<(i32, i32, i32)>> = LazyLock::new(DashSet::new);

/// Toggle a daylight detector between normal and inverted mode
pub fn toggle_daylight_detector(x: i32, y: i32, z: i32) -> bool {
    if INVERTED_DETECTORS.contains(&(x, y, z)) {
        INVERTED_DETECTORS.remove(&(x, y, z));
        false
    } else {
        INVERTED_DETECTORS.insert((x, y, z));
        true
    }
}

pub fn get_environmental_signal(id: u32, time_of_day: i64) -> u8 {
    match id {
        DAYLIGHT_DETECTOR_ID => {
            let day_progress = (time_of_day % 24000) as f64 / 24000.0;
            (15.0 - day_progress * 30.0).abs().min(15.0) as u8
        }
        // Lightning rod: always outputs 0 unless struck (strike handled externally)
        LIGHTNING_ROD_ID => 0,
        // Sculk sensors: 0 unless vibration detected
        SCULK_SENSOR_ID | CALIBRATED_SCULK_SENSOR_ID => 0,
        _ => 0,
    }
}

/// TNT 实体类型
pub const TNT_ENTITY_TYPE: i32 = 55;

// ══════════════════════════════════════════════════════════
// Rail system constants
// ══════════════════════════════════════════════════════════

/// Powered rail — accelerates minecarts when powered
pub const POWERED_RAIL_ID: u32 = 27;
/// Detector rail — outputs redstone signal when minecart is on top
pub const DETECTOR_RAIL_ID: u32 = 28;
/// Activator rail — ejects riders when powered, disables hopper minecart
pub const ACTIVATOR_RAIL_ID: u32 = 157;

/// Check if block is any rail type
pub fn is_rail(id: u32) -> bool {
    matches!(id, POWERED_RAIL_ID | DETECTOR_RAIL_ID | ACTIVATOR_RAIL_ID | 66 | 155 | 156)
}

/// Check if block is a detector rail (outputs signal when minecart present)
pub fn is_detector_rail(id: u32) -> bool {
    id == DETECTOR_RAIL_ID
}

/// Check if block is an activator rail (ejects riders when powered)
pub fn is_activator_rail(id: u32) -> bool {
    id == ACTIVATOR_RAIL_ID
}

/// 容器方块 ID → 可被比较器检测
fn is_container_block(id: u32) -> bool {
    matches!(id, 54 | 146 | 290 | 61 | 62 | 291 | 23 | 158 | 154)
}

/// Global container fill provider — set by server startup to enable actual comparator fill levels.
/// Signature: fn(x, y, z) -> fill_ratio (0.0-1.0)
static CONTAINER_FILL_PROVIDER: parking_lot::RwLock<
    Option<Arc<dyn Fn(i32, i32, i32) -> f32 + Send + Sync>>
> = parking_lot::RwLock::new(None);

/// Register a container fill provider (called from server startup with access to ContainerManager)
pub fn set_container_fill_provider(f: Arc<dyn Fn(i32, i32, i32) -> f32 + Send + Sync>) {
    *CONTAINER_FILL_PROVIDER.write() = Some(f);
}

/// Global entity-on-block provider — set by server startup to detect entities standing on pressure plates/tripwires.
/// Signature: fn(x, y, z) -> entity_count (number of entities standing on this block)
static ENTITY_ON_BLOCK_PROVIDER: parking_lot::RwLock<
    Option<Arc<dyn Fn(i32, i32, i32) -> u8 + Send + Sync>>
> = parking_lot::RwLock::new(None);

/// Register an entity-on-block provider (called from server startup)
pub fn set_entity_on_block_provider(f: Arc<dyn Fn(i32, i32, i32) -> u8 + Send + Sync>) {
    *ENTITY_ON_BLOCK_PROVIDER.write() = Some(f);
}

/// Get number of entities standing on a block (0 = none)
pub fn entity_count_on_block(x: i32, y: i32, z: i32) -> u8 {
    if let Some(ref provider) = *ENTITY_ON_BLOCK_PROVIDER.read() {
        provider(x, y, z)
    } else {
        0
    }
}

/// 获取容器填充比例 (0.0-1.0) — 使用已注册的 provider，否则返回 0.0
pub fn container_fill_ratio(cs: &crate::chunk_store::ChunkStore, x: i32, y: i32, z: i32) -> f32 {
    let cp = ChunkPos::new(x >> 4, z >> 4);
    if let Some(chunk) = cs.get(&cp) {
        let block = chunk.get_block((x & 0xF) as usize, y, (z & 0xF) as usize);
        if is_container_block(block.id) {
            // Use registered provider if available, otherwise return 0.5 as fallback
            if let Some(ref provider) = *CONTAINER_FILL_PROVIDER.read() {
                return provider(x, y, z);
            }
            return 0.5; // fallback: assume half-full
        }
    }
    0.0
}

/// 红石信号源 → 信号强度
pub fn component_power(id: u32, lit: bool) -> u8 {
    match id {
        994 => if lit { 15 } else { 0 },  // torch
        152 => 15,  // redstone_block
        852 => if lit { 15 } else { 0 },  // lever
        853 => 15,  // button (pressed)
        993 => 0,   // wire (handled in propagation)
        // Buttons (wood: 308-317, stone: 318): pulse 15 when pressed
        308..=318 if lit => 15,
        308..=318 => 0,
        _ => 0,
    }
}

/// Calculate signal for entity-based redstone components (pressure plates, tripwire)
pub fn entity_component_power(id: u32, x: i32, y: i32, z: i32) -> u8 {
    if !is_pressure_plate(id) && id != TRIPWIRE_ID {
        return 0;
    }
    let count = entity_count_on_block(x, y, z);
    if count == 0 { return 0; }
    match id {
        // Stone pressure plate: 15 if entities present
        STONE_PRESSURE_PLATE_ID => 15,
        // Wood pressure plates: 15 if entities present
        298..=307 => 15,
        // Light weighted (gold): 1 per entity
        LIGHT_WEIGHTED_PRESSURE_PLATE_ID => count.min(15),
        // Heavy weighted (iron): 1 signal per 10 entities, min 1
        HEAVY_WEIGHTED_PRESSURE_PLATE_ID => {
            let signal = (count as u32 / 10) as u8;
            if signal > 0 { signal } else { 1 }
        }
        // Tripwire: 15 if any entity touches it
        TRIPWIRE_ID => 15,
        _ => 0,
    }
}

pub fn is_constant_source(id: u32) -> bool { matches!(id, 152) }
pub fn is_toggle_component(id: u32) -> bool { matches!(id, 852) }
pub fn is_pulse_component(id: u32) -> bool { matches!(id, 853) }
pub fn is_input_component(id: u32) -> bool { matches!(id, 994 | 152 | 852 | 853) }
/// B7: Copper Bulb — toggles state on redstone pulse (T-flip-flop)
pub fn is_copper_bulb(id: u32) -> bool { matches!(id, 1260..=1267) }

/// 音符盒下方方块 → 乐器音色
pub fn note_block_instrument(below_id: u32) -> &'static str {
    match below_id {
        1 | 12 | 87 => "minecraft:block.stone.note_block.basedrum",
        24..=26 => "minecraft:block.sand.note_block.snare",
        66 | 67 => "minecraft:block.glass.note_block.click",
        17..=23 => "minecraft:block.wood.note_block.bass",
        9..=11 => "minecraft:block.dirt.note_block.piano",
        41 => "minecraft:block.gold_block.note_block.bell",
        78 => "minecraft:block.ice.note_block.chime",
        147 => "minecraft:block.bone_block.note_block.xylophone",
        _ => "minecraft:block.stone.note_block.harp",
    }
}

/// BFS state — per-tick scratch data (protected by Mutex, single writer)
struct BfsState {
    pending_updates: VecDeque<(i32, i32, i32)>,
    processed: HashSet<(i32, i32, i32)>,
}

/// 红石引擎 — lock-free signal map with interior mutability for BFS state
pub struct RedstoneEngine {
    bfs: Mutex<BfsState>,
    /// Signal strength cache: position → power (0-15). DashMap for lock-free reads.
    pub signal_map: DashMap<(i32, i32, i32), u8>,
    pulses: Mutex<Vec<(i32, i32, i32, u8)>>,
    repeater_delays: Mutex<Vec<(i32, i32, i32, u8, u8)>>,
    /// TNT fuse queue (separate lock to avoid contention with BFS)
    tnt_fuses: Mutex<Vec<(i32, i32, i32, u8)>>,
    /// Pending explosions (drained by main tick loop)
    pub pending_explosions: Mutex<Vec<(i32, i32, i32, f32)>>,
    /// Observer: tracks last-seen front block state per observer position
    observer_states: DashMap<(i32, i32, i32), u32>,
    /// Note block: current pitch per position (0-24)
    note_block_notes: DashMap<(i32, i32, i32), u8>,
    /// Deferred dispenser activations (next tick)
    pending_dispenser_activations: Mutex<Vec<(i32, i32, i32)>>,
    /// Hopper transfer tracking: (x, y, z, cooldown_ticks)
    hoppers: Mutex<Vec<(i32, i32, i32, u8)>>,
}

impl Default for RedstoneEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RedstoneEngine {
    pub fn new() -> Self {
        Self {
            bfs: Mutex::new(BfsState {
                pending_updates: VecDeque::new(),
                processed: HashSet::new(),
            }),
            signal_map: DashMap::new(),
            pulses: Mutex::new(Vec::new()),
            repeater_delays: Mutex::new(Vec::new()),
            tnt_fuses: Mutex::new(Vec::new()),
            pending_explosions: Mutex::new(Vec::new()),
            observer_states: DashMap::new(),
            note_block_notes: DashMap::new(),
            pending_dispenser_activations: Mutex::new(Vec::new()),
            hoppers: Mutex::new(Vec::new()),
        }
    }

    /// 方块放置/破坏/交互触发 (lock-free — called from connection handlers)
    pub fn on_block_change(&self, _chunk_store: &crate::chunk_store::ChunkStore, x: i32, y: i32, z: i32) {
        self.bfs.lock().pending_updates.push_back((x, y, z));
    }

    /// 每 2 tick 运行 (takes &self — no outer RwLock needed)
    /// C2 optimization: only propagates from components whose signal actually changed,
    /// with a per-tick node budget to prevent CPU spikes on large redstone networks.
    pub fn tick(&self, chunk_store: &crate::chunk_store::ChunkStore) {
        const MAX_NODES_PER_TICK: usize = 4096; // propagation budget
        let mut bfs = self.bfs.lock();

        // ── Observer detection (check before processing updates) ──
        let mut observer_pulses: Vec<(i32, i32, i32)> = Vec::new();
        for entry in self.observer_states.iter() {
            let (ox, oy, oz) = *entry.key();
            let prev_state = *entry.value();
            let cp = ChunkPos::new(ox >> 4, oz >> 4);
            if let Some(_chunk) = chunk_store.get(&cp) {
                let fx = ox; let fz = oz + 1; let fy = oy;
                let fcp = ChunkPos::new(fx >> 4, fz >> 4);
                if let Some(fchunk) = chunk_store.get(&fcp) {
                    let front_block = fchunk.get_block((fx & 0xF) as usize, fy, (fz & 0xF) as usize);
                    let current_state = front_block.id;
                    if current_state != prev_state {
                        observer_pulses.push((ox, oy, oz));
                        self.observer_states.insert((ox, oy, oz), current_state);
                    }
                }
            }
        }
        for (ox, oy, oz) in &observer_pulses {
            self.signal_map.insert((*ox, *oy, *oz), 15);
            for (dx, dz) in &[(1,0), (-1,0), (0,1), (0,-1)] {
                let nx = *ox + dx; let nz = *oz + dz;
                bfs.pending_updates.push_back((nx, *oy, nz));
            }
        }

        // ── BFS Signal propagation (C2: change-detection + budget) ──
        let mut nodes_processed = 0usize;
        while let Some((x, y, z)) = bfs.pending_updates.pop_front() {
            if bfs.processed.contains(&(x, y, z)) { continue; }
            if nodes_processed >= MAX_NODES_PER_TICK {
                // Budget exceeded — defer remaining updates to next tick
                bfs.pending_updates.push_back((x, y, z));
                break;
            }
            bfs.processed.insert((x, y, z));
            nodes_processed += 1;

            let cp = ChunkPos::new(x >> 4, z >> 4);
            if let Some(chunk) = chunk_store.get(&cp) {
                let block = chunk.get_block((x & 0xF) as usize, y, (z & 0xF) as usize);
                if !is_redstone_component(block.id) { continue; }

                let power = if is_constant_source(block.id) || is_toggle_component(block.id) || is_pulse_component(block.id) { 15 }
                else if is_pressure_plate(block.id) || is_tripwire_component(block.id) {
                    // Entity-detection components: signal = entities on block
                    entity_component_power(block.id, x, y, z)
                }
                else if block.id == 149 {
                    // ── Comparator ──
                    let back = get_back_signal(&self.signal_map, x, y, z);
                    let side_a = get_side_signal(&self.signal_map, x, y, z, true);
                    let side_b = get_side_signal(&self.signal_map, x, y, z, false);
                    let max_side = side_a.max(side_b);
                    if is_comparator_subtract(&self.signal_map, x, y, z) {
                        // Subtraction mode: output = back - max_side (min 0)
                        back.saturating_sub(max_side)
                    } else {
                        // Comparison mode: output = back if back > max_side, else 0
                        if back > max_side { back } else { 0 }
                    }
                }
                else if block.id == 317 { 0 } // Observer output handled above
                else if block.id == 993 {
                    // Redstone wire
                    let mut max_power = 0u8;
                    for (dx, dz) in &[(1,0), (-1,0), (0,1), (0,-1)] {
                        let nx = x + dx; let nz = z + dz;
                        if let Some(p) = self.signal_map.get(&(nx, y, nz)) { max_power = max_power.max(*p); }
                        if let Some(p) = self.signal_map.get(&(nx, y - 1, nz)) { max_power = max_power.max(*p); }
                        if let Some(p) = self.signal_map.get(&(nx, y + 1, nz)) { max_power = max_power.max(*p); }
                    }
                    max_power.saturating_sub(1)
                } else { component_power(block.id, true) };

                // Quasi-connectivity: pistons, dispensers, droppers also check block above
                let qc_powered = matches!(block.id, 137 | 138 | 23 | 158)
                    && self.signal_map.get(&(x, y + 1, z)).map(|v| *v > 0).unwrap_or(false);
                let effective_power = if qc_powered { power.max(15) } else { power };

                // C2: Only propagate if signal actually changed (skip redundant updates)
                let prev_signal = self.signal_map.get(&(x, y, z)).map(|v| *v);
                let signal_changed = prev_signal != Some(effective_power);
                if signal_changed {
                    self.signal_map.insert((x, y, z), effective_power);
                } else {
                    // Signal unchanged — skip neighbor propagation (Alternate Current key optimization)
                    continue;
                }

                // Propagate to neighbors only if signal changed
                if effective_power > 0 {
                    for (dx, dy, dz) in &[(1,0,0), (-1,0,0), (0,1,0), (0,-1,0), (0,0,1), (0,0,-1)] {
                        let nx = x + dx; let ny = y + dy; let nz = z + dz;
                        if !(-64..=319).contains(&ny) { continue; }
                        let ncp = ChunkPos::new(nx >> 4, nz >> 4);
                        if let Some(nchunk) = chunk_store.get(&ncp) {
                            let nblock = nchunk.get_block((nx & 0xF) as usize, ny, (nz & 0xF) as usize);
                            if is_redstone_component(nblock.id) && !bfs.processed.contains(&(nx, ny, nz)) {
                                bfs.pending_updates.push_back((nx, ny, nz));
                            }
                        }
                    }
                }

                // ── Component side effects ──
                // Piston (4-directional, sticky pull) — QC aware (effective_power from above)
                if matches!(block.id, 137 | 138) {
                    let is_sticky = block.id == 138;
                    if effective_power > 0 {
                        // Extended: push block in facing direction (3D: east/west/north/south/up/down)
                        let (dx, dy, dz) = detect_piston_facing(x, y, z, self);
                        let px = x + dx; let py = y + dy; let pz = z + dz;
                        let pcp = ChunkPos::new(px >> 4, pz >> 4);
                        if (-64..=319).contains(&py)
                            && let Some(mut pchunk) = chunk_store.get_mut(&pcp)
                        {
                            let pushed = pchunk.get_block((px & 0xF) as usize, py, (pz & 0xF) as usize);
                            if !pushed.is_air() && pushed.id != 266 {
                                let (ppx, ppy, ppz) = (px + dx, py + dy, pz + dz);
                                let ppcp = ChunkPos::new(ppx >> 4, ppz >> 4);
                                if (-64..=319).contains(&ppy)
                                    && let Some(mut ppchunk) = chunk_store.get_mut(&ppcp)
                                {
                                    let dest = ppchunk.get_block((ppx & 0xF) as usize, ppy, (ppz & 0xF) as usize);
                                    if dest.is_air() {
                                        ppchunk.set_block((ppx & 0xF) as usize, ppy, (ppz & 0xF) as usize, pushed);
                                        pchunk.set_block((px & 0xF) as usize, py, (pz & 0xF) as usize, BlockState::AIR);
                                    }
                                }
                            }
                        }
                    }
                    // Sticky piston: pull block when depowered (power == 0)
                    if is_sticky && effective_power == 0 {
                        let (dx, dy, dz) = detect_piston_facing(x, y, z, self);
                        let behind_x = x - dx; let behind_y = y - dy; let behind_z = z - dz;
                        if (-64..=319).contains(&behind_y) {
                            let bcp = ChunkPos::new(behind_x >> 4, behind_z >> 4);
                            if let Some(mut bchunk) = chunk_store.get_mut(&bcp) {
                                let behind = bchunk.get_block((behind_x & 0xF) as usize, behind_y, (behind_z & 0xF) as usize);
                                let piston_front = bchunk.get_block((x & 0xF) as usize, y, (z & 0xF) as usize);
                                if !behind.is_air() && behind.id != 266 && piston_front.is_air() {
                                    bchunk.set_block((x & 0xF) as usize, y, (z & 0xF) as usize, behind);
                                    bchunk.set_block((behind_x & 0xF) as usize, behind_y, (behind_z & 0xF) as usize, BlockState::AIR);
                                }
                            }
                        }
                    }
                }
/// Detect piston facing direction from surrounding signal context.
/// Returns (dx, dy, dz) — the direction the piston head extends.
fn detect_piston_facing(x: i32, y: i32, z: i32, engine: &RedstoneEngine) -> (i32, i32, i32) {
    // Check 6 cardinal directions for signal presence
    let dirs: [(i32, i32, i32); 6] = [
        (1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, 1, 0), (0, -1, 0),
    ];
    for &(dx, dy, dz) in &dirs {
        if let Some(p) = engine.signal_map.get(&(x + dx, y + dy, z + dz))
            && *p > 0 { return (dx, dy, dz); }
    }
    // Default: check for solid block behind (piston base)
    for &(dx, dy, dz) in &dirs {
        if let Some(p) = engine.signal_map.get(&(x - dx, y - dy, z - dz))
            && *p > 0 { return (dx, dy, dz); }
    }
    (1, 0, 0) // default east
}

                // TNT ignite
                if block.id == 25 && effective_power > 0 {
                    self.ignite_tnt(x, y, z);
                }
                // Dispenser/Dropper activation (QC-powered too, deferred to post-propagation)
                if matches!(block.id, 23 | 158) && effective_power > 0 {
                    self.pending_dispenser_activations.lock().push((x, y, z));
                }
                // Note block
                if block.id == 74 && effective_power > 0 {
                    if let Some(mut note) = self.note_block_notes.get_mut(&(x, y, z)) {
                        *note = (*note + 1) % 25;
                    } else {
                        self.note_block_notes.insert((x, y, z), 1);
                    }
                }
                // B7: Copper Bulb — toggle state on rising edge
                if is_copper_bulb(block.id) && signal_changed && effective_power > 0 && prev_signal == Some(0) {
                    let new_id = if block.id >= 1264 { block.id - 4 } else { block.id + 4 };
                    if let Some(mut ch) = chunk_store.get_mut(&cp) {
                        ch.set_block((x & 0xF) as usize, y, (z & 0xF) as usize, BlockState::new(new_id));
                    }
                }
            }
        }

        // ── Tick timers (drop BFS lock before acquiring other locks) ──
        drop(bfs);

        // Button decay
        {
            let mut pulses = self.pulses.lock();
            let mut expired = Vec::new();
            pulses.retain_mut(|(px, py, pz, rem)| {
                if *rem > 0 { *rem -= 1; true } else { expired.push((*px, *py, *pz)); false }
            });
            for pos in expired { self.on_block_change(chunk_store, pos.0, pos.1, pos.2); }
        }

        // Repeater delays
        {
            let mut repeaters = self.repeater_delays.lock();
            let mut ready = Vec::new();
            repeaters.retain_mut(|(rx, ry, rz, rem, _)| {
                if *rem > 0 { *rem -= 1; true } else { ready.push((*rx, *ry, *rz)); false }
            });
            for pos in ready { self.on_block_change(chunk_store, pos.0, pos.1, pos.2); }
        }

        // TNT fuses
        {
            let mut fuses = self.tnt_fuses.lock();
            let mut detonated = Vec::new();
            fuses.retain_mut(|(tx, ty, tz, rem)| {
                if *rem > 0 { *rem -= 1; false } else { detonated.push((*tx, *ty, *tz)); true }
            });
            for pos in detonated {
                self.pending_explosions.lock().push((pos.0, pos.1, pos.2, 4.0));
            }
        }

        // Observer pulse decay
        let obs_expired: Vec<(i32, i32, i32)> = {
            let mut o = Vec::new();
            for entry in self.observer_states.iter() {
                let pos = *entry.key();
                if self.signal_map.get(&pos).map(|v| *v == 15).unwrap_or(false)
                    && !observer_pulses.contains(&pos) {
                    o.push(pos);
                }
            }
            o
        };
        // Re-acquire BFS lock for observer cleanup
        let mut bfs2 = self.bfs.lock();
        for pos in obs_expired {
            self.signal_map.remove(&(pos.0, pos.1, pos.2));
            for (dx, dz) in &[(1,0), (-1,0), (0,1), (0,-1)] {
                bfs2.pending_updates.push_back((pos.0 + dx, pos.1, pos.2 + dz));
            }
        }

        bfs2.processed.clear();
        self.pending_dispenser_activations.lock().clear();
    }

    /// Ignite TNT (lock-free — called from BFS propagation)
    pub fn ignite_tnt(&self, x: i32, y: i32, z: i32) {
        self.tnt_fuses.lock().push((x, y, z, 80));
    }

    pub fn toggle_lever(&self, _chunk_store: &crate::chunk_store::ChunkStore, x: i32, y: i32, z: i32) {
        self.on_block_change(_chunk_store, x, y, z);
    }

    pub fn press_button(&self, _chunk_store: &crate::chunk_store::ChunkStore, x: i32, y: i32, z: i32) {
        self.pulses.lock().push((x, y, z, 20));
        self.on_block_change(_chunk_store, x, y, z);
    }

    /// 注册观察者位置 (放置方块时调用)
    pub fn register_observer(&self, x: i32, y: i32, z: i32, chunk_store: &crate::chunk_store::ChunkStore) {
        let cp = ChunkPos::new(x >> 4, z >> 4);
        if let Some(_chunk) = chunk_store.get(&cp) {
            let fx = x; let fz = z + 1; let fy = y;
            let fcp = ChunkPos::new(fx >> 4, fz >> 4);
            if let Some(fchunk) = chunk_store.get(&fcp) {
                let front = fchunk.get_block((fx & 0xF) as usize, fy, (fz & 0xF) as usize);
                self.observer_states.insert((x, y, z), front.id);
            }
        }
    }

    /// 移除观察者
    pub fn remove_observer(&self, x: i32, y: i32, z: i32) {
        self.observer_states.remove(&(x, y, z));
    }

    /// 音符盒音高递增
    pub fn increment_note(&self, x: i32, y: i32, z: i32) -> u8 {
        // DashMap: use get_mut or insert/get pattern for atomic update
        if let Some(mut entry) = self.note_block_notes.get_mut(&(x, y, z)) {
            *entry = (*entry + 1) % 25;
            *entry
        } else {
            self.note_block_notes.insert((x, y, z), 1);
            1
        }
    }

    /// 获取音符盒音高
    pub fn get_note(&self, x: i32, y: i32, z: i32) -> u8 {
        self.note_block_notes.get(&(x, y, z)).map(|v| *v).unwrap_or(0)
    }

    /// 获取待处理的发射器/投掷器激活列表
    pub fn take_dispenser_activations(&self) -> Vec<(i32, i32, i32)> {
        std::mem::take(&mut *self.pending_dispenser_activations.lock())
    }

    /// Register a hopper for item transfer processing
    pub fn register_hopper(&self, x: i32, y: i32, z: i32) {
        self.hoppers.lock().push((x, y, z, 8));
    }

    /// Remove a hopper (called when block is broken)
    pub fn remove_hopper(&self, x: i32, y: i32, z: i32) {
        self.hoppers.lock().retain(|(hx, hy, hz, _)| *hx != x || *hy != y || *hz != z);
    }

    /// Tick hopper item transfer (every 8 ticks ≈ 0.4s)
    pub fn tick_hoppers(&self, _chunk_store: &crate::chunk_store::ChunkStore) {
        let mut hoppers = self.hoppers.lock();
        for (hx, hy, hz, cd) in hoppers.iter_mut() {
            if *cd > 0 { *cd -= 1; continue; }
            *cd = 8;
            // Skip if powered (redstone locked)
            if self.signal_map.get(&(*hx, *hy, *hz)).map(|v| *v > 0).unwrap_or(false) {
                continue;
            }
            // Item transfer handled by main.rs via get_ready_hoppers()
        }
    }

    /// Returns positions of hoppers that are ready for item transfer (cooldown=0, not locked)
    pub fn get_ready_hoppers(&self) -> Vec<(i32, i32, i32)> {
        let hoppers = self.hoppers.lock();
        hoppers.iter()
            .filter(|(hx, hy, hz, cd)| {
                *cd == 0 && !self.signal_map.get(&(*hx, *hy, *hz))
                    .map(|v| *v > 0).unwrap_or(false)
            })
            .map(|(hx, hy, hz, _)| (*hx, *hy, *hz))
            .collect()
    }
}

/// 检查比较器减法模式 — 检测比较器前方是否被红石火把或拉杆供电
pub fn is_comparator_subtract(signal_map: &DashMap<(i32, i32, i32), u8>, x: i32, y: i32, z: i32) -> bool {
    // Check if the comparator output side has a powered redstone torch
    // (in vanilla, a powered comparator side switches to subtract mode)
    let front = signal_map.get(&(x, y, z + 1)).map(|v| *v).unwrap_or(0);
    front > 0
}

/// 比较器辅助: 后方信号
fn get_back_signal(signal_map: &DashMap<(i32, i32, i32), u8>, x: i32, y: i32, z: i32) -> u8 {
    signal_map.get(&(x, y, z - 1)).map(|v| *v).unwrap_or(0)
}

/// 比较器辅助: 侧方信号 (取两侧最大值)
fn get_side_signal(signal_map: &DashMap<(i32, i32, i32), u8>, x: i32, y: i32, z: i32, _left: bool) -> u8 {
    let a = signal_map.get(&(x - 1, y, z)).map(|v| *v).unwrap_or(0);
    let b = signal_map.get(&(x + 1, y, z)).map(|v| *v).unwrap_or(0);
    a.max(b)
}

/// 注册中继器延迟
pub fn set_repeater_delay(engine: &RedstoneEngine, x: i32, y: i32, z: i32, delay: u8, strength: u8) {
    engine.repeater_delays.lock().push((x, y, z, delay, strength));
}

/// Crafter 方块 ID
pub const CRAFTER_ID: u32 = 364;
/// Copper Bulb IDs (四种氧化阶段)
pub const COPPER_BULB_IDS: &[u32] = &[400, 401, 402, 403]; // copper_bulb through oxidized_copper_bulb

/// 检查是否为 Crafter 或 Copper Bulb (特殊红石组件)
pub fn is_special_redstone_component(id: u32) -> bool {
    id == CRAFTER_ID || COPPER_BULB_IDS.contains(&id)
}

/// 根据氧化阶段获取铜灯的亮度
pub fn copper_bulb_light_level(id: u32) -> u8 {
    match id {
        332 => 15, // normal
        333 => 12, // exposed
        334 => 8,  // weathered
        335 => 4,  // oxidized
        _ => 0,
    }
}

/// 铜灯氧化进阶 (自然氧化: ~0.01% 概率/tick)
pub fn oxidize_copper_bulb(current_id: u32) -> u32 {
    match current_id {
        332 => 333, // normal → exposed
        333 => 334, // exposed → weathered
        334 => 335, // weathered → oxidized
        _ => current_id, // already oxidized or not copper
    }
}

/// 检查邻居是否有红石信号
pub fn has_neighbor_power(engine: &RedstoneEngine, x: i32, y: i32, z: i32) -> bool {
    for (dx, dy, dz) in &[(1,0,0), (-1,0,0), (0,1,0), (0,-1,0), (0,0,1), (0,0,-1)] {
        if let Some(p) = engine.signal_map.get(&(x + dx, y + dy, z + dz))
            && *p > 0 { return true; }
    }
    false
}

/// 获取某位置的信号强度
pub fn get_signal_strength(engine: &RedstoneEngine, x: i32, y: i32, z: i32) -> u8 {
    engine.signal_map.get(&(x, y, z)).map(|v| *v).unwrap_or(0)
}
