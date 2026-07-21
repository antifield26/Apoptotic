# Apoptotic — 全面评估与长期完善方案

> **评估日期**: 2026-07-21
> **实施完成日期**: 2026-07-21
> **实施状态**: ✅ 37/38 项完成 (97%)
> **测试状态**: 167 passed, 0 failed, 10 ignored (E2E + doc-tests)

---

## 目录

1. [总体评估摘要](#1-总体评估摘要)
2. [Vanilla 26.2 完整性检查](#2-vanilla-262-完整性检查)
3. [社区开源项目参考](#3-社区开源项目参考)
4. [Raspberry Pi 5 优化评估](#4-raspberry-pi-5-优化评估)
5. [长期完善路线图](#5-长期完善路线图)
6. [Phase 详解](#6-phase-详解)

---

## 1. 总体评估摘要

### 项目定位

Apoptotic 是一个 **相当成熟** 的 Rust Minecraft 26.2 服务端实现，代码量 ~33,000 行，10 crates。其在同类 Rust 项目中处于领先地位——Valence 停留在 1.20.1 alpha，Pumpkin 仍在积极开发中但功能覆盖率不到 50%。本项目在 **vanilla 完整度** 和 **RPi 5 针对性优化** 两方面均有独特优势。

### 核心指标

| 维度 | 覆盖率 | 评级 | 说明 |
|------|--------|------|------|
| 物品注册 | 1,694/1,694 (100%) | **A+** | 官方 26.2 protocol ID，来自本地客户端 dump |
| 实体类型 | 155/158 (98%) | **A** | 官方 26.2 entity_type registry ID |
| 附魔连线 | 42/42 (100%) | **A+** | 全部 42 种注册并连线 |
| 实体 AI | ~84/~92 (91%) | **A-** | 8 种缺失 AI 的实体 |
| 配方覆盖率 | ~1,600/~1,700 (94%) | **A-** | 缺失约 100 条，以旗帜/烟花/纹饰为主 |
| C2S 处理器 | 37/54 (69%) | **B+** | 17 个缺失均为管理/创意模式包，核心玩法全覆盖 |
| 状态效果 | 40/40 定义 (100%), 27 连线 (68%) | **B** | 7 种 26.2 新增 + 5 种经典效果待连线 |
| 红石组件 | ~39/~50 (78%) | **B** | 核心组件完整，缺失部分高级组件 |
| 命令 | 67/80+ (83%) | **B+** | 核心命令完整，缺失 datapack/advancement 等 |
| 进度触发器 | 9/17 (53%) | **C+** | 定义完整但仅 9 种触发连线 |
| 结构生成 | 14/~20 (70%) | **B** | 缺失 Bastion/PillagerOutpost/WoodlandMansion 等 |

**综合完整度**: ~85%
**社区 Rust 项目对比**: 领先 Pumpkin (~50%)，远超 Valence (框架模式，不含 gameplay)

### 补充发现：本地 Minecraft 26.2 客户端数据

**已确认本地安装了完整的 Minecraft 26.2 客户端**，位于 `D:\HMCL\.minecraft`：

| 资源 | 路径 | 大小 | 用途 |
|------|------|------|------|
| 26.2 客户端 JAR | `versions/26.2-Fabric-Caustica/26.2-Fabric-Caustica.jar` | 39 MB | 协议参考 |
| Registry Dump | `generated/reports/reports/registries.json` | 529 KB | **95 个注册表, 7,074 protocol_id 条目** |
| Block States | `generated/reports/reports/blocks.json` | 6.8 MB | 完整方块状态 + 数值 ID |
| 网络包协议 ID | `generated/reports/reports/packets.json` | 18.7 KB | 所有阶段的包 ID 映射 |
| 物品组件 | `generated/reports/minecraft/components/item/` | 1,537 个文件 | 每个物品的默认数据组件 |
| 命令树 | `generated/reports/reports/commands.json` | 480 KB | 完整命令语法树 |
| 数据包注册表列表 | `generated/reports/reports/datapack.json` | 16.5 KB | 147 个注册表元数据 |

> **关键发现**: `registries.json` 是从 26.1.2 生成的（2026-07-21 生成，最新），包含 95 个注册表中 7,074 个条目的 protocol_id。这是进行精确注册表对账的权威参考数据。26.2 客户端 JAR 也已本地就绪。**服务器已经使用这个 dump 作为数据源**（CLAUDE.md 确认了 1,694 个条目来自本地客户端 registry dump）。

---

## 2. Vanilla 26.2 完整性检查

### 2.1 协议层 (S2C/C2S)

**S2C Play 包**: 73 个定义完整（[play.rs](crates/mc-protocol/src/packets/play.rs)），包括：
- 基础: JoinGame, PlayerPosition, KeepAlive, SystemChatMessage, PlayDisconnect
- 世界: ChunkData, BlockUpdate, UpdateSectionBlocks, Explosion, WorldEvent
- 实体: SpawnEntity, SpawnPlayer, RemoveEntities, SetEntityMetadata, TeleportEntity
- 容器: OpenScreen, ContainerSetContent, SetContainerSlot, ContainerSetData
- 状态: SetHealth, SetExperience, PlayerAbilities, UpdateAttributes
- UI: BossBar, Scoreboard, Teams, SetTitles, UpdateAdvancements
- 26.2 新增: Transfer, CookieRequest/Response, ServerLinks, UpdateEnabledFeatures

**C2S Play 处理器**: 37 个完整功能实现（[connection.rs:1841-4578](crates/mc-network/src/connection.rs)），包括：

| Packet ID | 名称 | 实现方式 |
|-----------|------|---------|
| 0x00 | Confirm Teleportation | 完整校验 |
| 0x01 | Message Acknowledgment | typed decode |
| 0x05 | Chat Command | typed decode + 命令路由 |
| 0x07 | Chat Message | raw parse + 广播 |
| 0x08 | Command Suggestions | raw parse + 空响应 |
| 0x09 | Container Click | 完整 typed decode |
| 0x0A | Client Command | typed decode + 重生 |
| 0x0C | Client Information | typed decode |
| 0x0E | Edit Book | raw parse + NBT |
| 0x0F | Container Close | typed decode |
| 0x10 | Lock Difficulty | OP 校验 |
| 0x11 | Advancement Tab | raw parse |
| 0x12 | Rename Item | raw parse (铁砧) |
| 0x13 | Recipe Book Data | raw parse |
| 0x16 | Cookie Response | typed decode |
| 0x17 | Pick Item | raw parse (创造) |
| 0x18 | Interact Entity | raw parse |
| 0x19 | Paddle Boat | raw parse |
| 0x1A | Keep Alive | raw parse + RTT |
| 0x1B | Place Recipe | typed decode |
| 0x1C | Player Position | raw parse + 反作弊 |
| 0x1D | Player Position And Rotation | raw parse + 方块交互 |
| 0x1E | Player Rotation | raw parse |
| 0x1F | Player Command | typed decode (潜行/疾跑) |
| 0x20 | Vehicle Move | raw parse |
| 0x21 | Client Tick End | raw parse (延迟追踪) |
| 0x23 | Select Trade | raw parse + 进度触发 |
| 0x24 | Resource Pack Response | typed decode |
| 0x25 | Pick From Block | raw parse |
| 0x27 | Player Action (挖掘) | raw parse |
| 0x29 | Player Input | typed decode |
| 0x2B | Set Beacon | legacy confirm |
| 0x2C | Update Sign | raw parse + 验证 |
| 0x33 | Set Held Item | raw parse |
| 0x36 | Set Creative Mode Slot | raw parse |
| 0x3E | Use Item On | raw parse (方块) |
| 0x3F | Use Item | raw parse (钓鱼/弹射物) |

**缺失的 17 个 C2S 包**（均为管理/创意/开发工具，不影响核心玩法）：
- JigsawGenerate, StructureBlock, CommandBlock 更新系列 (6个)
- ChunkBatchResponse, DebugSample, PingRequest (3个)
- RecipeBookSeen, TeleportToEntity (2个)
- TestInstanceBlock, 及其他管理包 (6个)

**评估**: 核心玩法 C2S 覆盖率接近 100%，缺失的包主要属于结构方块/拼图方块/命令方块等管理工具，以及 26.2 新增的调试/测试工具。**对多人联机体验无影响**。

### 2.2 实体系统

**实体类型**: 155 个常量定义（[constants.rs](crates/mc-core/src/constants.rs)），覆盖：
- 被动生物: 34 种 (Cow-Wolf-Cat-Parrot 等)
- 敌对生物: 38 种 (Creeper-Warden-Breeze-26.2 Parched 等)
- 载具: 27 种 (全系列矿车+船+木筏+箱船)
- 弹射物: 19 种 (Arrow-Trident-26.2 WindCharge 等)
- 展示/工具: 32 种 (Villager-WanderingTrader-Allay-Manifest-IronGolem-26.2 SulfurCube 等)
- 26.2 专属: SulfurCube, Nautilus, ZombieNautilus, HappyGhast (4 种)

**实体 AI 覆盖**: ~38-42 条独特 AI 分支 + 通用追逐/漫游:
- Boss AI: Wither (飞行+回血), EnderDragon (环绕飞行)
- 远程 AI: Skeleton, Ghast, Blaze, Drowned, Guardian, ElderGuardian, Breeze
- 近战 AI: Creeper (自爆), Spider, CaveSpider, Silverfish, Hoglin, Ravager, ZombieHorse, Piglin, Evoker, Shulker (浮空射击), Warden (震地), Witch (药水), IronGolem, MagmaCube
- 被动 AI: Villager (漫游+交易), WanderingTrader, Axolotl, Goat (冲击), Strider, Bat (倒挂), Fox (夜间狩猎), Panda, Wolf (驯服跟随), Bee, Mooshroom, SkeletonHorse, SulfurCube (12 archetype)
- 通用: 驯服宠物跟随主人, 敌对生物追逐最近玩家

**缺失 AI 的实体** (~8 种): Parrot, Ocelot, Turtle, Dolphin (水中), Nautilus, ZombieNautilus, Creaking (26.2), Pufferfish (膨胀)

### 2.3 状态效果

**40 种定义**（官方 26.2 registry, 0-based ID），**27 种连线** (68%):

| 已连线效果 (27) | 连线位置 |
|---|---|
| Speed, Slowness | tick_effects: 移动速度 |
| Haste, MiningFatigue | tick_effects: 挖掘速度 |
| InstantHealth, InstantDamage | add_effect: 瞬发 |
| JumpBoost | tick_effects: 跳跃高度 |
| Regeneration | tick_effects: 周期性回血 |
| FireResistance | 环境伤害: 火免 |
| WaterBreathing, ConduitPower | 环境伤害: 溺水免 |
| Invisibility | tick_effects: 生物检测降低 |
| Blindness | tick_effects: 禁止疾跑 |
| Hunger | tick_effects + 饥饿系统 |
| Poison | mob.rs: 洞穴蜘蛛毒伤 |
| Wither | main.rs + mob.rs: 凋零伤害 |
| HealthBoost | 生命值计算 |
| Absorption | tick_effects: 金心 |
| Saturation | tick_effects: 恢复饱食度 |
| Glowing | 客户端侧 (光谱箭) |
| Levitation | tick_effects: 浮空 |
| Luck, Unluck | 钓鱼战利品表 |
| SlowFalling | tick_effects + 坠落免伤 |
| DolphinGrace | tick_effects: 游泳速度 |
| BadOmen, HeroOfTheVillage | 袭击系统 |
| Darkness | mob.rs: Warden 黑暗 |

**待连线效果 (13)**:
- 经典 (5): Nausea, Resistance, Weakness, NightVision, Strength
- 26.2 新增 (7): TrialOmen, RaidOmen, WindCharged, Weaving, Oozing, Infested, BreathOfTheNautilus
- 1 个计数差异: ConduitPower 部分连线

### 2.4 红石组件

**约 39 种组件实现**（[redstone.rs](crates/mc-world/src/redstone.rs)），核心组件全部连线：

| 类别 | 已实现 |
|------|--------|
| 信号源 | RedstoneTorch, RedstoneBlock, Lever, Buttons×11, PressurePlates×13, DaylightDetector, SculkSensor, Observer, TargetBlock |
| 传输 | RedstoneWire (BFS 衰减), Repeater, Comparator (容器填充率检测+减法模式) |
| 机械 | Piston (3D+QC), StickyPiston (推+拉), Dispenser (弹射), Dropper (转移), Hopper (每 8 tick) |
| 逻辑 | NoteBlock (音符), TNT (爆炸), Doors/Trapdoors/FenceGates (全类型) |
| 铁轨 | PoweredRail, DetectorRail, ActivatorRail |
| 26.2 | LightningRod, CalibratedSculkSensor, SculkShrieker |
| 方块实体 | TrappedChest (人数信号), Lectern (翻页信号) |

**缺失的高级红石组件** (~11):
- CopperBulb (氧化变体), Crafter (合成器——基本框架存在但无红石连线)
- 红石相关方块: 部分 Sculk 变体, RedstoneRepeater/Comparator 的锁定状态
- DaylightDetector 反转模式已实现 (26.2)

### 2.5 世界生成

**群系**: 54 种 (发送给客户端 62 种，含 sub-biome 映射)
**生成器**: 7 种 (flat, noise, empty, nether, end, custom, compose)
**结构**: 14-15 种:

| 结构 | 状态 |
|------|------|
| Village (Plains/Desert) | ✅ |
| Desert Temple | ✅ |
| Jungle Temple | ✅ |
| Swamp Hut | ✅ |
| Igloo | ✅ |
| Mineshaft | ✅ |
| Ocean Monument | ✅ |
| Nether Fortress | ✅ |
| End City | ✅ |
| Shipwreck | ✅ |
| Ruined Portal | ✅ |
| Trial Chambers | ✅ |
| Ancient City | ✅ |
| Stronghold (end portal) | ✅ |
| Sulfur Springs (26.2) | ✅ |

**缺失结构** (~5): Pillager Outpost, Woodland Mansion, Bastion Remnant, Fossil, Trail Ruins

**树木**: 9 种 (oak, spruce, birch, jungle, acacia, dark_oak, mangrove, cherry, pale_oak)
**矿石**: 8 种 + 深层变体
**洞穴**: 3D Perlin 噪声 (双 octave 分支) + 4 种地下群系

### 2.6 游戏系统完整性

| 系统 | 状态 | 备注 |
|------|------|------|
| 饥饿系统 | ✅ 完整 | 36 种食物营养值 + 消耗/恢复/饥饿伤害 |
| 经验系统 | ✅ 完整 | 原版 3 层等级公式 + 经验球吸收 |
| 战斗系统 | ✅ 完整 | PvP/PvE + 1.9+ 攻击冷却 + 暴击 + 无敌帧 |
| 横扫之刃 | ✅ 完整 | 剑+地面+冷却≥0.848 → 2.5 block AOE |
| 盾牌 | ✅ 完整 | 180° 正面弧检测 + 斧禁用 |
| 护甲减伤 | ✅ 完整 | 原版公式 |
| 爆炸系统 | ✅ 完整 | 苦力怕/TNT + 衰减伤害 + 方块破坏 |
| 弹射物 | ✅ 完整 | 12 种 + 附魔效果 + 喷溅药水 |
| 合成 | ✅ 完整 | 2×2 + 3×3, ~1600 配方 |
| 熔炉 | ✅ 完整 | 14 燃料 + 34 熔炼 |
| 酿造 | ✅ 完整 | 50+ 配方 + BrewingStandManager |
| 钓鱼 | ✅ 完整 | 浮漂投射/收线 + 附魔修正 |
| 铁砧 | ✅ 完整 | 合并/修复/重命名 + 附魔书合并 |
| 信标 | ✅ 完整 | 4 层金字塔检测 + GUI + 矿物支付 |
| 锻造台 | ✅ 完整 | 下界合金升级 + 盔甲纹饰 |
| 村民 | ✅ 完整 | 14 职业 + 2-4 级交易 + 自动补货 + Gossip |
| 鞘翅 | ✅ 完整 | 滑翔 + 烟花推进 |
| 袭击 | ✅ 完整 | BadOmen 触发 → 波次生成 |
| 睡眠 | ✅ 完整 | 夜晚跳过 → 天亮满血 |
| Piglin 交易 | ✅ 完整 | 金锭右键 → 8 种随机战利品 |
| 铜氧化/蜡化 | ✅ 完整 | 时间氧化 + 蜜脾蜡化 |
| 唱片机 | ✅ 完整 | 右键插入/弹出 + 跨重启追踪 |
| 容器/GUI | ✅ 完整 | 21 类型 + 6 种点击模式 |
| 统计系统 | ✅ 完整 | 12 种统计类型 |
| 持久化 | ✅ 完整 | SQLite + LZ4 Linear + 原子写入 + 自动保存 |

---

## 3. 社区开源项目参考

### 3.1 Rust 生态位对比

| 项目 | 语言 | 成熟度 | 版本目标 | 方法论 | 独特优势 |
|------|------|--------|---------|--------|---------|
| **Apoptotic (本项目)** | Rust | **高** (核心玩法完整) | 26.2 (protocol 776) | 整体实现, RPi 优化 | 功能最完整, ARM 优化 |
| Valence | Rust | Alpha (0.2.0) | 1.20.1 | Bevy ECS 框架 | 模块化, 构建时代码生成 |
| Pumpkin | Rust | 开发中 | 26.x | Tokio + Rayon | 多协议(JE+BE), SIMD |
| Feather (存档) | Rust | 不活跃 → 并入 Valence | 1.16 | ECS + WASM 插件 | WASM 沙盒 |

**核心发现**: Apoptotic 在 **功能完整度** 上领先所有 Rust 实现。Valence 是框架模式（不含 gameplay），Pumpkin 仍在追赶基本功能。Apoptotic 的核心竞争力在于：
1. 唯一完整实现生存游戏循环的 Rust 服务端
2. 唯一针对 ARM/RPi 5 深度优化的 Rust 服务端
3. 完整的插件系统 (Native + WASM)

### 3.2 值得借鉴的 PaperMC 优化

| PaperMC 优化 | Apoptotic 状态 | 优先度 |
|---|---|---|
| **实体激活范围 (EAR)** | ✅ 已实现: hostile=48, passive=32, ambient=24 | 完成 |
| **异步区块加载/保存** | ❌ 同步在主 tick 线程 | **极高** |
| **Alternate Current 红石** | ❌ 每 2 tick 全量 BFS 传播 | **高** |
| **Starlight 光照引擎** | 🟡 基础 BFS, 非增量, 无跨区块泛洪 | **高** |
| **按玩家生物生成** | ❌ 使用全局上限 | **中** |
| **实体追踪范围限制** | ❌ 所有实体对所有玩家可见 | **高** |
| **区块预生成/优先级** | 🟡 基础距离排序，无优先级 | **中** |
| **Moonrise 漏斗优化** | 🟡 每 8 tick, 但无事件跳过 | **低** |
| **Folia 区域化多线程** | ❌ 单 tick 线程 | **低** (RPi 5 仅 4 核) |

### 3.3 值得借鉴的架构模式

1. **Valence 的 TrackedData 同步**: 自动检测组件变更 → 增量同步。比当前的全量广播效率高。
2. **Pumpkin 的 Ticker 系统**: Sprint/Freeze 模式，TPS 自适应。当前 Apoptotic 仅使用固定间隔。
3. **Feather 的 WASM 插件沙盒**: Apoptotic 已有 WASM 运行时 (extism)，但沙盒隔离机制可加强。
4. **Minestom 的实例系统**: 轻量级独立世界实例，适合小游戏扩展。
5. **Pumpkin 的多语言插件**: Rust 原生 + Lua + Java Bridge。Apoptotic 可扩展 Lua 绑定。

### 3.4 从 Pumpkin 基准测试看性能天花板

Pumpkin 官方基准 (1.21.1, 视距 10, AMD Ryzen 7600X):
- 10 玩家: **27 MB RAM, 1.5% CPU**
- 启动: **8ms**
- 二进制: **12.3 MB**

**对 Apoptotic 的参考意义**: Pumpkin 的功能覆盖率远低于 Apoptotic（无生存循环、无完整红石、AI 更少），所以直接对比不公平。但 Apoptotic 在 RPi 5 上 2-8 玩家场景下，内存应可控制在 **128-256 MB**，CPU 使用率 **20-40%**（基于当前 16 子系统 tick 的复杂度估算）。

---

## 4. Raspberry Pi 5 优化评估

### 4.1 已实现优化核查

| 优化项 | 状态 | 实现细节 |
|--------|------|---------|
| `target-cpu=cortex-a76` + NEON/SVE/LSE | ✅ | `.cargo/config.toml` + NEON 自动向量化 |
| jemalloc 全局分配器 | ✅ | `tikv-jemallocator` + MALLOC_CONF 调优 |
| `thread_local` PermutationTable | ✅ | Perlin 噪声生成缓存 |
| `thread_local` 光照 bitset | ✅ | BFS 缓冲区复用 (96KB→12KB) |
| ChunkData `Arc<Vec<u8>>` 缓存 | ✅ | 零分配重播 |
| Rayon `par_iter` 区块预生成 | ✅ | Chebyshev 距离排序 |
| LZ4 压缩 (NEON 自动向量化) | ✅ | `lz4_flex` ARM NEON 路径 |
| Spatial Hash Grid | ✅ | O(1) 邻近查询 |
| A* 64-entry LRU 缓存 | ✅ | chunk→chunk key, AtomicU64 |
| CPU affinity (tick 线程) | ✅ | `sched_setaffinity` |
| UpdateSectionBlocks 增量更新 | ✅ | ~200B vs ~50KB 全量 |
| DirtyBlockTracker | ✅ | 每 section 脏块追踪 |
| LTO fat + strip + panic=abort | ✅ | release profile |
| 错开 tick 速率 | ✅ | 16 子系统不同间隔 |
| `MissedTickBehavior::Skip` | ✅ | 过载跳过 |
| PGO/BOLT 脚本 | ✅ | 手动运行 |

### 4.2 已识别的问题与缺失

#### 严重问题 (影响性能和稳定性)

1. **IO 核心亲和性未实现**: `config.rs` 定义了 `io_core_affinity` 但从 `main.rs` 中从未调用。`spawn_blocking` 的 I/O 任务与 Rayon 线程池竞争 CPU 核心。

2. **Rayon 线程池未约束**: `default_chunk_threads()` 返回 3 但从未实际设置。Rayon 使用全局线程池 = 4 核心，与 tick 线程竞争。

3. **脏区块无写回 LRU**: 脏区块在内存中无限期累积直到 6000 tick 保存。长时间负载下可能占满 1024 区块槽位，阻止新区块加载。

4. **A* 缓存竞态条件**: LRU 驱逐的 find→remove→insert 是非原子操作，多线程下可能丢失条目或过度增长。

5. **无异步区块加载**: 区块加载在 tick 线程上同步执行。玩家移动过快或传送时会导致 TPS 下降。

#### 中等问题

6. **无 `use_hugepages` 实际调用**: 配置定义了该选项但从未使用 `madvise(MADV_HUGEPAGE)`。

7. **光照跨区块传播不完整**: 仅传播到直接邻居区块，无法处理多区块泛洪填充。

8. **区块预加载无方向性**: 仅基于 Chebyshev 距离排序，未考虑玩家移动方向。

9. **实体无追踪范围限制**: 所有实体对所有玩家广播，浪费带宽和客户端处理。

10. **区块序列化重复分配**: `serialize_chunk_binary` 每次创建新的 65536 容量 Vec，缺少缓冲区复用。

#### 轻微问题

11. `setup-rpi.sh` 未设置 CPU governor 为 `performance`。
12. 热数据结构无显式 `#[repr(align(64))]` 缓存行对齐。
13. 并行区块保存可能（当前顺序执行）。
14. 无自适应压缩策略（LZ4 vs Zstd 基于 I/O 延迟切换）。

### 4.3 RPi 5 硬件最佳配置建议

```
[server]
tick_core_affinity = 0      # CPU0 专用于 tick
chunk_threads = 3            # CPU1-3 用于 Rayon + IO
max_chunks = 1536            # 8GB → 可安全提升 (每区块 ~100KB)

[io]
io_core_affinity = 1         # 绑定到 Rayon 池内的核心
region_format = "linear"     # 每个区块一个文件, SD 卡友好
chunk_compression = "zstd"   # SD 卡推荐 Zstd (减少写放大)
save_interval = 12000        # SD 卡: 降低写入频率到每 10 分钟

[jemalloc]
MALLOC_CONF = "background_thread:true,dirty_decay_ms:5000,muzzy_decay_ms:5000,narenas:4,lg_tcache_max:16,metadata_thp:always"

[system]
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
sudo sysctl -w vm.max_map_count=262144
```

---

## 5. 长期完善路线图

按优先级分五个阶段，每个阶段 2-4 周（兼职开发节奏）。

### Phase A: 性能与稳定性基础 (2-3 周) ⭐⭐⭐⭐⭐

**目标**: 修复已发现的严重性能问题和稳定性风险

| # | 任务 | 文件 | 工作量 |
|---|------|------|--------|
| A1 | 实现 IO 核心亲和性绑定 | `main.rs`, `config.rs` | 小 |
| A2 | Rayon ThreadPool 配置为 3 核心 + 核心固定 | `main.rs` | 小 |
| A3 | 脏区块写回 LRU 驱逐 | `chunk_store.rs` | 中 |
| A4 | A* 缓存原子化 (使用 `crossbeam` 或 `clru`) | `pathfind.rs` | 中 |
| A5 | 异步区块加载 (Tokio task + 预加载方向感知) | `connection.rs`, `chunk_store.rs` | 大 |
| A6 | 实体追踪范围限制 | `connection.rs`, `player.rs` | 中 |
| A7 | 区块序列化缓冲区复用 (thread_local) | `chunk_store.rs` | 小 |

### Phase B: Vanilla 完整性补齐 (3-4 周) ⭐⭐⭐⭐

**目标**: 补齐核心游戏循环的缺失功能，将完整度从 ~85% 提升到 ~93%

| # | 任务 | 说明 | 工作量 |
|---|------|------|--------|
| B1 | 5 种经典效果连线 | Resistance, Strength, Weakness, Nausea, NightVision | 中 |
| B2 | 7 种 26.2 效果连线 | TrialOmen, RaidOmen, WindCharged, Weaving, Oozing, Infested, BreathOfTheNautilus | 大 |
| B3 | 实体 AI 补齐 (8 种) | Parrot, Ocelot, Turtle, Dolphin, Nautilus, ZombieNautilus, Creaking, Pufferfish | 中 |
| B4 | 缺失结构生成 (5 种) | PillagerOutpost, WoodlandMansion, BastionRemnant, Fossil, TrailRuins | 大 |
| B5 | 进度触发器补齐 (8 种) | CuredZombieVillager, BredAnimals, PlacedBlock, ConsumeItem, RaidWin, EntityKilled, FishCaught, LocationChanged | 中 |
| B6 | ~100 条缺失配方 | 旗帜/烟花/纹饰/染色箭矢/26.2 新增 | 中 |
| B7 | 红石补齐 (11 种高级组件) | CopperBulb, Crafter 连线, Sculk 变体等 | 中 |
| B8 | 缺失命令 (~13 个) | advancement, schedule, datapack, function, place 等 | 大 |

### Phase C: 社区最佳实践借鉴 (3-4 周) ⭐⭐⭐⭐

**目标**: 引入 PaperMC/Pumpkin 的核心优化模式

| # | 任务 | 来源参考 | 工作量 |
|---|------|---------|--------|
| C1 | Starlight 风格增量光照引擎 | PaperMC/Starlight | 极大 |
| C2 | Alternate Current 红石优化 | PaperMC/AlternateCurrent | 大 |
| C3 | 按玩家生物生成 (per-player mob cap) | PaperMC | 中 |
| C4 | TrackedData 增量实体同步 | Valence | 大 |
| C5 | Ticker Sprint/Freeze 模式 | Pumpkin | 中 |
| C6 | 实体碰撞空间划分 (BVH/Spatial Hash) | Valence/Paper | 中 |
| C7 | 生成尝试节流 (跳过持续失败的区块) | PaperMC | 小 |
| C8 | 物品合并优化 (merge-radius) | PaperMC | 小 |

### Phase D: RPi 5 深度调优 (2-3 周) ⭐⭐⭐

**目标**: 充分发挥 RPi 5 硬件特性

| # | 任务 | 说明 | 工作量 |
|---|------|------|--------|
| D1 | hugepages 支持 (MADV_HUGEPAGE) | 区块存储 + jemalloc 元数据 | 小 |
| D2 | 显式 NEON SIMD 路径 | PalettedContainer 编解码、区块序列化 | 大 |
| D3 | CPU governor 自动配置 | setup-rpi.sh 增强 | 小 |
| D4 | SD 卡优化方案 (Zstd + 批量写入) | 自适应压缩 + 写入合并 | 中 |
| D5 | 内存预算系统 | 动态 max_chunks + 实体数上限 | 中 |
| D6 | PGO/BOLT 集成到 CI | 自动化性能档案训练 | 中 |
| D7 | 基准测试增强 | 每阶段 TPS + 延迟 vs 当前 CPU/RSS | 小 |

### Phase E: 生产化与生态 (3-4 周) ⭐⭐⭐

**目标**: 使项目可用于实际 LAN 场景

| # | 任务 | 说明 | 工作量 |
|---|------|------|--------|
| E1 | 管理面板 (Web UI) | 控制台/玩家管理/世界预览 | 大 |
| E2 | 反作弊基础框架 | 移动预测 + 违规缓冲 + 回弹 | 大 |
| E3 | 插件市场/仓库 | 社区插件索引 + 安装 CLI | 中 |
| E4 | 数据包 (Datapack) 完整支持 | 函数/战利品表/谓词/结构文件 | 大 |
| E5 | 多语言插件绑定 (Lua/Python) | 降低插件开发门槛 | 中 |
| E6 | 自动化集成测试 (E2E) | 客户端模拟 + 回归测试 | 大 |
| E7 | 性能回归检测 (CI) | 基准测试自动对比 | 中 |
| E8 | 文档完善 | 架构文档 + 插件开发指南 + 运维手册 | 中 |

---

## 6. Phase 详解

### Phase A: 性能与稳定性基础

这是**最高优先级**阶段——修复影响稳定性和性能的已知问题。

**A1-A2: CPU 亲和性与线程池**
```
// main.rs — 当前仅设置了 tick_core_affinity
// 需要新增:
let io_core = config.io_core_affinity; // 默认 1
if io_core >= 0 {
    // 包装 spawn_blocking 以设置亲和性
}
// Rayon 线程池配置
rayon::ThreadPoolBuilder::new()
    .num_threads(config.chunk_threads as usize) // 3
    .build_global()?;
```

**A3: 脏区块写回 LRU**
当前 `chunk_store.rs` 的 LRU 驱逐检查 `dirty == false` 才驱逐。修复方案:
- 添加 `DirtyChunkFlush` 线程 (Tokio `spawn_blocking`)
- 当脏区块数量 > `max_dirty_chunks` 时，异步将最老脏区块序列化并写回磁盘
- 保持 LRU 顺序不变，驱逐前检查区块是否仍为脏

**A5: 异步区块加载**
```
// 使用 Tokio mpsc channel 管道
// 主线程发送 ChunkLoadRequest → Rayon 工作线程生成/加载
// 主线程在每 tick 轮询已完成的区块并发送给玩家
```

### Phase B: Vanilla 完整性补齐

**B1-B2: 效果连线**
- **Resistance**: 在 `apply_damage` 中乘以 `1/(1 + level)` 系数
- **Strength**: 在近战伤害计算中加 `3 * level` 基础伤害
- **Weakness**: 在近战伤害计算中减 `4 * level` 基础伤害（最低 0）
- **NightVision**: 客户端侧，发送效果即可
- **Nausea**: 客户端侧，发送效果即可
- **26.2 新效果**: 参考 vanilla 行为实现
  - `TrialOmen`/`RaidOmen`: 袭击触发前兆
  - `WindCharged`: 死亡时产生风爆
  - `Weaving`: 死亡时产生蜘蛛网
  - `Oozing`: 死亡时生成史莱姆
  - `Infested`: 受伤时有概率生成蠹虫
  - `BreathOfTheNautilus`: 水下呼吸增强

### Phase C: 社区最佳实践借鉴

**C1: Starlight 光照引擎** 是最具挑战性的任务。当前实现仅在一个区块内做 BFS，且跨区块仅处理直接邻居。Starlight 的核心改进:
- 从光源而非接收点传播
- 使用按区块分组的 BFS 队列而非全局队列
- 无状态光照部分,无需区块锁协调
- 预计光照计算时间减少 5-10 倍

**C2: Alternate Current 红石**
- 将红石线建模为 `WireNode` 图
- 仅传播到受影响的节点，而非整个网络
- 跳过红石更新期间的光照/高度图更新
- 预计红石 tick 时间减少 80%

**C4: TrackedData 同步**
借鉴 Valence 模式:
```rust
// 对实体组件使用变更检测
// 仅将 Changed<Position> 写入追踪数据缓冲区
// 初始 spawn → 全部数据
// 后续更新 → 仅变更字段
```

### Phase D: RPi 5 深度调优

**D2: NEON SIMD** 的关键路径:
- `PalettedContainer::encode_binary`: 24 sections × 4096 entries 循环 → NEON 向量化处理 16 entries/指令
- 区块序列化: 批量字节复制使用 ARM NEON `vld1q`/`vst1q`
- Perlin 噪声: 使用 NEON 向量化浮点运算

**D5: 内存预算**
```
max_memory_mb = 512  // 为 OS 和其他进程预留
chunk_memory = max_chunks * avg_chunk_size (~100KB)
entity_memory = max_entities * entity_size (~2KB)
player_memory = max_players * player_size (~500KB)
total = chunk_memory + entity_memory + player_memory + reserve
```

### Phase E: 生产化

**E2: 反作弊框架** 的基本架构:
1. 服务端运行 Minecraft 移动方程（摩擦、阻力、跳跃速度、药水效果）
2. 预测每个玩家每 tick 的合法位置范围
3. 维护违规缓冲区（允许偶尔的延迟峰值）
4. 缓冲衰减: 如果没有持续的违规，逐渐减少
5. 触发阈值 → 回弹到最后已知的好位置
6. 使用 ping/pong 数据包锚定时钟（客户端无法加速的服务器锚定时序）

---

## 总结

### 项目优势
- **功能最完整的 Rust Minecraft 服务端**（在现有开源实现中）
- **架构清晰**: 10 crates 分层，职责分明
- **测试充分**: 158 tests, 0 failures
- **RPi 5 深度优化**: jemalloc, NEON, CPU affinity, LZ4 等
- **性能框架到位**: SpatialHash, A* cache, EAR, ChunkData 缓存

### 关键缺口
- **5 个严重性能/稳定性问题** 需在 Phase A 立即修复
- **~15% vanilla 功能缺失** 集中在效果/进度/结构/红石
- **异步区块加载** 是最大的架构改进
- **光照和红石** 需要 PaperMC 风格的重写
- **反作弊和管理面板** 是走向生产化的必经之路

### 建议时间线
```
Phase A:  ████████░░░░░░░░░░  2-3 周 (立即启动)
Phase B:  ░░░░░░░░████████░░  3-4 周 (第 3-7 周)
Phase C:  ░░░░░░░░░░░░░░████  3-4 周 (第 7-11 周) 
Phase D:  ░░░░░░░░░░░░░░░░██  2-3 周 (第 11-14 周)
Phase E:  ░░░░░░░░░░░░░░░░░█  3-4 周 (按需)
---------------------------
总计: 13-18 周 (约 3-4 个月兼职开发)
```

### 与社区项目定位差异化

| 维度 | Apoptotic | Valence | Pumpkin |
|------|-----------|---------|---------|
| 目标用户 | LAN 2-8 人 (RPi 5) | 自定义服务端开发者 | 通用高性能服务端 |
| 游戏循环 | **完整生存** | 无 (框架) | 部分 |
| ARM 优化 | **深度** | 无 | 部分 |
| 插件系统 | Native + WASM | 无 (ECS 编程) | Native + Lua + Java |

**核心定位**: Apoptotic 应继续保持 **"RPi 5 上体验完整的 vanilla 生存服务端"** 这一独特定位，不与 Valence（框架定位）或 Pumpkin（高性能通用定位）直接竞争。
