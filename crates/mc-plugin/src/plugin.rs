//! 原生插件系统 — Rust trait-based plugin API

use dashmap::DashMap;
use mc_command::dispatcher::CommandDispatcher;
use mc_core::world_state::SharedWorldState;
use mc_player::mob::MobManager;
use mc_player::container::ContainerManager;
use mc_player::player::SharedPlayerManager;
use mc_world::chunk_store::ChunkStore;
use std::sync::Arc;
use tokio::sync::broadcast;

/// 插件可访问的服务器上下文
#[derive(Clone)]
pub struct PluginContext {
    /// 玩家管理器 (在线玩家)
    pub player_manager: SharedPlayerManager,
    /// 命令分发器 (注册自定义命令)
    pub command_dispatcher: Arc<parking_lot::Mutex<CommandDispatcher>>,
    /// 世界状态 (时间/天气/边界)
    pub world_state: SharedWorldState,
    /// 区块存储
    pub chunk_store: ChunkStore,
    /// 生物管理器 (非玩家实体)
    pub mob_manager: Arc<MobManager>,
    /// 容器管理器 (GUI 容器)
    pub container_manager: Arc<ContainerManager>,
    /// 关闭信号发送端
    pub shutdown_tx: broadcast::Sender<()>,
    /// 插件数据目录
    pub data_dir: std::path::PathBuf,
}

/// 原生插件 trait — 实现此 trait 以创建插件
///
/// # 生命周期
/// ```text
/// new() → on_enable(ctx) → [on_tick(ctx, n)...] → on_disable()
/// ```
pub trait NativePlugin: Send + Sync {
    /// 插件唯一名称
    fn name(&self) -> &str;
    /// 插件版本
    fn version(&self) -> &str { "0.1.0" }
    /// 插件作者
    fn author(&self) -> &str { "unknown" }

    /// 插件被加载/启用时调用 (注册命令、初始化状态)
    fn on_enable(&mut self, _ctx: &PluginContext) {}
    /// 每个游戏 tick 调用 (频率 = tick_rate, 默认 20/s)
    fn on_tick(&mut self, _ctx: &PluginContext, _tick: u64) {}
    /// 玩家加入时调用
    fn on_player_join(&mut self, _ctx: &PluginContext, _uuid: &uuid::Uuid, _username: &str) {}
    /// 玩家退出时调用
    fn on_player_leave(&mut self, _ctx: &PluginContext, _uuid: &uuid::Uuid) {}
    /// 插件被禁用/卸载时调用 (清理资源)
    fn on_disable(&mut self) {}

    /// 是否已启用
    fn is_enabled(&self) -> bool { true }
    /// 设置启用状态
    fn set_enabled(&mut self, _enabled: bool) {}
}

/// 插件管理器 — 管理所有已加载插件的生命周期
pub struct PluginManager {
    plugins: DashMap<String, Box<dyn NativePlugin>>,
    enabled: DashMap<String, bool>,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: DashMap::new(),
            enabled: DashMap::new(),
        }
    }

    /// 注册一个插件
    pub fn register(&self, plugin: Box<dyn NativePlugin>) {
        let name = plugin.name().to_string();
        tracing::info!("Registered plugin: {} v{}", name, plugin.version());
        self.plugins.insert(name.clone(), plugin);
        self.enabled.insert(name, true);
    }

    /// 启用所有插件
    pub fn enable_all(&self, ctx: &PluginContext) {
        for mut entry in self.plugins.iter_mut() {
            let name = entry.key().clone();
            if !self.enabled.get(&name).map(|e| *e).unwrap_or(true) {
                continue;
            }
            tracing::info!("Enabling plugin: {}", name);
            entry.value_mut().on_enable(ctx);
        }
    }

    /// 每个 tick 调用所有已启用插件的 on_tick
    pub fn tick_all(&self, ctx: &PluginContext, tick: u64) {
        for mut entry in self.plugins.iter_mut() {
            let name = entry.key().clone();
            if self.enabled.get(&name).map(|e| *e).unwrap_or(false) {
                entry.value_mut().on_tick(ctx, tick);
            }
        }
    }

    /// 通知所有插件玩家加入
    pub fn notify_player_join(&self, ctx: &PluginContext, uuid: &uuid::Uuid, username: &str) {
        for mut entry in self.plugins.iter_mut() {
            entry.value_mut().on_player_join(ctx, uuid, username);
        }
    }

    /// 通知所有插件玩家离开
    pub fn notify_player_leave(&self, ctx: &PluginContext, uuid: &uuid::Uuid) {
        for mut entry in self.plugins.iter_mut() {
            entry.value_mut().on_player_leave(ctx, uuid);
        }
    }

    /// 禁用并卸载指定插件
    pub fn disable(&self, name: &str) {
        if let Some(mut plugin) = self.plugins.get_mut(name) {
            plugin.value_mut().on_disable();
            self.enabled.insert(name.to_string(), false);
            tracing::info!("Disabled plugin: {}", name);
        }
    }

    /// 获取已注册插件列表
    pub fn list(&self) -> Vec<(String, String, bool)> {
        self.plugins.iter()
            .map(|entry| {
                let name = entry.key().clone();
                let ver = entry.value().version().to_string();
                let en = self.enabled.get(&name).map(|e| *e).unwrap_or(false);
                (name, ver, en)
            })
            .collect()
    }
}
