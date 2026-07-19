//! 命令参数解析器

use mc_core::types::GameMode;

/// 解析游戏模式参数
pub fn parse_gamemode(input: &str) -> Result<GameMode, String> {
    match input.to_lowercase().as_str() {
        "0" | "s" | "survival" => Ok(GameMode::Survival),
        "1" | "c" | "creative" => Ok(GameMode::Creative),
        "2" | "a" | "adventure" => Ok(GameMode::Adventure),
        "3" | "sp" | "spectator" => Ok(GameMode::Spectator),
        _ => Err(format!("Unknown gamemode: {}", input)),
    }
}

/// 解析整数
pub fn parse_i32(input: &str) -> Result<i32, String> {
    input.parse::<i32>().map_err(|e| format!("Invalid number: {}", e))
}

/// 解析 f64
pub fn parse_f64(input: &str) -> Result<f64, String> {
    input.parse::<f64>().map_err(|e| format!("Invalid number: {}", e))
}

/// 解析 bool
pub fn parse_bool(input: &str) -> Result<bool, String> {
    match input.to_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err(format!("Invalid boolean: {}", input)),
    }
}

/// 规范化物品名: 去除 "minecraft:" 前缀，转为小写
pub fn normalize_item_name(name: &str) -> String {
    name.to_lowercase()
        .trim_start_matches("minecraft:")
        .to_string()
}
