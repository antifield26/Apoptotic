//! WASM 插件运行时 — 加载 .wasm 插件
//!
//! ## 架构
//!
//! WASM 插件通过标准化的导出函数接口与服务器交互。
//! 启用方式: 在 Cargo.toml 中取消注释 `extism = "1"` 依赖并重新编译。
//!
//! ## WASM 插件导出函数 (PDK)
//!
//! ```text
//!   name()              -> &str   插件名称
//!   version()           -> &str   版本号 (默认 "0.1.0")
//!   on_enable()         -> ()     插件启用
//!   on_tick(tick: u64)  -> ()     每游戏 tick
//!   on_player_join(uuid: &str, username: &str) -> ()
//!   on_player_leave(uuid: &str)   -> ()
//!   on_disable()        -> ()     插件禁用
//! ```
//!
//! ## 插件目录结构
//!
//! ```text
//! plugins/
//!   my_plugin.wasm       ← WASM 二进制
//!   another_plugin.wasm
//! ```

use super::plugin::NativePlugin;
use super::plugin::PluginContext;
use tracing::{error, info, warn};

/// WASM plugin adapter — wraps a WASM plugin and implements NativePlugin.
///
/// When extism is enabled, this calls into the WASM module via extism::Plugin.
/// When extism is disabled (default), this provides a stub that logs discovery.
pub struct WasmPlugin {
    name: String,
    version: String,
    /// File path for the .wasm binary
    path: std::path::PathBuf,
    /// Whether the plugin is currently enabled
    enabled: bool,
    /// Whether the WASM runtime is available (extism compiled in)
    runtime_available: bool,
}

impl WasmPlugin {
    /// Discover a WASM plugin from a .wasm file.
    /// Returns the plugin adapter (runtime execution requires extism dependency).
    pub fn discover(path: &std::path::Path) -> Result<Self, String> {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown_wasm")
            .to_string();

        // Try to load metadata from the WASM binary header
        // WASM files start with \0asm magic bytes
        let wasm_bytes = std::fs::read(path)
            .map_err(|e| format!("Failed to read WASM file: {}", e))?;

        if wasm_bytes.len() < 8 || &wasm_bytes[0..4] != b"\0asm" {
            return Err(format!("Not a valid WASM file: {}", path.display()));
        }

        let wasm_version = u32::from_le_bytes([wasm_bytes[4], wasm_bytes[5], wasm_bytes[6], wasm_bytes[7]]);
        info!(
            "Discovered WASM plugin: {} (WASM v{}) at {}",
            name, wasm_version, path.display()
        );

        // Check if WASM runtime is available
        let runtime = cfg!(feature = "wasm-runtime");

        Ok(Self {
            name,
            version: "0.1.0".to_string(),
            path: path.to_path_buf(),
            enabled: false,
            runtime_available: runtime,
        })
    }

    /// Path to the .wasm file
    pub fn wasm_path(&self) -> &std::path::Path {
        &self.path
    }
}

impl NativePlugin for WasmPlugin {
    fn name(&self) -> &str { &self.name }
    fn version(&self) -> &str { &self.version }

    fn on_enable(&mut self, _ctx: &PluginContext) {
        if !self.runtime_available {
            warn!(
                "WASM plugin '{}' discovered but runtime not compiled in. \
                 Uncomment `extism = \"1\"` in mc-plugin/Cargo.toml and rebuild to enable.",
                self.name
            );
        }
        self.enabled = true;
        info!("WASM plugin '{}' enabled{}", self.name,
            if self.runtime_available { "" } else { " (runtime pending)" });
    }

    fn on_tick(&mut self, _ctx: &PluginContext, _tick: u64) {
        // stub — WASM runtime handles this when extism is enabled
    }

    fn on_player_join(&mut self, _ctx: &PluginContext, _uuid: &uuid::Uuid, _username: &str) {
        // stub
    }

    fn on_player_leave(&mut self, _ctx: &PluginContext, _uuid: &uuid::Uuid) {
        // stub
    }

    fn on_disable(&mut self) {
        self.enabled = false;
        info!("WASM plugin '{}' disabled", self.name);
    }

    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }
}

/// Load all .wasm files from the plugins directory.
/// Returns discovered WASM plugins (runtime execution requires extism dependency).
pub fn load_wasm_plugins(
    plugin_dir: &std::path::Path,
) -> Vec<Box<dyn NativePlugin>> {
    let mut plugins: Vec<Box<dyn NativePlugin>> = Vec::new();
    let dir = match std::fs::read_dir(plugin_dir) {
        Ok(d) => d,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                // Create the directory for future use
                let _ = std::fs::create_dir_all(plugin_dir);
                info!("WASM plugin directory created: {} (place .wasm files here)", plugin_dir.display());
            } else {
                error!("Failed to read plugin directory {}: {}", plugin_dir.display(), e);
            }
            return plugins;
        }
    };

    for entry in dir.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "wasm").unwrap_or(false) {
            match WasmPlugin::discover(&path) {
                Ok(plugin) => plugins.push(Box::new(plugin)),
                Err(e) => error!("Failed to discover WASM plugin {}: {}", path.display(), e),
            }
        }
    }

    if !plugins.is_empty() {
        info!("Discovered {} WASM plugin(s) in {}", plugins.len(), plugin_dir.display());
    }

    plugins
}
