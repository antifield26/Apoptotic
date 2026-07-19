//! 烟火系统 — 烟花火箭实体追踪与爆炸
//!
//! 实体类型: firework_rocket=72

/// 烟花火箭数据
#[derive(Debug, Clone)]
pub struct FireworkData {
    pub entity_id: i32,
    pub pos: (f64, f64, f64),
    pub lifetime: u8,       // 剩余飞行 tick 数 (20-40)
    pub colors: Vec<u32>,   // 爆炸颜色 (RGB)
    pub has_flicker: bool,
    pub has_trail: bool,
    pub flight_duration: u8, // 1-3
}
