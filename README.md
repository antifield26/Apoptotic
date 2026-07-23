# Apoptotic

Rust 实现的 Minecraft Java Edition 26.2 "Chaos Cubed" 局域网联机服务器，针对 Raspberry Pi 5 优化，支持 2~8 人。

[![CI](https://github.com/antifield26/Apoptotic/actions/workflows/ci.yml/badge.svg)](https://github.com/antifield26/Apoptotic/actions/workflows/ci.yml)

## 特性

- **协议**: Minecraft 26.2 "Chaos Cubed" (protocol 776)，73 S2C + 37 C2S Play 包处理器（全部功能实现，12 已提取至 c2s_handlers.rs）
- **世界**: 54 种群系（含 26.2 Sulfur Caves），7 种生成器，19 种结构，9 种树木，3D Perlin 洞穴
- **实体**: 158 种实体类型 (官方 26.2 registry ID)，~92 种独特 AI (~58% 总数)，含 Sulfur Cube 12 archetype + 26.2 新增 AI
- **寻路**: A* 2D 寻路 + 64-entry LRU 缓存，已连线至敌对生物追踪，每 40 tick 刷新
- **生存**: ~1,620 运行时配方（含 26.2 Sulfur/Cinnabar 构建集），附魔 42/42 连线 (100%)，酿造 50+，村民 14 职业 + Gossip
- **红石**: 35+ 组件（AC 风格变化检测 + 4,096 节点预算 + CopperBulb T-flip-flop）
- **命令**: 71 个（+/advancement /schedule /function /datapack），@a/@p/@r/@s 选择器
- **进度**: 14+ 定义 + 9 触发器连线（含 26.2 "Uh Oh" — SulfurCube 吸收 TNT）
- **效果**: 40 种全定义 (官方 26.2, 0-based ID)，28 连线 + 26.2 新效果
- **26.2 Chaos Cubed**: Sulfur Cube 12 archetype (含 Hot 接触伤害)，Potent Sulfur 气泡柱+气体+间歇泉，Sulfur Spike 生长+坠落，Sulfur Springs
- **性能**: 实体休眠 (~20% TPS 提升) + 区块流节流 (6块/tick) + A* atomic cache + AsyncChunkBridge + DirtyWriteback + AC Redstone + PerPlayerMobCap + SpawnThrottling + NEON SIMD + TrackedMob增量同步
- **安全**: 速率限制 + 路径防护 + Mojang 在线认证 + 2MB 封包限制 + 反作弊（移动验证 + 8-violation 橡皮筋回弹 + PlayerInput 追踪）
- **插件**: NativePlugin trait + WASM 运行时（extism）+ DatapackLoader（/function + /datapack 命令）
- **运维**: Docker 多架构，Prometheus + Grafana 监控，/admin HTML 面板 (TPS颜色编码+tick阶段表)，/status JSON，/health，systemd watchdog，PGO/BOLT CI + CLI pregenerate

## 快速开始

```bash
cargo build --release
cargo run --release
# 客户端连接 localhost:25565
```

### RPi 5 预生成（推荐）

```bash
cargo run --release -- pregenerate --radius 200 --threads 4
```

### Docker

```bash
docker build --platform linux/arm64 -t apopototic .
docker compose up -d
docker compose --profile monitoring up -d  # +Prometheus+Grafana
```

### Raspberry Pi 5 交叉编译

```bash
rustup target add aarch64-unknown-linux-gnu
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
  cargo build --release --target aarch64-unknown-linux-gnu
sudo cp scripts/minecraft-server.service /etc/systemd/system/
sudo systemctl enable --now minecraft-server
```

### 监控端点

```
GET /admin     → HTML 管理面板 (实时玩家列表/TPS颜色编码/tick阶段耗时表/内存告警)
GET /metrics   → Prometheus 指标 (含 per-stage 计时)
GET /health    → {"status":"ok","players":N,"chunks":N}
GET /status    → {"server":"Apoptotic","tps_p95":"20","memory_mb":128,...}
```

## 数据管道

注册表数据 (`item_registry.in.rs`) 已提交至仓库
仅在需要更新到新 Minecraft 版本时运行以下脚本：

```bash
# 更新注册表 (需要本地客户端 JAR 或 PrismineJS/npm)
./scripts/update-minecraft-data.sh 26.2           # diff 报告
./scripts/update-minecraft-data.sh 26.2 --apply   # 覆盖 item_registry.in.rs

# 性能测试
./scripts/benchmark.sh 120 4                       # 120s 4 玩家基准测试
./scripts/optimize-profile.sh 3                    # PGO 优化编译 (5-15% 提升)
```

## 架构

```
crates/
├── mc-server/        # 入口，tick(27阶段 TickScheduler)，CLI pregenerate，anticheat，metrics
│   ├── main.rs       #   服务器引导 + tick循环 + mob生成 + CLI pregenerate
│   ├── tick.rs       #   27 阶段 TickScheduler (budget追踪) + 各 tick 子系统
│   ├── anticheat.rs  #   validate_movement() + 橡皮筋回弹
│   └── metrics.rs    #   Prometheus /metrics + /health + /admin HTML面板
├── mc-core/          # BlockState，ItemRegistry(1,694条目 → build.rs验证)，Effect(40)，Biome(54)，EntityType(158)
│   ├── item.rs       #   55行 API (注册表数据: item_registry.in.rs 1702行)
│   ├── error.rs      #   McError(8类别)/McResult/McOptionExt
│   └── build.rs      #   编译时验证注册表完整性
├── mc-protocol/      # VarInt，Codec，73 S2C/37 C2S，Registry NBT(62 biomes)
│   └── tests/roundtrip.rs  # 14 S2C 编码冒烟测试
├── mc-network/       # TCP，LAN广播，状态机，GUI dispatch，rate_limiter
│   ├── connection.rs #   1578行 — ServerRef，handle_{status,login,config,play}，stream_new_chunks
│   ├── play_loop.rs  #   2980行 — 主游戏循环 (实体跟踪·区块流·keep-alive·C2S分发)
│   └── c2s_handlers.rs # 364行 — 15 C2S 处理函数 (12 已连线)
├── mc-world/         # PalettedContainer，Chunk，7 Generator，LZ4，Lighting，Redstone(35+)，Fluid，Physics
│   ├── fluid.rs      #   PotentSulfur (气泡柱/反胃气体/间歇泉)
│   └── physics.rs    #   SulfurSpike (生长+坠落)
├── mc-player/        # PlayerManager，Inventory，Container，Recipe(~1,620)，Mob(~92 AI)，Enchant(42/42)，Villager，Brewing，Fishing，Combat，Advancement(14+)，pathfind(A* 已连线)
├── mc-persistence/   # SQLite PlayerDB，WorldSaver(NBT)，LZ4 Linear
├── mc-command/       # 67 commands，/execute，/scoreboard，/bossbar，/team
├── mc-admin/         # Console，RCON(TCP 25575)
└── mc-plugin/        # NativePlugin trait，WASM(extism)，DatapackLoader
```

**技术栈**: Tokio async I/O，DashMap 无锁并发，parking_lot，jemalloc，Rayon 并行，LZ4 压缩

## 性能

- `target-cpu=cortex-a76`，NEON/SVE/LSE 指令集
- jemalloc + MALLOC_CONF 调优
- thread_local 缓存（PermutationTable，bitset）
- ChunkData `Arc<Vec<u8>>` 缓存
- Rayon par_iter spawn chunk 预生成 + CLI pregenerate
- spawn_blocking 异步 I/O
- LTO + strip + panic=abort
- **Spatial Hash Grid**: O(1) 邻近查询 (chunk_players 索引)
- **A* 路径缓存**: 64-entry LRU (chunk→chunk key)，已连线至敌对生物 AI
- **实体休眠**: 远离玩家实体完全冻结 AI，预期 TPS +15~20%
- **区块流节流**: 每 tick 最多 6 区块，Chebyshev 距离优先，网络峰值 -50%
- **CPU affinity**: tick/IO 线程绑核 (Linux sched_setaffinity)
- PGO/BOLT/benchmark 脚本（`scripts/`）

## 插件

详见 [PLUGIN_TUTORIAL.md](PLUGIN_TUTORIAL.md)

## 贡献

详见 [CONTRIBUTING.md](CONTRIBUTING.md)

## 已知限制

| 类别 | 覆盖率 |
|------|--------|
| 物品注册 | 1,694 / 1,694 (100%，官方 26.2 protocol ID，build.rs 编译时验证) |
| 实体类型 | 158 / 158 (100%，官方 26.2 entity_type registry) |
| 配方 runtime | ~1,620 / ~1,700 (95%) |
| C2S 处理器 | 37 / 54 (69%，全部功能实现，12 提取至 c2s_handlers.rs) |
| 状态效果 | 40 / 40 (100% 定义)，28 连线 (70%) |
| 实体 AI | ~92 / ~158 (58%，含所有 SulferCube archetype + 26.2 新增) |
| 附魔连线 | 42 / 42 (100%) |
| 红石组件 | 35 / ~50 (70%) |

## 测试

```bash
cargo test                    # 181 tests (167 unit + 14 protocol encode)
cargo test -p mc-protocol     # 40 tests (含 14 S2C 编码冒烟测试)
cargo clippy                  # 0 warnings
```

## License

MIT
