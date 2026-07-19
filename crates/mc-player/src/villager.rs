//! 村民交易系统 — 职业、交易表、交易逻辑、繁殖、铁傀儡
//!
//! 14 职业, 每职业 3-5 级交易


/// 交易条目
#[derive(Debug, Clone)]
pub struct TradeOffer {
    pub input_item: u32,
    pub input_count: u8,
    pub input_item2: Option<u32>,
    pub input_count2: Option<u8>,
    pub output_item: u32,
    pub output_count: u8,
    pub max_uses: u8,
    pub uses: u8,
    pub required_level: u8, // 1-5
}

/// 村民职业
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Profession {
    Farmer = 0, Librarian = 1, Blacksmith = 2, Butcher = 3, Cleric = 4,
    Armorer = 5, Weaponsmith = 6, Toolsmith = 7, Fletcher = 8,
    Shepherd = 9, Leatherworker = 10, Mason = 11, Cartographer = 12, Fisherman = 13,
}

impl Profession {
    pub fn from_id(id: i32) -> Self {
        match id {
            0 => Self::Farmer, 1 => Self::Librarian, 2 => Self::Blacksmith,
            3 => Self::Butcher, 4 => Self::Cleric, 5 => Self::Armorer,
            6 => Self::Weaponsmith, 7 => Self::Toolsmith, 8 => Self::Fletcher,
            9 => Self::Shepherd, 10 => Self::Leatherworker, 11 => Self::Mason,
            12 => Self::Cartographer, _ => Self::Fisherman,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Farmer => "Farmer", Self::Librarian => "Librarian",
            Self::Blacksmith => "Blacksmith", Self::Butcher => "Butcher",
            Self::Cleric => "Cleric", Self::Armorer => "Armorer",
            Self::Weaponsmith => "Weaponsmith", Self::Toolsmith => "Toolsmith",
            Self::Fletcher => "Fletcher", Self::Shepherd => "Shepherd",
            Self::Leatherworker => "Leatherworker", Self::Mason => "Mason",
            Self::Cartographer => "Cartographer", Self::Fisherman => "Fisherman",
        }
    }

    pub fn trades(&self) -> Vec<TradeOffer> {
        let emerald = 839u32; // minecraft:emerald
        let mut t = Vec::new();
        match self {
            Self::Farmer => {
                // wheat→emerald, emerald→bread, pumpkin→emerald, emerald→cake, emerald→golden_carrot
                t.push(TradeOffer { input_item: 809, input_count: 20, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 1, input_item2: None, input_count2: None, output_item: 810, output_count: 6, max_uses: 12, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: 124, input_count: 6, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 12, uses: 0, required_level: 2 });
                t.push(TradeOffer { input_item: emerald, input_count: 1, input_item2: None, input_count2: None, output_item: 880, output_count: 1, max_uses: 12, uses: 0, required_level: 2 });
                t.push(TradeOffer { input_item: emerald, input_count: 3, input_item2: None, input_count2: None, output_item: 871, output_count: 3, max_uses: 12, uses: 0, required_level: 3 });
            }
            Self::Librarian => {
                // paper→emerald, emerald+book→enchanted_book, emerald→bookshelf, emerald→clock
                t.push(TradeOffer { input_item: 891, input_count: 24, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 5, input_item2: Some(892), input_count2: Some(1), output_item: 1050, output_count: 1, max_uses: 12, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 1, input_item2: None, input_count2: None, output_item: 105, output_count: 1, max_uses: 12, uses: 0, required_level: 2 });
                t.push(TradeOffer { input_item: emerald, input_count: 4, input_item2: None, input_count2: None, output_item: 893, output_count: 1, max_uses: 12, uses: 0, required_level: 3 });
            }
            Self::Blacksmith | Self::Weaponsmith => {
                // coal→emerald, emerald→iron_sword, emerald→diamond_sword
                t.push(TradeOffer { input_item: 775, input_count: 15, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 3, input_item2: None, input_count2: None, output_item: 780, output_count: 1, max_uses: 12, uses: 0, required_level: 2 });
                t.push(TradeOffer { input_item: emerald, input_count: 8, input_item2: None, input_count2: None, output_item: 792, output_count: 1, max_uses: 8, uses: 0, required_level: 3 });
            }
            Self::Armorer => {
                // coal→emerald, emerald→iron_chestplate, emerald→diamond_chestplate
                t.push(TradeOffer { input_item: 775, input_count: 15, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 4, input_item2: None, input_count2: None, output_item: 820, output_count: 1, max_uses: 12, uses: 0, required_level: 2 });
                t.push(TradeOffer { input_item: emerald, input_count: 10, input_item2: None, input_count2: None, output_item: 824, output_count: 1, max_uses: 8, uses: 0, required_level: 3 });
            }
            Self::Toolsmith => {
                // coal→emerald, emerald→iron_pickaxe, emerald→diamond_pickaxe
                t.push(TradeOffer { input_item: 775, input_count: 15, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 3, input_item2: None, input_count2: None, output_item: 769, output_count: 1, max_uses: 12, uses: 0, required_level: 2 });
                t.push(TradeOffer { input_item: emerald, input_count: 8, input_item2: None, input_count2: None, output_item: 790, output_count: 1, max_uses: 8, uses: 0, required_level: 3 });
            }
            Self::Butcher => {
                // raw_chicken→emerald, raw_beef→emerald, emerald→cooked_beef, emerald→cooked_chicken
                t.push(TradeOffer { input_item: 863, input_count: 14, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: 859, input_count: 10, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 1, input_item2: None, input_count2: None, output_item: 858, output_count: 5, max_uses: 12, uses: 0, required_level: 2 });
            }
            Self::Cleric => {
                // rotten_flesh→emerald, emerald→redstone, emerald→ender_pearl, emerald→glowstone
                t.push(TradeOffer { input_item: 903, input_count: 32, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 1, input_item2: None, input_count2: None, output_item: 993, output_count: 2, max_uses: 12, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 5, input_item2: None, input_count2: None, output_item: 908, output_count: 1, max_uses: 12, uses: 0, required_level: 2 });
                t.push(TradeOffer { input_item: emerald, input_count: 3, input_item2: None, input_count2: None, output_item: 903, output_count: 1, max_uses: 12, uses: 0, required_level: 3 });
            }
            Self::Fletcher => {
                // stick→emerald, emerald→arrow, emerald→bow, string→emerald, emerald→crossbow
                t.push(TradeOffer { input_item: 794, input_count: 32, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 1, input_item2: None, input_count2: None, output_item: 774, output_count: 16, max_uses: 12, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: 838, input_count: 14, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 2 });
                t.push(TradeOffer { input_item: emerald, input_count: 2, input_item2: None, input_count2: None, output_item: 773, output_count: 1, max_uses: 8, uses: 0, required_level: 2 });
            }
            Self::Shepherd => {
                // wool→emerald, emerald→shears, emerald→white_wool, dye→emerald
                t.push(TradeOffer { input_item: 64, input_count: 18, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 2, input_item2: None, input_count2: None, output_item: 845, output_count: 1, max_uses: 12, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 1, input_item2: None, input_count2: None, output_item: 64, output_count: 1, max_uses: 12, uses: 0, required_level: 2 });
                t.push(TradeOffer { input_item: emerald, input_count: 1, input_item2: None, input_count2: None, output_item: 60, output_count: 1, max_uses: 12, uses: 0, required_level: 3 });
            }
            Self::Leatherworker => {
                // leather→emerald, emerald→leather_boots, emerald→leather_chestplate
                t.push(TradeOffer { input_item: 831, input_count: 6, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 3, input_item2: None, input_count2: None, output_item: 814, output_count: 1, max_uses: 12, uses: 0, required_level: 2 });
                t.push(TradeOffer { input_item: emerald, input_count: 5, input_item2: None, input_count2: None, output_item: 812, output_count: 1, max_uses: 12, uses: 0, required_level: 3 });
            }
            Self::Mason => {
                // clay→emerald, emerald→brick, emerald→quartz_block, stone→emerald, emerald→glazed_terracotta
                t.push(TradeOffer { input_item: 72, input_count: 10, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 1, input_item2: None, input_count2: None, output_item: 250, output_count: 10, max_uses: 12, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: 1, input_count: 20, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 2 });
                t.push(TradeOffer { input_item: emerald, input_count: 1, input_item2: None, input_count2: None, output_item: 155, output_count: 1, max_uses: 12, uses: 0, required_level: 3 });
            }
            Self::Cartographer => {
                // paper→emerald, emerald+compass→map, emerald→banner
                t.push(TradeOffer { input_item: 891, input_count: 24, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 7, input_item2: Some(894), input_count2: Some(1), output_item: 895, output_count: 1, max_uses: 12, uses: 0, required_level: 2 });
                t.push(TradeOffer { input_item: emerald, input_count: 3, input_item2: None, input_count2: None, output_item: 898, output_count: 1, max_uses: 12, uses: 0, required_level: 3 });
            }
            Self::Fisherman => {
                // string→emerald, raw_cod→emerald, emerald→cooked_cod, emerald→fishing_rod
                t.push(TradeOffer { input_item: 838, input_count: 20, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: 875, input_count: 6, input_item2: None, input_count2: None, output_item: emerald, output_count: 1, max_uses: 16, uses: 0, required_level: 1 });
                t.push(TradeOffer { input_item: emerald, input_count: 1, input_item2: None, input_count2: None, output_item: 874, output_count: 6, max_uses: 12, uses: 0, required_level: 2 });
                t.push(TradeOffer { input_item: emerald, input_count: 6, input_item2: None, input_count2: None, output_item: 844, output_count: 1, max_uses: 8, uses: 0, required_level: 3 });
            }
        }
        t
    }
}

/// 村民数据 (附在 TrackedMob 上)
#[derive(Debug, Clone)]
pub struct VillagerData {
    pub profession: u8,
    pub level: u8,
    pub xp: u32,
    pub restock_timer: u32,
    pub last_work_tick: u64,
}

impl VillagerData {
    pub fn new(profession: u8) -> Self {
        Self { profession, level: 1, xp: 0, restock_timer: 0, last_work_tick: 0 }
    }

    /// 交易 XP 增加, 可能升级
    pub fn add_xp(&mut self, amount: u32) -> bool {
        self.xp += amount;
        let needed = self.level as u32 * 10;
        if self.xp >= needed && self.level < 5 {
            self.level += 1;
            self.xp = 0;
            return true; // 升级
        }
        false
    }

    /// Tick the restock timer. Returns true if restock should occur now.
    /// Vanilla: villagers restock twice per day (~every 12000 ticks).
    /// Schedule: at work start (2000) and afternoon (9000).
    pub fn tick_restock(&mut self, current_tick: u64) -> bool {
        if self.restock_timer > 0 {
            self.restock_timer = self.restock_timer.saturating_sub(1);
            return false;
        }
        // Schedule next restock in ~12000 ticks (half a Minecraft day)
        self.restock_timer = 12000;
        // Only restock if enough time has passed since last work
        if current_tick - self.last_work_tick >= 2400 {
            self.last_work_tick = current_tick;
            return true;
        }
        false
    }

    /// Get the number of trade uses to replenish (based on level)
    pub fn restock_amount(&self) -> u8 {
        match self.level {
            1 => 2,
            2 => 2,
            3 => 3,
            4 => 3,
            _ => 4, // Master level gets more restocks
        }
    }
}

/// 获取职业的交易列表
pub fn get_trade_offers(profession_id: i32) -> Vec<TradeOffer> {
    Profession::from_id(profession_id).trades()
}

/// 验证交易是否可行 (玩家有足够物品)
pub fn can_trade(offer: &TradeOffer, player_inventory: &[Option<crate::inventory::ItemStack>]) -> bool {
    let mut needed = offer.input_count as u32;
    let mut needed2 = offer.input_count2.unwrap_or(0) as u32;
    for slot in player_inventory.iter().flatten() {
        if slot.item.id == offer.input_item { needed = needed.saturating_sub(slot.count as u32); }
        if let Some(id2) = offer.input_item2 && slot.item.id == id2 { needed2 = needed2.saturating_sub(slot.count as u32); }
    }
    needed == 0 && needed2 == 0
}
