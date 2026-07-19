//! 共享世界状态 — 命令系统可读写的运行时世界属性

use crate::types::GameMode;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// 可变的运行时世界状态
#[derive(Debug, Clone)]
pub struct WorldState {
    pub time: u64,
    pub daytime: u64,
    pub weather: Weather,
    pub weather_timer: u64,
    pub difficulty: Difficulty,
    pub seed: u64,
    /// 新玩家加入时的默认游戏模式
    pub default_gamemode: GameMode,
    /// 默认出生点
    pub spawn_x: f64,
    pub spawn_y: f64,
    pub spawn_z: f64,
    /// 游戏规则 (key → value as string)
    pub game_rules: HashMap<String, String>,
    /// 世界边界
    pub world_border: WorldBorder,
    /// 强制加载的区块 (用于 /forceload)
    pub force_loaded: HashMap<(i32, i32), bool>,
    /// 调试模式 (用于 /debug)
    pub debug_mode: bool,
    /// Tick 冻结 (用于 /tick freeze)
    pub tick_frozen: bool,
    /// 加速 tick 速率 (0=默认20tps, >0=自定义tps)
    pub tick_sprint_rate: u32,
}

/// 世界边界配置
#[derive(Debug, Clone)]
pub struct WorldBorder {
    pub center_x: f64,
    pub center_z: f64,
    pub size: f64,
    pub target_size: f64,
    pub lerp_time_ticks: i64,
    pub lerp_start_tick: u64,
    pub damage_per_block: f64,
    pub safe_zone: f64,
    pub warning_blocks: i32,
    pub warning_time: i32,
}

impl Default for WorldBorder {
    fn default() -> Self {
        Self {
            center_x: 0.0, center_z: 0.0,
            size: 60000000.0, target_size: 60000000.0,
            lerp_time_ticks: 0, lerp_start_tick: 0,
            damage_per_block: 0.2, safe_zone: 5.0,
            warning_blocks: 5, warning_time: 15,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weather {
    Clear,
    Rain,
    Thunder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

impl Default for WorldState {
    fn default() -> Self {
        let mut rules = HashMap::new();
        rules.insert("doDaylightCycle".into(), "true".into());
        rules.insert("doWeatherCycle".into(), "true".into());
        rules.insert("keepInventory".into(), "false".into());
        rules.insert("doFireTick".into(), "true".into());
        rules.insert("doMobSpawning".into(), "true".into());
        rules.insert("randomTickSpeed".into(), "3".into());
        rules.insert("announceAdvancements".into(), "true".into());
        Self {
            time: 0,
            daytime: 0,
            weather: Weather::Clear,
            weather_timer: 12000, // start with 10 min clear weather
            difficulty: Difficulty::Normal,
            seed: 0,
            default_gamemode: GameMode::Survival,
            spawn_x: 0.0,
            spawn_y: 64.0,
            spawn_z: 0.0,
            game_rules: rules,
            world_border: WorldBorder::default(),
            force_loaded: HashMap::new(),
            debug_mode: false,
            tick_frozen: false,
            tick_sprint_rate: 0,
        }
    }
}

impl WorldState {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            spawn_x: 0.0,
            spawn_y: 64.0,
            spawn_z: 0.0,
            ..Default::default()
        }
    }

    pub fn set_time(&mut self, time: u64) {
        self.time = time;
        self.daytime = time % 24000;
    }

    pub fn add_time(&mut self, ticks: u64) {
        // Respect doDaylightCycle gamerule
        let cycle = self.game_rules.get("doDaylightCycle")
            .map(|v| v == "true")
            .unwrap_or(true);
        if cycle {
            self.time = self.time.wrapping_add(ticks);
            self.daytime = self.time % 24000;
        }
    }

    pub fn set_weather(&mut self, weather: Weather, duration: u32) {
        self.weather = weather;
        // Note: duration tracking not yet implemented
        let _ = duration;
    }

    pub fn set_difficulty(&mut self, difficulty: Difficulty) {
        self.difficulty = difficulty;
    }

    pub fn set_default_gamemode(&mut self, gm: GameMode) {
        self.default_gamemode = gm;
    }

    pub fn set_default_spawn(&mut self, x: f64, y: f64, z: f64) {
        self.spawn_x = x;
        self.spawn_y = y;
        self.spawn_z = z;
    }
}

/// 可共享的世界状态引用
pub type SharedWorldState = Arc<RwLock<WorldState>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_state_time() {
        let mut ws = WorldState::new(42);
        assert_eq!(ws.time, 0);
        assert_eq!(ws.seed, 42);

        ws.set_time(1000);
        assert_eq!(ws.time, 1000);
        assert_eq!(ws.daytime, 1000);

        ws.add_time(500);
        assert_eq!(ws.time, 1500);

        ws.add_time(23000);
        assert_eq!(ws.daytime, 1500 + 23000 - 24000); // wraps
    }

    #[test]
    fn test_world_state_weather_difficulty() {
        let mut ws = WorldState::default();
        assert!(matches!(ws.weather, Weather::Clear));
        assert!(matches!(ws.difficulty, Difficulty::Normal));

        ws.set_weather(Weather::Rain, 6000);
        assert!(matches!(ws.weather, Weather::Rain));

        ws.set_difficulty(Difficulty::Hard);
        assert!(matches!(ws.difficulty, Difficulty::Hard));
    }

    #[test]
    fn test_world_state_default_gamemode() {
        let mut ws = WorldState::default();
        assert_eq!(ws.default_gamemode, crate::types::GameMode::Survival);

        ws.set_default_gamemode(crate::types::GameMode::Creative);
        assert_eq!(ws.default_gamemode, crate::types::GameMode::Creative);

        ws.set_default_gamemode(crate::types::GameMode::Adventure);
        assert_eq!(ws.default_gamemode, crate::types::GameMode::Adventure);
    }
}
