//! 袭击 (Raid) 系统
//! 当带有 BadOmen 效果的玩家进入村庄时触发多波次灾厄村民攻击。
//! 击败所有波次后参与者获得 HeroOfTheVillage 效果。

use mc_core::position::Position;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// 袭击波次定义
#[derive(Debug, Clone)]
pub struct RaidWave {
    pub mob_types: Vec<(i32, u32)>, // (entity_type_id, count)
}

/// 袭击状态
#[derive(Debug, Clone)]
pub struct RaidState {
    pub center: (i32, i32, i32),       // 村庄中心 (x, y, z)
    pub current_wave: u32,             // 当前波次 (1-based)
    pub total_waves: u32,              // 总波次
    pub mobs_alive: HashSet<i32>,      // 存活的袭击生物 entity_id
    pub participants: HashSet<Uuid>,   // 参与玩家
    pub active: bool,
    pub ticks_since_last_wave: u32,    // 波间计时器
}

/// 袭击管理器
pub struct RaidManager {
    pub active_raids: RwLock<HashMap<(i32, i32, i32), RaidState>>,
    pub village_centers: RwLock<Vec<(i32, i32, i32)>>,
    /// Recently completed raid centers (for advancement triggering)
    pub completed_raids: RwLock<Vec<(i32, i32, i32)>>,
}

impl Default for RaidManager {
    fn default() -> Self { Self::new() }
}

impl RaidManager {
    pub fn new() -> Self {
        Self {
            active_raids: RwLock::new(HashMap::new()),
            village_centers: RwLock::new(Vec::new()),
            completed_raids: RwLock::new(Vec::new()),
        }
    }

    /// Drain completed raid centers
    pub fn take_completed_raids(&self) -> Vec<(i32, i32, i32)> {
        std::mem::take(&mut *self.completed_raids.write())
    }

    /// 获取指定难度的袭击波次定义
    pub fn waves_for_difficulty(difficulty: u8) -> Vec<RaidWave> {
        match difficulty {
            0 => vec![ // Peaceful — no raids
            ],
            1 => vec![ // Easy: 3 waves
                RaidWave { mob_types: vec![(59, 3)] }, // 3 Pillagers
                RaidWave { mob_types: vec![(59, 2), (51, 1)] }, // 2 Pillagers + 1 Vindicator
                RaidWave { mob_types: vec![(59, 2), (51, 1), (48, 1)] }, // +1 Witch
            ],
            2 => vec![ // Normal: 5 waves
                RaidWave { mob_types: vec![(59, 3)] },
                RaidWave { mob_types: vec![(59, 3), (51, 1)] },
                RaidWave { mob_types: vec![(59, 2), (51, 1), (48, 1)] },
                RaidWave { mob_types: vec![(59, 3), (51, 2), (48, 1)] },
                RaidWave { mob_types: vec![(59, 3), (51, 2), (61, 1)] }, // +1 Ravager
            ],
            _ => vec![ // Hard: 7 waves
                RaidWave { mob_types: vec![(59, 4)] },
                RaidWave { mob_types: vec![(59, 3), (51, 2)] },
                RaidWave { mob_types: vec![(59, 3), (51, 1), (48, 1)] },
                RaidWave { mob_types: vec![(59, 4), (51, 2), (48, 1)] },
                RaidWave { mob_types: vec![(59, 4), (51, 2), (48, 2)] },
                RaidWave { mob_types: vec![(59, 3), (51, 2), (61, 2)] }, // +2 Ravagers
                RaidWave { mob_types: vec![(59, 2), (51, 2), (61, 1), (52, 1)] }, // +1 Evoker
            ],
        }
    }

    /// 尝试在村庄触发袭击 (玩家进入村庄且带有 BadOmen)
    /// 返回 Some(wave_count) 表示袭击已开始, None 表示条件不满足
    pub fn try_start_raid(&self, player_uuid: Uuid, player_pos: &Position, has_bad_omen: bool, difficulty: u8) -> Option<u32> {
        if !has_bad_omen || difficulty == 0 { return None; }

        let centers = self.village_centers.read();
        for center in centers.iter() {
            let dx = player_pos.x - center.0 as f64;
            let dz = player_pos.z - center.2 as f64;
            if dx * dx + dz * dz < 4096.0 { // within 64 blocks of village center
                let mut raids = self.active_raids.write();
                if raids.contains_key(center) { return None; } // already active

                let waves = Self::waves_for_difficulty(difficulty);
                if waves.is_empty() { return None; }

                let total_waves = waves.len() as u32;
                let mut participants = HashSet::new();
                participants.insert(player_uuid);

                raids.insert(*center, RaidState {
                    center: *center,
                    current_wave: 0, // starts at 0, first wave spawned on next tick
                    total_waves,
                    mobs_alive: HashSet::new(),
                    participants,
                    active: true,
                    ticks_since_last_wave: 0,
                });
                return Some(total_waves);
            }
        }
        None
    }

    /// 生成当前波次的灾厄村民
    /// 返回需要生成的生物列表: (entity_type, x, y, z)
    pub fn spawn_wave(&self, center: (i32, i32, i32)) -> Vec<(i32, Position)> {
        let mut raids = self.active_raids.write();
        if let Some(raid) = raids.get_mut(&center) {
            if raid.current_wave >= raid.total_waves { return vec![]; }

            let waves = Self::waves_for_difficulty(2); // default normal
            if raid.current_wave as usize >= waves.len() { return vec![]; }

            let wave = &waves[raid.current_wave as usize];
            let mut spawns = Vec::new();

            for (mob_type, count) in &wave.mob_types {
                for _ in 0..*count {
                    // Spawn at random position around village center
                    let angle = fastrand::f64() * std::f64::consts::TAU;
                    let dist = 5.0 + fastrand::f64() * 20.0;
                    let sx = center.0 as f64 + angle.cos() * dist;
                    let sz = center.2 as f64 + angle.sin() * dist;
                    // Find ground level (simplified: use center Y)
                    let sy = center.1 as f64;
                    spawns.push((*mob_type, Position::new(sx, sy, sz)));
                }
            }

            raid.current_wave += 1;
            raid.ticks_since_last_wave = 0;
            spawns
        } else {
            vec![]
        }
    }

    /// 检查波次是否已清除 (所有生物死亡)
    pub fn check_wave_complete(&self, center: (i32, i32, i32)) -> bool {
        let raids = self.active_raids.read();
        if let Some(raid) = raids.get(&center) {
            raid.mobs_alive.is_empty() && raid.ticks_since_last_wave > 60
        } else {
            false
        }
    }

    /// 袭击完成 — 所有波次已击败
    pub fn complete_raid(&self, center: (i32, i32, i32)) -> Option<HashSet<Uuid>> {
        let mut raids = self.active_raids.write();
        if let Some(raid) = raids.remove(&center)
            && raid.current_wave >= raid.total_waves {
                return Some(raid.participants.clone());
            }
        None
    }

    /// 注册一个袭击生物
    pub fn register_raid_mob(&self, center: (i32, i32, i32), entity_id: i32) {
        let mut raids = self.active_raids.write();
        if let Some(raid) = raids.get_mut(&center) {
            raid.mobs_alive.insert(entity_id);
        }
    }

    /// 移除一个已死亡的袭击生物
    pub fn remove_raid_mob(&self, center: (i32, i32, i32), entity_id: i32) {
        let mut raids = self.active_raids.write();
        if let Some(raid) = raids.get_mut(&center) {
            raid.mobs_alive.remove(&entity_id);
        }
    }

    /// 添加玩家到参与者列表
    pub fn add_participant(&self, center: (i32, i32, i32), uuid: Uuid) {
        let mut raids = self.active_raids.write();
        if let Some(raid) = raids.get_mut(&center) {
            raid.participants.insert(uuid);
        }
    }

    /// Tick 所有活跃袭击 (递增计时器)
    pub fn tick(&self) -> Vec<(i32, i32, i32)> {
        let mut waves_ready = Vec::new();
        let mut raids = self.active_raids.write();
        for (center, raid) in raids.iter_mut() {
            if !raid.active { continue; }
            raid.ticks_since_last_wave += 1;
            // Spawn next wave after 200 ticks (10 seconds) delay, or immediately if first wave
            let delay = if raid.current_wave == 0 { 0 } else { 200 };
            if raid.ticks_since_last_wave >= delay && raid.mobs_alive.is_empty() {
                waves_ready.push(*center);
            }
        }
        waves_ready
    }

    /// 取消袭击 (玩家离开区域)
    pub fn cancel_raid(&self, center: (i32, i32, i32)) {
        self.active_raids.write().remove(&center);
    }

    /// 注册村庄中心 (在区块加载时自动检测)
    pub fn register_village(&self, center: (i32, i32, i32)) {
        let mut villages = self.village_centers.write();
        // Check for duplicates
        for existing in villages.iter() {
            let dx = existing.0 - center.0;
            let dz = existing.2 - center.2;
            if dx * dx + dz * dz < 4096 { return; } // within 64 blocks of existing village
        }
        villages.push(center);
    }
}
