//! 繁殖系统 — 动物繁殖/喂养注册表

use std::collections::HashMap;

/// 繁殖数据: 每种生物对应的喂养物品
#[derive(Debug, Clone)]
pub struct BreedData {
    pub breed_item: u32,     // 用于触发繁殖的物品 ID
    pub cooldown_ticks: u16, // 繁殖后冷却时间
}

/// 繁殖注册表
pub struct BreedRegistry {
    breeds: HashMap<i32, BreedData>,
}

impl Default for BreedRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BreedRegistry {
    pub fn new() -> Self {
        let mut breeds = HashMap::new();
        use mc_core::constants::entity_type::*;
        // Cow (11): wheat
        breeds.insert(COW, BreedData { breed_item: 809, cooldown_ticks: 6000 }); // wheat
        // Sheep (14): wheat
        breeds.insert(SHEEP, BreedData { breed_item: 809, cooldown_ticks: 6000 });
        // Pig (12): carrot
        breeds.insert(PIG, BreedData { breed_item: 870, cooldown_ticks: 6000 }); // carrot
        // Chicken (13): wheat_seeds
        breeds.insert(CHICKEN, BreedData { breed_item: 830, cooldown_ticks: 6000 }); // wheat_seeds
        // Rabbit (15): carrot
        breeds.insert(RABBIT, BreedData { breed_item: 870, cooldown_ticks: 6000 });
        // Horse (118): golden_apple
        breeds.insert(HORSE, BreedData { breed_item: 871, cooldown_ticks: 6000 });
        // Donkey (119): golden_apple
        breeds.insert(DONKEY, BreedData { breed_item: 871, cooldown_ticks: 6000 });
        // Wolf (114): cooked_beef
        breeds.insert(WOLF, BreedData { breed_item: 858, cooldown_ticks: 6000 });
        // Cat (115): raw_cod
        breeds.insert(CAT, BreedData { breed_item: 854, cooldown_ticks: 6000 });
        // Ocelot (116): raw_cod
        breeds.insert(OCELOT, BreedData { breed_item: 854, cooldown_ticks: 6000 });
        // Llama (120): hay_block
        breeds.insert(LLAMA, BreedData { breed_item: 179, cooldown_ticks: 6000 });
        // Turtle (19): seagrass
        breeds.insert(TURTLE, BreedData { breed_item: 897, cooldown_ticks: 6000 });
        // Fox (44): sweet_berries (approximate)
        breeds.insert(FOX, BreedData { breed_item: 865, cooldown_ticks: 6000 });
        // Bee (65): any flower
        breeds.insert(BEE, BreedData { breed_item: 812, cooldown_ticks: 6000 }); // dandelion
        // Frog (106): slimeball
        breeds.insert(FROG, BreedData { breed_item: 876, cooldown_ticks: 6000 }); // slime_ball
        // Hoglin (58): crimson_fungus
        breeds.insert(HOGLIN, BreedData { breed_item: 830, cooldown_ticks: 6000 });

        Self { breeds }
    }

    /// 检查生物是否可繁殖
    pub fn is_breedable(&self, mob_type: i32) -> bool {
        self.breeds.contains_key(&mob_type)
    }

    /// 检查物品是否是某种生物的繁殖食物
    pub fn is_breed_item(&self, mob_type: i32, held_item: u32) -> bool {
        self.breeds.get(&mob_type)
            .map(|d| d.breed_item == held_item)
            .unwrap_or(false)
    }

    /// 获取繁殖冷却时间
    pub fn cooldown(&self, mob_type: i32) -> u16 {
        self.breeds.get(&mob_type)
            .map(|d| d.cooldown_ticks)
            .unwrap_or(6000)
    }
}
