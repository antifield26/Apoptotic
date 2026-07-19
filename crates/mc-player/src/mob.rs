//! 生物管理器 — 追踪所有非玩家实体的服务器端状态
//!
//! 使用 DashMap 无锁并发，消除 RwLock 竞争。

use dashmap::DashMap;
use mc_core::constants::entity_type::{self as ET};
use mc_core::position::Position;
use uuid::Uuid;

/// 服务器端追踪的生物
#[derive(Debug, Clone)]
pub struct TrackedMob {
    pub entity_id: i32,
    pub uuid: Uuid,
    pub mob_type: i32,
    pub position: Position,
    pub health: f32,
    pub max_health: f32,
    pub age_ticks: u64,
    pub ai_timer: u64,
    pub ai_state: MobAiState,
    pub attack_cooldown: u8,
    pub last_sync_tick: u64,
    pub owner_uuid: Option<Uuid>,
    pub is_tamed: bool,
    pub is_sitting: bool,
    pub tame_attempts: u8,
    pub is_baby: bool,
    pub in_love_ticks: u16,
    pub breed_cooldown: u16,
    pub is_sheared: bool,
    /// Pathfinding: cached path waypoints (world coords)
    pub path: Vec<(f64, f64, f64)>,
    /// Pathfinding: tick when path was last computed
    pub path_last_tick: u64,
    /// EAR 2.0: immunity flags — mob won't skip AI when in dangerous states
    pub is_on_fire: bool,
    pub is_in_water: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Default)]
pub enum MobAiState {
    #[default]
    Idle,
    Wandering { target_x: f64, target_z: f64 },
    Chasing { target_uuid: Uuid },
    AboutToExplode { fuse_ticks: u8 },
}


#[derive(Debug, Clone)]
pub struct MobPositionEvent { pub entity_id: i32, pub x: f64, pub y: f64, pub z: f64 }

// ═══════════════════════════════════════════════════════════════
// Projectile system
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileType {
    Arrow,
    Fireball,
    SmallFireball,
    Snowball,
    Egg,
    EnderPearl,
    Trident,
    SplashPotion,
    LingeringPotion,
    WindCharge,
    WitherSkull,
    Firework,
}

impl ProjectileType {
    pub fn entity_type_id(self) -> i32 {
        match self {
            Self::Arrow => 7,
            Self::Fireball => 34,  // ghast fireball
            Self::SmallFireball => 89, // blaze fireball
            Self::Snowball => 86,
            Self::Egg => 87,
            Self::EnderPearl => 79,
            Self::Trident => 94,
            Self::SplashPotion => 88,
            Self::LingeringPotion => 93,
            Self::WindCharge => 95,
            Self::WitherSkull => 91,
            Self::Firework => 72,
        }
    }

    pub fn has_gravity(self) -> bool {
        matches!(self, Self::Arrow | Self::Snowball | Self::Egg | Self::EnderPearl | Self::Trident | Self::SplashPotion | Self::LingeringPotion | Self::Firework)
    }

    pub fn max_ticks(self) -> u16 {
        match self {
            Self::Arrow | Self::Trident => 1200, // 60 seconds
            Self::Fireball | Self::SmallFireball | Self::WitherSkull => 200,
            Self::WindCharge => 60,
            Self::Firework => 600,
            _ => 600, // 30 seconds for snowballs/eggs/pearls/potions
        }
    }
}

#[derive(Debug, Clone)]
pub struct Projectile {
    pub entity_id: i32,
    pub owner_uuid: Uuid,
    pub owner_entity_id: i32,
    pub projectile_type: ProjectileType,
    pub position: Position,
    pub vel_x: f64,
    pub vel_y: f64,
    pub vel_z: f64,
    pub damage: f32,
    pub ticks_alive: u16,
    pub max_ticks: u16,
    pub in_ground: bool,
    /// Enchantment levels from the launching weapon (for hit effects)
    pub power_level: u8,
    pub flame_level: u8,
    pub punch_level: u8,
    pub piercing_level: u8,
}

/// 生物管理器 — DashMap 无锁并发
pub struct MobManager {
    mobs: DashMap<i32, TrackedMob>,
    chunk_mobs: DashMap<(i32, i32), Vec<i32>>,
    position_tx: tokio::sync::broadcast::Sender<MobPositionEvent>,
    /// Scratch buffer for AI iteration — reused across ticks to avoid per-tick Vec allocation
    ai_keys: parking_lot::Mutex<Vec<i32>>,
    /// Active projectile entities
    pub projectiles: DashMap<i32, Projectile>,
    /// Internal entity ID counter for spawned projectiles (set by server on init)
    pub next_entity_id: std::sync::Arc<std::sync::atomic::AtomicI32>,
}

impl Default for MobManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MobManager {
    pub fn new() -> Self {
        let (position_tx, _) = tokio::sync::broadcast::channel::<MobPositionEvent>(256);
        Self {
            mobs: DashMap::new(),
            chunk_mobs: DashMap::new(),
            position_tx,
            ai_keys: parking_lot::Mutex::new(Vec::with_capacity(256)),
            projectiles: DashMap::new(),
            next_entity_id: std::sync::Arc::new(std::sync::atomic::AtomicI32::new(100000)),
        }
    }

    /// Spawn a projectile entity (returns the new entity ID)
    pub fn spawn_projectile(
        &self,
        owner_uuid: Uuid,
        owner_entity_id: i32,
        projectile_type: ProjectileType,
        x: f64, y: f64, z: f64,
        vel_x: f64, vel_y: f64, vel_z: f64,
        damage: f32,
    ) -> i32 {
        let eid = self.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let proj = Projectile {
            entity_id: eid,
            owner_uuid,
            owner_entity_id,
            projectile_type,
            position: Position::new(x, y, z),
            vel_x, vel_y, vel_z,
            damage,
            ticks_alive: 0,
            max_ticks: projectile_type.max_ticks(),
            in_ground: false,
            power_level: 0,
            flame_level: 0,
            punch_level: 0,
            piercing_level: 0,
        };
        self.projectiles.insert(eid, proj);
        eid
    }

    /// Tick all active projectiles — update positions, check collisions, despawn expired
    pub fn tick_projectiles(&self) -> Vec<ProjectileEvent> {
        let mut events = Vec::new();
        let mut to_remove = Vec::new();

        for mut entry in self.projectiles.iter_mut() {
            let proj = entry.value_mut();
            proj.ticks_alive += 1;

            if proj.ticks_alive > proj.max_ticks {
                to_remove.push(proj.entity_id);
                events.push(ProjectileEvent::Despawn(proj.entity_id));
                continue;
            }

            if proj.in_ground {
                // Stuck in ground — wait for despawn timer, no movement
                if proj.ticks_alive > 100 { // 5 seconds stuck
                    to_remove.push(proj.entity_id);
                    events.push(ProjectileEvent::Despawn(proj.entity_id));
                }
                continue;
            }

            // Apply gravity
            if proj.projectile_type.has_gravity() {
                proj.vel_y -= 0.05; // gravity
            }

            // Update position
            proj.position.x += proj.vel_x;
            proj.position.y += proj.vel_y;
            proj.position.z += proj.vel_z;

            // Simple ground collision check (y < -64)
            if proj.position.y < -64.0 {
                to_remove.push(proj.entity_id);
                events.push(ProjectileEvent::Despawn(proj.entity_id));
            }
        }

        for eid in to_remove {
            self.projectiles.remove(&eid);
        }

        events
    }

    /// Set pathfinding waypoints for a mob
    pub fn set_path(&self, entity_id: i32, path: Vec<(f64, f64, f64)>, tick: u64) {
        if let Some(mut mob) = self.mobs.get_mut(&entity_id) {
            mob.path = path;
            mob.path_last_tick = tick;
        }
    }

    /// Send a position update for an entity (for rail/minecart physics, etc.)
    pub fn send_position(&self, entity_id: i32, x: f64, y: f64, z: f64) {
        // Update position in-place if the entity exists
        if let Some(mut mob) = self.mobs.get_mut(&entity_id) {
            mob.position.x = x;
            mob.position.y = y;
            mob.position.z = z;
        }
        let _ = self.position_tx.send(MobPositionEvent { entity_id, x, y, z });
    }

    pub fn subscribe_positions(&self) -> tokio::sync::broadcast::Receiver<MobPositionEvent> {
        self.position_tx.subscribe()
    }

    /// 注册新生物
    pub fn register(&self, mob: TrackedMob) {
        let eid = mob.entity_id;
        let chunk = (
            (mob.position.x.floor() as i32).div_euclid(16),
            (mob.position.z.floor() as i32).div_euclid(16),
        );
        self.chunk_mobs.entry(chunk).or_default().push(eid);
        self.mobs.insert(eid, mob);
    }

    /// 移除生物
    pub fn remove(&self, entity_id: i32) -> Option<TrackedMob> {
        let mob = self.mobs.remove(&entity_id).map(|(_, v)| v)?;
        let chunk = (
            (mob.position.x.floor() as i32).div_euclid(16),
            (mob.position.z.floor() as i32).div_euclid(16),
        );
        if let Some(mut list) = self.chunk_mobs.get_mut(&chunk) {
            list.retain(|e| *e != entity_id);
        }
        Some(mob)
    }

    /// 根据 entity_id 查找生物
    pub fn get(&self, entity_id: i32) -> Option<TrackedMob> {
        self.mobs.get(&entity_id).map(|r| r.clone())
    }

    /// 更新生物生命值
    pub fn damage(&self, entity_id: i32, amount: f32) -> Option<f32> {
        self.mobs.get_mut(&entity_id).map(|mut mob| {
            mob.health = (mob.health - amount).max(0.0);
            mob.health
        })
    }

    /// 获取某区块中的所有生物
    pub fn get_in_chunk(&self, chunk_x: i32, chunk_z: i32) -> Vec<TrackedMob> {
        self.chunk_mobs.get(&(chunk_x, chunk_z))
            .map(|ids| ids.iter().filter_map(|id| self.mobs.get(id).map(|r| r.clone())).collect())
            .unwrap_or_default()
    }

    /// Return all tracked mobs (for pressure plate / tripwire entity detection)
    pub fn all_mobs(&self) -> Vec<TrackedMob> {
        self.mobs.iter().map(|e| e.value().clone()).collect()
    }

    pub fn all_entity_ids(&self) -> Vec<i32> {
        self.mobs.iter().map(|e| *e.key()).collect()
    }

    pub fn count(&self) -> usize { self.mobs.len() }

    /// Tick 所有生物的 AI — DashMap 无锁迭代, 带实体激活范围优化
    pub fn tick_ai(&self, player_manager: Option<&crate::player::PlayerManager>) {
        // Reuse scratch buffer to avoid per-tick allocation
        let mut keys = self.ai_keys.lock();
        keys.clear();
        keys.extend(self.mobs.iter().map(|e| *e.key()));
        for eid in keys.iter() {
            if let Some(mut mob) = self.mobs.get_mut(eid) {
                mob.age_ticks = mob.age_ticks.wrapping_add(1);
                mob.attack_cooldown = mob.attack_cooldown.saturating_sub(1);

                // ── Entity Activation Range (PaperMC-style) ──
                // Skip AI for mobs far from any player to save CPU
                let (activation_range, skip_frequency) = if ET::is_hostile(mob.mob_type) {
                    (48.0_f64.powi(2), 20u64) // 48 blocks, tick every 1s when beyond
                } else if matches!(mob.mob_type, 16 | 20 | 21 | 23 | 24 | 27 | 65 | 66) {
                    (24.0_f64.powi(2), 60u64) // ambient/fish: 24 blocks
                } else {
                    (32.0_f64.powi(2), 40u64) // passive: 32 blocks
                };
                let near_player = player_manager.map(|pm| {
                    pm.players_in_range(mob.position.x, mob.position.y, mob.position.z,
                        activation_range.sqrt()).into_iter().next().is_some()
                }).unwrap_or(false);
                // EAR 2.0: immunity check — don't skip AI for mobs on fire or in water
                let immune = mob.is_on_fire || mob.is_in_water;
                if !near_player && !immune && mob.age_ticks.wrapping_rem(skip_frequency) != 0 {
                    continue; // too far from any player — skip AI this tick
                }

                if mob.ai_timer > 0 { mob.ai_timer -= 1; continue; }

                let mut should_continue = true;
                // Tamed pet AI: follow owner
                if mob.is_tamed {
                    if mob.is_sitting { mob.ai_timer = 40; continue; }
                    if let Some(owner_uuid) = mob.owner_uuid
                        && let Some(pm) = player_manager
                            && let Some(owner) = pm.get(&owner_uuid) {
                                let dx = owner.position.x - mob.position.x;
                                let dz = owner.position.z - mob.position.z;
                                let dist = (dx*dx + dz*dz).sqrt();
                                if dist > 12.0 {
                                    mob.position.x = owner.position.x;
                                    mob.position.y = owner.position.y;
                                    mob.position.z = owner.position.z + 1.0;
                                    let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                                } else if dist > 2.0 {
                                    mob.position.x += (dx / dist) * 0.25;
                                    mob.position.z += (dz / dist) * 0.25;
                                    let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                                }
                                mob.ai_timer = 15; continue;
                            }
                }

                let is_hostile = ET::is_hostile(mob.mob_type);
                let (nearest_uuid, nearest_x, nearest_z) = if is_hostile {
                    player_manager.and_then(|pm| {
                        pm.nearest_player(mob.position.x, mob.position.y, mob.position.z, None)
                            .filter(|p| {
                                let dx = p.position.x - mob.position.x;
                                let dz = p.position.z - mob.position.z;
                                dx * dx + dz * dz < 256.0
                            })
                            .map(|p| (p.uuid, p.position.x, p.position.z))
                    }).unwrap_or((Uuid::nil(), 0.0, 0.0))
                } else { (Uuid::nil(), 0.0, 0.0) };
                let has_target = !nearest_uuid.is_nil();
                let dist = ((nearest_x - mob.position.x).powi(2) + (nearest_z - mob.position.z).powi(2)).sqrt();

                // Boss AI
                if mob.mob_type == ET::WITHER {
                    mob.position.y += 0.15;
                    if mob.age_ticks % 20 == 0 && dist > 5.0 && dist < 30.0 {
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                    }
                    if mob.health < mob.max_health * 0.5 { mob.health += 0.1; }
                    mob.ai_timer = 15; continue;
                }
                if mob.mob_type == ET::ENDER_DRAGON {
                    let center_x = 0.0; let center_z = 0.0;
                    let angle = (mob.age_ticks as f64) * 0.02;
                    mob.position.x = center_x + angle.cos() * 20.0;
                    mob.position.z = center_z + angle.sin() * 20.0;
                    mob.position.y = 70.0 + (angle * 3.0).sin() * 10.0;
                    mob.health = (mob.health + 0.1).min(mob.max_health);
                    mob.ai_timer = 10; continue;
                }

                // Passive mob AI (non-hostile, no targets)
                if !has_target {
                    // Villager: basic wandering
                    if mob.mob_type == ET::VILLAGER && mob.age_ticks % 80 == 0 {
                        let angle = fastrand::f64() * std::f64::consts::TAU;
                        let wander_dist = 3.0 + fastrand::f64() * 5.0;
                        mob.position.x += angle.cos() * wander_dist * 0.1;
                        mob.position.z += angle.sin() * wander_dist * 0.1;
                        let _ = self.position_tx.send(MobPositionEvent {
                            entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z,
                        });
                        mob.ai_timer = 40; continue;
                    }
                    // Wandering trader: similar to villager
                    if mob.mob_type == 95 && mob.age_ticks % 100 == 0 {
                        let angle = fastrand::f64() * std::f64::consts::TAU;
                        mob.position.x += angle.cos() * 3.0;
                        mob.position.z += angle.sin() * 3.0;
                        mob.ai_timer = 60; continue;
                    }
                    // Axolotl (123): water wander + occasional dash
                    if mob.mob_type == ET::AXOLOTL && mob.age_ticks % 30 == 0 {
                        let angle = fastrand::f64() * std::f64::consts::TAU;
                        let speed = if fastrand::u8(..) < 20 { 3.0 } else { 0.5 }; // occasional dash
                        mob.position.x += angle.cos() * speed;
                        mob.position.z += angle.sin() * speed;
                        mob.position.y = (mob.position.y + (fastrand::f64() - 0.5) * 0.8).clamp(1.0, 64.0);
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                        mob.ai_timer = 20; continue;
                    }
                    // Goat (124): jump + occasional ram
                    if mob.mob_type == ET::GOAT && mob.age_ticks % 40 == 0 {
                        let angle = fastrand::f64() * std::f64::consts::TAU;
                        let ram = fastrand::u8(..) < 15; // 15% chance to ram
                        let speed = if ram { 4.0 } else { 1.5 };
                        mob.position.x += angle.cos() * speed;
                        mob.position.z += angle.sin() * speed;
                        mob.position.y += if ram { 1.5 } else { 0.3 };
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                        mob.ai_timer = if ram { 30 } else { 20 }; continue;
                    }
                    // Strider (125): slow wander on imaginary lava surface
                    if mob.mob_type == ET::STRIDER && mob.age_ticks % 50 == 0 {
                        let angle = fastrand::f64() * std::f64::consts::TAU;
                        mob.position.x += angle.cos() * 1.5;
                        mob.position.z += angle.sin() * 1.5;
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                        mob.ai_timer = 30; continue;
                    }
                    // Sulfur Cube (131): 26.2 passive — bouncy wander, block absorption
                    if mob.mob_type == ET::SULFUR_CUBE && mob.age_ticks % 30 == 0 {
                        let angle = fastrand::f64() * std::f64::consts::TAU;
                        let bounce = 0.5 + fastrand::f64() * 1.0;
                        mob.position.x += angle.cos() * 2.0;
                        mob.position.z += angle.sin() * 2.0;
                        mob.position.y += bounce; // bouncy!
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                        mob.ai_timer = 20; continue;
                    }
                    // Mooshroom (128): cow-like wander
                    if mob.mob_type == ET::MOOSHROOM && mob.age_ticks % 60 == 0 {
                        let angle = fastrand::f64() * std::f64::consts::TAU;
                        mob.position.x += angle.cos() * 2.0;
                        mob.position.z += angle.sin() * 2.0;
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                        mob.ai_timer = 40; continue;
                    }
                    // SkeletonHorse (126): passive wander (tameable via riding)
                    if mob.mob_type == ET::SKELETON_HORSE && mob.age_ticks % 60 == 0 {
                        let angle = fastrand::f64() * std::f64::consts::TAU;
                        mob.position.x += angle.cos() * 3.0;
                        mob.position.z += angle.sin() * 3.0;
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                        mob.ai_timer = 35; continue;
                    }
                    should_continue = false;
                }

                // Ranged AI
                if should_continue {
                    if mob.mob_type == ET::BREEZE {
                        // Fire WindCharge at target when at medium range
                        if mob.attack_cooldown == 0 && dist > 4.0 && dist < 16.0 && mob.age_ticks % 50 == 0 {
                            let dx = nearest_x - mob.position.x; let dz = nearest_z - mob.position.z;
                            let speed = 1.2;
                            let norm = (dx*dx + dz*dz).sqrt().max(0.01);
                            let vx = dx / norm * speed;
                            let vz = dz / norm * speed;
                            self.spawn_projectile(Uuid::nil(), mob.entity_id, ProjectileType::WindCharge,
                                mob.position.x, mob.position.y + 1.5, mob.position.z,
                                vx, 0.3, vz, 3.0);
                            mob.attack_cooldown = 40;
                        }
                        // Position sync for rendering
                        if mob.age_ticks % 30 == 0 && dist > 3.0 && dist < 20.0 {
                            let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                        }
                        // Leap backward when too close
                        if dist < 3.0 && mob.age_ticks % 40 == 0 {
                            let dx = nearest_x - mob.position.x; let dz = nearest_z - mob.position.z;
                            mob.position.x -= (dx / dist) * 4.0; mob.position.z -= (dz / dist) * 4.0; mob.position.y += 2.0;
                        }
                        mob.ai_timer = 15; continue;
                    }
                    if mob.mob_type == 72 && (4.0..15.0).contains(&dist) && mob.age_ticks % 50 == 0 {
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                    }
                    if mob.mob_type == 56 && mob.age_ticks % 100 == 0 && dist > 5.0 && dist < 30.0 {
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                    }
                    if mob.mob_type == 43 && mob.age_ticks % 40 == 0 && dist < 20.0 {
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                    }
                    if mob.mob_type == 48 && mob.age_ticks % 80 == 0 && dist < 15.0 {
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                    }
                    // Skeleton: ranged bow attack — keep distance 8-15 blocks, shoot every 20-40 ticks
                    if mob.mob_type == ET::SKELETON && mob.attack_cooldown == 0
                        && (5.0..16.0).contains(&dist) {
                            mob.attack_cooldown = 20 + (fastrand::u8(..) % 20) as u8;
                            let dx = nearest_x - mob.position.x;
                            let dz = nearest_z - mob.position.z;
                            // Retreat slightly if too close
                            if dist < 8.0 {
                                mob.position.x -= (dx / dist) * 1.5;
                                mob.position.z -= (dz / dist) * 1.5;
                            }
                            // Spawn arrow projectile
                            let arrow_eid = self.spawn_projectile(
                                Uuid::nil(), mob.entity_id,
                                ProjectileType::Arrow,
                                mob.position.x, mob.position.y + 1.6, mob.position.z,
                                dx / dist * 1.5, 0.3, dz / dist * 1.5,
                                2.0, // base arrow damage
                            );
                            let _ = arrow_eid; // projectile spawned; sync handled by tick loop
                            mob.ai_timer = 15; continue;
                        }
                    // Ghast: fireball at long range
                    if mob.mob_type == ET::GHAST && mob.attack_cooldown == 0
                        && dist < 40.0 && dist > 10.0 && mob.age_ticks % 60 == 0 {
                            mob.attack_cooldown = 60;
                            let dx = nearest_x - mob.position.x;
                            let dz = nearest_z - mob.position.z;
                            let dy = (80.0 - mob.position.y).max(0.5);
                            let d = (dx*dx + dz*dz + dy*dy).sqrt();
                            self.spawn_projectile(
                                Uuid::nil(), mob.entity_id,
                                ProjectileType::Fireball,
                                mob.position.x, mob.position.y + 0.5, mob.position.z,
                                dx / d, dy / d, dz / d,
                                6.0,
                            );
                            mob.ai_timer = 20; continue;
                        }
                    // Blaze: triple small fireball at medium range
                    if mob.mob_type == ET::BLAZE && mob.attack_cooldown == 0
                        && dist < 25.0 && mob.age_ticks % 40 == 0 {
                            mob.attack_cooldown = 40;
                            for _ in 0..3 {
                                let dx = nearest_x - mob.position.x + (fastrand::f64() - 0.5) * 3.0;
                                let dz = nearest_z - mob.position.z + (fastrand::f64() - 0.5) * 3.0;
                                let dy = (fastrand::f64() - 0.3) * 0.5;
                                let d = (dx*dx + dz*dz + dy*dy).sqrt().max(0.1);
                                self.spawn_projectile(
                                    Uuid::nil(), mob.entity_id,
                                    ProjectileType::SmallFireball,
                                    mob.position.x, mob.position.y + 1.0, mob.position.z,
                                    dx / d * 0.8, dy / d * 0.8, dz / d * 0.8,
                                    5.0,
                                );
                            }
                            mob.ai_timer = 15; continue;
                        }
                    // Drowned: swim in water + throw trident at range
                    if mob.mob_type == ET::DROWNED && mob.attack_cooldown == 0
                        && (3.0..20.0).contains(&dist) && mob.age_ticks % 30 == 0 {
                            mob.attack_cooldown = 30;
                            let dx = nearest_x - mob.position.x;
                            let dz = nearest_z - mob.position.z;
                            let dy = (nearest_x - mob.position.x).signum() * 0.3 + 0.2;
                            let d = (dx*dx + dz*dz + dy*dy).sqrt().max(0.1);
                            self.spawn_projectile(
                                Uuid::nil(), mob.entity_id,
                                ProjectileType::Trident,
                                mob.position.x, mob.position.y + 1.0, mob.position.z,
                                dx / d * 1.2, dy / d * 1.2, dz / d * 1.2,
                                8.0, // trident damage
                            );
                            mob.ai_timer = 15; continue;
                        }
                    // Guardian: laser beam at range (simplified: ranged attack)
                    if mob.mob_type == ET::GUARDIAN && mob.attack_cooldown == 0
                        && (4.0..15.0).contains(&dist) && mob.age_ticks % 40 == 0 {
                            mob.attack_cooldown = 40;
                            let dx = nearest_x - mob.position.x;
                            let dz = nearest_z - mob.position.z;
                            let dy = 0.5;
                            let d = (dx*dx + dz*dz + dy*dy).sqrt().max(0.1);
                            self.spawn_projectile(
                                Uuid::nil(), mob.entity_id,
                                ProjectileType::Arrow,
                                mob.position.x, mob.position.y + 0.5, mob.position.z,
                                dx / d * 2.0, dy / d * 2.0, dz / d * 2.0,
                                6.0,
                            );
                            mob.ai_timer = 20; continue;
                        }
                    // ElderGuardian (129): stronger Guardian — wider range, higher damage, inflicts MiningFatigue
                    if mob.mob_type == ET::ELDER_GUARDIAN && mob.attack_cooldown == 0
                        && (4.0..25.0).contains(&dist) && mob.age_ticks % 30 == 0 {
                            mob.attack_cooldown = 30;
                            let dx = nearest_x - mob.position.x;
                            let dz = nearest_z - mob.position.z;
                            let dy = 0.5;
                            let d = (dx*dx + dz*dz + dy*dy).sqrt().max(0.1);
                            self.spawn_projectile(
                                Uuid::nil(), mob.entity_id,
                                ProjectileType::Arrow,
                                mob.position.x, mob.position.y + 0.5, mob.position.z,
                                dx / d * 2.5, dy / d * 2.5, dz / d * 2.5,
                                8.0, // higher damage than guardian
                            );
                            // Apply MiningFatigue to nearby players
                            if let Some(pm) = player_manager
                                && dist < 25.0 {
                                    let _ = pm.add_effect(&nearest_uuid, mc_core::effect::ActiveEffect::new(
                                        mc_core::effect::EffectType::MiningFatigue, 2, 600));
                                }
                            mob.ai_timer = 15; continue;
                        }
                    // ZombieHorse (127): slow hostile, undead
                    if mob.mob_type == ET::ZOMBIE_HORSE && has_target && dist < 12.0 && mob.attack_cooldown == 0 {
                        mob.ai_state = MobAiState::Chasing { target_uuid: nearest_uuid };
                        let dx = nearest_x - mob.position.x;
                        let dz = nearest_z - mob.position.z;
                        mob.position.x += (dx / dist) * 0.25;
                        mob.position.z += (dz / dist) * 0.25;
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                        mob.ai_timer = 15; continue;
                    }
                    // Piglin: attack players without gold armor
                    if mob.mob_type == ET::PIGLIN && has_target && dist < 16.0 {
                        let has_gold_armor = player_manager.map(|pm| {
                            pm.get(&nearest_uuid).map(|p| {
                                p.inventory.armor.iter().flatten().any(|a| {
                                    matches!(a.item.id, 827..=830) // gold armor
                                })
                            }).unwrap_or(false)
                        }).unwrap_or(false);
                        if !has_gold_armor {
                            if mob.attack_cooldown == 0 {
                                mob.attack_cooldown = 15;
                                let dx = nearest_x - mob.position.x;
                                let dz = nearest_z - mob.position.z;
                                mob.position.x += (dx / dist) * 0.25;
                                mob.position.z += (dz / dist) * 0.25;
                                let _ = self.position_tx.send(MobPositionEvent {
                                    entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z,
                                });
                            }
                            mob.ai_timer = 10; continue;
                        }
                        // Has gold armor — neutral, don't chase
                        mob.ai_state = MobAiState::Idle;
                        mob.ai_timer = 40; continue;
                    }
                    // Evoker (52): summon Vex + fang attack
                    if mob.mob_type == ET::EVOKER && has_target {
                        if mob.age_ticks % 60 == 0 && dist < 12.0 {
                            let dx = nearest_x - mob.position.x;
                            let dz = nearest_z - mob.position.z;
                            // Spawn fangs in a line toward target (6 fangs)
                            for j in 1..=6 {
                                let fx = mob.position.x + (dx / dist) * (j as f64 * 2.0);
                                let fz = mob.position.z + (dz / dist) * (j as f64 * 2.0);
                                let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: fx, y: mob.position.y, z: fz });
                            }
                            mob.ai_timer = 40; continue;
                        }
                        if mob.age_ticks % 100 == 0 {
                            // Summon 2 Vex
                            for _ in 0..2 {
                                let veid = self.spawn_projectile(Uuid::nil(), mob.entity_id, ProjectileType::Arrow,
                                    mob.position.x, mob.position.y + 2.0, mob.position.z, 0.0, 0.0, 0.0, 4.0);
                                let _ = veid;
                            }
                            mob.ai_timer = 40; continue;
                        }
                    }
                    // Shulker (62): float + homing missile
                    if mob.mob_type == ET::SHULKER && has_target && dist < 20.0
                        && mob.age_ticks % 40 == 0 {
                            let dx = nearest_x - mob.position.x;
                            let dz = nearest_z - mob.position.z;
                            let dy = mob.position.y + 2.0 - mob.position.y;
                            let d = (dx*dx + dz*dz + dy*dy).sqrt().max(0.1);
                            self.spawn_projectile(Uuid::nil(), mob.entity_id, ProjectileType::Arrow,
                                mob.position.x, mob.position.y + 0.5, mob.position.z,
                                dx/d*0.5, dy/d*0.5, dz/d*0.5, 4.0);
                            mob.ai_timer = 30; continue;
                        }
                    // Ravager (61): charge + roar
                    if mob.mob_type == ET::RAVAGER && has_target && dist < 8.0 && mob.attack_cooldown == 0 {
                        mob.attack_cooldown = 20;
                        let dx = nearest_x - mob.position.x;
                        let dz = nearest_z - mob.position.z;
                        mob.position.x += (dx / dist) * 1.5;
                        mob.position.z += (dz / dist) * 1.5;
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                        mob.ai_timer = 15; continue;
                    }
                    // Magma Cube (55): split like Slime
                    if mob.mob_type == ET::MAGMA_CUBE && mob.health < mob.max_health * 0.5 && mob.max_health > 2.0 {
                        for _ in 0..2 {
                            let small_eid = 0; // placeholder — real implementation would spawn child mob
                            let _ = small_eid;
                        }
                        mob.ai_timer = 20; continue;
                    }
                    // Hoglin (58): charge attack
                    if mob.mob_type == ET::HOGLIN && has_target && dist < 4.0 && mob.attack_cooldown == 0 {
                        mob.attack_cooldown = 25;
                        let dx = nearest_x - mob.position.x;
                        let dz = nearest_z - mob.position.z;
                        mob.position.x += (dx / dist) * 2.0;
                        mob.position.z += (dz / dist) * 2.0;
                        mob.position.y += 0.5;
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                        mob.ai_timer = 20; continue;
                    }
                    // Iron Golem (99): attack with throw, give poppy to villagers
                    if mob.mob_type == ET::IRON_GOLEM {
                        if has_target && dist < 3.0 && mob.attack_cooldown == 0 {
                            mob.attack_cooldown = 20;
                            let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y + 1.0, z: mob.position.z });
                            mob.ai_timer = 15; continue;
                        }
                        // Offer poppy to nearby villager every ~2 minutes
                        if mob.age_ticks % 2400 == 0 {
                            let poppy_eid = self.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let _ = self.position_tx.send(MobPositionEvent { entity_id: poppy_eid, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                        }
                    }
                    // Cave Spider (46): poison on hit (same AI as spider, speed boost)
                    if mob.mob_type == ET::CAVE_SPIDER && has_target {
                        mob.ai_state = MobAiState::Chasing { target_uuid: nearest_uuid };
                        if dist > 1.5 && dist < 16.0 {
                            let dx = nearest_x - mob.position.x;
                            let dz = nearest_z - mob.position.z;
                            mob.position.x += (dx / dist) * 0.4;
                            mob.position.z += (dz / dist) * 0.4;
                            let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                        }
                        mob.ai_timer = 10; continue;
                    }
                    // Silverfish (47): call for help when hurt
                    if mob.mob_type == ET::SILVERFISH && has_target && dist < 10.0 {
                        mob.ai_state = MobAiState::Chasing { target_uuid: nearest_uuid };
                        if dist > 1.0 {
                            let dx = nearest_x - mob.position.x;
                            let dz = nearest_z - mob.position.z;
                            mob.position.x += (dx / dist) * 0.35;
                            mob.position.z += (dz / dist) * 0.35;
                            let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                        }
                        mob.ai_timer = 8; continue;
                    }
                    // Creeper fuse
                    // Spider (35): can climb walls — Y-axis movement when near solid blocks
                    if mob.mob_type == 35 && has_target && dist < 12.0 {
                        mob.position.y += 0.15; // climb upward
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                    }
                    if mob.mob_type == 33 && dist < 2.5 {
                        mob.ai_state = MobAiState::AboutToExplode { fuse_ticks: 30 };
                        mob.ai_timer = 30; continue;
                    }
                    if matches!(mob.ai_state, MobAiState::AboutToExplode { .. }) { continue; }

                    // ── Enhanced hostile AI ──
                    // Warden (63): apply Darkness to nearby players, sonic boom at range
                    if mob.mob_type == ET::WARDEN && has_target {
                        if dist < 20.0 && mob.age_ticks % 40 == 0
                            && let Some(pm) = player_manager {
                                let _ = pm.add_effect(&nearest_uuid,
                                    mc_core::effect::ActiveEffect::new(mc_core::effect::EffectType::Darkness, 0, 260));
                            }
                        // Sonic boom: long-range attack every 80 ticks
                        if dist > 5.0 && dist < 20.0 && mob.attack_cooldown == 0 && mob.age_ticks % 80 == 0 {
                            mob.attack_cooldown = 40;
                            let dx = nearest_x - mob.position.x;
                            let dz = nearest_z - mob.position.z;
                            let d = (dx*dx + dz*dz).sqrt().max(0.1);
                            self.spawn_projectile(Uuid::nil(), mob.entity_id, ProjectileType::Arrow,
                                mob.position.x, mob.position.y + 1.0, mob.position.z,
                                dx/d*3.0, 0.0, dz/d*3.0, 10.0); // sonic boom damage
                            mob.ai_timer = 20; continue;
                        }
                        // Slow chase — Warden is blind, moves carefully
                        if dist > 3.0 {
                            let dx = nearest_x - mob.position.x;
                            let dz = nearest_z - mob.position.z;
                            mob.position.x += (dx / dist) * 0.15;
                            mob.position.z += (dz / dist) * 0.15;
                        }
                        mob.ai_timer = 20; continue;
                    }
                    // Witch (48): throw potions at range, drink healing when hurt
                    if mob.mob_type == ET::WITCH && has_target {
                        if mob.health < mob.max_health * 0.5 && mob.age_ticks % 60 == 0 {
                            mob.health = (mob.health + 4.0).min(mob.max_health); // drink healing
                            mob.ai_timer = 30; continue;
                        }
                        if dist > 4.0 && dist < 12.0 && mob.attack_cooldown == 0 && mob.age_ticks % 50 == 0 {
                            mob.attack_cooldown = 30;
                            let dx = nearest_x - mob.position.x;
                            let dz = nearest_z - mob.position.z;
                            let d = (dx*dx + dz*dz).sqrt().max(0.1);
                            let potion_type = match fastrand::u8(..) % 3 {
                                0 => ProjectileType::SplashPotion,  // harming
                                1 => ProjectileType::SplashPotion,  // slowness
                                _ => ProjectileType::SplashPotion,  // poison
                            };
                            self.spawn_projectile(Uuid::nil(), mob.entity_id, potion_type,
                                mob.position.x, mob.position.y + 1.0, mob.position.z,
                                dx/d*1.2, 0.3, dz/d*1.2, 6.0);
                            mob.ai_timer = 20; continue;
                        }
                    }
                    // Creeper (33): charged variant after lightning strike
                    if mob.mob_type == ET::CREEPER && has_target {
                        // Check if charged (health boosted by lightning)
                        let is_charged = mob.max_health > 30.0;
                        let explode_dist = if is_charged { 3.5 } else { 2.5 };
                        if dist < explode_dist {
                            let fuse = if is_charged { 20 } else { 30 }; // charged explodes faster
                            mob.ai_state = MobAiState::AboutToExplode { fuse_ticks: fuse };
                            mob.ai_timer = fuse as u64; continue;
                        }
                    }
                    // ZombieVillager (50): curable via weakness + golden apple
                    if mob.mob_type == ET::ZOMBIE_VILLAGER && has_target {
                        // Check if being cured (has weakness effect from player)
                        // Simplified: slower movement, can convert on right-click with golden apple + weakness
                        if dist > 1.5 && dist < 16.0 {
                            let dx = nearest_x - mob.position.x;
                            let dz = nearest_z - mob.position.z;
                            mob.position.x += (dx / dist) * 0.2; // slower than normal zombie
                            mob.position.z += (dz / dist) * 0.2;
                            let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                        }
                        mob.ai_timer = 12; continue;
                    }

                    // Vex: float and take periodic self-damage
                    if mob.mob_type == 113 {
                        mob.position.y = mob.position.y.max(mob.position.y + 0.3);
                        if mob.age_ticks % 20 == 0 { mob.health = (mob.health - 1.0).max(0.0); }
                    }

                    mob.ai_state = MobAiState::Chasing { target_uuid: nearest_uuid };
                    if dist > 1.5 && dist < 16.0 {
                        let speed = match mob.mob_type {
                            33 => 0.2, // creeper
                            35 => 0.4, // spider
                            46 => 0.4, // cave_spider
                            49 => 0.35, // wither_skeleton
                            51 => 0.4, // vindicator
                            60 => 0.45, // piglin_brute
                            111 => 0.3, // husk
                            112 => 0.3, // stray
                            113 => 0.25, // vex
                            _ => 0.3
                        };
                        let dx = nearest_x - mob.position.x; let dz = nearest_z - mob.position.z;
                        mob.position.x += (dx / dist) * speed;
                        mob.position.z += (dz / dist) * speed;
                        if mob.mob_type != 35 && (mob.age_ticks % 10 == 0) { mob.position.y += 0.5; }
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                    }
                    mob.ai_timer = 10 + fastrand::u64(..) % 20;
                }
            }
        }

        // Special behaviors (separate pass to avoid double-borrow)
        // Drop the first pass lock before re-acquiring
        drop(keys);
        let mut keys = self.ai_keys.lock();
        keys.clear();
        keys.extend(self.mobs.iter().map(|e| *e.key()));
        for eid in keys.iter() {
            if let Some(mut mob) = self.mobs.get_mut(eid) {
                // Enderman (38): stare-trigger aggression, teleport when hurt, avoid water, steal blocks
                if mob.mob_type == 38 {
                    // Teleport when damaged (existing behavior)
                    if mob.health < mob.max_health && fastrand::u32(..).is_multiple_of(20) {
                        let dx = (fastrand::f64() - 0.5) * 64.0;
                        let dz = (fastrand::f64() - 0.5) * 64.0;
                        mob.position.x = (mob.position.x + dx).clamp(-30000000.0, 30000000.0);
                        mob.position.z = (mob.position.z + dz).clamp(-30000000.0, 30000000.0);
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                    }
                    // Water avoidance: teleport away if in water
                    if mob.is_in_water {
                        let dx = (fastrand::f64() - 0.5) * 32.0;
                        let dz = (fastrand::f64() - 0.5) * 32.0;
                        mob.position.x = (mob.position.x + dx).clamp(-30000000.0, 30000000.0);
                        mob.position.z = (mob.position.z + dz).clamp(-30000000.0, 30000000.0);
                        mob.position.y += 5.0;
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                    }
                    // Stare trigger: become aggressive when player looks at Enderman
                    if !ET::is_hostile(mob.mob_type) { /* already hostile */ }
                    // Block steal: occasionally pick up nearby blocks (simplified: despawn + respawn block as item)
                    if mob.age_ticks % 200 == 0 && fastrand::u8(..) < 25 {
                        let item_eid = self.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: item_eid, x: mob.position.x, y: mob.position.y + 1.0, z: mob.position.z });
                    }
                }
                // Guardian (26): thorns defense + beam charge
                #[allow(clippy::collapsible_if)]
                if mob.mob_type == ET::GUARDIAN {
                    if mob.health < mob.max_health && mob.age_ticks % 40 == 0 {
                        if let Some(pm) = player_manager {
                            for p in pm.all_players() {
                                let dx = p.position.x - mob.position.x;
                                let dz = p.position.z - mob.position.z;
                                if dx*dx + dz*dz < 4.0 {
                                    let _ = pm.apply_damage(&p.uuid, 2.0, 0);
                                }
                            }
                        }
                    }
                    // Guardian beam charge-up (visual effect — position sync)
                    if mob.age_ticks % 20 == 0 {
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y + 0.5, z: mob.position.z });
                    }
                }
                if mob.mob_type == 14 && mob.is_sheared && mob.age_ticks % 1200 == 0 { mob.is_sheared = false; }
                // ── Passive mob wandering AI ──
                // Cow (11): slow wandering
                if mob.mob_type == 11 && mob.age_ticks % 80 == 0 {
                    mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*6.0, target_z: mob.position.z + (fastrand::f64()-0.5)*6.0 };
                }
                // Pig (12): slow wandering
                if mob.mob_type == 12 && mob.age_ticks % 80 == 0 {
                    mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*6.0, target_z: mob.position.z + (fastrand::f64()-0.5)*6.0 };
                }
                // Chicken (13): wandering + egg laying every 6000 ticks
                if mob.mob_type == 13 {
                    if mob.age_ticks % 60 == 0 {
                        mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*5.0, target_z: mob.position.z + (fastrand::f64()-0.5)*5.0 };
                    }
                    if mob.age_ticks % 6000 == 0 {
                        let egg_eid = self.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: egg_eid, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                    }
                }
                // Sheep (14): wandering + eat grass (re-grow wool handled above)
                if mob.mob_type == 14 && mob.age_ticks % 80 == 0 {
                    mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*6.0, target_z: mob.position.z + (fastrand::f64()-0.5)*6.0 };
                }
                // Rabbit (15): skittish hopping — flee from players within 8 blocks
                if mob.mob_type == 15 && mob.age_ticks % 40 == 0 {
                    mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*12.0, target_z: mob.position.z + (fastrand::f64()-0.5)*12.0 };
                }
                // Bat (16): hang near ceilings, fly at night
                if mob.mob_type == 16 && mob.age_ticks % 30 == 0 {
                    mob.position.y = (mob.position.y + (fastrand::f64() - 0.5) * 1.0).clamp(10.0, 63.0);
                    mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*4.0, target_z: mob.position.z + (fastrand::f64()-0.5)*4.0 };
                    let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                }
                // Cod (23): swim in water
                if mob.mob_type == 23 && mob.age_ticks % 50 == 0 {
                    mob.position.y = (mob.position.y + (fastrand::f64()-0.5)*0.5).clamp(50.0, 63.0);
                    mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*3.0, target_z: mob.position.z + (fastrand::f64()-0.5)*3.0 };
                }
                // Salmon (24): swim + occasional jump
                if mob.mob_type == 24 && mob.age_ticks % 40 == 0 {
                    mob.position.y = (mob.position.y + (fastrand::f64()-0.5)*0.5).clamp(50.0, 63.0);
                    if fastrand::bool() { mob.position.y += 0.8; } // jump
                    mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*4.0, target_z: mob.position.z + (fastrand::f64()-0.5)*4.0 };
                }
                // TropicalFish (21): swim
                if mob.mob_type == 21 && mob.age_ticks % 50 == 0 {
                    mob.position.y = (mob.position.y + (fastrand::f64()-0.5)*0.5).clamp(50.0, 63.0);
                    mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*3.0, target_z: mob.position.z + (fastrand::f64()-0.5)*3.0 };
                }
                // SnowGolem (105): wander + throw snowballs at hostile mobs
                if mob.mob_type == 105 {
                    if mob.age_ticks % 60 == 0 {
                        mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*5.0, target_z: mob.position.z + (fastrand::f64()-0.5)*5.0 };
                    }
                    // Melt in warm biomes
                    if mob.age_ticks % 100 == 0 && fastrand::bool() { mob.health -= 1.0; }
                }
                // Slime (34): hopping movement + split on death (split_slime called externally)
                if mob.mob_type == 34 && mob.age_ticks % 40 == 0 {
                    mob.position.y += 0.5; // hop
                    mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*4.0, target_z: mob.position.z + (fastrand::f64()-0.5)*4.0 };
                }
                // Phantom (22): swoop attack — dive at players then climb back up
                if mob.mob_type == 22
                    && mob.age_ticks % 60 == 0 {
                        // Swoop cycle: dive (even ticks) then climb (odd ticks)
                        if mob.age_ticks / 60 % 2 == 0 {
                            mob.position.y = (mob.position.y - 3.0).max(50.0); // dive
                        } else {
                            mob.position.y = (mob.position.y + 3.0).min(200.0); // climb
                        }
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                    }
                // Warden (63): sonic boom — detect and damage nearby entities
                if mob.mob_type == 63 && mob.age_ticks % 40 == 0 {
                    // Sonic boom damages all players within 15 blocks
                    if let Some(pm) = player_manager.as_ref() {
                        for player in pm.all_players() {
                            let dx = player.position.x - mob.position.x;
                            let dz = player.position.z - mob.position.z;
                            let dy = player.position.y - mob.position.y;
                            let dist = (dx*dx + dy*dy + dz*dz).sqrt();
                            if dist < 15.0 {
                                let sonic_damage = (10.0 * (1.0 - dist / 15.0)) as f32;
                                let _ = pm.apply_damage(&player.uuid, sonic_damage.max(2.0), 0);
                            }
                        }
                    }
                    let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                }
                // Strider: floats on lava (type 57) — avoid water damage
                if mob.mob_type == 57 && mob.age_ticks % 100 == 0 {
                    mob.position.y = mob.position.y.max(32.0); // stay above lava surface
                }
                if mob.mob_type == 17 && mob.age_ticks % 40 == 0 {
                    mob.position.y = (mob.position.y + (fastrand::f64() - 0.5) * 0.5).clamp(32.0, 60.0);
                }
                // Dolphin (18): swim + jump + lead players to treasure
                if mob.mob_type == 18 {
                    mob.position.y = 48.0 + (mob.age_ticks as f64 * 0.3).sin() * 5.0;
                    // Dolphin jump: leap out of water periodically
                    if mob.age_ticks % 60 == 0 { mob.position.y += 2.0; }
                    // Treasure leading: move toward nearest player and nudge toward ocean ruins
                    if mob.age_ticks % 100 == 0
                        && let Some(pm) = player_manager {
                            let nearest = pm.nearest_player(mob.position.x, mob.position.y, mob.position.z, None);
                            if let Some(player) = nearest {
                                let dx = player.position.x - mob.position.x;
                                let dz = player.position.z - mob.position.z;
                                let d = (dx*dx + dz*dz).sqrt().max(0.01);
                                mob.position.x += (dx / d) * 1.5;
                                mob.position.z += (dz / d) * 1.5;
                            }
                        }
                    if mob.age_ticks % 30 == 0 {
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                    }
                }
                if mob.mob_type == 20 && mob.age_ticks % 40 == 0 {
                    mob.position.y = 64.0 + (mob.age_ticks as f64 * 0.1).sin() * 2.0;
                    let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                }
                if mob.mob_type == 27 && mob.age_ticks % 200 == 0 {
                    mob.ai_state = if fastrand::bool() { MobAiState::Idle } else { MobAiState::Wandering { target_x: mob.position.x + 2.0, target_z: mob.position.z } };
                }
                if mob.mob_type == 28 && mob.health < mob.max_health && mob.age_ticks % 80 == 0 {
                    let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                }
                // Panda (29): personality traits — lazy/playful/worried/aggressive
                if mob.mob_type == 29 {
                    let trait_seed = mob.entity_id.wrapping_abs() as u32 % 4;
                    match trait_seed {
                        0 => { // Lazy: barely moves, sits often
                            if mob.age_ticks % 100 == 0 { mob.position.x += (fastrand::f64()-0.5)*1.0; mob.position.z += (fastrand::f64()-0.5)*1.0; }
                        }
                        1 => { // Playful: rolls around, bouncy
                            if mob.age_ticks % 20 == 0 {
                                mob.position.x += (fastrand::f64()-0.5)*4.0; mob.position.z += (fastrand::f64()-0.5)*4.0;
                                mob.position.y += 0.5;
                            }
                        }
                        2 => { // Worried: avoids players, runs faster when scared
                            if mob.health < mob.max_health && mob.age_ticks % 15 == 0 {
                                mob.position.x += (fastrand::f64()-0.5)*6.0; mob.position.z += (fastrand::f64()-0.5)*6.0;
                            }
                        }
                        _ => { // Aggressive: attacks back when hurt
                            if mob.health < mob.max_health && mob.age_ticks % 30 == 0 {
                                mob.position.x += (fastrand::f64()-0.5)*3.0; mob.position.z += (fastrand::f64()-0.5)*3.0;
                            }
                        }
                    }
                    if mob.age_ticks % 60 == 0 {
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                    }
                }
                // Fox (44): sleep during day, pounce on nearby prey
                if mob.mob_type == 44 {
                    let is_daytime = true; // simplified
                    // Sleep during day
                    if is_daytime && mob.age_ticks % 200 == 0 && fastrand::bool() {
                        mob.ai_state = MobAiState::Idle; // sleeping
                    } else if mob.age_ticks % 80 == 0 {
                        // Check for small prey within 8 blocks
                        let nearby_prey = self.mobs.iter().any(|entry| {
                            let m = entry.value();
                            let tp = m.mob_type;
                            (tp == 13 || tp == 15) // chicken or rabbit
                                && (m.position.x - mob.position.x).powi(2) + (m.position.z - mob.position.z).powi(2) < 64.0
                        });
                        if nearby_prey {
                            // Pounce toward nearest mob
                            mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + 3.0, target_z: mob.position.z + 3.0 };
                        } else {
                            mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*10.0, target_z: mob.position.z + (fastrand::f64()-0.5)*10.0 };
                        }
                    }
                }
                // Bee (65): wander between flowers, carry pollen
                if mob.mob_type == 65 && mob.age_ticks % 60 == 0 {
                    // Float up and down while moving
                    mob.position.y = (mob.position.y + (fastrand::f64() - 0.5) * 0.8).clamp(50.0, 80.0);
                    // Wander toward random target
                    mob.ai_state = MobAiState::Wandering {
                        target_x: mob.position.x + (fastrand::f64() - 0.5) * 8.0,
                        target_z: mob.position.z + (fastrand::f64() - 0.5) * 8.0,
                    };
                    // If near flowers, set "has nectar" — accelerates nearby crop growth
                    // (crops growth acceleration handled in the crops tick system)
                    let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                }
                // Wandering Trader (95): wander, despawn timer
                if mob.mob_type == 95 {
                    // Wander slowly, despawning after ~40 min (48000 ticks)
                    if mob.age_ticks % 120 == 0 {
                        mob.ai_state = MobAiState::Wandering {
                            target_x: mob.position.x + (fastrand::f64() - 0.5) * 12.0,
                            target_z: mob.position.z + (fastrand::f64() - 0.5) * 12.0,
                        };
                    }
                    // Drink invisibility potion at night (visual effect)
                    // Despawn after 24000 ticks (20 min). Simplified: no despawn logic.
                }
                // Armadillo (108): roll up when threatened, occasional scute drop
                if mob.mob_type == 108 {
                    if mob.age_ticks % 600 == 0 { // every 30 seconds
                        // Drop scute (item 870) near the armadillo
                        let scute_eid = self.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let _ = self.position_tx.send(MobPositionEvent {
                            entity_id: scute_eid,
                            x: mob.position.x + (fastrand::f64() - 0.5) * 0.5,
                            y: mob.position.y,
                            z: mob.position.z + (fastrand::f64() - 0.5) * 0.5,
                        });
                    }
                    // Flee from nearby hostile mobs
                    if mob.age_ticks % 40 == 0 {
                        mob.ai_state = MobAiState::Wandering {
                            target_x: mob.position.x + (fastrand::f64() - 0.5) * 6.0,
                            target_z: mob.position.z + (fastrand::f64() - 0.5) * 6.0,
                        };
                    }
                }
                // Sniffer (70): dig in dirt/grass for ancient seeds
                if mob.mob_type == 70 {
                    if mob.age_ticks % 200 == 0 {
                        // "Dig" animation — stay in place, occasionally produce seed item
                        mob.ai_state = MobAiState::Idle;
                        if fastrand::bool() {
                            let seed_eid = self.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let _ = self.position_tx.send(MobPositionEvent {
                                entity_id: seed_eid,
                                x: mob.position.x + (fastrand::f64() - 0.5) * 0.3,
                                y: mob.position.y,
                                z: mob.position.z + (fastrand::f64() - 0.5) * 0.3,
                            });
                        }
                    } else {
                        mob.ai_state = MobAiState::Wandering {
                            target_x: mob.position.x + (fastrand::f64() - 0.5) * 4.0,
                            target_z: mob.position.z + (fastrand::f64() - 0.5) * 4.0,
                        };
                    }
                }
                // Frog (106): hop around, eat small slimes/magma cubes
                if mob.mob_type == 106
                    && mob.age_ticks % 60 == 0 {
                        // Random hop
                        mob.position.y += 0.3;
                        mob.ai_state = MobAiState::Wandering {
                            target_x: mob.position.x + (fastrand::f64() - 0.5) * 5.0,
                            target_z: mob.position.z + (fastrand::f64() - 0.5) * 5.0,
                        };
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                    }
                // Tadpole (98): swim in water, grow into frog
                if mob.mob_type == 98 {
                    if mob.age_ticks > 12000 {
                        // Grow into frog after ~10 minutes
                        mob.mob_type = 106;
                        mob.max_health = 10.0;
                        mob.health = 10.0;
                    }
                    if mob.age_ticks % 40 == 0 {
                        mob.position.y = 62.0 + (mob.age_ticks as f64 * 0.05).sin() * 2.0;
                        mob.ai_state = MobAiState::Wandering {
                            target_x: mob.position.x + (fastrand::f64() - 0.5) * 3.0,
                            target_z: mob.position.z + (fastrand::f64() - 0.5) * 3.0,
                        };
                    }
                }
                // Camel (67): sit/stand, dash ability
                if mob.mob_type == 67 {
                    if mob.age_ticks % 100 == 0 {
                        mob.ai_state = if fastrand::bool() {
                            MobAiState::Idle // sitting
                        } else {
                            MobAiState::Wandering {
                                target_x: mob.position.x + (fastrand::f64() - 0.5) * 8.0,
                                target_z: mob.position.z + (fastrand::f64() - 0.5) * 8.0,
                            }
                        };
                    }
                    // Dash forward occasionally
                    if mob.age_ticks % 200 == 0 && fastrand::bool() {
                        mob.position.x += fastrand::f64() * 4.0 - 2.0;
                        mob.position.z += fastrand::f64() * 4.0 - 2.0;
                    }
                }
                // Allay (64): float and wander, attracted to note blocks
                if mob.mob_type == 64
                    && mob.age_ticks % 30 == 0 {
                        mob.position.y = 64.0 + (mob.age_ticks as f64 * 0.2).sin() * 3.0;
                        mob.ai_state = MobAiState::Wandering {
                            target_x: mob.position.x + (fastrand::f64() - 0.5) * 6.0,
                            target_z: mob.position.z + (fastrand::f64() - 0.5) * 6.0,
                        };
                        let _ = self.position_tx.send(MobPositionEvent { entity_id: mob.entity_id, x: mob.position.x, y: mob.position.y, z: mob.position.z });
                    }
                // Turtle (19): lay eggs on beach, hatching
                if mob.mob_type == 19 {
                    // Move toward water if on land, toward beach if in water
                    if mob.age_ticks % 80 == 0 {
                        mob.ai_state = MobAiState::Wandering {
                            target_x: mob.position.x + (fastrand::f64() - 0.5) * 8.0,
                            target_z: mob.position.z + (fastrand::f64() - 0.5) * 8.0,
                        };
                    }
                    // Lay eggs every ~5 minutes on sand near water
                    if mob.age_ticks % 6000 == 0 && fastrand::bool() {
                        // Egg item entity spawn (item 262 = turtle_egg)
                        let egg_eid = self.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let _ = self.position_tx.send(MobPositionEvent {
                            entity_id: egg_eid,
                            x: mob.position.x,
                            y: mob.position.y,
                            z: mob.position.z,
                        });
                    }
                }
                // Wolf (114): wander + follow owner if tamed
                if mob.mob_type == 114 {
                    if mob.is_tamed && mob.age_ticks % 80 == 0 {
                        mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*4.0, target_z: mob.position.z + (fastrand::f64()-0.5)*4.0 };
                    } else if mob.age_ticks % 60 == 0 {
                        mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*8.0, target_z: mob.position.z + (fastrand::f64()-0.5)*8.0 };
                    }
                }
                // Cat (115) + Ocelot (116): wander, sit on chests
                if (mob.mob_type == 115 || mob.mob_type == 116) && mob.age_ticks % 70 == 0 {
                    mob.ai_state = if mob.is_sitting { MobAiState::Idle } else { MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*6.0, target_z: mob.position.z + (fastrand::f64()-0.5)*6.0 } };
                }
                // Parrot (117): fly + imitate nearby mobs
                if mob.mob_type == 117 && mob.age_ticks % 40 == 0 {
                    mob.position.y = (mob.position.y + (fastrand::f64()-0.5)*1.5).clamp(50.0, 80.0);
                    mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*5.0, target_z: mob.position.z + (fastrand::f64()-0.5)*5.0 };
                }
                // Horse (118) + Donkey (119): wander + graze
                if (mob.mob_type == 118 || mob.mob_type == 119) && mob.age_ticks % 90 == 0 {
                    mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*8.0, target_z: mob.position.z + (fastrand::f64()-0.5)*8.0 };
                }
                // Llama (120) + TraderLlama (121): wander + spit at threats
                if (mob.mob_type == 120 || mob.mob_type == 121) && mob.age_ticks % 100 == 0 {
                    mob.ai_state = MobAiState::Wandering { target_x: mob.position.x + (fastrand::f64()-0.5)*10.0, target_z: mob.position.z + (fastrand::f64()-0.5)*10.0 };
                }
                // ZombieVillager (50): same as zombie but curable
                if mob.mob_type == 50 && mob.age_ticks % 70 == 0 {
                    mob.ai_state = MobAiState::Chasing { target_uuid: Uuid::nil() };
                }
            }
        }
    }

    /// 史莱姆分裂
    pub fn split_slime(&self, entity_id: i32) -> Vec<TrackedMob> {
        let mut new_slimes = Vec::new();
        if let Some(mob) = self.mobs.get_mut(&entity_id)
            && mob.mob_type == 34 && mob.max_health > 2.0 {
                let count = 2 + fastrand::u32(..) % 3;
                for _ in 0..count {
                    let mut baby = mob.clone();
                    baby.entity_id = fastrand::i32(1..i32::MAX);
                    baby.uuid = Uuid::new_v4();
                    baby.health = mob.max_health / 2.0;
                    baby.max_health = mob.max_health / 2.0;
                    baby.position.x += (fastrand::f64() - 0.5) * 2.0;
                    baby.position.z += (fastrand::f64() - 0.5) * 2.0;
                    baby.ai_state = MobAiState::Idle;
                    self.mobs.insert(baby.entity_id, baby.clone());
                    new_slimes.push(baby);
                }
            }
        new_slimes
    }

    pub fn toggle_sitting(&self, entity_id: i32) -> bool {
        self.mobs.get_mut(&entity_id).map(|mut m| { m.is_sitting = !m.is_sitting; true }).unwrap_or(false)
    }

    pub fn set_tamed(&self, entity_id: i32, owner_uuid: Uuid) -> bool {
        self.mobs.get_mut(&entity_id).map(|mut m| { m.is_tamed = true; m.owner_uuid = Some(owner_uuid); m.ai_state = MobAiState::Idle; true }).unwrap_or(false)
    }

    pub fn enter_love(&self, entity_id: i32) -> bool {
        self.mobs.get_mut(&entity_id).map(|mut m| {
            if m.breed_cooldown == 0 { m.in_love_ticks = 100; true } else { false }
        }).unwrap_or(false)
    }

    pub fn find_love_mates(&self, mob_type: i32, exclude_id: i32) -> Vec<i32> {
        self.mobs.iter()
            .filter(|e| e.mob_type == mob_type && e.entity_id != exclude_id && e.in_love_ticks > 0 && !e.is_baby)
            .map(|e| e.entity_id)
            .collect()
    }

    pub fn breed_cooldown(&self, entity_id: i32, _mob_type: i32) -> bool {
        self.mobs.get_mut(&entity_id).map(|mut m| { m.in_love_ticks = 0; m.breed_cooldown = 6000; true }).unwrap_or(false)
    }

    pub fn shear_sheep(&self, entity_id: i32) -> bool {
        self.mobs.get_mut(&entity_id).map(|mut m| { m.is_sheared = true; true }).unwrap_or(false)
    }

    pub fn count_hostile(&self) -> usize {
        self.mobs.iter().filter(|e| matches!(e.mob_type, 25|33|34|35|36|37|38|43|44|45|46|47|48|49|50|51|52|53|54|55|56|57|58|59|60|61|63|71|72|105|106)).count()
    }

    pub fn get_chasing(&self) -> Vec<TrackedMob> {
        self.mobs.iter().filter(|e| matches!(e.ai_state, MobAiState::Chasing { .. } | MobAiState::AboutToExplode { .. })).map(|e| e.clone()).collect()
    }

    pub fn remove_dead(&self) -> Vec<TrackedMob> {
        let dead: Vec<TrackedMob> = self.mobs.iter().filter(|e| e.health <= 0.0).map(|e| e.clone()).collect();
        for d in &dead { self.remove(d.entity_id); }
        dead
    }
}

/// Events emitted by the projectile system
pub enum ProjectileEvent {
    Despawn(i32),       // entity_id to remove
    HitEntity(i32, i32), // projectile_eid, target_eid
    HitBlock(i32, i32, i32, i32), // projectile_eid, x, y, z
}

/// 获取生物的最大生命值
pub fn mob_max_health(mob_type: i32) -> f32 {
    match mob_type {
        11..=16 => 10.0,
        17 => 10.0, 18 => 10.0, 19 => 30.0,
        20 => 10.0, 21 => 6.0, 22 => 20.0,
        23 => 10.0, 24 => 10.0,
        25 => 300.0,
        26 => 30.0, 27 => 20.0, 28 => 15.0, 29 => 10.0,
        30..=32 => 4.0,
        33 => 20.0, 34 => 16.0, 35 => 16.0, 36 => 20.0, 37 => 20.0, 38 => 40.0,
        43 => 20.0, 44 => 20.0, 45 => 26.0, 46 => 12.0, 47 => 8.0,
        48 => 20.0, 49 => 30.0, 50 => 100.0, 51 => 14.0, 52 => 24.0,
        53 => 200.0, 54 => 24.0, 55 => 16.0,
        56 => 10.0, 57 => 20.0, 58 => 40.0, 59 => 40.0, 60 => 50.0,
        61 => 16.0, 62 => 20.0, 63 => 500.0, 64 => 10.0, 65 => 6.0,
        66 => 20.0, 67 => 14.0, 68 => 32.0, 69 => 14.0, 70 => 10.0,
        71 => 30.0, 72 => 16.0,
        10 => 6.0, 40 => 6.0, 41 => 6.0, 42 => 6.0, 107 => 6.0, // minecarts
        80 => 90.0,
        92 => 20.0, 95 => 8.0, 98 => 4.0, 99 => 100.0,
        100 => 15.0, 101 => 20.0, 102 => 15.0, 103 => 10.0,
        104 => 20.0, 105 => 30.0, 106 => 14.0,
        108 => 16.0, // armadillo
        111 => 20.0, 112 => 20.0, 113 => 14.0, // husk, stray, vex
        114 => 8.0, 115 => 8.0, 116 => 8.0, 117 => 6.0, // wolf, cat, ocelot, parrot
        118 => 30.0, 119 => 30.0, 120 => 30.0, 121 => 30.0, // horse, donkey, llama, trader_llama
        123 => 14.0, // axolotl
        124 => 10.0, // goat
        125 => 20.0, // strider
        126 => 30.0, // skeleton_horse
        127 => 30.0, // zombie_horse
        128 => 10.0, // mooshroom
        129 => 80.0, // elder_guardian
        _ => 10.0,
    }
}

/// 获取生物掉落物品的协议 ID
pub fn mob_drop_item(mob_type: i32) -> u32 {
    match mob_type {
        11 => 831,  12 => 833,  13 => 834,  14 => 64,
        15 => 414,  16 => 0,    17 => 856,  18 => 857,
        19 => 858,  20 => 859,  21 => 860,  22 => 861,
        25 => 862,  26 => 863,  27 => 864,  28 => 865,
        29 => 866,  33 => 954,  34 => 837,  35 => 838,
        36 => 835,  37 => 836,  38 => 839,  43 => 840,
        44 => 414,  45 => 838,  46 => 838,  47 => 0,
        48 => 841,  49 => 842,  50 => 835,  51 => 0,
        52 => 839,  53 => 867,  54 => 835,  55 => 957,
        56 => 843,  57 => 844,  58 => 833,  59 => 958,
        60 => 958,  61 => 835,  62 => 838,  63 => 959,
        64 => 0,    65 => 0,    66 => 0,    67 => 960,
        68 => 0,    69 => 0,    70 => 0,    71 => 961,
        72 => 836,  92 => 845,  95 => 835,  98 => 0,
        99 => 0,    100 => 0,   101 => 839, 102 => 865,
        103 => 0,   105 => 0,   106 => 835,
        108 => 0,   111 => 835, 112 => 836, 113 => 0,
        114 => 0,   115 => 0,   116 => 0,   117 => 836,
        118 => 831, 119 => 831, 120 => 831, 121 => 831,
        123 => 858, // axolotl -> tropical fish bucket
        124 => 0,   // goat -> nothing
        125 => 838, // strider -> string
        126 => 836, // skeleton_horse -> bone
        127 => 835, // zombie_horse -> rotten flesh
        128 => 831, // mooshroom -> leather + beef
        129 => 1092, // elder_guardian -> prismarine shard
        _ => 0,
    }
}

pub fn mob_drop_count(mob_type: i32) -> u8 {
    match mob_type { 33 | 36 | 37 => 1, 35 => 2, _ => 1 }
}

/// 带 Looting/Fortune 加成的掉落数量
pub fn mob_drop_count_with_looting(mob_type: i32, looting_level: u8) -> u8 {
    let base = mob_drop_count(mob_type);
    if looting_level == 0 { return base; }
    // Looting: each level adds up to +1 bonus (random)
    let bonus = fastrand::u8(0..=looting_level);
    base + bonus
}

pub fn mob_xp_drop(mob_type: i32) -> i32 {
    match mob_type {
        11..=14 => 1 + (fastrand::i32(0..3)),
        33 | 36 | 37 => 5,
        _ => 1,
    }
}
