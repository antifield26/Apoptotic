# Apoptotic

Rust 实现的 Minecraft Java Edition 26.2 局域网联机服务器，针对 Raspberry Pi 5 优化，支持 2~8 人。

[![CI](https://github.com/antifield26/Apoptotic/actions/workflows/ci.yml/badge.svg)](https://github.com/antifield26/Apoptotic/actions/workflows/ci.yml)

## 特性

- **协议**: Minecraft 26.2 (protocol 776)，73 S2C + 37 C2S Play 包处理器（全部 stub 已功能实现）
- **世界**: 59 种群系（含 26.2 Sulfur Caves），7 种生成器，13 种结构，9 种树木，4 种地下群系
- **实体**: 91 种实体类型，~74 种独特 AI，含忠诚三叉戟/火弩烟花/弹射物附魔/驯服/繁殖/骑乘
- **生存**: 1534 运行时配方 + 480 source 定义（含旗帜/烟花/锻造纹饰/药箭），熔炉，附魔 42 种，酿造 50+，村民 14 职业完整交易，钓鱼，战斗
- **红石**: 35 组件（含压力板/拌线钩/比较器减法/观察者/hopper 传输/容器填充率检测）
- **命令**: 63 个，@a/@p/@r/@s 选择器，/execute，tab 补全
- **效果**: 27/33 连线（82%），含 Speed/Slowness/Haste/MiningFatigue/JumpBoost/Luck/Unluck/ConduitPower/SlowFalling/DolphinGrace/Blindness
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
├── mc-server/        # 入口，tick(15子系统)，自动保存，插件
├── mc-core/          # BlockState，ItemRegistry(1055)，Effect(33)，Biome(59)，EntityType(91)
├── mc-protocol/      # VarInt，Codec，73 S2C/37 C2S，Registry NBT(62 biomes)
├── mc-network/       # TCP，LAN广播，状态机，GUI dispatch(21容器)，rate_limiter
├── mc-world/         # PalettedContainer，Chunk，7 Generator，LZ4，Lighting，Redstone，Fluid
├── mc-player/        # PlayerManager，Inventory，Container，Recipe(1534)，Mob(~74 AI)，Enchant(42)，Villager(14)，Brewing，Fishing，Combat
├── mc-persistence/   # SQLite PlayerDB，WorldSaver(NBT)，LZ4 Linear
├── mc-command/       # 63 commands，/execute，/scoreboard，/bossbar，/team
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
- PGO/BOLT/benchmark 脚本（`scripts/`）

## 插件

详见 [PLUGIN_TUTORIAL.md](PLUGIN_TUTORIAL.md)

## 贡献

详见 [CONTRIBUTING.md](CONTRIBUTING.md)

## 已知限制

| 类别 | 覆盖率 |
|------|--------|
| 物品注册 | 1055 / ~2200 (48%) |
| 配方 source | 480 / ~1700 (28% — runtime 1534 覆盖 90%) |
| C2S 处理器 | 37 / 54 (69%，全部 stub 已功能实现) |
| 状态效果 | 27 / 33 (82%) |
| 实体 AI | ~74 / 91 (81%) |

## License

MIT
