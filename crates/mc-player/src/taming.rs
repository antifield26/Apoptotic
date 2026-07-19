//! 驯服系统 — 可驯服实体注册表、驯服逻辑
//!
//! 支持: 狼(骨头), 猫/豹猫(生鱼), 鹦鹉(种子), 马(反复骑乘)

use std::collections::HashMap;

/// 驯服数据
#[derive(Debug, Clone)]
pub struct TameData {
    pub tame_item: u32,     // 用于驯服的物品 ID
    pub tame_chance: f32,   // 每次尝试成功率
    pub follow_range: f64,  // 跟随范围
}

/// 驯服注册表
pub struct TameRegistry {
    tamables: HashMap<i32, TameData>,
}

impl Default for TameRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TameRegistry {
    pub fn new() -> Self {
        let mut tamables = HashMap::new();
        use mc_core::constants::entity_type::*;
        // Wolf (114): bones
        tamables.insert(WOLF, TameData { tame_item: 876, tame_chance: 0.333, follow_range: 20.0 });
        // Cat (115): raw cod or raw salmon
        tamables.insert(CAT, TameData { tame_item: 854, tame_chance: 0.333, follow_range: 20.0 });
        // Ocelot (116): raw cod or raw salmon
        tamables.insert(OCELOT, TameData { tame_item: 854, tame_chance: 0.333, follow_range: 20.0 });
        // Parrot (117): wheat_seeds
        tamables.insert(PARROT, TameData { tame_item: 830, tame_chance: 0.333, follow_range: 15.0 });
        // Horse (118): golden_apple or golden_carrot
        tamables.insert(HORSE, TameData { tame_item: 871, tame_chance: 0.2, follow_range: 25.0 });
        // Donkey (119): golden_apple or golden_carrot
        tamables.insert(DONKEY, TameData { tame_item: 871, tame_chance: 0.2, follow_range: 25.0 });
        // Llama (120): hay_bale
        tamables.insert(LLAMA, TameData { tame_item: 853, tame_chance: 0.3, follow_range: 20.0 });

        Self { tamables }
    }

    /// 检查实体是否可驯服
    pub fn is_tamable(&self, entity_type: i32) -> bool {
        self.tamables.contains_key(&entity_type)
    }

    /// 获取驯服数据
    pub fn get(&self, entity_type: i32) -> Option<&TameData> {
        self.tamables.get(&entity_type)
    }

    /// 执行驯服尝试 — 返回是否成功
    pub fn attempt_tame(&self, entity_type: i32, held_item: u32) -> bool {
        if let Some(data) = self.tamables.get(&entity_type)
            && (held_item == data.tame_item || Self::alt_food(entity_type, held_item)) {
                return fastrand::f32() < data.tame_chance;
            }
        false
    }

    /// 替代食物检查
    fn alt_food(entity_type: i32, held_item: u32) -> bool {
        use mc_core::constants::entity_type::*;
        match entity_type {
            t if t == CAT || t == OCELOT => held_item == 855, // cat/ocelot: also accepts salmon
            t if t == HORSE || t == DONKEY => held_item == 872, // horse/donkey: also golden_carrot
            t if t == LLAMA => held_item == 853, // llama: hay_bale
            _ => false,
        }
    }

    /// 获取跟随范围
    pub fn follow_range(&self, entity_type: i32) -> f64 {
        self.tamables.get(&entity_type).map(|d| d.follow_range).unwrap_or(20.0)
    }
}
