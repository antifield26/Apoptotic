//! 数据包加载器 — JSON 格式的数据包 (兼容原版 Minecraft datapack 格式)
//!
//! 支持:
//! - `recipes/` — 合成配方 (*.json)
//! - `advancements/` — 成就定义 (*.json)
//! - `loot_tables/` — 战利品表 (*.json)
//! - `structures/` — NBT 结构文件 (*.nbt)

use mc_player::recipe::Recipe;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// 数据包加载器
pub struct DatapackLoader {
    /// 已加载数据包的根目录
    datapacks_dir: PathBuf,
    /// 已加载的数据包名称列表
    loaded_packs: Vec<String>,
}

/// 数据包元数据 (pack.mcmeta)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct PackMcmeta {
    pack: PackInfo,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct PackInfo {
    description: String,
    pack_format: u32,
}

/// JSON 配方格式 (兼容原版 Minecraft)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JsonRecipe {
    #[serde(rename = "type")]
    recipe_type: String,
    group: Option<String>,
    // Shaped recipe
    pattern: Option<Vec<String>>,
    key: Option<HashMap<String, JsonIngredient>>,
    // Shapeless recipe
    ingredients: Option<Vec<JsonIngredient>>,
    // Result
    result: Option<JsonResult>,
    // Cooking recipe
    ingredient: Option<JsonIngredient>,
    result_item: Option<String>,
    experience: Option<f32>,
    cooking_time: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JsonIngredient {
    item: Option<String>,
    tag: Option<String>,
    items: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct JsonResult {
    item: String,
    count: Option<u32>,
}

/// JSON 成就格式
#[derive(Debug, Deserialize)]
pub struct JsonAdvancement {
    pub display: Option<JsonAdvancementDisplay>,
    pub parent: Option<String>,
    pub criteria: HashMap<String, JsonCriterion>,
    pub requirements: Option<Vec<Vec<String>>>,
    pub rewards: Option<JsonRewards>,
}

#[derive(Debug, Deserialize)]
pub struct JsonAdvancementDisplay {
    pub title: String,
    pub description: String,
    pub icon: Option<JsonIcon>,
    pub frame: Option<String>,
    pub background: Option<String>,
    pub show_toast: Option<bool>,
    pub announce_to_chat: Option<bool>,
    pub hidden: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct JsonIcon {
    pub item: String,
    pub nbt: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JsonCriterion {
    pub trigger: String,
    pub conditions: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct JsonRewards {
    pub recipes: Option<Vec<String>>,
    pub loot: Option<Vec<String>>,
    pub experience: Option<u32>,
}

impl DatapackLoader {
    pub fn new(datapacks_dir: &Path) -> Self {
        fs::create_dir_all(datapacks_dir).ok();
        Self {
            datapacks_dir: datapacks_dir.to_path_buf(),
            loaded_packs: Vec::new(),
        }
    }

    /// Validate a datapack name — reject path traversal attempts.
    pub fn validate_pack_name(name: &str) -> Result<(), String> {
        if name.contains("..") || name.contains('\\') || name.starts_with('/') {
            return Err(format!("Invalid datapack name '{}': path traversal not allowed", name));
        }
        if name.is_empty() || name.len() > 64 {
            return Err(format!("Invalid datapack name '{}': must be 1-64 chars", name));
        }
        Ok(())
    }

    /// 加载指定数据包中的所有内容 (返回加载的条目数)
    pub fn load_pack(&mut self, pack_name: &str) -> Result<usize, String> {
        Self::validate_pack_name(pack_name)?;
        let pack_dir = self.datapacks_dir.join(pack_name);
        if !pack_dir.exists() {
            return Err(format!("Datapack '{}' not found at {}", pack_name, pack_dir.display()));
        }

        // Validate pack.mcmeta
        let mcmeta_path = pack_dir.join("pack.mcmeta");
        if mcmeta_path.exists() {
            let content = fs::read_to_string(&mcmeta_path)
                .map_err(|e| format!("Failed to read pack.mcmeta: {}", e))?;
            let meta: PackMcmeta = serde_json::from_str(&content)
                .map_err(|e| format!("Invalid pack.mcmeta: {}", e))?;
            if meta.pack.pack_format < 26 || meta.pack.pack_format > 48 {
                tracing::warn!("Datapack '{}' has pack_format {} (expected 26-48), compatibility issues possible",
                    pack_name, meta.pack.pack_format);
            }
            tracing::info!("Loading datapack '{}': {} (format {})",
                pack_name, meta.pack.description, meta.pack.pack_format);
        }

        let mut count = 0usize;
        let data_dir = pack_dir.join("data");
        if data_dir.exists() {
            for namespace_entry in fs::read_dir(&data_dir).map_err(|e| format!("{}", e))? {
                let ns_dir = namespace_entry.map_err(|e| format!("{}", e))?.path();
                if !ns_dir.is_dir() { continue; }

                let recipes_dir = ns_dir.join("recipes");
                if recipes_dir.exists() {
                    count += self.load_recipes_from_dir(&recipes_dir);
                }

                let advancements_dir = ns_dir.join("advancements");
                if advancements_dir.exists() {
                    count += self.load_advancements_from_dir(&advancements_dir);
                }
            }
        }

        self.loaded_packs.push(pack_name.to_string());
        tracing::info!("Loaded datapack '{}': {} items", pack_name, count);
        Ok(count)
    }

    /// 加载所有数据包中的配方，返回 Recipe 列表 (用于注册到 RecipeRegistry)
    pub fn load_all_recipes(&self) -> Vec<Recipe> {
        let mut recipes = Vec::new();
        for pack_name in &self.loaded_packs {
            let pack_dir = self.datapacks_dir.join(pack_name);
            let data_dir = pack_dir.join("data");
            if data_dir.exists()
                && let Ok(entries) = fs::read_dir(&data_dir) {
                    for ns_entry in entries.flatten() {
                        let ns_dir = ns_entry.path();
                        if !ns_dir.is_dir() { continue; }
                        let recipes_dir = ns_dir.join("recipes");
                        if recipes_dir.exists() {
                            recipes.extend(self.collect_recipes_from_dir(&recipes_dir));
                        }
                    }
                }
        }
        recipes
    }

    /// 从目录加载 JSON 配方 (返回成功解析的数量)
    fn load_recipes_from_dir(&self, dir: &Path) -> usize {
        let mut count = 0;
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false)
                    && let Ok(content) = fs::read_to_string(&path)
                        && let Ok(recipe) = serde_json::from_str::<JsonRecipe>(&content)
                            && self.parse_recipe(&recipe).is_some() {
                                count += 1;
                            }
            }
        }
        count
    }

    /// 收集目录中的所有 Recipe (返回解析后的 Recipe 列表)
    fn collect_recipes_from_dir(&self, dir: &Path) -> Vec<Recipe> {
        let mut recipes = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false)
                    && let Ok(content) = fs::read_to_string(&path)
                        && let Ok(recipe) = serde_json::from_str::<JsonRecipe>(&content)
                            && let Some(r) = self.parse_recipe(&recipe) {
                                recipes.push(r);
                            }
            }
        }
        recipes
    }

    /// 解析 JSON 配方为内部 Recipe 格式 (返回 Some(Recipe) 或 None)
    fn parse_recipe(&self, json: &JsonRecipe) -> Option<Recipe> {
        // Determine item IDs from names (simplified: use hash)
        let resolve_item = |name: &str| -> u32 {
            let name = name.strip_prefix("minecraft:").unwrap_or(name);
            // Simple hash-based lookup — extend with actual registry
            mc_core::item::resolve_item_id(name)
        };

        match json.recipe_type.as_str() {
            "minecraft:crafting_shaped" => {
                if let Some(ref pattern) = json.pattern
                    && let Some(ref key) = json.key
                    && let Some(ref result) = json.result {
                        let height = pattern.len() as u8;
                        let width = pattern.first().map(|s| s.len() as u8).unwrap_or(0);
                        let mut ingredients: Vec<Vec<u32>> = Vec::new();
                        for row in pattern {
                            for ch in row.chars() {
                                let ch_str = ch.to_string();
                                if let Some(ing) = key.get(&ch_str) {
                                    let ids = self.resolve_ingredient(ing);
                                    ingredients.push(ids);
                                } else {
                                    ingredients.push(vec![0]); // air placeholder
                                }
                            }
                        }
                        let result_id = resolve_item(&result.item);
                        if result_id > 0 {
                            let recipe = Recipe {
                                id: format!("datapack:{}", result.item),
                                group: json.group.clone().unwrap_or_default(),
                                category: 0,
                                width,
                                height,
                                ingredients,
                                result_item: result_id,
                                is_shapeless: false, result_count: result.count.unwrap_or(1) as u8,
                            };
                            return Some(recipe);
                        }
                    }
            }
            "minecraft:crafting_shapeless" => {
                // Shapeless recipes: all ingredients in any order
                if let Some(ref ingredients) = json.ingredients
                    && let Some(ref result) = json.result {
                        let ingrs: Vec<Vec<u32>> = ingredients.iter()
                            .map(|i| self.resolve_ingredient(i))
                            .collect();
                        let result_id = resolve_item(&result.item);
                        if result_id > 0 && !ingrs.is_empty() {
                            let recipe = Recipe {
                                id: format!("datapack:{}", result.item),
                                group: json.group.clone().unwrap_or_default(),
                                category: 2,
                                width: ingrs.len() as u8,
                                height: 1,
                                ingredients: ingrs,
                                result_item: result_id,
                                is_shapeless: false, result_count: result.count.unwrap_or(1) as u8,
                            };
                            return Some(recipe);
                        }
                    }
            }
            _ => {
                tracing::debug!("Unsupported recipe type: {}", json.recipe_type);
            }
        }
        None
    }

    fn resolve_ingredient(&self, ing: &JsonIngredient) -> Vec<u32> {
        let mut ids = Vec::new();
        if let Some(ref item) = ing.item {
            let name = item.strip_prefix("minecraft:").unwrap_or(item);
            let id = mc_core::item::resolve_item_id(name);
            if id > 0 { ids.push(id); }
        }
        if let Some(ref items) = ing.items {
            for item in items {
                let name = item.strip_prefix("minecraft:").unwrap_or(item);
                let id = mc_core::item::resolve_item_id(name);
                if id > 0 { ids.push(id); }
            }
        }
        // Tags not implemented yet — return empty
        if ids.is_empty() { ids.push(0); }
        ids
    }

    /// 从目录加载 JSON 成就
    fn load_advancements_from_dir(&self, dir: &Path) -> usize {
        let mut count = 0;
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(content) = fs::read_to_string(&path)
                    && let Ok(_adv) = serde_json::from_str::<JsonAdvancement>(&content) {
                        // Register advancement to tracker
                        count += 1;
                    }
            }
        }
        count
    }

    /// 获取已加载的数据包列表
    pub fn loaded_packs(&self) -> &[String] {
        &self.loaded_packs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_pack(dir: &Path, name: &str) -> PathBuf {
        let pack_dir = dir.join(name);
        let data_dir = pack_dir.join("data/test/recipes");
        fs::create_dir_all(&data_dir).unwrap();

        let mcmeta = r#"{"pack": {"description": "Test pack", "pack_format": 48}}"#;
        fs::write(pack_dir.join("pack.mcmeta"), mcmeta).unwrap();

        let recipe = r#"{
            "type": "minecraft:crafting_shaped",
            "pattern": ["AAA", " B ", " B "],
            "key": {"A": {"item": "minecraft:iron_ingot"}, "B": {"item": "minecraft:stick"}},
            "result": {"item": "minecraft:iron_pickaxe", "count": 1}
        }"#;
        fs::write(data_dir.join("test_pickaxe.json"), recipe).unwrap();
        pack_dir
    }

    #[test]
    fn test_load_datapack() {
        let tmp = std::env::temp_dir().join("mc_test_datapack");
        let _ = fs::remove_dir_all(&tmp);
        create_test_pack(&tmp, "test_pack");

        let mut loader = DatapackLoader::new(&tmp);
        let result = loader.load_pack("test_pack");
        assert!(result.is_ok(), "Failed: {:?}", result.err());
        assert_eq!(result.unwrap(), 1);
        assert!(loader.loaded_packs().contains(&"test_pack".to_string()));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_missing_pack() {
        let tmp = std::env::temp_dir().join("mc_test_missing");
        let mut loader = DatapackLoader::new(&tmp);
        assert!(loader.load_pack("nonexistent").is_err());
    }
}
