//! 钓鱼系统 — 钓鱼浮漂实体、咬钩计时、战利品表
//!
//! 支持: 基础钓鱼 + Luck of the Sea / Lure 附魔影响

use mc_core::block::BlockState;
use crate::inventory::ItemStack;
use std::collections::HashMap;

fn simple_item(id: u32, count: u8) -> ItemStack {
    ItemStack::new(BlockState::new(id), count)
}

/// 玩家钓鱼状态
#[derive(Debug, Clone)]
pub struct FishingState {
    pub bobber_entity_id: i32,
    pub wait_ticks: u32,
    pub bites: bool,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// 战利品类别
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LootCategory {
    Fish,
    Junk,
    Treasure,
}

/// 战利品条目
#[derive(Debug, Clone)]
pub struct FishingLoot {
    pub item: u32,
    pub weight: u32,
    pub min_count: u8,
    pub max_count: u8,
    pub category: LootCategory,
}

/// 钓鱼管理器
pub struct FishingManager {
    active_bobbers: HashMap<i32, FishingState>,
    loot_table: Vec<FishingLoot>,
}

impl Default for FishingManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FishingManager {
    pub fn new() -> Self {
        let loot_table = vec![
            // 鱼 (85% 总权重)
            FishingLoot { item: 854, weight: 60, min_count: 1, max_count: 1, category: LootCategory::Fish }, // cod
            FishingLoot { item: 855, weight: 25, min_count: 1, max_count: 1, category: LootCategory::Fish }, // salmon
            FishingLoot { item: 853, weight: 13, min_count: 1, max_count: 1, category: LootCategory::Fish }, // pufferfish
            FishingLoot { item: 82,  weight: 2,  min_count: 1, max_count: 1, category: LootCategory::Fish }, // tropical_fish (seagrass proxy)

            // 垃圾 (10%)
            FishingLoot { item: 1051, weight: 15, min_count: 1, max_count: 1, category: LootCategory::Junk }, // lily_pad
            FishingLoot { item: 844, weight: 10, min_count: 1, max_count: 1, category: LootCategory::Junk }, // fishing_rod (damaged)
            FishingLoot { item: 835, weight: 10, min_count: 1, max_count: 1, category: LootCategory::Junk }, // rotten_flesh
            FishingLoot { item: 836, weight: 10, min_count: 1, max_count: 1, category: LootCategory::Junk }, // bone
            FishingLoot { item: 81,  weight: 5,  min_count: 1, max_count: 3, category: LootCategory::Junk }, // string (grass proxy)
            FishingLoot { item: 829, weight: 5,  min_count: 1, max_count: 2, category: LootCategory::Junk }, // wheat (stick proxy)
            FishingLoot { item: 841, weight: 2,  min_count: 1, max_count: 1, category: LootCategory::Junk }, // leather_boots (glass_bottle proxy)

            // 宝藏 (5%)
            FishingLoot { item: 1045, weight: 5, min_count: 1, max_count: 1, category: LootCategory::Treasure }, // enchanted_book
            FishingLoot { item: 1043, weight: 4, min_count: 1, max_count: 1, category: LootCategory::Treasure }, // saddle
            FishingLoot { item: 1042, weight: 4, min_count: 1, max_count: 1, category: LootCategory::Treasure }, // name_tag
            FishingLoot { item: 1000, weight: 2, min_count: 1, max_count: 1, category: LootCategory::Treasure }, // nautilus_shell (prismarine_crystals)
            FishingLoot { item: 831,  weight: 2, min_count: 1, max_count: 1, category: LootCategory::Treasure }, // leather (bowl proxy)
        ];

        Self {
            active_bobbers: HashMap::new(),
            loot_table,
        }
    }

    /// 创建新的钓鱼浮漂
    pub fn cast(&mut self, entity_id: i32, x: f64, y: f64, z: f64, lure_level: u8) -> FishingState {
        // 基础等待: 100-600 ticks, Lure 每级减少 100 tick
        let base_wait = 100 + (fastrand::u32(100..600));
        let wait = base_wait.saturating_sub(lure_level as u32 * 100).max(20);

        let state = FishingState {
            bobber_entity_id: entity_id,
            wait_ticks: wait,
            bites: false,
            x, y, z,
        };
        self.active_bobbers.insert(entity_id, state.clone());
        state
    }

    /// 收线 — 返回战利品
    pub fn reel_in(&mut self, entity_id: i32, luck_level: u8) -> (Option<ItemStack>, bool) {
        let state = match self.active_bobbers.remove(&entity_id) {
            Some(s) => s,
            None => return (None, false),
        };

        if !state.bites {
            return (None, false); // 未咬钩, 无战利品
        }

        let loot = self.roll_loot(luck_level);
        (Some(loot), true)
    }

    /// 取消钓鱼 (玩家切换物品)
    pub fn cancel(&mut self, entity_id: i32) {
        self.active_bobbers.remove(&entity_id);
    }

    /// 检查浮漂是否属于某玩家
    pub fn get_bobber(&self, entity_id: i32) -> Option<&FishingState> {
        self.active_bobbers.get(&entity_id)
    }

    /// 每 20 tick 更新所有浮漂
    pub fn tick(&mut self) -> Vec<FishingTickEvent> {
        let mut events = Vec::new();
        let mut to_bite = Vec::new();
        let to_expire = Vec::new();

        for (eid, state) in self.active_bobbers.iter_mut() {
            if state.bites { continue; }
            if state.wait_ticks > 0 {
                state.wait_ticks -= 1;
                if state.wait_ticks == 0 {
                    state.bites = true;
                    to_bite.push(*eid);
                }
            } else {
                // 咬钩后等待 400 tick 自动超时
                // (此处简化: 咬钩后持续等待)
            }
        }

        // 超时回收 (>1200 ticks total 未收线)
        for eid in to_bite {
            events.push(FishingTickEvent::Bite { entity_id: eid });
        }
        for eid in to_expire {
            self.active_bobbers.remove(&eid);
            events.push(FishingTickEvent::Expire { entity_id: eid });
        }
        events
    }

    /// 随机掷出战利品
    fn roll_loot(&self, luck_level: u8) -> ItemStack {
        let mut total_weight = 0u32;
        let mut candidates: Vec<&FishingLoot> = Vec::new();

        for loot in &self.loot_table {
            let weight = match loot.category {
                LootCategory::Treasure => loot.weight + luck_level as u32 * 2,
                LootCategory::Junk => loot.weight.saturating_sub(luck_level as u32),
                _ => loot.weight,
            };
            if weight > 0 {
                total_weight += weight;
                candidates.push(loot);
            }
        }

        if total_weight == 0 {
            return simple_item(854, 1); // 默认鳕鱼
        }

        let mut roll = fastrand::u32(..) % total_weight;
        for loot in &candidates {
            let w = match loot.category {
                LootCategory::Treasure => loot.weight + luck_level as u32 * 2,
                LootCategory::Junk => loot.weight.saturating_sub(luck_level as u32),
                _ => loot.weight,
            };
            if roll < w {
                let count = if loot.min_count == loot.max_count {
                    loot.min_count
                } else {
                    loot.min_count + (fastrand::u32(..) % (loot.max_count - loot.min_count + 1) as u32) as u8
                };
                return simple_item(loot.item, count as u8);
            }
            roll -= w;
        }

        simple_item(854, 1) // fallback cod
    }
}

/// 钓鱼 tick 事件
#[derive(Debug, Clone)]
pub enum FishingTickEvent {
    Bite { entity_id: i32 },
    Expire { entity_id: i32 },
}
