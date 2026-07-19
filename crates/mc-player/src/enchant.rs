//! 附魔引擎 — 附魔台逻辑、附魔选择、等级计算
//!
//! 22+ 附魔, 冲突检测, 宝藏/非宝藏标记

use std::collections::HashSet;

/// 附魔条目
#[derive(Debug, Clone)]
pub struct EnchantmentEntry {
    pub name: String,
    pub max_level: u8,
    pub category: String,
    pub conflicts: Vec<String>,  // 互斥的其他附魔名
    pub is_treasure: bool,       // 宝藏附魔 (不可从附魔台获取)
}

/// 附魔注册表
pub struct EnchantmentRegistry {
    enchants: Vec<EnchantmentEntry>,
}

impl Default for EnchantmentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl EnchantmentRegistry {
    pub fn new() -> Self {
        let mut reg = Self { enchants: Vec::new() };
        reg.init();
        reg
    }

    fn add(&mut self, name: &str, max: u8, cat: &str, conflicts: Vec<&str>, treasure: bool) {
        self.enchants.push(EnchantmentEntry {
            name: name.into(), max_level: max, category: cat.into(),
            conflicts: conflicts.iter().map(|s| s.to_string()).collect(),
            is_treasure: treasure,
        });
    }

    fn init(&mut self) {
        // Armor
        self.add("protection", 4, "armor", vec!["fire_protection","blast_protection","projectile_protection"], false);
        self.add("fire_protection", 4, "armor", vec!["protection","blast_protection","projectile_protection"], false);
        self.add("blast_protection", 4, "armor", vec!["protection","fire_protection","projectile_protection"], false);
        self.add("projectile_protection", 4, "armor", vec!["protection","fire_protection","blast_protection"], false);
        self.add("thorns", 3, "armor", vec![], false);
        self.add("depth_strider", 3, "armor", vec!["frost_walker"], false);
        self.add("frost_walker", 2, "armor", vec!["depth_strider"], true);
        self.add("respiration", 3, "armor", vec![], false);
        self.add("aqua_affinity", 1, "armor", vec![], false);

        // Weapon
        self.add("sharpness", 5, "weapon", vec!["smite","bane_of_arthropods"], false);
        self.add("smite", 5, "weapon", vec!["sharpness","bane_of_arthropods"], false);
        self.add("bane_of_arthropods", 5, "weapon", vec!["sharpness","smite"], false);
        self.add("fire_aspect", 2, "weapon", vec![], false);
        self.add("knockback", 2, "weapon", vec![], false);
        self.add("looting", 3, "weapon", vec![], false);
        self.add("sweeping_edge", 3, "weapon", vec![], false);

        // Tool
        self.add("efficiency", 5, "tool", vec![], false);
        self.add("fortune", 3, "tool", vec!["silk_touch"], false);
        self.add("silk_touch", 1, "tool", vec!["fortune"], false);

        // Bow
        self.add("power", 5, "bow", vec![], false);
        self.add("infinity", 1, "bow", vec![], false);
        self.add("flame", 1, "bow", vec![], false);
        self.add("punch", 2, "bow", vec![], false);

        // Crossbow
        self.add("piercing", 4, "crossbow", vec!["multishot"], false);
        self.add("multishot", 1, "crossbow", vec!["piercing"], false);
        self.add("quick_charge", 3, "crossbow", vec![], false);

        // Trident
        self.add("loyalty", 3, "trident", vec!["riptide"], false);
        self.add("riptide", 3, "trident", vec!["loyalty","channeling"], false);
        self.add("channeling", 1, "trident", vec!["riptide"], false);
        self.add("impaling", 5, "trident", vec![], false);

        // All
        self.add("unbreaking", 3, "all", vec![], false);
        self.add("mending", 1, "all", vec![], true);

        // Boots
        self.add("feather_falling", 4, "armor", vec![], false);
        self.add("soul_speed", 3, "armor", vec![], true);
        self.add("swift_sneak", 3, "armor", vec![], true);

        // Fishing
        self.add("luck_of_the_sea", 3, "fishing_rod", vec![], false);
        self.add("lure", 3, "fishing_rod", vec![], false);
        // 1.21 Mace
        self.add("wind_burst", 3, "mace", vec![], true);
        self.add("density", 5, "mace", vec![], true);
        self.add("breach", 4, "mace", vec![], true);
        // Curses
        self.add("vanishing_curse", 1, "all", vec![], true);
        self.add("binding_curse", 1, "armor", vec![], true);
    }

    /// 根据物品类型和附魔等级随机选择附魔
    pub fn roll_enchantment(&self, item_id: u32, level: i32) -> Vec<(String, u8)> {
        let category = item_category(item_id);
        let mut chosen_names: HashSet<String> = HashSet::new();
        let mut result = Vec::new();

        let count = 1 + (level / 15).min(2) as usize;
        let candidates: Vec<&EnchantmentEntry> = self.enchants.iter()
            .filter(|e| (e.category == category || e.category == "all") && !e.is_treasure)
            .collect();

        for _ in 0..count {
            if candidates.is_empty() { break; }
            // 排除冲突和已选
            let available: Vec<&&EnchantmentEntry> = candidates.iter()
                .filter(|e| !chosen_names.contains(&e.name)
                    && e.conflicts.iter().all(|c| !chosen_names.contains(c)))
                .collect();
            if available.is_empty() { break; }

            let idx = (level as u32 as usize * 13 + item_id as usize * 7) % available.len();
            let enchant = available[idx];
            let lvl = ((level as u32 % enchant.max_level as u32) + 1).min(enchant.max_level as u32) as u8;
            chosen_names.insert(enchant.name.clone());
            result.push((enchant.name.clone(), lvl));
        }
        result
    }

    pub fn bookshelf_level(bookshelf_count: u8) -> i32 {
        bookshelf_count.min(15) as i32 * 2
    }

    pub fn xp_cost(enchant_level: i32) -> i32 {
        1 + enchant_level / 5
    }

    pub fn lapis_cost(enchant_level: i32) -> u8 {
        1 + (enchant_level / 15) as u8
    }

    /// 按名称查找附魔
    pub fn find(&self, name: &str) -> Option<&EnchantmentEntry> {
        self.enchants.iter().find(|e| e.name == name)
    }
}

/// 从物品 NBT 中解析附魔: "sharpness(5) fire_aspect(2)" → HashMap
pub fn parse_item_enchants(nbt: &Option<Vec<u8>>) -> std::collections::HashMap<String, u8> {
    let mut map = std::collections::HashMap::new();
    if let Some(data) = nbt
        && let Ok(text) = std::str::from_utf8(data) {
            for part in text.split_whitespace() {
                if let Some(open) = part.find('(')
                    && let Some(close) = part.find(')') {
                        let name = &part[..open];
                        if let Ok(level) = part[open+1..close].parse::<u8>() {
                            map.insert(name.to_string(), level);
                        }
                    }
            }
        }
    map
}

/// 检查物品是否有指定附魔
pub fn has_enchant(nbt: &Option<Vec<u8>>, name: &str) -> bool {
    parse_item_enchants(nbt).contains_key(name)
}

/// 获取物品上指定附魔的等级 (0 = 无此附魔)
pub fn enchant_level(nbt: &Option<Vec<u8>>, name: &str) -> u8 {
    parse_item_enchants(nbt).get(name).copied().unwrap_or(0)
}

fn item_category(item_id: u32) -> &'static str {
    match item_id {
        785 | 786 | 787 | 788 | 789 | 792 => "weapon",
        690..=695 => "weapon",
        796..=801 => "tool",
        802..=807 => "tool",
        810..=849 => "armor",
        773 => "bow",
        941 => "crossbow",
        940 => "trident",
        _ => "all",
    }
}
