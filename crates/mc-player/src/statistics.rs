//! 统计系统 — 玩家行为追踪
//! Tracks minecraft custom statistics per player, synced to client periodically.

use parking_lot::RwLock;
use std::collections::HashMap;
use uuid::Uuid;

/// 统计类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatType {
    PlayTime,       // ticks played
    Jumps,          // spacebar presses
    DamageTaken,    // total damage taken
    Deaths,         // death count
    MobKills,       // mobs killed
    FishCaught,     // fish caught
    BlocksMined,    // blocks broken
    ItemsCrafted,   // items crafted
    ItemsUsed,      // items used
    DistanceWalked, // blocks walked
    DistanceSprinted, // blocks sprinted
    DistanceFallen, // blocks fallen
}

impl StatType {
    pub fn id(&self) -> &'static str {
        match self {
            StatType::PlayTime => "play_time",
            StatType::Jumps => "jump",
            StatType::DamageTaken => "damage_taken",
            StatType::Deaths => "deaths",
            StatType::MobKills => "mob_kills",
            StatType::FishCaught => "fish_caught",
            StatType::BlocksMined => "blocks_mined",
            StatType::ItemsCrafted => "items_crafted",
            StatType::ItemsUsed => "items_used",
            StatType::DistanceWalked => "distance_walked",
            StatType::DistanceSprinted => "distance_sprinted",
            StatType::DistanceFallen => "distance_fallen",
        }
    }
}

/// Per-player statistics tracker
pub struct StatTracker {
    stats: RwLock<HashMap<Uuid, HashMap<StatType, i32>>>,
}

impl Default for StatTracker {
    fn default() -> Self { Self::new() }
}

impl StatTracker {
    pub fn new() -> Self {
        Self { stats: RwLock::new(HashMap::new()) }
    }

    /// Increment a statistic for a player
    pub fn increment(&self, uuid: &Uuid, stat: StatType, amount: i32) {
        let mut stats = self.stats.write();
        let player_stats = stats.entry(*uuid).or_default();
        *player_stats.entry(stat).or_insert(0) += amount;
    }

    /// Get a specific statistic value
    pub fn get(&self, uuid: &Uuid, stat: StatType) -> i32 {
        self.stats.read().get(uuid)
            .and_then(|ps| ps.get(&stat).copied())
            .unwrap_or(0)
    }

    /// Get all statistics for a player
    pub fn get_all(&self, uuid: &Uuid) -> HashMap<StatType, i32> {
        self.stats.read().get(uuid).cloned().unwrap_or_default()
    }

    /// Remove a player's stats (on disconnect cleanup)
    pub fn remove_player(&self, uuid: &Uuid) {
        self.stats.write().remove(uuid);
    }

    /// Serialize stats to a simple key-value format for persistence
    pub fn serialize(&self, uuid: &Uuid) -> Vec<u8> {
        let stats = self.get_all(uuid);
        let mut buf = Vec::new();
        buf.extend_from_slice(&(stats.len() as u16).to_le_bytes());
        for (stat, value) in &stats {
            let name = stat.id();
            buf.push(name.len() as u8);
            buf.extend_from_slice(name.as_bytes());
            buf.extend_from_slice(&value.to_le_bytes());
        }
        buf
    }
}
