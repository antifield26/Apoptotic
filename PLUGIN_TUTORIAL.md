# WASM Plugin Development Guide

Apoptotic supports WebAssembly plugins via the [extism](https://extism.org/) runtime. Plugins are loaded from `.wasm` files in the `plugins/` directory.

## Quick Start

### 1. Enable WASM Runtime

```bash
cargo build --release --features mc-plugin/wasm-runtime
```

### 2. Create a Plugin (Rust)

```rust
// src/lib.rs
use extism_pdk::*;

#[plugin_fn]
pub fn name() -> FnResult<String> {
    Ok("my_first_plugin".to_string())
}

#[plugin_fn]
pub fn version() -> FnResult<String> {
    Ok("0.1.0".to_string())
}

#[plugin_fn]
pub fn on_enable() -> FnResult<()> {
    log::info!("My plugin enabled!");
    Ok(())
}

#[plugin_fn]
pub fn on_tick(tick: String) -> FnResult<()> {
    // Called every game tick — tick is the tick number as string
    let t: u64 = tick.parse().unwrap_or(0);
    if t % 1200 == 0 { // every minute
        log::info!("Server has been running for {} ticks", t);
    }
    Ok(())
}

#[plugin_fn]
pub fn on_player_join(input: String) -> FnResult<()> {
    // Format: "uuid|username"
    let parts: Vec<&str> = input.split('|').collect();
    log::info!("Welcome {} to the server!", parts.get(1).unwrap_or(&"unknown"));
    Ok(())
}

#[plugin_fn]
pub fn on_player_leave(uuid: String) -> FnResult<()> {
    log::info!("Player {} left", uuid);
    Ok(())
}

#[plugin_fn]
pub fn on_disable() -> FnResult<()> {
    log::info!("Plugin shutting down");
    Ok(())
}
```

### 3. Compile to WASM

```bash
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/my_plugin.wasm plugins/
```

### 4. Start Server

```bash
cargo run --release --features mc-plugin/wasm-runtime
# Output: Discovered WASM plugin: my_plugin (WASM v1) at plugins/my_plugin.wasm
```

## Plugin API Reference

### Required Exports

| Function | Signature | Description |
|----------|-----------|-------------|
| `name()` | `() -> String` | Plugin display name |
| `version()` | `() -> String` | Semantic version |
| `on_enable()` | `() -> ()` | Called when plugin is loaded |
| `on_tick(tick)` | `(String) -> ()` | Called every game tick |
| `on_player_join(input)` | `(String) -> ()` | Format: `"uuid\|username"` |
| `on_player_leave(uuid)` | `(String) -> ()` | Player UUID string |
| `on_disable()` | `() -> ()` | Called on server shutdown |

### NativePlugin Trait (Rust)

For plugins compiled directly into the server:

```rust
use mc_plugin::plugin::{NativePlugin, PluginContext};

struct MyPlugin;
impl NativePlugin for MyPlugin {
    fn name(&self) -> &str { "my_plugin" }
    fn version(&self) -> &str { "0.1.0" }
    fn on_enable(&mut self, ctx: &PluginContext) { /* access server state */ }
    fn on_tick(&mut self, ctx: &PluginContext, tick: u64) { }
    fn on_player_join(&mut self, ctx: &PluginContext, uuid: &uuid::Uuid, username: &str) { }
    fn on_player_leave(&mut self, ctx: &PluginContext, uuid: &uuid::Uuid) { }
    fn on_disable(&mut self) { }
    fn is_enabled(&self) -> bool { true }
    fn set_enabled(&mut self, enabled: bool) { }
}
```

## Plugin Ideas

- **Welcome Message**: Greet players on join with custom messages
- **Auto-Save Reminder**: Announce auto-saves in chat
- **Vote System**: Day/night/weather voting
- **Economy**: Track player balances and transactions
- **Mini-Games**: Spleef, parkour, PvP arenas
- **Discord Bridge**: Relay chat between Discord and in-game
