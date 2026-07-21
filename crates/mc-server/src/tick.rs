//! Tick 子系统 — 从 main.rs 提取的周期性任务
//!
//! 每个函数返回执行耗时 (微秒), 用于 Prometheus 阶段计时指标.
//! TickScheduler 按 interval 调度, 自动计时 + 超预算告警.

use mc_core::world_state::WorldState;
use mc_player::mob::MobAiState;
use mc_player::player::SharedPlayerManager;
use mc_world::chunk_store::ChunkStore;
use std::sync::Arc;
use std::time::Instant;

// Type aliases for complex types (clippy::type_complexity)
type SharedWorldState = Arc<parking_lot::RwLock<WorldState>>;
type SharedDroppedItems = Arc<parking_lot::RwLock<std::collections::HashMap<i32, (u32, f64, f64, f64)>>>;
type SharedDirtyChunks = Arc<parking_lot::RwLock<std::collections::HashSet<mc_core::position::ChunkPos>>>;
type SharedAdvancementTracker = Arc<parking_lot::RwLock<mc_player::advancement::AdvancementTracker>>;

// ═══════════════════════════════════════════════════════════════
// TickStage — 调度器条目
// ═══════════════════════════════════════════════════════════════

/// Tick 阶段定义: 名称 + 间隔 + 超时预算
/// (reserved for Phase E — tick timing instrumentation)
#[allow(dead_code)]
pub struct TickStage {
    pub name: &'static str,
    pub interval: u64,        // 每 N tick 执行一次
    pub max_budget_us: u64,   // 超时告警阈值 (微秒)
}

impl TickStage {
    pub const fn new(name: &'static str, interval: u64, max_budget_us: u64) -> Self {
        Self { name, interval, max_budget_us }
    }
}

/// 内置 tick 阶段表 (按 interval 排序)
/// (reserved for Phase E — tick timing instrumentation)
#[allow(dead_code)]
pub const TICK_STAGES: &[TickStage] = &[
    // 每 tick: 饥饿 + 效果 + AI + 经验球
    TickStage::new("hunger",         1,    200),
    TickStage::new("mob_ai",         1,   3000),
    TickStage::new("xp",             1,    100),
    TickStage::new("furnace",        1,    200),
    TickStage::new("brewing",        5,    200),
    TickStage::new("fluid",          5,   1000),
    // 每 2 tick: 红石
    TickStage::new("redstone",       2,   2000),
    // 每 8 tick: 漏斗
    TickStage::new("hopper",         8,    500),
    // 每 10 tick: 矿车加速
    TickStage::new("rails",         10,    200),
    // 每 20 tick: 天气 + 环境伤害 + 物理 + 信标 + BossBar + 传送门 + 世界边界 + Crafter
    TickStage::new("weather",       20,    500),
    TickStage::new("env_damage",    20,    500),
    TickStage::new("physics",       20,   1000),
    TickStage::new("fishing",       20,    200),
    TickStage::new("beacon",        20,    200),
    TickStage::new("bossbar",       20,    100),
    TickStage::new("portal",        20,    200),
    TickStage::new("world_border",  20,    100),
    // 每 40 tick: 寻路
    TickStage::new("pathfind",      40,    500),
    // 每 80 tick: 信标效果
    TickStage::new("beacon_effect", 80,    200),
    // 每 100 tick: 敌对生成
    TickStage::new("hostile_spawn", 100,  2000),
    // 每 200 tick: 被动生成 + 作物
    TickStage::new("passive_spawn", 200,  2000),
    TickStage::new("crops",        200,    500),
    // 每 2400 tick: 村民
    TickStage::new("villagers",   2400,    500),
    // 每 6000 tick: 自动保存
    TickStage::new("save",        6000,   5000),
    // 每 72000 tick: 铜氧化
    TickStage::new("copper",     72000,    200),
];

// ═══════════════════════════════════════════════════════════════
// Timing helper
// ═══════════════════════════════════════════════════════════════

/// 执行 tick 子系统 (带耗时统计 + Prometheus 指标)
pub fn run_stage(name: &'static str, f: impl FnOnce()) -> u64 {
    let start = Instant::now();
    f();
    let us = start.elapsed().as_micros() as u64;
    if us > 5000 {
        tracing::warn!("Tick stage '{}' took {}us (exceeds 5ms budget)", name, us);
    }
    crate::metrics::record_stage_time(name, us);
    us
}

// ═══════════════════════════════════════════════════════════════
// Weather & Environment
// ═══════════════════════════════════════════════════════════════

/// 天气循环 (每 20 tick)
pub fn tick_weather(ws: &SharedWorldState, pm: &SharedPlayerManager) -> u64 {
    run_stage("weather", || {
        let old_weather;
        let new_weather;
        {
            let mut w = ws.write();
            old_weather = w.weather;
            let cycle = w.game_rules.get("doWeatherCycle").map(|v| v == "true").unwrap_or(true);
            if cycle {
                w.weather_timer = w.weather_timer.saturating_sub(1);
                if w.weather_timer == 0 {
                    match w.weather {
                        mc_core::world_state::Weather::Clear => {
                            w.weather = mc_core::world_state::Weather::Rain;
                            w.weather_timer = 6000 + fastrand::u64(0..12000);
                        }
                        mc_core::world_state::Weather::Rain => {
                            w.weather = mc_core::world_state::Weather::Thunder;
                            w.weather_timer = 3000 + fastrand::u64(0..6000);
                        }
                        mc_core::world_state::Weather::Thunder => {
                            w.weather = mc_core::world_state::Weather::Clear;
                            w.weather_timer = 12000 + fastrand::u64(0..120000);
                        }
                    }
                }
            }
            new_weather = w.weather;
        }
        if old_weather != new_weather {
            let event_id: u8 = match new_weather {
                mc_core::world_state::Weather::Rain => 1,
                mc_core::world_state::Weather::Thunder => 2,
                mc_core::world_state::Weather::Clear => 0,
            };
            pm.broadcast_global(mc_player::player::PlayerStateEventKind::GameEventGlobal(event_id, 0.0));
        }
        // Lightning spawns during thunder (1/100k per tick per player)
        if new_weather == mc_core::world_state::Weather::Thunder {
            for p in pm.all_players() {
                if fastrand::u32(0..100000) == 0 {
                    let lx = p.position.x + (fastrand::f64() - 0.5) * 32.0;
                    let lz = p.position.z + (fastrand::f64() - 0.5) * 32.0;
                    let ly = p.position.y + 10.0;
                    pm.broadcast_global(mc_player::player::PlayerStateEventKind::GameEventGlobal(3, 0.0)); // lightning event
                    // Deal damage to entities within 3 block radius of lightning
                    for target in pm.all_players() {
                        let dx = target.position.x - lx;
                        let dz = target.position.z - lz;
                        if dx*dx + dz*dz < 9.0 && (target.position.y - ly).abs() < 3.0 {
                            let _ = pm.apply_damage(&target.uuid, 5.0, 0); // lightning damage
                            let _ = pm.add_effect(&target.uuid, mc_core::effect::ActiveEffect {
                                effect: mc_core::effect::EffectType::InstantDamage,
                                amplifier: 0, duration_ticks: 0,
                            });
                        }
                    }
                }
            }
        }
    })
}

/// 环境伤害检测 (每 20 tick)
pub fn tick_environmental_damage(pm: &SharedPlayerManager, cs: &ChunkStore) -> u64 {
    run_stage("env_damage", || {
        for p in pm.all_players() {
            if p.position.y < -64.0 { pm.apply_environmental_damage(&p.uuid, "void"); }
            if p.fall_distance > 3.0 && p.position.y <= 0.0 {
                pm.apply_fall_damage(&p.uuid, p.fall_distance, 0);
            }
            let px = p.position.x as i32;
            let py = (p.position.y + 1.6) as i32;
            let pz = p.position.z as i32;
            let cp = mc_core::position::ChunkPos::new(px >> 4, pz >> 4);
            if let Some(chunk) = cs.get(&cp) {
                let head_block = chunk.get_block((px & 0xF) as usize, py, (pz & 0xF) as usize);
                if head_block.id == 267 || head_block.id == 268 {
                    // ConduitPower (28): grants complete drowning immunity while active
                    // WaterBreathing (12): also prevents drowning
                    // BreathOfTheNautilus (39): enhanced underwater breathing + drowning immunity
                    let has_water_breathing = pm.get_effect_level(&p.uuid, 12) > 0
                        || pm.get_effect_level(&p.uuid, 28) > 0
                        || pm.get_effect_level(&p.uuid, 39) > 0;
                    if !has_water_breathing {
                        // Respiration enchantment: reduce drowning damage probability
                        let respiration_lvl = pm.get_armor_enchant_level(&p.uuid, 39, "respiration");
                        if respiration_lvl > 0 && fastrand::u32(0..(respiration_lvl + 1)) > 0 {
                            // Skip drowning damage (higher level = more skips)
                        } else {
                            pm.apply_environmental_damage(&p.uuid, "drowning");
                        }
                    }
                }
                // DepthStrider: water movement speed boost (handled in movement handler)
                // FrostWalker: create frosted ice under player feet
                if pm.get_armor_enchant_level(&p.uuid, 36, "frost_walker") > 0 {
                    let bx = p.position.x as i32;
                    let by = (p.position.y - 1.0) as i32;
                    let bz = p.position.z as i32;
                    let cp = mc_core::position::ChunkPos::new(bx >> 4, bz >> 4);
                    if let Some(mut chunk) = cs.get_mut(&cp) {
                        let block_below = chunk.get_block((bx & 0xF) as usize, by, (bz & 0xF) as usize);
                        if block_below.id == 267 {
                            // Replace water with frosted_ice
                            chunk.set_block((bx & 0xF) as usize, by, (bz & 0xF) as usize,
                                mc_core::block::BlockState::new(383)); // frosted_ice
                        }
                        // Also check adjacent blocks for larger radius
                        for dx in -1i32..=1 {
                            for dz in -1i32..=1 {
                                if dx == 0 && dz == 0 { continue; }
                                let nx = bx + dx; let nz = bz + dz;
                                let ncp = mc_core::position::ChunkPos::new(nx >> 4, nz >> 4);
                                if ncp == cp {
                                    let nb = chunk.get_block((nx & 0xF) as usize, by, (nz & 0xF) as usize);
                                    if nb.id == 267 {
                                        chunk.set_block((nx & 0xF) as usize, by, (nz & 0xF) as usize,
                                            mc_core::block::BlockState::new(383));
                                    }
                                }
                            }
                        }
                    }
                }
                // Suffocation: player head inside a solid block (not air/liquid/plants)
                let is_solid = head_block.id != 0 // not air
                    && head_block.id != 267 && head_block.id != 268 // not water/lava
                    && head_block.id != 51 // not fire
                    && !(30..=31).contains(&head_block.id) // not cobweb/grass variants
                    ;
                if is_solid && head_block.id < 256 {
                    // Apply 1 HP damage per 10 ticks (half heart per half second)
                    pm.apply_environmental_damage(&p.uuid, "suffocation");
                }
            }
        }
    })
}

// ═══════════════════════════════════════════════════════════════
// Mob Spawning
// ═══════════════════════════════════════════════════════════════

/// 敌对生物生成 (每 100 tick)
pub fn tick_hostile_spawning(
    pm: &SharedPlayerManager, mob_mgr: &Arc<mc_player::mob::MobManager>,
    cs: &ChunkStore, ws: &SharedWorldState,
    next_eid: &Arc<std::sync::atomic::AtomicI32>,
) -> u64 {
    run_stage("hostile_spawn", || {
        let (can_spawn, seed) = {
            let w = ws.read();
            (w.game_rules.get("doMobSpawning").map(|v| v == "true").unwrap_or(false)
                && w.daytime >= 13000 && w.daytime <= 23000
                && !matches!(w.difficulty, mc_core::world_state::Difficulty::Peaceful),
             w.seed)
        };
        if !can_spawn { return; }
        // C3: Per-player mob cap — each player gets a fair share of the cap
        let online = pm.online_count().max(1) as usize;
        let global_max = 50 + online * 10;
        if mob_mgr.count_hostile() >= global_max { return; }
        let per_player_cap = (global_max / online) as i32; // fair share per player

        // C7: Chunk spawn failure tracking — skip chunks that repeatedly fail
        // Uses a DashMap to track (chunk_pos, consecutive_failures)
        static FAILED_SPAWN_CHUNKS: std::sync::LazyLock<dashmap::DashMap<mc_core::position::ChunkPos, u8>> =
            std::sync::LazyLock::new(dashmap::DashMap::new);

        for player in pm.all_players() {
            // C3: Check per-player mob count
            let near_mobs = mob_mgr.count_near((player.position.x as i32).div_euclid(16), (player.position.z as i32).div_euclid(16));
            if near_mobs >= per_player_cap { continue; }

            if !fastrand::u32(..).is_multiple_of(3) { continue; }
            let angle = fastrand::f64() * std::f64::consts::TAU;
            let dist = 8.0 + fastrand::f64() * 16.0;
            let sx = player.position.x + angle.cos() * dist;
            let sz = player.position.z + angle.sin() * dist;
            let cp = mc_core::position::ChunkPos::new((sx as i32).div_euclid(16), (sz as i32).div_euclid(16));

            // C7: Skip chunks that have repeatedly failed spawns
            if let Some(fail_count) = FAILED_SPAWN_CHUNKS.get(&cp)
                && *fail_count >= 5 {
                    continue; // too many failures — skip this chunk
                }

            let spawn_y = if let Some(chunk) = cs.get(&cp) {
                let lx = (sx as i32).rem_euclid(16) as usize;
                let lz = (sz as i32).rem_euclid(16) as usize;
                let h = chunk.height_at(lx, lz);
                if chunk.combined_light(lx, h - 1, lz) > 7 {
                    // C7: Track failure — too bright
                    let mut entry = FAILED_SPAWN_CHUNKS.entry(cp).or_insert(0);
                    *entry += 1;
                    continue;
                }
                if !chunk.is_spawn_surface(lx, h - 1, lz) {
                    // C7: Track failure — no valid surface
                    let mut entry = FAILED_SPAWN_CHUNKS.entry(cp).or_insert(0);
                    *entry += 1;
                    continue;
                }
                // Spawn succeeded — reset failure counter
                FAILED_SPAWN_CHUNKS.remove(&cp);
                h as f64
            } else {
                let mut entry = FAILED_SPAWN_CHUNKS.entry(cp).or_insert(0);
                *entry += 1;
                64.0
            };
            let biome = mc_world::generator::sample_biome(sx as i32, sz as i32, seed);
            let mob_type = pick_hostile_mob(biome);
            let eid = next_eid.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let tracked = make_tracked_mob(eid, mob_type, sx, spawn_y, sz);
            mob_mgr.register(tracked);
            pm.broadcast_mob_spawn(eid, uuid::Uuid::new_v4(), mob_type, sx, spawn_y, sz);
        }

        // C7: Periodically clean stale failure entries (every 600 ticks accessible via tick counter)
        if fastrand::u32(0..600) == 0 {
            FAILED_SPAWN_CHUNKS.retain(|_, v| *v < 3); // keep only chunks with recent failures
        }
    })
}

fn pick_hostile_mob(biome: mc_core::biome::BiomeId) -> i32 {
    use mc_core::constants::entity_type::*;
    match fastrand::u32(..) % 20 {
        0 => ZOMBIE, 1 => SKELETON, 2 => CREEPER, 3 => SPIDER, 4 => SLIME, 5 => ENDERMAN,
        // Desert/Badlands → Husk
        6 => if matches!(biome,
            mc_core::biome::BiomeId::Desert | mc_core::biome::BiomeId::Badlands |
            mc_core::biome::BiomeId::ErodedBadlands | mc_core::biome::BiomeId::WoodedBadlands
        ) { HUSK } else { ZOMBIE },
        // Cold biomes → Stray
        7 => if matches!(biome,
            mc_core::biome::BiomeId::SnowyPlains | mc_core::biome::BiomeId::IceSpikes |
            mc_core::biome::BiomeId::SnowyTaiga | mc_core::biome::BiomeId::FrozenPeaks |
            mc_core::biome::BiomeId::SnowySlopes
        ) { STRAY } else { SKELETON },
        8 => DROWNED, 9 => WITCH,
        // Nether mobs — only in nether biomes
        10 => if biome.is_nether() { BLAZE } else { ZOMBIE },
        11 => if biome.is_nether() { GHAST } else { SKELETON },
        12 => if biome.is_nether() { PIGLIN } else { SPIDER },
        13 => if biome.is_nether() { HOGLIN } else { CREEPER },
        14 => if biome.is_nether() { MAGMA_CUBE } else { SLIME },
        15 => if biome.is_nether() { ZOMBIFIED_PIGLIN } else { ENDERMAN },
        16 => BREEZE, 17 => BOGGED,
        // Cave-specific: underground biomes → cave spider / silverfish
        18 => if matches!(biome,
            mc_core::biome::BiomeId::DripstoneCaves | mc_core::biome::BiomeId::LushCaves |
            mc_core::biome::BiomeId::DeepDark | mc_core::biome::BiomeId::SulfurCaves
        ) { CAVE_SPIDER } else { SILVERFISH },
        // DeepDark → Warden (rare spawn)
        19 => if biome == mc_core::biome::BiomeId::DeepDark { WARDEN } else { ZOMBIE },
        _ => ZOMBIE,
    }
}

fn make_tracked_mob(eid: i32, mob_type: i32, x: f64, y: f64, z: f64) -> mc_player::mob::TrackedMob {
    mc_player::mob::TrackedMob {
        entity_id: eid, uuid: uuid::Uuid::new_v4(), mob_type,
        position: mc_core::position::Position::new(x, y, z),
        health: mc_player::mob::mob_max_health(mob_type),
        max_health: mc_player::mob::mob_max_health(mob_type),
        age_ticks: 0, ai_timer: 0,
        ai_state: MobAiState::Idle, attack_cooldown: 0, last_sync_tick: 0,
        owner_uuid: None, is_tamed: false, is_sitting: false, tame_attempts: 0,
        is_baby: false, in_love_ticks: 0, breed_cooldown: 0, is_sheared: false,
        is_on_fire: false, is_in_water: false,
        path: Vec::new(), path_last_tick: 0,
        sulfur_cube_archetype: None, absorbed_block_id: None, is_small_cube: false,
        dirty_flags: mc_player::mob::TrackedMob::DIRTY_ALL, // C4: new entity needs full sync
    }
}

/// 被动生物生成 (每 200 tick)
pub fn tick_passive_spawning(
    pm: &SharedPlayerManager, mob_mgr: &Arc<mc_player::mob::MobManager>,
    cs: &ChunkStore, ws: &SharedWorldState,
    next_eid: &Arc<std::sync::atomic::AtomicI32>,
) -> u64 {
    run_stage("passive_spawn", || {
        let can_spawn = {
            let w = ws.read();
            w.game_rules.get("doMobSpawning").map(|v| v == "true").unwrap_or(false)
                && w.daytime < 13000
                && !matches!(w.difficulty, mc_core::world_state::Difficulty::Peaceful)
        };
        if !can_spawn { return; }
        let passive_count = mob_mgr.count() - mob_mgr.count_hostile();
        if passive_count >= 30 + pm.online_count() * 5 { return; }

        for player in pm.all_players() {
            if !fastrand::u32(..).is_multiple_of(5) { continue; }
            let angle = fastrand::f64() * std::f64::consts::TAU;
            let dist = 24.0 + fastrand::f64() * 24.0;
            let sx = player.position.x + angle.cos() * dist;
            let sz = player.position.z + angle.sin() * dist;
            let cp = mc_core::position::ChunkPos::new((sx as i32).div_euclid(16), (sz as i32).div_euclid(16));
            let spawn_y = if let Some(chunk) = cs.get(&cp) {
                let lx = (sx as i32).rem_euclid(16) as usize;
                let lz = (sz as i32).rem_euclid(16) as usize;
                let h = chunk.height_at(lx, lz);
                if let Some(Some(sec)) = chunk.sections.get(mc_world::chunk::section_index(h))
                    && sec.get_sky_light(lx, h.rem_euclid(16) as usize, lz) < 9 { continue; }
                if !chunk.is_spawn_surface(lx, h - 1, lz) { continue; }
                h as f64
            } else { 64.0 };
            use mc_core::constants::entity_type::*;
            // All valid passive/ambient mob types for surface spawning
            let surface_passive = [COW, PIG, CHICKEN, SHEEP, RABBIT, FOX, TURTLE, POLAR_BEAR, PANDA, ARMADILLO];
            let water_passive = [SQUID, DOLPHIN, COD, SALMON, PUFFERFISH, TROPICAL_FISH, GLOW_SQUID];
            let cave_passive = [BAT, SULFUR_CUBE]; // 26.2: Sulfur Caves adds SulfurCube
            let _nether_passive: [i32; 0] = []; // striders etc. — not yet implemented
            // Pick based on biome context and spawn position
            let biome = mc_world::generator::sample_biome(sx as i32, sz as i32, {
                let w = ws.read(); w.seed
            });
            let mob_type: i32 = if let Some(chunk) = cs.get(&cp) {
                let lx = (sx as i32).rem_euclid(16) as usize;
                let lz = (sz as i32).rem_euclid(16) as usize;
                let h = chunk.height_at(lx, lz);
                let is_water = chunk.get_block(lx, h, lz).id == 267; // water block at surface
                let is_cave = h < 50 && chunk.combined_light(lx, h, lz) < 5;
                if is_water {
                    water_passive[fastrand::usize(0..water_passive.len())]
                } else if is_cave {
                    // In SulfurCaves, bias toward SulfurCube over Bat (2:1 ratio via fastrand)
                    if biome == mc_core::biome::BiomeId::SulfurCaves && !fastrand::u32(..).is_multiple_of(3) {
                        SULFUR_CUBE
                    } else {
                        cave_passive[fastrand::usize(0..cave_passive.len())]
                    }
                } else if biome == mc_core::biome::BiomeId::MushroomFields {
                    MOOSHROOM // exclusive to mushroom fields
                } else {
                    surface_passive[fastrand::usize(0..surface_passive.len())]
                }
            } else {
                surface_passive[fastrand::usize(0..surface_passive.len())]
            };
            let eid = next_eid.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let tracked = make_tracked_mob(eid, mob_type, sx, spawn_y, sz);
            mob_mgr.register(tracked);
            pm.broadcast_mob_spawn(eid, uuid::Uuid::new_v4(), mob_type, sx, spawn_y, sz);
        }
    })
}

// ═══════════════════════════════════════════════════════════════
// BossBar auto-sync
// ═══════════════════════════════════════════════════════════════

/// Turtle egg ticking: every 400 ticks, advance crack stage (0→1→2→3=hatch)
#[allow(dead_code)]
pub fn tick_turtle_eggs(cs: &ChunkStore, mob_mgr: &Arc<mc_player::mob::MobManager>, next_eid: &Arc<std::sync::atomic::AtomicI32>) -> u64 {
    run_stage("turtle_egg", || {
        for entry in cs.all_chunks() {
            let (cpos, _) = entry;
            if !fastrand::u32(..).is_multiple_of(10) { continue; }
            if let Some(mut ch) = cs.get_mut(&cpos) {
                for x in 0..16usize { for z in 0..16usize {
                    for y in 50..70 { // eggs on beach sand
                        let b = ch.get_block(x, y, z);
                        if b.id >= 420 && b.id <= 423 && fastrand::bool() { // turtle_egg crack stages
                            let next_stage = b.id + 1;
                            if next_stage > 423 {
                                // Hatch: spawn baby turtle
                                ch.set_block(x, y, z, mc_core::block::BlockState::AIR);
                                let eid = next_eid.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let tracked = make_tracked_mob(eid, 19, (cpos.x * 16 + x as i32) as f64 + 0.5, y as f64 + 1.0, (cpos.z * 16 + z as i32) as f64 + 0.5);
                                mob_mgr.register(tracked);
                            } else {
                                ch.set_block(x, y, z, mc_core::block::BlockState::new(next_stage));
                            }
                            break; // one egg per tick per chunk
                        }
                    }
                }}
            }
        }
    })
}

/// Patrol spawning: every ~10-20 minutes, spawn illager patrol near a village
pub fn tick_patrol_spawning(
    pm: &SharedPlayerManager, mob_mgr: &Arc<mc_player::mob::MobManager>,
    next_eid: &Arc<std::sync::atomic::AtomicI32>,
) -> u64 {
    run_stage("patrol", || {
        use mc_core::constants::entity_type::*;
        if pm.online_count() == 0 { return; }
        let target_player = pm.all_players().first().cloned();
        if let Some(player) = target_player {
            let angle = fastrand::f64() * std::f64::consts::TAU;
            let dist = 24.0 + fastrand::f64() * 32.0;
            let sx = player.position.x + angle.cos() * dist;
            let sz = player.position.z + angle.sin() * dist;
            let sy = player.position.y;
            // Spawn patrol: 1 captain (pillager with banner) + 3-5 pillagers
            let patrol_size = 3 + fastrand::u32(0..3) as usize;
            for i in 0..=patrol_size {
                let eid = next_eid.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let ox = sx + (fastrand::f64() - 0.5) * 6.0;
                let oz = sz + (fastrand::f64() - 0.5) * 6.0;
                let mob_type = if i == 0 { RAVAGER } else { PIGLIN }; // captain + pillagers
                let tracked = make_tracked_mob(eid, mob_type, ox, sy, oz);
                mob_mgr.register(tracked);
                pm.broadcast_mob_spawn(eid, uuid::Uuid::new_v4(), mob_type, ox, sy, oz);
            }
        }
    })
}

/// BossBar 自动同步 — 凋灵 (25) 和末影龙 (53) 每 20 tick 更新血条
pub fn tick_bossbar_sync(pm: &SharedPlayerManager, mob_mgr: &Arc<mc_player::mob::MobManager>) -> u64 {
    run_stage("bossbar", || {
        for eid in mob_mgr.all_entity_ids() {
            if let Some(mob) = mob_mgr.get(eid) {
                match mob.mob_type {
                    25 => { // Wither
                        let health_pct = (mob.health / mob.max_health).clamp(0.0, 1.0);
                        pm.broadcast_global(
                            mc_player::player::PlayerStateEventKind::BossBarUpdate(
                                "minecraft:wither".into(), 0,
                                "Wither".into(), health_pct,
                                0, 0, 0x01,
                            )
                        );
                    }
                    53 => { // EnderDragon
                        let health_pct = (mob.health / mob.max_health).clamp(0.0, 1.0);
                        pm.broadcast_global(
                            mc_player::player::PlayerStateEventKind::BossBarUpdate(
                                "minecraft:dragon".into(), 0,
                                "Ender Dragon".into(), health_pct,
                                1, 0, 0x01,
                            )
                        );
                    }
                    _ => {}
                }
            }
        }
    })
}

// ═══════════════════════════════════════════════════════════════
// Portal detection
// ═══════════════════════════════════════════════════════════════

/// 下界传送门检测 — 站在传送门方块上时触发维度切换 (每 20 tick)
pub fn tick_portal_detection(
    pm: &SharedPlayerManager,
    cs: &ChunkStore,
    advancement_tracker: &SharedAdvancementTracker,
    advancement_registry: &Arc<mc_player::advancement::AdvancementRegistry>,
) -> u64 {
    run_stage("portal", || {
        let portal_events: Vec<(uuid::Uuid, String, f64, f64, f64)> = {
            pm.all_players().iter().filter_map(|player| {
                let px = player.position.x as i32;
                let py = player.position.y as i32;
                let pz = player.position.z as i32;
                let cp = mc_core::position::ChunkPos::new(px >> 4, pz >> 4);
                if let Some(chunk) = cs.get(&cp) {
                    let at_feet = chunk.get_block((px & 0xF) as usize, py, (pz & 0xF) as usize);
                    let below = chunk.get_block((px & 0xF) as usize, py - 1, (pz & 0xF) as usize);
                    if at_feet.id == 90 || below.id == 90 {
                        let current_dim = &player.dimension;
                        if current_dim == "minecraft:the_nether" {
                            let tx = player.position.x * 8.0;
                            let tz = player.position.z * 8.0;
                            let ty = 128.0_f64.max(player.position.y);
                            Some((player.uuid, "minecraft:overworld".to_string(), tx, ty, tz))
                        } else {
                            let tx = player.position.x / 8.0;
                            let tz = player.position.z / 8.0;
                            let ty = 64.0_f64.min(player.position.y);
                            Some((player.uuid, "minecraft:the_nether".to_string(), tx, ty, tz))
                        }
                    } else { None }
                } else { None }
            }).collect()
        };
        for (uuid, dim, tx, ty, tz) in &portal_events {
            let _ = pm.set_dimension(uuid, dim);
            let _ = pm.broadcast_player_respawn(uuid, dim, *tx, *ty, *tz);
            let dim_key = if dim == "minecraft:the_nether" { "the_nether" }
                else if dim == "minecraft:the_end" { "the_end" }
                else { "overworld" };
            let _ = advancement_tracker.write().check_criterion(
                uuid, &mc_player::advancement::Criterion::LocationChanged { dimension: dim_key.to_string() },
                advancement_registry);
            tracing::info!("Portal: {} → {} at ({:.0}, {:.0}, {:.0})", uuid, dim, tx, ty, tz);
        }
    })
}

// ═══════════════════════════════════════════════════════════════
// Copper oxidation
// ═══════════════════════════════════════════════════════════════

/// 铜灯自然氧化 — 每 72000 tick (≈1 小时) 扫描并推进氧化阶段
pub fn tick_copper_oxidation(
    cs: &ChunkStore,
    dirty_chunks: &SharedDirtyChunks,
) -> u64 {
    run_stage("copper", || {
        // All oxidizable copper pairs: (from, to)
        let copper_pairs: &[(u32, u32)] = &[
            // Copper blocks
            (322, 323), (323, 324), (324, 325), // copper→exposed→weathered→oxidized
            (326, 327), (327, 328), (328, 329), // cut_copper variants
            // Copper doors
            (388, 389), (389, 390), (390, 391),
            // Copper trapdoors
            (392, 393), (393, 394), (394, 395),
            // Copper grates
            (396, 397), (397, 398), (398, 399),
            // Copper bulbs
            (400, 401), (401, 402), (402, 403),
        ];
        for &(from_id, to_id) in copper_pairs {
                for entry in cs.all_chunks() {
                    let (cpos, _) = entry;
                    if let Some(mut ch) = cs.get_mut(&cpos) {
                        let mut changed = false;
                        for x in 0..16usize { for z in 0..16usize {
                            for y in -64..320 {
                                let b = ch.get_block(x, y, z);
                                if b.id == from_id {
                                    ch.set_block(x, y, z, mc_core::block::BlockState::new(to_id));
                                    changed = true;
                                }
                            }
                        }}
                        if changed {
                            dirty_chunks.write().insert(cpos);
                        }
                    }
                }
        }
    })
}

// ═══════════════════════════════════════════════════════════════
// XP orb absorption
// ═══════════════════════════════════════════════════════════════

/// 经验球吸收 — 检测玩家附近 XP orb 并吸收 (每 tick)
pub fn tick_xp_absorption(
    pm: &SharedPlayerManager,
    dropped: &SharedDroppedItems,
) -> u64 {
    run_stage("xp", || {
        let orbs_to_check: Vec<(i32, f64, f64, f64)> = {
            let dropped_lock = dropped.read();
            dropped_lock.iter()
                .filter(|(_, v)| v.0 == 0)
                .map(|(k, v)| (*k, v.1, v.2, v.3))
                .collect()
        };
        let mut absorbed_eids = Vec::new();
        for (eid, x, y, z) in &orbs_to_check {
            for p in pm.all_players() {
                let dx = p.position.x - *x;
                let dy = p.position.y - *y;
                let dz = p.position.z - *z;
                let dist = (dx*dx + dy*dy + dz*dz).sqrt();
                if dist < 1.5 {
                    let xp = 3 + fastrand::i32(0..5);
                    let mut mending_used = false;
                    if let Some(held) = pm.get_held_item(&p.uuid) {
                        let has_mending = mc_player::enchant::has_enchant(&held.nbt, "mending");
                        if has_mending
                            && let Some(ref dur) = held.durability
                                && *dur > 0 {
                                    let _ = pm.repair_held_item(&p.uuid, xp as u16 * 2);
                                    mending_used = true;
                                }
                    }
                    if !mending_used {
                        let _ = pm.add_xp(&p.uuid, xp);
                    }
                    absorbed_eids.push(*eid);
                    break;
                }
            }
        }
        if !absorbed_eids.is_empty() {
            let mut dropped_lock = dropped.write();
            for eid in &absorbed_eids {
                dropped_lock.remove(eid);
            }
        }
    })
}

// ═══════════════════════════════════════════════════════════════
// Mob pathfinding
// ═══════════════════════════════════════════════════════════════

/// 生物寻路更新 — 为追逐中的敌对生物计算 A* 路径 (每 40 tick)
pub fn tick_mob_pathfinding(
    pm: &SharedPlayerManager,
    mob_mgr: &Arc<mc_player::mob::MobManager>,
    cs: &ChunkStore,
    tick_count: u64,
) -> u64 {
    run_stage("pathfind", || {
        let chasing: Vec<(i32, f64, f64, f64, f64, f64, f64)> = mob_mgr.all_entity_ids()
            .iter()
            .filter_map(|eid| {
                let mob = mob_mgr.get(*eid)?;
                if let MobAiState::Chasing { target_uuid } = &mob.ai_state {
                    let target = pm.get(target_uuid)?;
                    Some((*eid, mob.position.x, mob.position.y, mob.position.z,
                          target.position.x, target.position.y, target.position.z))
                } else { None }
            })
            .collect();
        for (eid, mx, my, mz, tx, ty, tz) in &chasing {
            let path = mc_player::pathfind::find_path(*mx, *my, *mz, *tx, *ty, *tz, cs);
            mob_mgr.set_path(*eid, path, tick_count);
        }
    })
}

// ═══ 26.2: Wandering Trader spawning (every 24000 ticks = 1 MC day) ═══

/// Attempt to spawn a Wandering Trader near a random player.
/// Returns list of (entity_id, mob_type, x, y, z) for spawned entities.
pub fn tick_wandering_trader(
    tick_count: u64, pm: &SharedPlayerManager,
    mob_mgr: &Arc<mc_player::mob::MobManager>,
    next_eid: &Arc<std::sync::atomic::AtomicI32>,
    cs: &ChunkStore,
) -> Vec<(i32, i32, f64, f64, f64)> {
    // First spawn at 24000, then every 48000 ticks (2 MC days)
    if tick_count < 24000 || (tick_count - 24000) % 48000 != 0 {
        return Vec::new();
    }
    // Check current trader count (max 1 at a time)
    let trader_count = mob_mgr.all_mobs().iter()
        .filter(|m| m.mob_type == mc_core::constants::entity_type::WANDERING_TRADER)
        .count();
    if trader_count >= 1 { return Vec::new(); }

    let players = pm.all_players();
    if players.is_empty() { return Vec::new(); }
    let player = &players[fastrand::usize(0..players.len())];
    let angle = fastrand::f64() * std::f64::consts::TAU;
    let dist = 16.0 + fastrand::f64() * 24.0;
    let sx = player.position.x + angle.cos() * dist;
    let sz = player.position.z + angle.sin() * dist;
    let sy = {
        let cp = mc_core::position::ChunkPos::new((sx as i32) >> 4, (sz as i32) >> 4);
        cs.get(&cp).map(|ch| {
            let lx = (sx as i32 & 0xF) as usize; let lz = (sz as i32 & 0xF) as usize;
            (0..=255).rev().find(|&y| ch.get_block(lx, y, lz).id != 0).unwrap_or(64) as f64 + 1.0
        }).unwrap_or(64.0)
    };

    let mut spawned = Vec::new();
    // Spawn Wandering Trader
    let trader_eid = next_eid.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let trader = make_tracked_mob(trader_eid, mc_core::constants::entity_type::WANDERING_TRADER, sx, sy, sz);
    mob_mgr.register(trader);
    spawned.push((trader_eid, mc_core::constants::entity_type::WANDERING_TRADER, sx, sy, sz));

    // Spawn 1-2 Trader Llamas
    let llama_count = 1 + (fastrand::u32(..) % 2) as usize;
    for _ in 0..llama_count {
        let llama_eid = next_eid.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let lx = sx + (fastrand::f64() - 0.5) * 3.0;
        let lz = sz + (fastrand::f64() - 0.5) * 3.0;
        let llama = make_tracked_mob(llama_eid, mc_core::constants::entity_type::TRADER_LLAMA, lx, sy, lz);
        mob_mgr.register(llama);
        spawned.push((llama_eid, mc_core::constants::entity_type::TRADER_LLAMA, lx, sy, lz));
    }
    spawned
}

/// Wandering Trader trade offers (6 random trades from pool)
#[allow(dead_code)]
pub fn wandering_trader_trades() -> Vec<(u32, u8, u32, u8, i32, i32)> {
    // (input_item, input_count, output_item, output_count, max_uses, xp)
    let pool: &[(u32, u8, u32, u8, i32, i32)] = &[
        (134, 1, 78, 3, 12, 1),    // emerald → ice
        (134, 2, 79, 1, 12, 1),    // emerald → packed_ice
        (134, 1, 37, 8, 16, 1),    // emerald → fern
        (134, 1, 47, 1, 12, 1),    // emerald → blue_ice
        (134, 3, 834, 1, 8, 1),    // emerald → nautilus_shell
        (134, 1, 177, 3, 12, 1),   // emerald → podzol
        (134, 1, 174, 3, 16, 1),   // emerald → packed_mud
        (134, 1, 65, 1, 12, 1),    // emerald → cactus
        (134, 1, 139, 1, 8, 1),    // emerald → sand
        (134, 1, 966, 2, 5, 1),    // emerald → lead
        (134, 1, 1048, 1, 8, 1),   // emerald → glow_lichen
        (134, 1, 897, 1, 8, 1),    // emerald → slimeball
        (134, 5, 828, 1, 8, 1),    // emerald → heart_of_the_sea
        (134, 1, 168, 2, 16, 1),   // emerald → moss_block
        (134, 1, 1055, 1, 12, 1),  // emerald → frosted_ice
    ];
    let mut rng = fastrand::Rng::new();
    let mut selected: Vec<(u32, u8, u32, u8, i32, i32)> = Vec::with_capacity(6);
    let mut indices: Vec<usize> = (0..pool.len()).collect();
    // Pick up to 6 random unique trades
    for _ in 0..6.min(pool.len()) {
        let idx = rng.usize(0..indices.len());
        selected.push(pool[indices[idx]]);
        indices.remove(idx);
    }
    selected
}
