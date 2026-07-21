//! 成就/进度系统 — 定义进度树、追踪玩家进度、触发检测
//!
//! 23 个预设进度: story×14, nether×3, end×2, husbandry×3, adventure×3

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// 进度框架类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FrameType {
    Task,
    Goal,
    Challenge,
}

/// 进度条件
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Criterion {
    InventoryChanged { item_id: u32 },
    ItemUsed { item_id: u32 },
    EntityKilled { entity_type: i32 },
    LocationChanged { dimension: String },
    EnchantedItem,
    TamedAnimal,
    FishCaught,
    BredAnimals,
    PlacedBlock { block_id: u32 },
    BrewedPotion,
    ConstructedBeacon,
    ConsumeItem { item_id: u32 },
    VillagerTrade,
    CuredZombieVillager,
    ShotCrossbow,
    RaidWin,
    /// 26.2: Sulfur Cube absorbs TNT
    UhOh,
}

/// 进度定义
#[derive(Debug, Clone)]
pub struct Advancement {
    pub id: String,
    pub parent: Option<String>,
    pub title: String,
    pub description: String,
    pub icon_item: u32,
    pub frame: FrameType,
    pub criteria: Vec<Criterion>,
}

/// 进度注册表
pub struct AdvancementRegistry {
    pub advancements: HashMap<String, Advancement>,
    pub roots: Vec<String>,
}

fn mk(id: &str, parent: &str, title: &str, desc: &str, icon: u32, criteria: Vec<Criterion>) -> (String, Advancement) {
    (id.to_string(), Advancement {
        id: id.to_string(),
        parent: Some(parent.to_string()),
        title: title.to_string(),
        description: desc.to_string(),
        icon_item: icon,
        frame: FrameType::Task,
        criteria,
    })
}

impl Default for AdvancementRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvancementRegistry {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        let mut roots = Vec::new();

        let story_root = Advancement {
            id: "minecraft:story/root".into(),
            parent: None,
            title: "Minecraft".into(),
            description: "The heart and story of the game".into(),
            icon_item: 8, frame: FrameType::Task,
            criteria: vec![],
        };
        roots.push(story_root.id.clone());
        map.insert(story_root.id.clone(), story_root);

        for (k, v) in [
            mk("minecraft:story/mine_stone", "minecraft:story/root", "Stone Age", "Mine stone with your pickaxe", 1, vec![Criterion::InventoryChanged { item_id: 1 }]),
            mk("minecraft:story/upgrade_tools", "minecraft:story/mine_stone", "Getting an Upgrade", "Craft a stone pickaxe", 774, vec![Criterion::InventoryChanged { item_id: 774 }]),
            mk("minecraft:story/smelt_iron", "minecraft:story/upgrade_tools", "Acquire Hardware", "Smelt an iron ingot", 996, vec![Criterion::InventoryChanged { item_id: 996 }]),
            mk("minecraft:story/obtain_armor", "minecraft:story/smelt_iron", "Suit Up", "Protect yourself with a piece of iron armor", 787, vec![Criterion::InventoryChanged { item_id: 787 }]),
            mk("minecraft:story/lava_bucket", "minecraft:story/smelt_iron", "Hot Stuff", "Fill a bucket with lava", 87, vec![Criterion::InventoryChanged { item_id: 87 }]),
            mk("minecraft:story/iron_tools", "minecraft:story/smelt_iron", "Isn't It Iron Pick", "Upgrade your pickaxe", 775, vec![Criterion::InventoryChanged { item_id: 775 }]),
            mk("minecraft:story/deflect_arrow", "minecraft:story/obtain_armor", "Not Today", "Block a projectile with a shield", 845, vec![Criterion::ItemUsed { item_id: 845 }]),
            mk("minecraft:story/form_obsidian", "minecraft:story/lava_bucket", "Ice Bucket Challenge", "Obtain obsidian", 49, vec![Criterion::InventoryChanged { item_id: 49 }]),
            mk("minecraft:story/mine_diamond", "minecraft:story/iron_tools", "Diamonds!", "Acquire diamonds", 57, vec![Criterion::InventoryChanged { item_id: 57 }]),
            mk("minecraft:story/enter_nether", "minecraft:story/form_obsidian", "We Need to Go Deeper", "Enter the Nether dimension", 49, vec![Criterion::LocationChanged { dimension: "the_nether".into() }]),
            mk("minecraft:story/enchant_item", "minecraft:story/mine_diamond", "Enchanter", "Enchant an item at an Enchanting Table", 151, vec![Criterion::EnchantedItem]),
            mk("minecraft:story/cure_zombie_villager", "minecraft:story/enter_nether", "Zombie Doctor", "Weaken and then cure a Zombie Villager", 1033, vec![Criterion::ItemUsed { item_id: 1033 }]),
            mk("minecraft:story/enter_end", "minecraft:story/enter_nether", "The End?", "Enter the End dimension", 122, vec![Criterion::LocationChanged { dimension: "the_end".into() }]),
            mk("minecraft:nether/root", "minecraft:story/enter_nether", "Nether", "Bring summer clothes", 88, vec![Criterion::LocationChanged { dimension: "the_nether".into() }]),
            mk("minecraft:nether/obtain_blaze_rod", "minecraft:nether/root", "Into Fire", "Relieve a Blaze of its rod", 903, vec![Criterion::InventoryChanged { item_id: 903 }]),
            mk("minecraft:nether/brew_potion", "minecraft:nether/obtain_blaze_rod", "Local Brewery", "Brew a potion", 1004, vec![Criterion::InventoryChanged { item_id: 1004 }]),
            mk("minecraft:end/root", "minecraft:story/enter_end", "The End", "Or the beginning?", 122, vec![Criterion::LocationChanged { dimension: "the_end".into() }]),
            mk("minecraft:end/kill_dragon", "minecraft:end/root", "Free the End", "Kill the Ender Dragon", 122, vec![Criterion::EntityKilled { entity_type: 53 }]),
            mk("minecraft:husbandry/root", "minecraft:story/root", "Husbandry", "The world is full of friends and food", 829, vec![Criterion::InventoryChanged { item_id: 829 }]),
            mk("minecraft:husbandry/tame_animal", "minecraft:husbandry/root", "Best Friends Forever", "Tame an animal", 836, vec![Criterion::TamedAnimal]),
            // 26.2 Chaos Cubed: "Uh Oh" — Sulfur Cube absorbs TNT
            mk("minecraft:husbandry/uh_oh", "minecraft:husbandry/root", "Uh Oh", "Have a Sulfur Cube absorb a TNT block", 25, vec![Criterion::UhOh]),
            mk("minecraft:husbandry/fish_fish", "minecraft:husbandry/root", "Fishy Business", "Catch a fish", 844, vec![Criterion::FishCaught]),
            mk("minecraft:adventure/root", "minecraft:story/root", "Adventure", "Adventure, exploration and combat", 940, vec![Criterion::EntityKilled { entity_type: 36 }]),
            mk("minecraft:adventure/kill_a_mob", "minecraft:adventure/root", "Monster Hunter", "Kill any hostile monster", 785, vec![Criterion::EntityKilled { entity_type: 0 }]),
            mk("minecraft:adventure/shoot_arrow", "minecraft:adventure/root", "Take Aim", "Shoot something with an arrow", 774, vec![Criterion::ItemUsed { item_id: 773 }]),
            mk("minecraft:adventure/sniper_duel", "minecraft:adventure/shoot_arrow", "Sniper Duel", "Kill a Skeleton from at least 50 meters", 774, vec![Criterion::EntityKilled { entity_type: 37 }]),
            mk("minecraft:adventure/ol_betsy", "minecraft:adventure/shoot_arrow", "Ol' Betsy", "Shoot a crossbow", 352, vec![Criterion::ShotCrossbow]),
            mk("minecraft:adventure/hero_of_the_village", "minecraft:adventure/root", "Hero of the Village", "Defend a village from a raid", 134, vec![Criterion::RaidWin]),
            mk("minecraft:nether/brew_potion", "minecraft:nether/obtain_blaze_rod", "Local Brewery", "Brew a potion", 374, vec![Criterion::BrewedPotion]),
            mk("minecraft:nether/create_beacon", "minecraft:nether/obtain_blaze_rod", "Bring Home the Beacon", "Construct and activate a beacon", 138, vec![Criterion::ConstructedBeacon]),
            mk("minecraft:husbandry/trade", "minecraft:husbandry/root", "What a Deal!", "Trade with a Villager", 134, vec![Criterion::VillagerTrade]),
            mk("minecraft:husbandry/cure_zombie_villager", "minecraft:husbandry/root", "Zombie Doctor", "Cure a Zombie Villager", 373, vec![Criterion::CuredZombieVillager]),
            mk("minecraft:husbandry/balanced_diet", "minecraft:husbandry/root", "A Balanced Diet", "Eat everything that is edible", 357, vec![Criterion::ConsumeItem { item_id: 0 }]),
        ] {
            map.insert(k, v);
        }

        Self { advancements: map, roots }
    }

    /// 获取所有进度
    pub fn all(&self) -> Vec<&Advancement> {
        self.advancements.values().collect()
    }

    /// 获取根进度
    pub fn roots(&self) -> &[String] {
        &self.roots
    }

    /// 获取指定进度
    pub fn get(&self, id: &str) -> Option<&Advancement> {
        self.advancements.get(id)
    }
}

/// 玩家进度追踪器
pub struct AdvancementTracker {
    /// 每玩家已完成的进度 ID 集合
    pub completed: HashMap<Uuid, HashSet<String>>,
    /// 每玩家已完成的判据
    pub criteria_done: HashMap<Uuid, HashSet<Criterion>>,
}

impl Default for AdvancementTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvancementTracker {
    pub fn new() -> Self {
        Self {
            completed: HashMap::new(),
            criteria_done: HashMap::new(),
        }
    }

    /// 检查并授予匹配的进度
    pub fn check_criterion(&mut self, uuid: &Uuid, criterion: &Criterion, registry: &AdvancementRegistry) -> Vec<String> {
        let criteria = self.criteria_done.entry(*uuid).or_default();
        criteria.insert(criterion.clone());

        let completed_set = self.completed.entry(*uuid).or_default();
        let mut newly_completed = Vec::new();

        for adv in registry.all() {
            if completed_set.contains(&adv.id) { continue; }
            if adv.criteria.is_empty() {
                completed_set.insert(adv.id.clone());
                newly_completed.push(adv.id.clone());
            } else if adv.criteria.iter().all(|c| criteria.contains(c)) {
                let parent_ok = match &adv.parent {
                    Some(parent_id) => completed_set.contains(parent_id),
                    None => true,
                };
                if parent_ok {
                    completed_set.insert(adv.id.clone());
                    newly_completed.push(adv.id.clone());
                }
            }
        }

        newly_completed
    }

    /// 直接授予进度
    pub fn grant(&mut self, uuid: &Uuid, adv_id: &str) -> bool {
        let completed = self.completed.entry(*uuid).or_default();
        completed.insert(adv_id.to_string())
    }

    /// 获取玩家已完成的进度
    pub fn get_completed(&self, uuid: &Uuid) -> Vec<String> {
        self.completed.get(uuid).map(|s| s.iter().cloned().collect()).unwrap_or_default()
    }
}
