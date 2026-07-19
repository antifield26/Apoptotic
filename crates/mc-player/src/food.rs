//! 食物系统 — 物品营养价值注册表 + 食用逻辑

/// 食物数值: (nutrition: 恢复饥饿值, saturation: 饱和值加成)
pub struct FoodValue {
    pub nutrition: i32,
    pub saturation_mod: f32,  // saturation = nutrition * saturation_mod * 2
}

/// 获取物品的食物属性 (如果是食物)
/// 返回 (nutrition, saturation)
pub fn get_food_value(item_id: u32) -> Option<(i32, f32)> {
    match item_id {
        // Basic foods
        772 => Some((4, 2.4)),    // apple
        810 => Some((5, 6.0)),    // bread
        858 => Some((8, 12.8)),   // cooked_beef
        860 => Some((8, 12.8)),   // cooked_porkchop
        859 => Some((3, 1.8)),    // raw_beef
        861 => Some((3, 1.8)),    // raw_porkchop
        862 => Some((6, 7.2)),    // cooked_chicken
        863 => Some((2, 1.2)),    // raw_chicken
        864 => Some((6, 9.6)),    // cooked_mutton
        865 => Some((2, 1.2)),    // raw_mutton
        866 => Some((5, 6.0)),    // cooked_rabbit
        874 => Some((5, 6.0)),    // cooked_cod
        855 => Some((5, 6.0)),    // cooked_cod (alias)
        875 => Some((2, 0.4)),    // raw_cod
        873 => Some((5, 7.2)),    // baked_potato
        872 => Some((1, 0.6)),    // potato (raw)
        870 => Some((3, 3.6)),    // carrot
        871 => Some((4, 2.4)),    // golden_carrot
        773 => Some((4, 9.6)),    // golden_apple
        1033 => Some((4, 9.6)),   // enchanted_golden_apple
        882 => Some((2, 0.4)),    // cookie
        881 => Some((8, 4.8)),    // pumpkin_pie
        878 => Some((2, 1.2)),    // melon_slice
        876 => Some((6, 7.2)),    // beetroot_soup
        796 => Some((6, 7.2)),    // mushroom_stew
        867 => Some((10, 12.0)),  // rabbit_stew
        879 => Some((2, 0.4)),    // sweet_berries
        880 => Some((2, 0.4)),    // glow_berries
        883 => Some((2, 0.4)),    // cake (per slice)
        894 => Some((2, 0.4)),    // dried_kelp
        885 => Some((1, 0.4)),     // sugar (minor)
        // Fish
        896 => Some((2, 0.4)),    // tropical_fish raw
        897 => Some((5, 6.0)),    // cooked_salmon
        898 => Some((2, 0.4)),    // raw_salmon
        899 => Some((1, 0.2)),    // pufferfish
        _ => None,
    }
}

/// 判断物品是否可食用
pub fn is_food(item_id: u32) -> bool {
    get_food_value(item_id).is_some()
}

/// 食用时间 (ticks, 20 = 1秒)
/// 大部分食物 32 ticks, 海带 16 ticks
pub fn eating_duration_ticks(item_id: u32) -> u32 {
    match item_id {
        894 => 16, // dried_kelp is fast
        _ => 32,   // default 1.6 seconds
    }
}
