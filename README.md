# Apoptotic

Rust 实现的 Minecraft Java Edition 26.2 "Chaos Cubed" 局域网联机服务器，针对 Raspberry Pi 5 优化，支持 2~8 人。

[![CI](https://github.com/antifield26/Apoptotic/actions/workflows/ci.yml/badge.svg)](https://github.com/antifield26/Apoptotic/actions/workflows/ci.yml)

## 特性

- **协议**: Minecraft 26.2 "Chaos Cubed" (protocol 776)，73 S2C + 38 C2S Play 包处理器（全部 stub 已功能实现，含 SwingArm/EntityAnimation/Explosion 广播）
- **世界**: 59 种群系（含 26.2 Sulfur Caves），7 种生成器，14 种结构（含 Sulfur Springs），9 种树木，4 种地下群系
- **实体**: 155 种实体类型 (官方 26.2 registry ID)，~84 种独特 AI (91%)，含 Sulfur Cube 12 archetype
- **生存**: ~1,600 运行时配方（含 Stonecutter 48 + 26.2 Sulfur/Cinnabar/Potent/Waxed），附魔 42/42 连线 (100%)，酿造 50+（含 BrewedPotion 进度），村民 14 职业 + Gossip 声誉
- **红石**: 39 组件（含 Piston 3D 垂直, Daylight Detector 反转, Sculk Sensor 振动, Lightning Rod）
- **命令**: 67 个（+/ban-ip /pardon-ip /setidletimeout），@a/@p/@r/@s 选择器，/execute，tab 补全
- **进度**: 14 个定义 + 4 个连线（Brewing/Trade/Crossbow/Beacon）+ "Uh Oh" (26.2)
- **效果**: 40 种全定义 (官方 26.2, 0-based ID)，27 连线 — 含 TrialOmen/RaidOmen/WindCharged/Weaving/Oozing/Infested/BreathOfTheNautilus
- **26.2 Chaos Cubed**: Sulfur Cube archetype 完整，Potent Sulfur 气体+间歇泉，Sulfur Spike 坠落，Sulfur Springs 结构
- **安全**: 速率限制 + 路径防护 + Mojang 在线认证 + 2MB 封包限制 + 最大玩家数硬限制 + RCON SHA-1
- **插件**: NativePlugin trait + WASM 运行时（extism） + DatapackLoader，完整 PDK 文档
- **运维**: Docker 多架构，Prometheus + Grafana 监控，/status JSON 端点，systemd watchdog，自动备份，CI 三平台 + 安全审计

## 快速开始

```bash
cargo build --release
cargo run --release
# 客户端连接 localhost:25565
```

### Docker

```bash
docker build --platform linux/arm64 -t apopototic .
docker compose up -d
docker compose --profile monitoring up -d  # +Prometheus+Grafana
```

### Raspberry Pi 5

```bash
rustup target add aarch64-unknown-linux-gnu
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
  cargo build --release --target aarch64-unknown-linux-gnu
sudo cp scripts/minecraft-server.service /etc/systemd/system/
sudo systemctl enable --now minecraft-server
```

### 监控端点

```
GET /metrics   → Prometheus 指标
GET /health    → {"status":"ok","players":N,"chunks":N}
GET /status    → {"server":"Apoptotic","tps_p95":"20","memory_mb":128,...}
```

## 数据管道

```bash
./scripts/update-minecraft-data.sh 26.2           # diff 报告
./scripts/update-minecraft-data.sh 26.2 --apply   # 覆盖 item.rs
./scripts/benchmark.sh 120 4                       # 120s 4 玩家基准测试
```

## 架构

```
crates/
├── mc-server/        # 入口，tick(16子系统)，自动保存，插件，CPU affinity
├── mc-core/          # BlockState，ItemRegistry(1,694, 官方26.2 ID)，Effect(40)，Biome(54)，EntityType(155, 官方26.2 ID)
├── mc-protocol/      # VarInt，Codec，73 S2C/38 C2S，Registry NBT(62 biomes)，EntityAnimation/Explosion
├── mc-network/       # TCP，LAN广播，状态机，GUI dispatch(21容器)，rate_limiter，spatial index
├── mc-world/         # PalettedContainer，Chunk，7 Generator，LZ4，Lighting，Redstone(39)，Fluid，Physics
├── mc-player/        # PlayerManager，Inventory，Container，Recipe(~1600)，Mob(~84 AI)，Enchant(42/42)，Villager(14+Gossip)，Brewing，Fishing，Combat，Advancement(14)
├── mc-persistence/   # SQLite PlayerDB，WorldSaver(NBT)，LZ4 Linear
├── mc-command/       # 67 commands (+ban-ip/pardon-ip/setidletimeout)，/execute，/scoreboard，/bossbar，/team
├── mc-admin/         # Console，RCON(TCP 25575)
└── mc-plugin/        # NativePlugin trait，WASM(extism)，DatapackLoader
```

**技术栈**: Tokio async I/O，DashMap 无锁并发，parking_lot，jemalloc，Rayon 并行，LZ4 压缩

## 性能

- `target-cpu=cortex-a76`，NEON/SVE/LSE 指令集
- jemalloc + MALLOC_CONF 调优
- thread_local 缓存（PermutationTable，bitset）
- ChunkData `Arc<Vec<u8>>` 缓存
- Rayon par_iter spawn chunk 预生成
- spawn_blocking 异步 I/O
- LTO + strip + panic=abort
- **Spatial Hash Grid**: O(1) 邻近查询 (chunk_players 索引)
- **A* 路径缓存**: 64-entry LRU (chunk→chunk key)
- **CPU affinity**: tick/IO 线程绑核 (Linux sched_setaffinity)
- PGO/BOLT/benchmark 脚本（`scripts/`）

## 插件

详见 [PLUGIN_TUTORIAL.md](PLUGIN_TUTORIAL.md)

## 贡献

详见 [CONTRIBUTING.md](CONTRIBUTING.md)

## 已知限制

| 类别 | 覆盖率 |
|------|--------|
| 物品注册 | 1,694 / 1,694 (100%，官方 26.2 protocol ID) |
| 实体类型 | 155 / 158 (98%，官方 26.2 entity_type registry) |
| 配方 runtime | ~1,600 / ~1,700 (94%) |
| C2S 处理器 | 37 / 54 (69%，全部 stub 已功能实现) |
| 状态效果 | 40 / 40 (100% 定义)，27 连线 (68%) |
| 实体 AI | ~84 / ~92 (91%) |
| 附魔连线 | 42 / 42 (100%) |
| 红石组件 | 39 / ~50 (78%) |

## License

MIT
