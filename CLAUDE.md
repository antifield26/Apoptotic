# Minecraft LAN Server

Rust 实现的 Minecraft Java Edition 局域网 2~8 人联机服务器，针对 Raspberry Pi 5 (8GB, Debian 13 ARM64) 优化。

## 项目状态

- **完成度**: 核心玩法完整, 多人联机就绪
- **测试**: 162+ tests (0 failures), 10 ignored (E2E + doc-tests)
- **代码量**: ~37,000 行 Rust, 10 crates, edition 2024
- **协议**: Minecraft 26.2 (protocol 776), 73 S2C + 37 C2S Play packet handler 分支 (31 完整 + 6 stub)
- **物品注册**: 1038 个唯一物品/方块, 0 个重复条目
- **配方**: 1534 运行时配方 (shaped + shapeless), 0 个 result_item 不匹配
- **附魔**: 42 种注册 (100%), 39 种连线 (93%)
- **效果**: 33 种定义, 24 种连线 (73%) — Luck/Unluck(钓鱼) + ConduitPower(溺水免疫) + Speed/Slowness/MiningFatigue/Haste(倍率实际消费)
- **命令**: 63 commands, @a/@p/@r/@s selectors, OP permissions, /execute, tab completion
- **红石**: 35 组件 (压力板×4 + 拌线钩 + 比较器减法模式 + TargetBlock 等)
- **实体 AI**: ~67 种有独特 AI (70%) — 新增 7 种类属敌对 (Zombie/BOGGED/Stray/Husk/WitherSkeleton/Vindicator/PiglinBrute)
- **Clippy**: 0 warnings

## 架构

```
crates/
├── mc-server/        # 入口 — main, config, context, tick(15子系统), 自动保存, 插件
├── mc-core/          # 基础 — BlockState, ItemRegistry(1038), Effect(33), Biome(54), constants(96 EntityType)
├── mc-protocol/      # 协议 — VarInt, Codec, 72 S2C/37 C2S, Registry NBT(62 biomes), DeclareCommands(63)
├── mc-network/       # 网络 — TCP, LAN广播, 状态机, keep-alive, 区块流, GUI dispatch(21容器), rate_limiter
├── mc-world/         # 世界 — PalettedContainer, Chunk, 7 Generator, LZ4, Lighting, Redstone, Fluid
├── mc-player/        # 玩家 — PlayerManager, Inventory, Container, Recipe(1534), Mob(~60 AI), Enchant, Villager, Brewing, Fishing, Anvil, Beacon, Breeding, Taming, Advancement, Raid, pathfind
├── mc-persistence/   # 持久化 — SQLite PlayerDB, WorldSaver(NBT), LZ4 Linear
├── mc-command/       # 命令 — 63 commands, /execute, /scoreboard, /bossbar, /team
├── mc-admin/         # 管理 — Console, RCON(TCP 25575, SHA-1)
└── mc-plugin/        # 插件 — NativePlugin trait, WASM runtime (extism), DatapackLoader
```

**技术栈**: DashMap lock-free concurrency, parking_lot sync primitives, jemalloc allocator, Rayon parallelism, Tokio async I/O

**安全**: 速率限制 (5/min/IP + 20/s 包, TTL 清理) + 路径遍历防护 + RCON SHA-1 + 输入边界校验 + 封包大小限制(2MB) + 最大玩家数硬限制 + Mojang 在线模式认证

## 核心系统

### 世界生成
- **群系**: 54 种 (含 26.2 Sulfur Caves), 62 种发送给客户端, 温度/湿度/海拔 3D 噪声分布
- **地下群系**: DeepDark, LushCaves, DripstoneCaves, SulfurCaves — Y 轴采样 (sample_biome_at_y)
- **地形**: Perlin 3D 分形布朗运动 (4 octaves), 群系感知表面 + Deepslate 层
- **洞穴**: 3D Perlin 噪声洞穴 (双 octave 分支)
- **矿石**: 8 种 (煤/铁/金/钻石/铜/红石/青金石/绿宝石) + 深层变体
- **树木**: 9 种 (橡树/云杉/桦树/丛林木/金合欢/深色橡木/红树/樱花/苍白橡木)
- **结构**: 13 种 (村庄/沙漠神殿/丛林神庙/沼泽小屋/冰屋/矿井/海底遗迹/下界要塞/末地城/沉船/废弃传送门/试炼密室/远古城市)
- **生成器**: 7 种 (flat/noise/empty/nether/end/custom/compose)

### 实体系统
- **类型**: 96 种常量, ~60 种有独特 AI (63%)
- **AI**: 追逐/漫游/空闲/自爆 + Boss (Wither/EnderDragon) + 远程 (骷髅/恶魂/烈焰人/溺尸/守卫者) + 村民(交易/补货) + 驯服(8种) + 繁殖(16种) + 新增10种被动 (Squid/GlowSquid/Pufferfish/PolarBear/Turtle/Camel/Sniffer/Frog/Armadillo/Parrot)
- **弹射物**: 12 种类型, 附魔支持 (Power/Flame/Punch/Piercing/Channeling)
- **武器**: 弓 (Power/Flame/Punch/Infinity), 弩 (QuickCharge/Multishot/Piercing), 三叉戟 (Loyalty/Riptide/Channeling/Impaling)
- **实体激活范围** (PaperMC 风格): hostile=48, passive=32, ambient=24 blocks
- **生物生成**: 敌对 20 类型 (群系感知), 被动 ~15 类型, 生成上限 + 光照验证
- **寻路**: A* 2D 16 格 + 缓存

### 生存系统
- **饥饿**: 完整消耗/恢复/饥饿伤害 + 36 种食物营养值
- **经验**: 原版 3 层等级公式 + 经验球吸收 + 附魔台/铁砧 XP 消耗
- **状态效果**: 33 种定义, 21 种连线 (Strength/Weakness/Resistance/Regeneration/Poison/Wither/Absorption/Levitation/SlowFalling/Haste/BadOmen/HeroOfTheVillage/Darkness/ConduitPower/HealthBoost + Speed/Slowness/MiningFatigue 已连线到 Player modifier)
- **合成**: 2×2 背包 + 3×3 工作台, 1534 配方, 10 木材变体, 16 色染色×12 类别
- **熔炉**: 14 燃料 + 34 熔炼配方
- **酿造**: 50+ 配方 + BrewingStandManager
- **钓鱼**: 完整浮漂投射/收线/战利品表 + Lure/LotS 修正
- **铁砧**: 合并/修复/重命名 + 附魔书合并
- **信标**: 4 层金字塔检测 + GUI + 矿物支付
- **锻造台**: 下界合金升级 + 盔甲纹饰
- **村民**: 14 职业 + 2-4 级交易 + 自动补货 (2400tick)
- **进度**: 9/9 触发器连线

### 战斗系统
- **PvP/PvE**: 武器伤害表 + 1.9+ 攻击冷却 + 暴击 + 0.5s 无敌帧
- **横扫之刃**: 剑+地面+冷却≥0.848 → 2.5 block AOE
- **盾牌**: 180° 正面弧检测 + 斧攻击→1.6s 禁用
- **护甲减伤**: `effective = min(20, max(armor*0.2, armor - 4*dmg/(toughness+8)))`
- **环境伤害**: 虚空/火焰/溺水/坠落/窒息/闪电 — 完整公式
- **爆炸**: 苦力怕/TNT 范围破坏 + 衰减伤害
- **弹射物**: 12 种 + 附魔效果 + 喷溅药水范围效果
- **死亡掉落**: 背包+盔甲+经验球 (50% XP)

### 容器/GUI (21 类型)
- 6 种点击模式 (左/右, Shift, 热键栏, 丢弃, 拖拽, 双击收集)
- cursor_item 光标追踪 + state_id 校验
- 持久化: containers.bin v2 + 原子写入 (.tmp→rename)

### 红石
- 线缆传播: 信号强度 0-15, BFS 衰减, DashMap 无锁
- 准连接性 (QC): 活塞/发射器/投掷器检测上方方块信号
- 比较器: 容器填充率检测 + 减法模式
- 观察者: 方块状态变化 → 1 tick 脉冲
- 压力板 + 拌线钩: 实体检测 provider 模式
- 组件: 35 种 (红石火把/中继器/活塞/发射器/投掷器/按钮×11/ DaylightDetector/SculkSensor/TargetBlock/SculkShrieker 等)

### 流体 & 维度
- **水/熔岩**: BFS 传播 + 无限水源 + 含水方块 + 气泡柱
- **下界**: NetherGenerator, 5 种群系, 下界要塞
- **末地**: EndGenerator, 5 种群系, 浮空岛屿, 末地城
- **传送门**: 80 tick 冷却 + 坐标缩放 (÷8↔×8)

### 鞘翅飞行 & 袭击
- 鞘翅滑翔 + 烟花推进 (boost=1.5 blocks/tick)
- 袭击系统: BadOmen 触发 → 波次生成 + 巡逻队 + HeroOfTheVillage 奖赏

### 其他系统
- **睡眠**: 夜晚跳过→天亮满血
- **Piglin 以物易物**: 金锭右键→8 种随机战利品
- **铜氧化/蜡化**: 所有铜变体随时间氧化 + 蜜脾右键蜡化
- **闪电**: 雷暴 1/100k/tick + Channeling 三叉戟
- **唱片机**: 右键插入/弹出 + 跨重启追踪
- **统计系统**: 12 种统计类型
- **插件系统**: NativePlugin trait (CorePlugin) + WASM (extism, `--features wasm-runtime`) + DatapackLoader

### 持久化
- SQLite 玩家存档 (WAL 模式) + OP + Ban + Whitelist
- Inventory BLOB v2 格式 (41 槽 + NBT + durability)
- level.dat 标准 NBT (GZip 压缩), DataVersion=19133
- LZ4 Linear 区块存储 (ARM NEON 自动向量化)
- 自动保存 (每 6000 tick) + 优雅关闭

### 多人同步
- Scoreboard/Team/BossBar 广播同步
- ChunkData Arc 缓存 + 修改自动失效
- UpdateSectionBlocks + DirtyBlockTracker
- 实体距离过滤 + 玩家列表 Tab (keep-alive RTT)

## RPi 5 性能优化

- **Rayon 并行**: spawn chunk 预生成 par_iter + Chebyshev 距离排序
- **LZ4 压缩**: ARM NEON 自动向量化 (target-cpu=cortex-a76)
- **Perlin 3D**: thread_local PermutationTable 缓存
- **光照 BFS**: thread_local bitset 复用 (96KB→12KB, 零分配)
- **AtomicI32 实体 ID**: lock-free 替代 RwLock
- **编码缓冲**: Vec::with_capacity + varint 零分配 + Zlib 32KB 预分配
- **异步 I/O**: spawn_blocking 卸载磁盘写入
- **锁统一**: 全部 parking_lot, DashMap 无锁并发
- **jemalloc**: Linux 全局分配器 + MALLOC_CONF 调优
- **LTO + strip + panic=abort**: 最小化 release 二进制
- **PGO/BOLT**: 性能调优脚本就绪 (scripts/optimize-profile.sh, bolt-optimize.sh)

## 运维

- **Docker**: 多架构 (amd64 + arm64), 多阶段构建, HEALTHCHECK
- **docker-compose**: 服务器 + Prometheus + Grafana
- **systemd**: minecraft-server.service + watchdog (15s ping)
- **备份**: backup.sh (增量 full/incr + SHA256 + 30 天保留)
- **监控**: Prometheus `/metrics` + `/health` (JSON + 503)
- **CI/CD**: GitHub Actions x86_64-linux + aarch64-linux + aarch64-darwin
- **数据管道**: scripts/extract_items.py + update-minecraft-data.sh (PrismarineJS → Rust)

## 常用命令

```bash
cargo build                     # debug
cargo build --release           # release (LTO fat + strip + panic=abort)
cargo test                      # 162+ tests (0 failures)
cargo clippy                    # 0 warnings
cargo run                       # 启动 (:25565)

# WASM 插件
cargo build --features mc-plugin/wasm-runtime   # 启用 WASM 插件运行时

# 交叉编译 (RPi5 ARM64)
rustup target add aarch64-unknown-linux-gnu
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
  cargo build --release --target aarch64-unknown-linux-gnu

# Docker
docker build --platform linux/arm64 -t mc-server .
docker compose up -d
docker compose --profile monitoring up -d  # +Prometheus+Grafana
```

## 已知限制

- **物品注册**: 1038/~2200+ (47%), 较多装饰/特殊方块未注册
- **配方**: 1534/~1700 (90%), 部分特殊配方 (旗帜图案/烟花/染色/锻造纹饰) 未实现
- **C2S 处理器**: 37/54 (69%), 31 完整 + 6 stub (TeleportConfirm/ClientInfo/MessageAck/LockDifficulty/CommandSuggestions 已升级), 缺失 JigsawGenerate/StructureBlock/CommandBlock/ChunkBatchResponse 等
- **效果**: 24/33 连线 (73%), Luck/Unluck/ConduitPower/Speed/Slowness/MiningFatigue/Haste 已连线, DolphinGrace/JumpBoost/SlowFalling 部分连线, 纯客户端视觉未连线
- **实体 AI**: ~67/96 (70%), 新增 7 种类属敌对 AI (Zombie/BOGGED/Stray/Husk/WitherSkeleton/Vindicator/PiglinBrute), 约 15 种仅通用漫游/游泳
- **实体 ID**: 自定义范围 111-132 非官方 vanilla ID, 需与 26.2 entities.json 核对
- **RPi 5**: target-cpu=cortex-a76, NEON/SVE/LSE, chunk_threads=3, PGO/BOLT 需手动运行
