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

// ═══ 26.2 Villager Gossip System ═══

/// Gossip types affecting villager reputation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GossipType {
    /// Curing a zombie villager (+20 reputation, permanent decay)
    MajorPositive,
    /// Trading with a villager (+1 reputation, short decay)
    MinorPositive,
    /// Trading (+1, spread to nearby villagers)
    Trade,
    /// Attacking a villager (-1 reputation)
    MinorNegative,
    /// Killing a villager (-25 reputation, permanent decay)
    MajorNegative,
}

impl GossipType {
    pub fn base_value(&self) -> i32 {
        match self {
            Self::MajorPositive => 20,
            Self::MinorPositive => 1,
            Self::Trade => 1,
            Self::MinorNegative => -1,
            Self::MajorNegative => -25,
        }
    }

    pub fn decay_ticks(&self) -> u32 {
        match self {
            Self::MajorPositive => 24000,  // 1 MC day
            Self::MinorPositive => 6000,   // 5 min
            Self::Trade => 1200,            // 1 min
            Self::MinorNegative => 6000,
            Self::MajorNegative => 24000,
        }
    }

    pub fn max_value(&self) -> i32 {
        match self {
            Self::MajorPositive => 100,
            Self::Trade => 200,
            _ => 25,
        }
    }
}

/// A single gossip entry — villager's opinion about a player
#[derive(Debug, Clone)]
pub struct GossipEntry {
    pub target: uuid::Uuid,   // the player this gossip is about
    pub gossip_type: GossipType,
    pub value: i32,
    pub age_ticks: u32,       // time since gossip was created
}

/// Per-villager gossip storage
#[derive(Debug, Clone)]
pub struct VillagerGossip {
    pub entries: Vec<GossipEntry>,
}

impl VillagerGossip {
    pub fn new() -> Self { Self { entries: Vec::new() } }

    /// Add a gossip entry, merging with existing entries of the same type+target
    pub fn add(&mut self, target: uuid::Uuid, gossip_type: GossipType) {
        let base = gossip_type.base_value();
        let max_val = gossip_type.max_value();
        // Find existing entry for same target+type
        if let Some(entry) = self.entries.iter_mut()
            .find(|e| e.target == target && e.gossip_type == gossip_type) {
            entry.value = (entry.value + base).min(max_val);
            entry.age_ticks = 0;
        } else {
            self.entries.push(GossipEntry {
                target, gossip_type, value: base.max(0).min(max_val), age_ticks: 0,
            });
        }
    }

    /// Tick gossip aging — remove expired entries
    pub fn tick(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.age_ticks = entry.age_ticks.saturating_add(1);
        }
        self.entries.retain(|e| {
            let max_age = e.gossip_type.decay_ticks() * 2;
            e.age_ticks < max_age && e.value.abs() > 2
        });
    }

    /// Get total reputation for a player (sum of all gossip values)
    pub fn reputation_for(&self, target: &uuid::Uuid) -> i32 {
        self.entries.iter()
            .filter(|e| &e.target == target)
            .map(|e| e.value * if e.gossip_type == GossipType::Trade { 1 } else { 1 })
            .sum()
    }

    /// Get the highest priority gossip (for price modification)
    pub fn trade_discount(&self, target: &uuid::Uuid) -> f64 {
        let rep = self.reputation_for(target);
        if rep > 0 {
            (rep as f64 * 0.01).min(0.3) // up to 30% discount
        } else if rep < 0 {
            (rep as f64 * 0.02).max(-0.5) // up to 50% price increase
        } else {
            0.0
        }
    }

    /// Spread gossip to nearby villager (called during villager interaction)
    pub fn spread_to(&self, other: &mut VillagerGossip) {
        for entry in &self.entries {
            if entry.gossip_type == GossipType::Trade
                || entry.gossip_type == GossipType::MinorPositive {
                other.add(entry.target, GossipType::Trade);
            }
        }
    }
}

/// Global village gossip manager (tracks reputation across all villagers)
pub struct GossipManager {
    /// villager entity_id → gossip
    pub villager_gossips: dashmap::DashMap<i32, VillagerGossip>,
}

impl GossipManager {
    pub fn new() -> Self { Self { villager_gossips: dashmap::DashMap::new() } }

    pub fn get_or_create(&self, entity_id: i32) -> dashmap::mapref::one::RefMut<'_, i32, VillagerGossip> {
        self.villager_gossips.entry(entity_id).or_insert_with(VillagerGossip::new)
    }

    pub fn tick(&self) {
        for mut entry in self.villager_gossips.iter_mut() {
            entry.value_mut().tick();
        }
    }
}

/// Global gossip manager singleton
pub static GLOBAL_GOSSIP: std::sync::LazyLock<GossipManager> =
    std::sync::LazyLock::new(GossipManager::new);

/// Record a trade with a villager — adds positive gossip
pub fn record_trade(villager_eid: i32, player_uuid: uuid::Uuid) {
    let mut gossip = GLOBAL_GOSSIP.get_or_create(villager_eid);
    gossip.add(player_uuid, GossipType::Trade);
    gossip.add(player_uuid, GossipType::MinorPositive);
}

/// Record attacking a villager — adds negative gossip
pub fn record_villager_hurt(villager_eid: i32, player_uuid: uuid::Uuid) {
    let mut gossip = GLOBAL_GOSSIP.get_or_create(villager_eid);
    gossip.add(player_uuid, GossipType::MinorNegative);
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
