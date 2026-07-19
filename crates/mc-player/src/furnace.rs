//! 熔炉管理器 — 燃料、熔炼进度、配方

use std::collections::HashMap;

/// 熔炉状态
#[derive(Debug, Clone)]
pub struct FurnaceData {
    pub input_item: Option<u32>,   // 输入槽物品 ID
    pub fuel_item: Option<u32>,    // 燃料槽物品 ID
    pub output_item: Option<u32>,  // 输出槽物品 ID
    pub output_count: u8,
    pub fuel_ticks: u32,   // 剩余燃烧 tick
    pub progress: u32,      // 熔炼进度 (0..200)
    pub max_progress: u32,  // 总熔炼 tick (200 = 10 秒)
}

/// 燃料注册表: 物品 ID → 燃烧 tick (1 tick = 0.05s)
fn fuel_ticks(item_id: u32) -> Option<u32> {
    match item_id {
        775 => Some(1600), // coal
        776 => Some(1600), // charcoal
        34..=40 => Some(300), // logs
        13..=20 => Some(300), // planks
        794 => Some(100),  // stick
        113 => Some(300),  // crafting_table
        105 => Some(300),  // bookshelf
        85 => Some(100),   // wool
        966 => Some(200),  // bamboo
        118 => Some(1600),  // coal_block
        119 => Some(16000), // lava_bucket
        120 => Some(1600),  // dried_kelp_block
        121 => Some(100),   // sapling
        _ => None,
    }
}

/// 熔炼配方: 输入 ID → (输出 ID, 数量)
fn smelt_result(input_id: u32) -> Option<(u32, u8)> {
    match input_id {
        // Ores → Ingots
        774 => Some((778, 1)),  // iron_ore → iron_ingot
        775 => Some((778, 1)),  // raw_iron → iron_ingot
        776 => Some((779, 1)),  // gold_ore → gold_ingot
        777 => Some((779, 1)),  // raw_gold → gold_ingot
        34 => Some((776, 1)),   // oak_log → charcoal
        35 => Some((776, 1)),   // spruce_log → charcoal
        36 => Some((776, 1)),   // birch_log → charcoal
        37 => Some((776, 1)),   // jungle_log → charcoal
        38 => Some((776, 1)),   // acacia_log → charcoal
        // Food
        32 => Some((33, 1)),    // raw_beef → steak (approximate IDs)
        859 => Some((858, 1)),  // raw_beef → cooked_beef
        861 => Some((860, 1)),  // raw_porkchop → cooked_porkchop
        863 => Some((862, 1)),  // raw_chicken → cooked_chicken
        865 => Some((864, 1)),  // raw_mutton → cooked_mutton
        875 => Some((874, 1)),  // raw_cod → cooked_cod
        898 => Some((897, 1)),  // raw_salmon → cooked_salmon
        872 => Some((873, 1)),  // potato → baked_potato
        839 => Some((840, 1)),  // raw_rabbit → cooked_rabbit
        // Blocks
        24 => Some((66, 1)),    // sand → glass
        12 => Some((1, 1)),     // cobblestone → stone
        71 => Some((72, 1)),    // clay → brick (approximate)
        120 => Some((121, 1)),  // netherrack → nether_brick
        // More ores
        109 => Some((880, 1)),   // copper_ore → copper_ingot
        150 => Some((881, 1)),   // redstone_ore → redstone
        59 => Some((882, 1)),    // lapis_ore → lapis_lazuli
        303 => Some((883, 1)),   // emerald_ore → emerald
        47 => Some((777, 1)),    // diamond_ore → diamond
        267 => Some((47, 1)),    // ancient_debris → netherite_scrap (approximate)
        // More food
        873 => Some((874, 1)),   // kelp → dried_kelp
        893 => Some((894, 1)),   // cactus → green_dye
        // More ores
        27 => Some((779, 1)),    // gold_ore → gold_ingot
        29 => Some((778, 1)),    // iron_ore → iron_ingot
        31 => Some((775, 1)),    // coal_ore → coal
        300 => Some((880, 1)),   // copper_ore → copper_ingot
        117 => Some((46, 1)),    // redstone_ore → redstone_dust
        // Deepslate ores
        337 => Some((778, 1)),   // deepslate_iron → iron_ingot
        338 => Some((779, 1)),   // deepslate_gold → gold_ingot
        339 => Some((777, 1)),   // deepslate_diamond → diamond
        340 => Some((880, 1)),   // deepslate_copper → copper_ingot
        341 => Some((46, 1)),    // deepslate_redstone → redstone_dust
        342 => Some((882, 1)),   // deepslate_lapis → lapis_lazuli
        343 => Some((883, 1)),   // deepslate_emerald → emerald
        _ => None,
    }
}

/// 熔炉管理器
pub struct FurnaceManager {
    furnaces: HashMap<(i32, i32, i32), FurnaceData>,
}

impl Default for FurnaceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FurnaceManager {
    pub fn new() -> Self {
        Self { furnaces: HashMap::new() }
    }

    /// 获取熔炉状态（用于发送初始容器内容）
    pub fn get_or_create(&mut self, pos: (i32, i32, i32)) -> &mut FurnaceData {
        self.furnaces.entry(pos).or_insert_with(|| FurnaceData {
            input_item: None, fuel_item: None, output_item: None, output_count: 0,
            fuel_ticks: 0, progress: 0, max_progress: 200,
        })
    }

    /// 设置槽位 (ContainerClick 处理)
    pub fn set_slot(&mut self, pos: (i32, i32, i32), slot: usize, item: Option<u32>) {
        let f = self.get_or_create(pos);
        match slot {
            0 => f.input_item = item,
            1 => f.fuel_item = item,
            2 => f.output_item = item,
            _ => {}
        }
    }

    /// 获取槽位
    pub fn get_slot(&self, pos: (i32, i32, i32), slot: usize) -> Option<u32> {
        self.furnaces.get(&pos).and_then(|f| match slot {
            0 => f.input_item, 1 => f.fuel_item, 2 => f.output_item, _ => None,
        })
    }

    /// 每 20 tick 推进 (在主 tick 循环中调用)
    pub fn tick(&mut self) {
        for f in self.furnaces.values_mut() {
            // 消耗燃料
            if f.fuel_ticks > 0 {
                f.fuel_ticks -= 1;
                if let Some(input) = f.input_item {
                    if let Some((output, count)) = smelt_result(input) {
                        f.progress += 1;
                        if f.progress >= f.max_progress {
                            // 完成熔炼
                            f.progress = 0;
                            f.input_item = None;
                            f.output_item = Some(output);
                            f.output_count = count;
                        }
                    } else {
                        f.progress = 0; // 无法熔炼，重置进度
                    }
                } else {
                    f.progress = 0;
                }
            } else if f.input_item.is_some() && smelt_result(f.input_item.unwrap()).is_some() {
                // 尝试消耗燃料
                if let Some(fuel) = f.fuel_item
                    && let Some(ft) = fuel_ticks(fuel) {
                        f.fuel_ticks = ft;
                        f.fuel_item = None; // 消耗 1 个燃料
                    }
            }
        }
    }

    /// 获取进度 (0.0-1.0) 用于客户端更新
    pub fn progress(&self, pos: (i32, i32, i32)) -> f32 {
        self.furnaces.get(&pos)
            .map(|f| f.progress as f32 / f.max_progress as f32)
            .unwrap_or(0.0)
    }

    /// 检查是否正在燃烧
    pub fn is_burning(&self, pos: (i32, i32, i32)) -> bool {
        self.furnaces.get(&pos)
            .map(|f| f.fuel_ticks > 0)
            .unwrap_or(false)
    }
}
