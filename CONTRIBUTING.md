# Contributing to Apoptotic

Thanks for your interest in contributing! This document outlines how to get started.

## Project Overview

Apoptotic is a Rust-based Minecraft Java Edition 26.2 LAN server optimized for Raspberry Pi 5. The codebase uses 10 crates in a workspace, with DashMap lock-free concurrency and jemalloc allocation.

## Getting Started

```bash
# Clone and build
git clone https://github.com/antifield26/Apoptotic.git
cd Apoptotic
cargo build
cargo test     # 181 tests
cargo clippy   # 0 warnings required
```

## Crate Map

| Crate | Purpose | Key Files |
|-------|---------|-----------|
| `mc-core` | Foundation types | `item.rs` (55行 API + item_registry.in.rs 1702行数据), `error.rs` (McError/McResult), `biome.rs`, `constants.rs`, `effect.rs`, `build.rs` |
| `mc-protocol` | Packet encoding | `packets/play.rs`, `registry.rs` (NBT), `codec.rs`, `tests/roundtrip.rs` (14 编码测试) |
| `mc-network` | Connection handling | `connection.rs` (1578行 — 状态机), `play_loop.rs` (2980行 — 游戏循环), `c2s_handlers.rs` (364行 — C2S 处理函数), `encryption.rs` |
| `mc-world` | World generation | `generator.rs`, `redstone.rs`, `chunk.rs`, `lighting.rs`, `fluid.rs` (PotentSulfur), `physics.rs` (SulfurSpike) |
| `mc-player` | Player & entity logic | `player.rs`, `mob.rs` (含 A* 路点消耗 + 实体休眠), `pathfind.rs`, `recipe.rs`, `enchant.rs` |
| `mc-persistence` | Storage | `player_data.rs`, `world_saver.rs` |
| `mc-command` | Command system | `dispatcher.rs`, per-command files in `commands/` |
| `mc-admin` | Administration | `console.rs`, `rcon.rs` |
| `mc-plugin` | Plugin system | `plugin.rs`, `wasm.rs`, `datapack.rs` |
| `mc-server` | Entry point | `main.rs`, `tick.rs` (27阶段 TickScheduler), `config.rs`, `metrics.rs` (/admin + /metrics), `anticheat.rs` |

## Adding Features

### Adding a New Item/Block

1. Run `./scripts/update-minecraft-data.sh 26.2 --apply` to regenerate `item_registry.in.rs`
2. If craftable, add recipe to `crates/mc-player/src/recipe.rs`
3. Run `cargo test -p mc-core` to verify registry integrity (build.rs validates)

### Adding a New Entity AI

1. Add entity type constant in `crates/mc-core/src/constants.rs`
2. Add AI branch in `crates/mc-player/src/mob.rs` `tick_ai()` function
3. Use existing patterns: `if mob.mob_type == MY_TYPE && condition { ... }`
4. If the mob needs pathfinding, the system is already wired — set `MobAiState::Chasing` and `tick_mob_pathfinding()` handles the rest

### Adding a New C2S Handler

1. Create handler function in `crates/mc-network/src/c2s_handlers.rs`:
   ```rust
   pub async fn handle_my_packet(io: &mut PacketStream, server: &ServerRef, uuid: &Uuid, frame: &[u8]) { ... }
   ```
2. Wire in `connection.rs` play_loop match:
   ```rust
   0xNN => { crate::c2s_handlers::handle_my_packet(io, server, &_uuid, &frame).await; }
   ```
3. If the handler needs `continue;` control flow, return `bool`:
   ```rust
   pub fn handle_my_packet(...) -> bool { ... }
   // Usage: if !crate::c2s_handlers::handle_my_packet(...) { continue; }
   ```
4. If needed, add helper methods to `crates/mc-player/src/player.rs` PlayerManager
5. If new S2C response needed, add packet struct in `crates/mc-protocol/src/packets/play.rs`

### Adding a New Effect

1. Add to `crates/mc-core/src/effect.rs` EffectType enum
2. Wire in `crates/mc-player/src/player.rs` `tick_effects()` function
3. Add Player field if needed (e.g., multiplier)

### Adding a New Protocol Test

1. Add test in `crates/mc-protocol/tests/roundtrip.rs`:
   ```rust
   #[test] fn s2c_my_packet() { encode_ok(&MyPacket { ... }); }
   ```

## Code Style

- Rust edition 2024
- 0 clippy warnings required (`cargo clippy -- -D warnings`)
- Use `parking_lot` for sync, `DashMap` for concurrent maps
- Follow existing patterns for naming and structure
- No `unsafe` blocks without explicit approval
- For error handling, use `McError`/`McResult` from `mc_core::error` in new code

## Testing

```bash
cargo test                      # Full suite (181 tests)
cargo test -p mc-protocol       # 40 tests (含 14 协议编码冒烟测试)
cargo test -p mc-player         # Specific crate
cargo test -p mc-core           # Registry + effect + biome tests
cargo clippy                    # Linting
```

Tests should cover:
- Recipe result validation
- Protocol encode smoke tests (no panic)
- Entity AI branch coverage
- Effect wiring correctness
- Registry integrity (build.rs validates at compile time)

## Architecture Decisions

### connection.rs Split
- **connection.rs** (1578行): ServerRef struct, state machine (handshake/status/login/config/play)
- **play_loop.rs** (2980行): Main game loop — entity tracking, chunk streaming, keep-alive, C2S dispatch
- **c2s_handlers.rs** (364行): Individual C2S handler functions (15 extracted, 12 wired)

### Registry Generation
- **item.rs**: Thin API layer (55行)
- **item_registry.in.rs**: Generated HashMap (1702行) — regenerated via `scripts/update-minecraft-data.sh`
- **build.rs**: Compile-time validation (entry count, structure integrity)

### Error Handling
- **mc_core::error**: `McError` enum (8 categories) + `McResult<T>` alias + `McOptionExt` trait
- New code should use these types; existing unwrap() calls are in safe contexts

## Data Pipeline

The `scripts/` directory contains tools for data management:

- `update-minecraft-data.sh` — Fetch PrismarineJS data and generate Rust code
- `extract_items.py` — Parse PrismarineJS JSON → Rust item registry
- `dedup_registry.py` — Find and fix duplicate item registrations
- `optimize-profile.sh` — PGO profiling workflow (enhanced with pregeneration + login simulation)
- `bolt-optimize.sh` — BOLT binary optimization

## Communication

- Issues: GitHub Issues
- Pull Requests: Standard fork-and-PR workflow
- Discussions: GitHub Discussions

## License

MIT — see LICENSE file.
