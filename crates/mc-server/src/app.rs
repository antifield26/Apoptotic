//! Application — 服务器核心组装器
//!
//! 负责初始化所有子系统并编排启动/关闭流程。

use crate::config::Config;
use mc_core::block::BlockRegistry;
use mc_world::world::World;
use tracing::info;

/// 服务器应用程序状态
pub struct App {
    pub config: Config,
    #[allow(dead_code)]
    pub block_registry: BlockRegistry,
    pub world: World,
    /// 服务器启动时间
    #[allow(dead_code)]
    pub start_time: std::time::Instant,
}

impl App {
    /// 创建并初始化服务器应用
    pub fn new(config: Config) -> Self {
        info!("Initializing Minecraft LAN Server...");
        info!("Host: {}:{}", config.server.host, config.server.port);
        info!(
            "Online mode: {}, LAN broadcast: {}",
            config.server.online_mode, config.lan.enabled
        );

        let block_registry = BlockRegistry::new();
        let seed = config
            .world
            .seed
            .as_ref()
            .map(|s| {
                use std::hash::{DefaultHasher, Hash, Hasher};
                let mut h = DefaultHasher::new();
                s.hash(&mut h);
                h.finish()
            })
            .unwrap_or_else(|| {
                use std::time::SystemTime;
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(42)
            });

        let active_gen = config.world.generator.clone();
        let gen_options = config.world.generator_options.clone();

        let world = World::with_generator_options(
            config.world.name.clone(),
            seed,
            config.world.view_distance,
            &active_gen,
            gen_options,
        );

        info!(
            "World '{}' initialized with seed: {}, generator: '{}'",
            world.level_name, seed, world.generators.active().name()
        );

        Self {
            config,
            block_registry,
            world,
            start_time: std::time::Instant::now(),
        }
    }

    /// 返回服务器运行时间
    #[allow(dead_code)]
    pub fn uptime(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
}
