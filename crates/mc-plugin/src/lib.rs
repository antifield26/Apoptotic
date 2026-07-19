//! 插件系统 — Rust 原生插件 trait + 数据包加载器
//!
//! ## 架构
//!
//! ```text
//! PluginManager
//!   ├── NativePlugin (Rust trait, 编译时或动态加载)
//!   └── DatapackLoader (JSON 数据包: recipes, advancements, loot_tables, structures)
//! ```
//!
//! ## 插件 Trait
//!
//! ```rust,ignore
//! struct MyPlugin;
//! impl NativePlugin for MyPlugin {
//!     fn name(&self) -> &str { "my_plugin" }
//!     fn on_enable(&mut self, ctx: &PluginContext) { /* init */ }
//!     fn on_tick(&mut self, ctx: &PluginContext, tick: u64) { /* per-tick */ }
//! }
//! ```
//!
//! ## 数据包格式
//!
//! ```text
//! datapacks/my_pack/
//!   pack.mcmeta
//!   data/my_namespace/
//!     recipes/       ← JSON 配方文件
//!     advancements/  ← JSON 成就文件
//!     loot_tables/   ← JSON 战利品表
//!     structures/    ← NBT 结构文件
//! ```
//!
//! ## 注册插件
//!
//! ```toml
//! # config/default.toml
//! [plugins]
//! enabled = ["my_plugin", "economy_plugin"]
//! datapacks = ["vanilla_extras", "custom_recipes"]
//! ```

pub mod datapack;
pub mod plugin;
pub mod wasm;

pub use datapack::DatapackLoader;
pub use plugin::{NativePlugin, PluginContext, PluginManager};
pub use wasm::load_wasm_plugins;

/// 预导入 — 插件开发常用类型
pub mod prelude {
    pub use super::plugin::{NativePlugin, PluginContext};
    pub use mc_command::dispatcher::{Command, CommandContext, CommandResult};
    pub use mc_core::block::BlockState;
    pub use mc_player::player::PlayerManager;
}
