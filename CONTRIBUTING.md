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
cargo test
cargo clippy
```

## Crate Map

| Crate | Purpose | Key Files |
|-------|---------|-----------|
| `mc-core` | Foundation types | `item.rs` (registry), `biome.rs`, `constants.rs`, `effect.rs` |
| `mc-protocol` | Packet encoding | `packets/play.rs`, `registry.rs` (NBT), `codec.rs` |
| `mc-network` | Connection handling | `connection.rs` (C2S dispatch), `encryption.rs` |
| `mc-world` | World generation | `generator.rs`, `redstone.rs`, `chunk.rs`, `lighting.rs` |
| `mc-player` | Player & entity logic | `player.rs`, `mob.rs`, `recipe.rs`, `enchant.rs` |
| `mc-persistence` | Storage | `player_data.rs`, `world_saver.rs` |
| `mc-command` | Command system | `dispatcher.rs`, per-command files in `commands/` |
| `mc-admin` | Administration | `console.rs`, `rcon.rs` |
| `mc-plugin` | Plugin system | `plugin.rs`, `wasm.rs`, `datapack.rs` |
| `mc-server` | Entry point | `main.rs`, `tick.rs`, `config.rs`, `metrics.rs` |

## Adding Features

### Adding a New Item/Block

1. Add to `crates/mc-core/src/item.rs`: `m.insert("my_block", ID);`
2. If craftable, add recipe to `crates/mc-player/src/recipe.rs`
3. Run `cargo test test_recipe_result_items_exist` to verify

### Adding a New Entity AI

1. Add entity type constant in `crates/mc-core/src/constants.rs`
2. Add AI branch in `crates/mc-player/src/mob.rs` `tick_ai()` function
3. Use existing patterns: `if mob.mob_type == MY_TYPE && condition { ... }`

### Adding a New C2S Handler

1. Add handler branch in `crates/mc-network/src/connection.rs` play loop
2. If needed, add helper methods to `crates/mc-player/src/player.rs` PlayerManager
3. If new S2C response needed, add packet struct in `crates/mc-protocol/src/packets/play.rs`

### Adding a New Effect

1. Add to `crates/mc-core/src/effect.rs` EffectType enum
2. Wire in `crates/mc-player/src/player.rs` `tick_effects()` function
3. Add Player field if needed (e.g., multiplier)

## Code Style

- Rust edition 2024
- 0 clippy warnings required (`cargo clippy -- -D warnings`)
- Use `parking_lot` for sync, `DashMap` for concurrent maps
- Follow existing patterns for naming and structure
- No `unsafe` blocks without explicit approval

## Testing

```bash
cargo test                    # Full suite (170+ tests)
cargo test -p mc-player       # Specific crate
cargo clippy                  # Linting
```

Tests should cover:
- Recipe result validation
- Protocol encode/decode round-trips
- Entity AI branch coverage
- Effect wiring correctness

## Data Pipeline

The `scripts/` directory contains tools for data management:

- `update-minecraft-data.sh` — Fetch PrismarineJS data and generate Rust code
- `extract_items.py` — Parse PrismarineJS JSON → Rust item registry
- `dedup_registry.py` — Find and fix duplicate item registrations
- `optimize-profile.sh` — PGO profiling workflow
- `bolt-optimize.sh` — BOLT binary optimization

## Communication

- Issues: GitHub Issues
- Pull Requests: Standard fork-and-PR workflow
- Discussions: GitHub Discussions

## License

MIT — see LICENSE file.
