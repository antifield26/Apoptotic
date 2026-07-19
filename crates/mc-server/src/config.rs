//! 配置系统 — TOML 文件 + 环境变量覆盖
//!
//! 环境变量格式: `MCS_SECTION__KEY=value` (双下划线分隔嵌套)
//! 例如: `MCS_SERVER__PORT=25566`

use serde::{Deserialize, Serialize};
use std::path::Path;

/// 服务器完整配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub lan: LanConfig,
    #[serde(default)]
    pub world: WorldConfig,
    #[serde(default)]
    pub performance: PerformanceConfig,
    #[serde(default)]
    pub persistence: PersistenceConfig,
    #[serde(default)]
    pub admin: AdminConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_max_players")]
    pub max_players: u32,
    #[serde(default = "default_motd")]
    pub motd: String,
    #[serde(default)]
    pub online_mode: bool,
    #[serde(default = "default_compression_threshold")]
    pub compression_threshold: u32,
    #[serde(default = "default_protocol_version")]
    pub protocol_version: i32,
    #[serde(default = "default_version_name")]
    pub version_name: String,
    #[serde(default)]
    pub server_links: Vec<ServerLinkConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerLinkConfig {
    pub label: String,
    pub url: String,
}

fn default_protocol_version() -> i32 {
    776 // 26.2 — Minecraft 2026 年最新版本
}
fn default_version_name() -> String {
    "26.2".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanConfig {
    #[serde(default = "default_lan_enabled")]
    pub enabled: bool,
    #[serde(default = "default_broadcast_interval_ms")]
    pub broadcast_interval_ms: u64,
    #[serde(default = "default_multicast_group")]
    pub multicast_group: String,
    #[serde(default = "default_multicast_port")]
    pub multicast_port: u16,
    #[serde(default = "default_broadcast_motd")]
    pub broadcast_motd: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldConfig {
    #[serde(default = "default_world_name")]
    pub name: String,
    #[serde(default)]
    pub seed: Option<String>,
    #[serde(default = "default_view_distance")]
    pub view_distance: u8,
    #[serde(default = "default_simulation_distance")]
    pub simulation_distance: u8,
    #[serde(default = "default_difficulty")]
    pub difficulty: String,
    #[serde(default = "default_gamemode")]
    pub gamemode: String,
    #[serde(default = "default_generator")]
    pub generator: String,
    #[serde(default)]
    pub generator_options: std::collections::HashMap<String, String>,
}

fn default_generator() -> String {
    "flat".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    #[serde(default = "default_tick_rate")]
    pub tick_rate: u32,
    #[serde(default = "default_chunk_threads")]
    pub chunk_threads: u32,
    #[serde(default = "default_max_chunks_in_memory")]
    pub max_chunks_in_memory: usize,
    #[serde(default = "default_entity_broadcast_radius")]
    pub entity_broadcast_radius: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    #[serde(default = "default_save_interval_ticks")]
    pub save_interval_ticks: u64,
    #[serde(default = "default_region_format")]
    pub region_format: String,
    #[serde(default = "default_player_db")]
    pub player_db: String,
    #[serde(default = "default_chunk_compression")]
    pub chunk_compression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminConfig {
    #[serde(default)]
    pub rcon_enabled: bool,
    #[serde(default = "default_rcon_port")]
    pub rcon_port: u16,
    #[serde(default)]
    pub rcon_password: String,
    #[serde(default = "default_console_enabled")]
    pub console_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    #[serde(default)]
    pub prometheus_enabled: bool,
    #[serde(default = "default_prometheus_port")]
    pub prometheus_port: u16,
}

// ── Default values ──

fn default_host() -> String {
    "0.0.0.0".into()
}
fn default_port() -> u16 {
    25565
}
fn default_max_players() -> u32 {
    20
}
fn default_motd() -> String {
    "Minecraft LAN Server".into()
}
fn default_compression_threshold() -> u32 {
    256
}
fn default_lan_enabled() -> bool {
    true
}
fn default_broadcast_interval_ms() -> u64 {
    1500
}
fn default_multicast_group() -> String {
    "224.0.2.60".into()
}
fn default_multicast_port() -> u16 {
    4445
}
fn default_broadcast_motd() -> bool {
    true
}
fn default_world_name() -> String {
    "world".into()
}
fn default_view_distance() -> u8 {
    8
}
fn default_simulation_distance() -> u8 {
    6
}
fn default_difficulty() -> String {
    "normal".into()
}
fn default_gamemode() -> String {
    "survival".into()
}
fn default_tick_rate() -> u32 {
    20
}
fn default_chunk_threads() -> u32 {
    // RPi 5 has 4× Cortex-A76 — use 3 for chunk gen, leave 1 for OS+I/O
    3
}
fn default_max_chunks_in_memory() -> usize {
    1024
}
fn default_entity_broadcast_radius() -> f64 {
    64.0
}
fn default_save_interval_ticks() -> u64 {
    6000
}
fn default_region_format() -> String {
    "anvil".into()
}
fn default_player_db() -> String {
    "sqlite://data/players.db".into()
}
fn default_chunk_compression() -> String {
    "lz4".into()
}
fn default_rcon_port() -> u16 {
    25575
}
fn default_console_enabled() -> bool {
    true
}
fn default_prometheus_port() -> u16 {
    9090
}

// ── Default trait impls ──

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            max_players: default_max_players(),
            motd: default_motd(),
            online_mode: false,
            compression_threshold: default_compression_threshold(),
            protocol_version: default_protocol_version(),
            version_name: default_version_name(),
            server_links: Vec::new(),
        }
    }
}

impl Default for LanConfig {
    fn default() -> Self {
        Self {
            enabled: default_lan_enabled(),
            broadcast_interval_ms: default_broadcast_interval_ms(),
            multicast_group: default_multicast_group(),
            multicast_port: default_multicast_port(),
            broadcast_motd: default_broadcast_motd(),
        }
    }
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            name: default_world_name(),
            seed: None,
            view_distance: default_view_distance(),
            simulation_distance: default_simulation_distance(),
            difficulty: default_difficulty(),
            gamemode: default_gamemode(),
            generator: default_generator(),
            generator_options: std::collections::HashMap::new(),
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            tick_rate: default_tick_rate(),
            chunk_threads: default_chunk_threads(),
            max_chunks_in_memory: default_max_chunks_in_memory(),
            entity_broadcast_radius: default_entity_broadcast_radius(),
        }
    }
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            save_interval_ticks: default_save_interval_ticks(),
            region_format: default_region_format(),
            player_db: default_player_db(),
            chunk_compression: default_chunk_compression(),
        }
    }
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            rcon_enabled: false,
            rcon_port: default_rcon_port(),
            rcon_password: String::new(),
            console_enabled: default_console_enabled(),
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            prometheus_enabled: false,
            prometheus_port: default_prometheus_port(),
        }
    }
}


// ── Loading ──

impl Config {
    /// 从 TOML 文件加载配置，若文件不存在则返回默认配置
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if path.exists() {
            let content =
                std::fs::read_to_string(path).map_err(ConfigError::Io)?;
            let mut config: Self =
                toml::from_str(&content).map_err(ConfigError::Parse)?;
            config.apply_env_overrides();
            Ok(config)
        } else {
            let config = Self::default();
            // 首次启动时写入默认配置文件
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let default_toml =
                toml::to_string_pretty(&config).map_err(ConfigError::Serialize)?;
            std::fs::write(path, default_toml).map_err(ConfigError::Io)?;
            Ok(config)
        }
    }

    /// 应用环境变量覆盖 (MCS_SECTION__KEY=value)
    fn apply_env_overrides(&mut self) {
        for (key, value) in std::env::vars() {
            if let Some(config_key) = key.strip_prefix("MCS_") {
                self.apply_env_override(config_key, &value);
            }
        }
    }

    fn apply_env_override(&mut self, key: &str, value: &str) {
        let parts: Vec<&str> = key.split("__").collect();
        if parts.len() != 2 {
            tracing::warn!("invalid env override key: MCS_{}", key);
            return;
        }
        let (section, field) = (parts[0].to_lowercase(), parts[1].to_lowercase());

        match (section.as_str(), field.as_str()) {
            ("server", "port") => self.server.port = parse_or_warn(value),
            ("server", "max_players") => self.server.max_players = parse_or_warn(value),
            ("server", "motd") => self.server.motd = value.to_string(),
            ("server", "online_mode") => self.server.online_mode = parse_or_warn(value),
            ("lan", "enabled") => self.lan.enabled = parse_or_warn(value),
            ("world", "seed") => self.world.seed = Some(value.to_string()),
            ("world", "view_distance") => self.world.view_distance = parse_or_warn(value),
            ("world", "difficulty") => self.world.difficulty = value.to_string(),
            _ => tracing::debug!("unknown config override: MCS_{}", key),
        }
    }
}

fn parse_or_warn<T: std::str::FromStr + Default>(s: &str) -> T
where
    T::Err: std::fmt::Display,
{
    match s.parse() {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("failed to parse config value '{}': {} — using default", s, e);
            T::default()
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(toml::de::Error),
    Serialize(toml::ser::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "IO error: {}", e),
            ConfigError::Parse(e) => write!(f, "TOML parse error: {}", e),
            ConfigError::Serialize(e) => write!(f, "TOML serialize error: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {}
