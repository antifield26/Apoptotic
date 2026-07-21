//! 酿造系统 — 药水酿造配方与酿造台管理
//!
//! 支持 12+ 药水配方: 基础 + 增强/延长/腐化 + 喷溅 + 滞留

use std::collections::HashMap;

/// 酿造配方
#[derive(Debug, Clone)]
pub struct BrewingRecipe {
    pub input: u32,
    pub ingredient: u32,
    pub output: u32,
}

/// 酿造注册表
pub struct BrewingRegistry {
    recipes: Vec<BrewingRecipe>,
}

impl Default for BrewingRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BrewingRegistry {
    #[allow(clippy::vec_init_then_push)]
    pub fn new() -> Self {
        let mut recipes = Vec::new();

        // 基础酿造: 地狱疣 + 水瓶 → 粗制药水
        recipes.push(recipe(1003, 1031, 1008)); // water + nether_wart → awkward
        recipes.push(recipe(1003, 906, 1022));  // water + spider_eye → weakness

        // 从粗制药水
        recipes.push(recipe(1008, 1032, 1011)); // awkward + melon → healing
        recipes.push(recipe(1008, 885, 1012));  // awkward + sugar → swiftness
        recipes.push(recipe(1008, 904, 1013));  // awkward + blaze_powder → strength
        recipes.push(recipe(1008, 906, 1014));  // awkward + spider_eye → poison
        recipes.push(recipe(1008, 905, 1015));  // awkward + ghast_tear → regen
        recipes.push(recipe(1008, 902, 1016));  // awkward + magma_cream → fire_resist
        recipes.push(recipe(1008, 871, 1017));  // awkward + golden_carrot → night_vision
        recipes.push(recipe(1008, 854, 1018));  // awkward + pufferfish → water_breath
        recipes.push(recipe(1008, 868, 1019));  // awkward + rabbit_foot → leaping

        // 腐化: 发酵蛛眼
        recipes.push(recipe(1011, 907, 1021));  // healing → harming
        recipes.push(recipe(1012, 907, 1020));  // swiftness → slowness
        recipes.push(recipe(1017, 907, 1023));  // night_vision → invisibility

        // 喷溅: 火药
        recipes.push(recipe(1011, 803, 1024)); // healing → splash_healing
        recipes.push(recipe(1012, 803, 1025)); // swiftness → splash_swiftness
        recipes.push(recipe(1013, 803, 1026)); // strength → splash_strength
        recipes.push(recipe(1014, 803, 1027)); // poison → splash_poison
        recipes.push(recipe(1015, 803, 1039)); // regen → splash_regen
        recipes.push(recipe(1016, 803, 1040)); // fire_resist → splash_fire_resist

        // 延长: 红石 (ID 993) → extended duration potions
        recipes.push(recipe(1011, 993, 1041)); // healing → long_healing
        recipes.push(recipe(1012, 993, 1042)); // swiftness → long_swiftness
        recipes.push(recipe(1013, 993, 1043)); // strength → long_strength
        recipes.push(recipe(1015, 993, 1044)); // regen → long_regen
        recipes.push(recipe(1016, 993, 1045)); // fire_resist → long_fire_resist
        recipes.push(recipe(1017, 993, 1046)); // night_vision → long_night_vision
        recipes.push(recipe(1018, 993, 1047)); // water_breath → long_water_breath
        recipes.push(recipe(1019, 993, 1048)); // leaping → long_leaping

        // 增强: 荧石粉 (ID 903) → enhanced potions (level II)
        recipes.push(recipe(1011, 903, 1049)); // healing → strong_healing
        recipes.push(recipe(1012, 903, 1050)); // swiftness → strong_swiftness
        recipes.push(recipe(1013, 903, 1051)); // strength → strong_strength
        recipes.push(recipe(1015, 903, 1052)); // regen → strong_regen
        recipes.push(recipe(1019, 903, 1053)); // leaping → strong_leaping

        // 滞留: 龙息 (ID 1028) → lingering potions (from splash)
        recipes.push(recipe(1024, 1028, 1054)); // splash_healing → lingering_healing
        recipes.push(recipe(1025, 1028, 1055)); // splash_swiftness → lingering_swiftness
        recipes.push(recipe(1026, 1028, 1056)); // splash_strength → lingering_strength
        recipes.push(recipe(1027, 1028, 1057)); // splash_poison → lingering_poison
        recipes.push(recipe(1039, 1028, 1058)); // splash_regen → lingering_regen
        recipes.push(recipe(1040, 1028, 1059)); // splash_fire_resist → lingering_fire_resist

        // 药箭: lingering potion + arrow (773) → tipped arrow
        // Each lingering potion type produces a corresponding tipped arrow
        recipes.push(recipe(1054, 773, 1060)); // lingering_healing → tipped_healing
        recipes.push(recipe(1055, 773, 1061)); // lingering_swiftness → tipped_swiftness
        recipes.push(recipe(1056, 773, 1062)); // tipped_strength
        recipes.push(recipe(1057, 773, 1063)); // tipped_poison
        recipes.push(recipe(1058, 773, 1064)); // tipped_regen
        recipes.push(recipe(1059, 773, 1065)); // tipped_fire_resist
        recipes.push(recipe(1022, 773, 1066)); // weakness → tipped_weakness
        recipes.push(recipe(1020, 773, 1067)); // slowness → tipped_slowness
        recipes.push(recipe(1023, 773, 1068)); // invisibility → tipped_invisibility
        recipes.push(recipe(1018, 773, 1069)); // water_breath → tipped_water_breath
        recipes.push(recipe(1019, 773, 1070)); // leaping → tipped_leaping
        recipes.push(recipe(1017, 773, 1071)); // night_vision → tipped_night_vision
        recipes.push(recipe(1021, 773, 1072)); // harming → tipped_harming

        Self { recipes }
    }

    pub fn find_recipe(&self, input_id: u32, ingredient_id: u32) -> Option<&BrewingRecipe> {
        self.recipes.iter().find(|r| r.input == input_id && r.ingredient == ingredient_id)
    }
}

fn recipe(input: u32, ingredient: u32, output: u32) -> BrewingRecipe {
    BrewingRecipe { input, ingredient, output }
}

/// 单个酿造台的状态
#[derive(Debug, Clone)]
pub struct BrewingStandData {
    pub pos: (i32, i32, i32),
    pub fuel: u32,
    pub brew_ticks: u32,
    pub ingredient_slot: Option<u32>,
    pub bottle_slots: [Option<u32>; 3],
}

impl BrewingStandData {
    pub fn new(pos: (i32, i32, i32)) -> Self {
        Self { pos, fuel: 0, brew_ticks: 0, ingredient_slot: None, bottle_slots: [None, None, None] }
    }

    pub fn can_brew(&self, registry: &BrewingRegistry) -> bool {
        let ingredient = match self.ingredient_slot { Some(id) => id, None => return false };
        self.bottle_slots.iter().any(|slot| {
            if let Some(input_id) = slot {
                registry.find_recipe(*input_id, ingredient).is_some()
            } else { false }
        })
    }
}

/// 酿造台管理
/// Brew completion event for advancement tracking
pub struct BrewCompletion {
    pub position: (i32, i32, i32),
    pub output_id: u32,
}

pub struct BrewingStandManager {
    pub stands: HashMap<(i32, i32, i32), BrewingStandData>,
    pub completed_brews: Vec<BrewCompletion>,
}

impl Default for BrewingStandManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BrewingStandManager {
    pub fn new() -> Self { Self { stands: HashMap::new(), completed_brews: Vec::new() } }

    /// Drain completed brew events for advancement triggering
    pub fn take_brew_completions(&mut self) -> Vec<BrewCompletion> {
        std::mem::take(&mut self.completed_brews)
    }

    pub fn get_or_create(&mut self, pos: (i32, i32, i32)) -> &mut BrewingStandData {
        self.stands.entry(pos).or_insert_with(|| BrewingStandData::new(pos))
    }

    pub fn remove(&mut self, pos: (i32, i32, i32)) { self.stands.remove(&pos); }

    pub fn get_brew_ticks(&self, pos: (i32, i32, i32)) -> u32 {
        self.stands.get(&pos).map(|s| s.brew_ticks).unwrap_or(0)
    }

    pub fn get_fuel(&self, pos: (i32, i32, i32)) -> u32 {
        self.stands.get(&pos).map(|s| s.fuel).unwrap_or(0)
    }

    pub fn tick(&mut self, registry: &BrewingRegistry, _container_manager: &crate::container::ContainerManager) {
        for stand in self.stands.values_mut() {
            if stand.fuel > 0 && stand.can_brew(registry) {
                stand.brew_ticks = stand.brew_ticks.saturating_add(1);
                if stand.brew_ticks >= 400 {
                    let ingredient = stand.ingredient_slot.take().unwrap_or(0);
                    for i in 0..3 {
                        if let Some(input_id) = stand.bottle_slots[i]
                            && let Some(recipe) = registry.find_recipe(input_id, ingredient) {
                                stand.bottle_slots[i] = Some(recipe.output);
                            }
                    }
                    stand.fuel = stand.fuel.saturating_sub(1);
                    stand.brew_ticks = 0;
                    // Record brew completion for advancement
                    for i in 0..3 {
                        if let Some(output_id) = stand.bottle_slots[i] {
                            self.completed_brews.push(BrewCompletion {
                                position: stand.pos, output_id,
                            });
                        }
                    }
                }
            }
        }
    }
}
