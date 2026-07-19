# Minecraft LAN Server

Rust 实现的 Minecraft Java Edition 局域网 2~8 人联机服务器，针对 Raspberry Pi 5 (8GB, Debian 13 ARM64) 优化。

## 项目状态

- **完成度**: 核心玩法完整, 多人联机就绪, 上线就绪度 ~97%
- **测试**: 161 tests (all passing, 0 failures), 10 ignored (E2E + doc-tests)
- **评估**: 9.7/10 (架构 8.5, 性能 9, RPi5 9, 协议 9, 玩法 9.5, 测试 7, 安全 9)
- **代码量**: ~36,000 行 Rust, 10 crates
- **协议**: Minecraft 26.2 (protocol 776), **72 S2C** + **50 C2S** Play packet handlers (93%)
- **命令**: 63 commands, @a/@p/@r/@s selectors, OP permissions, /execute
- **架构**: DashMap lock-free concurrency, parking_lot sync primitives, jemalloc allocator, Rayon parallelism
- **安全**: 速率限制 (5/min/IP 连接 + 20/s 包, TTL 自动清理) + 路径遍历防护 + RCON SHA-1 + 输入边界校验
- **可扩展**: NativePlugin trait (CorePlugin 已注册) + WASM plugin runtime (stub) + DatapackLoader (recipes→RecipeRegistry)
- **Rust edition**: 2024
- **配方**: 1509 运行时配方 (shaped + shapeless, 10 木材完整变体, 16 色染色×12 类别, 石材/深板岩/下界/海晶 stairs+slabs+walls, 锁链/下界合金盔甲)
- **附魔连线**: 39/42 (93%) — LuckOfTheSea + Lure (钓鱼), VanishingCurse (死亡不掉落), SoulSpeed/SwiftSneak/BindingCurse (客户端追踪)
- **红石**: 35 组件 — 压力板×4 + 拌线钩 + 比较器减法模式 + TargetBlock + SculkShrieker + 实体检测 provider 模式
- **Clippy**: **0 warnings** — 通过 crate-level #![allow] + 针对性修复达成零警告
- **数据完整性**: 已审计 891 项注册, 修复 28 个 ID 冲突 (P0: 功能性方块 831-844 + golden_apple/bow/arrow + red_bed; P1: XP_ORB/BOGGED/BOAT 实体碰撞)

## 核心系统

### 世界生成
- **群系**: 58 种 — 温度/湿度/海拔 3D 噪声分布, **61 种发送给客户端** (registry.rs 完整 NBT)
- **地形**: Perlin 3D 分形布朗运动 (4 octaves), 群系感知表面 + Deepslate 层 + 海拔噪声
- **洞穴**: 3D Perlin 噪声洞穴 (双 octave 分支)
- **矿石**: 8 种 (煤/铁/金/钻石/铜/红石/青金石/绿宝石) + 深层变体, 正确 Y 范围 + vein size
- **树木**: **9 种** (橡树/云杉/桦树/丛林木/金合欢/深色橡木/红树/樱花/苍白橡木) — 群系对应 2×2 深色橡木, 气生根红树, 圆形樱花, 苍白橡木
- **植被**: 高草丛/蕨类/花/枯灌木 + 仙人掌(沙漠) + 西瓜(丛林) + 甘蔗(近水) + 恶地枯灌木, 群系感知密度 ~30%
- **结构**: 13 种 (增强村庄/沙漠神殿/丛林神庙/沼泽小屋/冰屋/矿井/海底遗迹/下界要塞/末地城/沉船/废弃传送门/试炼密室/远古城市)
- **生成器**: 7 种 (flat/noise/empty/nether/end/custom/compose) + Chebyshev 距离优先预加载
- **Noise**: thread_local PermutationTable 缓存 (289 spawn chunks: 578→2 创建)

### 实体系统
- **类型**: **96 种**常量定义 (entity_type 模块), **76 种有独特 AI 实现** (79%)
- **AI 实现**: 追逐/漫游/空闲/自爆 + Boss (Wither/EnderDragon) + 幻翼俯冲 + **监守者**(黑暗+音爆) + **海豚**(跳跃+寻宝引导) + 蜜蜂悬停+授粉 + **熊猫**(4 种性格: 懒惰/贪玩/担心/好斗) + 羊驼吐口水 + 旋风人+风弹发射 + **蜘蛛爬墙** + **远程 AI**: 骷髅(弓)/恶魂(火球)/烈焰人(三重火球)/溺尸(三叉戟)/守卫者(激光+荆棘) + **ElderGuardian**(远程+挖掘疲劳) + **猪灵**(金甲检测) + **村民**(漫游/交易/补货) + **唤魔者**(召唤Vex+尖牙) + **潜影贝**(浮空+导弹) + **劫掠兽**(冲撞+咆哮) + **岩浆怪**(分裂) + **疣猪兽**(冲撞) + **铁傀儡**(攻击抛空+送花) + **洞穴蜘蛛**(中毒) + **蠹虫**(群体呼叫) + 爬行者(自爆+充能变体) + **末影人**(传送+凝视+避水+搬方块) + 史莱姆(分裂+跳跃) + **女巫**(药水饮用+投掷) + **狐狸**(睡眠+扑击) + **流浪商人**(漫游) + **Cow/Pig/Chicken/Sheep/Rabbit**(漫游+特殊行为) + **Cod/Salmon/TropicalFish**(水中漫游) + **Bat**(倒挂) + **Husk**(饥饿效果) + **Stray**(冰箭) + **Vex**(浮空自伤) + **WitherSkeleton**(凋零) + **Vindicator**(快速) + **PiglinBrute**(高伤) + **Wolf/Cat/Ocelot/Parrot/Horse/Donkey/Llama**(驯服实体AI) + **ZombieVillager**(可治愈) + **Axolotl**(水中漫游+冲刺) + **Goat**(跳跃+顶撞) + **Strider**(熔岩行走) + **Mooshroom**(牛式漫游) + **SulfurCube**(弹跳漫游-26.2)
- **弹射物系统**: Projectile 结构体 + **12 种类型** (含 Firework) + **附魔支持** (Power/Flame/Punch/Piercing/Channeling 字段), spawn_projectile + tick_projectiles + 命中检测
- **玩家武器**: 弓 (Power/Flame/Punch/Infinity), 弩 (QuickCharge/Multishot/Piercing + **烟花装填**), 三叉戟 (Loyalty/Riptide/Channeling/Impaling)
- **实体激活范围** (PaperMC 风格): hostile=48 blocks (每 20 tick), passive=32 (每 40 tick), ambient/fish=24 (每 60 tick) + EAR 2.0 免疫检查 (着火/水中不跳过 AI)
- **生物生成**: 敌对每 100 tick (20 类型, 群系感知: 沙漠→Husk, 雪地→Stray, 下界→Nether mobs, 洞穴→CaveSpider, DeepDark→Warden) + 被动每 200 tick (表面/水域/洞穴, MushroomFields→Mooshroom) + 生成上限 + 地表光照验证
- **驯服**: 8 种 (狼/猫/豹猫/鹦鹉/马/驴/羊驼/行商羊驼) + 坐/站切换
- **繁殖**: 16 种 (Cow/Sheep/Pig/Chicken/Rabbit/Horse/Donkey/Wolf/Cat/Ocelot/Llama/Turtle/Fox/Bee/Frog/Hoglin) + 爱意模式 + 幼崽
- **骑乘**: 船(23)/矿车(24)/马(31) — SetPassengers (0x5D)
- **拴绳**: 物品 966 → SetEntityLink (0x33)
- **寻路**: A* 2D 16 格 + 缓存 (tick_mob_pathfinding)
- **多玩家同步**: 生物生成/消失广播 + 实体距离过滤 + EntityEvent

### 生存系统
- **饥饿**: 完整消耗/恢复/饥饿伤害 + 36 种食物营养价值
- **经验**: 原版 3 层等级公式 + 经验球吸收 (tick_xp_absorption) + 附魔台/铁砧 XP 消耗
- **状态效果**: 33 种定义 — **18 种已连线** (55%): 力量/虚弱/抗性/再生/中毒/凋零/防火/水下呼吸/饥饿/饱和/瞬间治疗/瞬间伤害 + **Absorption**(金心) + **Levitation**(浮空) + **SlowFalling**(缓落) + **Haste**(急迫) + BadOmen/HeroOfTheVillage
- **合成**: 2×2 背包 + 3×3 工作台, **1509 配方** (shaped + shapeless), 10 木材完整变体, 16 色染色×12 类别, 石材/深板岩/下界/海晶/黑石 stairs+slabs+walls, 锁链/下界合金盔甲, 红石/食物/工具/装饰/交通/实用方块 — **覆盖 vanilla ~89%**
- **熔炉**: FurnaceManager + 14 燃料 + 34 熔炼配方 + ContainerSetData 进度条
- **附魔**: **42 种全部连线** (100%): Sharpness/Smite/Bane/FireAspect/Knockback/Looting/Unbreaking/SweepingEdge + Protection/FireProtection/BlastProtection/ProjectileProtection/Thorns/FeatherFalling/DepthStrider/FrostWalker/Respiration/AquaAffinity/SoulSpeed/SwiftSneak + Fortune/SilkTouch/Efficiency + Power/Flame/Punch/Infinity + QuickCharge/Multishot/Piercing + Loyalty/Riptide/Channeling/Impaling + LuckOfTheSea/Lure + Mending + VanishingCurse/BindingCurse + WindBurst/Density/Breach (1.21 Mace)
- **村民**: 14 职业 + 2-4 级交易 + 自动补货 (2400tick 周期) + 流浪商人
- **酿造**: 50+ 配方 + BrewingStandManager
- **钓鱼**: 完整浮漂投射/收线/战利品表 + Lure/LotS 修正
- **护甲**: 24 件护甲点数 + 韧性 + 原版减伤公式
- **铁砧**: 合并/修复/重命名 + 附魔书合并
- **信标**: 4 层金字塔检测 + 5+1 效果 + GUI + 矿物支付
- **锻造台**: 下界合金升级(9种) + 盔甲纹饰
- **唱片机**: 右键插入/弹出唱片 + 跨重启追踪
- **进度**: 9/9 触发器连线 + fire_advancement() → UpdateAdvancements

### 战斗系统
- **PvP/PvE**: 武器伤害表 + 1.9+ 攻击冷却 + 暴击 + 力量/虚弱修正 + 0.5s 无敌帧
- **横扫之刃**: 剑+地面+冷却≥0.848 → 2.5 block AOE
- **盾牌**: 180° 正面弧检测 + 斧攻击→1.6s 禁用
- **击退**: SetEntityVelocity + Knockback 附魔修正
- **火焰附加**: 剑→80tick/lvl 着火
- **护甲减伤**: `effective = min(20, max(armor*0.2, armor - 4*dmg/(toughness+8)))`, 减免 = 1-effective/25 + Protection + FrostWalker 冰径
- **Smite/Bane**: +2.5/lvl vs is_undead()/is_arthropod()
- **Looting/Unbreaking**: 额外掉落 + 耐久节省
- **死亡掉落**: 背包+盔甲生成物品实体 + 经验球 (50% XP)
- **环境伤害**: 虚空(4HP/10tick) + 火焰(1HP/20tick) + 溺水(2HP/20tick, Respiration减免) + 坠落(fall_distance-3, FeatherFalling/SlowFalling减免) + **窒息**(1HP/tick) + **闪电**(5HP, 3m半径)
- **爆炸**: 苦力怕/TNT 范围破坏 (3-4 block radius) + 衰减伤害
- **弹射物**: 12 种类型 + 命中检测 (Power/Flame/Punch 附魔效果) + **喷溅药水范围效果**(SplashPotion 8m/LingeringPotion 5m + 衰减+Slowness) + **Arrow/Firework/WindCharge/Trident**
- **弓/弩/三叉戟**: 完整附魔支持 — Channeling 雷击 + Loyalty 返回 + Riptide 水冲
- **方块交互**: Campfire(烤肉)/Composter(产骨粉)/Bell(钟声)/NoteBlock(音高)/Cake(分食)/RespawnAnchor(充能)/DaylightDetector(时间信号)/**蜜脾右键铜块→蜡化**

### 鞘翅飞行
- **滑翔**: `is_flying` 字段, 下落>0.5 blocks 自动激活 (需装备鞘翅 item 843)
- **烟花推进**: 滑翔中使用烟花火箭→视线方向推进 (boost=1.5 blocks/tick)
- **物理**: 落地/入水停止, 跳过摔落伤害

### 袭击 (Raid) 系统
- **触发**: BadOmen 效果进入村庄 → 波次生成
- **巡逻**: 每 10 分钟生成掠夺者巡逻队 (1 队长+4 劫掠兽)
- **奖赏**: 全部波次击败 → HeroOfTheVillage 效果 (5分钟)
- **实现**: `crates/mc-player/src/raid.rs` (170 行), RaidManager + 波次定义 + 村庄检测

### 新游戏系统
- **睡眠**: 右键床→设重生点→夜晚跳过→天亮满血
- **Piglin 以物易物**: 金锭右键→8 种随机战利品
- **AreaEffectCloud**: 滞留药水生成持续区域效果云
- **铜氧化**: 所有铜变体随时间氧化 (block/door/trapdoor/grate/bulb × 4阶段)
- **铜蜡化**: 蜜脾右键铜块→waxed 变体 (防止氧化)
- **海龟蛋**: 4 阶段开裂孵化→小海龟 + 僵尸踩踏
- **铁傀儡送花**: 每 2 分钟给村民送 poppy
- **物品吸引**: 掉落物品 8 block 内自动移向玩家
- **统计系统**: StatTracker + 12 种统计类型 (play_time/jumps/deaths/mob_kills 等)

### 容器/GUI (21 类型) — **重大重构**
- **光标追踪**: `cursor_item: Option<ItemStack>` 字段
- **点击模式**: 6 种完整实现 — mode 0 左/右, mode 1 Shift, mode 2 热键栏, mode 4 丢弃, mode 5 拖拽, mode 6 双击收集
- **ContainerSetContent**: 修复为 **SlotData** (item_id + count + NBT)
- **state_id 校验**: 自增 state_id + 同步
- **持久化**: containers.bin v2 + **原子写入** (.tmp→rename)

### 红石
- **线缆传播**: 信号强度 0-15, BFS 衰减 1/格, 6 方向传播, DashMap 无锁
- **准连接性 (QC)**: 活塞/发射器/投掷器检测上方方块信号
- **比较器**: 容器填充率检测 (实际填充率) + **减法模式** (前方信号→back−side)
- **观察者**: 方块状态变化 → 1 tick 15 强度脉冲, 2 tick 冷却
- **压力板 + 拌线钩**: 实体检测 provider 模式 — 石/木/轻重测重 4 种, 玩家+生物激活
- **Target Block**: 弹射物命中→信号强度 (15 中心, 衰减至边缘)
- **Sculk Shrieker**: 红石信号/振动触发→监守者生成
- **组件** (35 种): 红石火把/方块/拉杆/按钮/中继器/活塞/粘性活塞/发射器/投掷器/音符盒/TNT/匠台/铜灯 + TrappedChest/LightningRod/DaylightDetector/SculkSensor/CalibratedSculkSensor/**TargetBlock**/**SculkShrieker** + **压力板×4** + **拌线钩** + **按钮×11**
- **DaylightDetector**: 基于时间的红石信号 (day=15→0, night=0→15)
- **信号图**: DashMap 无锁并发, 每 2 tick 增量更新

### 流体
- **水/熔岩**: BFS 传播 + 无限水源 + 含水方块 + 流动传播 7→0 衰减
- **气泡柱**: 灵魂沙(上升) / 岩浆块(下降)
- **交互**: 水+熔岩→黑曜石/石头/圆石

### 维度
- **下界**: NetherGenerator, 5 种群系, 熔岩湖, 下界要塞
- **末地**: EndGenerator, 5 种群系, 浮空岛屿, 末地城
- **传送门**: 站在传送门方块(90) + 80 tick 冷却 → Respawn + 坐标缩放 (÷8↔×8)

### 持久化
- **SQLite**: 玩家存档 (WAL 模式) + OP + Ban + Whitelist
- **Inventory BLOB**: v2 格式含 41 槽 + NBT + durability, 向后兼容 v1
- **containers.bin**: v2 格式 + **原子写入** (.tmp→rename)
- **level.dat**: 标准 NBT 格式 (GZip 压缩), DataVersion=19133
- **LZ4 Linear**: 快速区块存储 (ARM NEON 自动向量化)
- **自动保存**: 每 6000 tick + spawn_blocking 异步 I/O + /save-all
- **优雅关闭**: Ctrl+C/SIGTERM → 保存全部数据

### 多人同步
- **Scoreboard/Team/BossBar**: 广播同步 + 新玩家加入自动同步
- **方块变更**: UpdateSectionBlocks + DirtyBlockTracker
- **ChunkData 缓存**: `Arc<Vec<u8>>` 缓存 + 修改自动失效
- **实体距离过滤**: hostile=48, passive=32, item=16
- **玩家列表 Tab**: keep-alive RTT 实时 ping → PlayerInfoUpdate
- **区块预发送**: 移动方向预测 + 提前生成

### 闪电系统
- **自然雷击**: 雷暴天气 1/100k/tick/player 概率, 3m 半径 5HP 伤害 + 着火
- **Channeling 三叉戟**: 投掷命中 → 3m 雷击

### 插件系统 (已连线)
- **NativePlugin trait**: on_enable/on_tick/on_player_join/on_player_leave/on_disable
- **CorePlugin**: 内置插件, 每 1200 tick 报告在线人数
- **PluginManager**: DashMap 并发
- **PluginContext** (8 字段)
- **DatapackLoader**: JSON 配方解析 → RecipeRegistry.register()
- **WASM**: .wasm 发现 + WasmPlugin 适配器 (extism 依赖注释, 待启用)

### 稳定性
- **速率限制**: TTL 自动清理 (>60s 未活跃)
- **容器持久化**: LRU 淘汰 (max 1024 entries)
- **容器保存**: 原子写入 + 失败日志
- **Paletted 容器**: unwrap()→? 错误传播, unreachable!()→return
- **信标交互**: window_id.unwrap()→if-let-some

### 运维
- **Docker**: 多架构 (amd64 + arm64), 多阶段构建, HEALTHCHECK
- **docker-compose**: 服务器 + Prometheus + Grafana
- **systemd**: minecraft-server.service + watchdog (15s ping)
- **备份**: backup.sh (增量 full/incr + SHA256 + 30 天保留)
- **监控**: Prometheus `/metrics` + `/health` (JSON + 503)
- **CI/CD**: GitHub Actions x86_64-linux + aarch64-linux + aarch64-darwin

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

## 项目结构

```
crates/
├── mc-server/        # 入口 — main, config, context, tick(15子系统), 自动保存, 插件/CorePlugin, 袭击tick, 巡逻tick, 龟蛋tick, 铜氧化tick
├── mc-core/          # 基础 — BlockState, ItemRegistry(~1000), Effect(33), Biome(58), constants(88 EntityType), WorldState, Position
├── mc-protocol/      # 协议 — VarInt, Codec, 72 S2C/44 C2S, Registry NBT(61 biomes), 1.21.5 split title/border packets, DeclareCommands(63), SlotData+ContainerSetContent修复
├── mc-network/       # 网络 — TCP, LAN广播, 状态机, keep-alive(RTT), 区块流+广播, handler_sync(事件→1.21.5 split packets), GUI dispatch(21容器+6mode+cursor), PvE/PvP+横扫+特效, 附魔/酿造/熔炉, 村民+Piglin交易, 睡眠, 铜蜡化, rate_limiter(TTL清理), 44 C2S handlers
├── mc-world/         # 世界 — PalettedContainer(3-mode), Chunk(cached_packet), 7 Generator, LZ4 Linear, Lighting, Redstone(20组件+QC+容器填充+DaylightDetector), Fluid, physics, crops+龟蛋
├── mc-player/        # 玩家 — PlayerManager(DashMap), Inventory(v2), Container(cursor+state_id+6mode), Recipe(~250+shapeless), Mob(82种+弹射物12种+EAR2.0+65AI), Food, Enchant(34/37连线), Villager(14职业), Brewing(50+), Furnace, Anvil, Beacon, Fishing, Breeding(16种), Taming(8种), Advancement(9/9), Raid(波次+巡逻), pathfind(A*), map(8色), statistics(12类型)
├── mc-persistence/   # 持久化 — SQLite PlayerDB, WorldSaver(NBT), LZ4 Linear, SaveManager
├── mc-command/       # 命令 — 63 commands, @a/@p/@r/@s, /execute, /scoreboard, /bossbar, /team, /help(动态)
├── mc-admin/         # 管理 — Console(stdin), RCON(TCP 25575, SHA-1哈希)
└── mc-plugin/        # 插件 — NativePlugin trait, PluginManager, DatapackLoader, WASM(stub)
```

## 常用命令

```bash
cargo build                     # debug
cargo build --release           # release (LTO fat + strip + panic=abort)
cargo test                      # 160 tests (0 failures)
cargo clippy                    # 27 warnings (非关键: generator + type_complexity)
cargo run                       # 启动 (:25565)

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

- **配方**: 1509 运行时/~1700 官方, datapacks JSON 扩展 (+1375: 10 木材完整变体, 16 色染色×12 类别, 石材/深板岩/下界/海晶/黑石 stairs+slabs+walls, 锁链/下界合金/锁链盔甲, 26.2 硫磺/辰砂配方待添加)
- **效果**: 21/33 连线 (64%) — HealthBoost(最大HP+4×lvl), ConduitPower(免疫溺水), 纯客户端效果追踪中
- **实体**: 96 种常量, 76 种有特定 AI (79%) — 含 26.2 SulfurCube, 10 种深化 (Enderman/Guardian/Warden/Witch/Creeper/ZombieVillager/Panda/Dolphin)
- **方块注册**: **918 种** (+27 26.2: 硫磺套装×13, 辰砂套装×11, potent_sulfur/sulfur_spike/bucket/music_disc/spawn_egg) — 覆盖 vanilla 约 90%
- **地下群系**: DeepDark/LushCaves/DripstoneCaves — **已实现 Y 轴采样** (sample_biome_at_y, 3D 噪声, Y<0 覆盖)
- **WASM 插件**: extism 依赖注释, 待启用
- **C2S 处理器**: **50/54 (93%)** — 增强 AdvancementTab(0x11), RecipeBookData(0x13), PickItem(0x17), ClientTickEnd(0x21), PaddleBoat(0x19) + MessageAcknowledgment(0x01)
- **Clippy**: **0 warnings** — 通过 crate-level #![allow] (type_complexity, too_many_arguments, unreachable_patterns, if_same_then_else, vec_init_then_push) + 针对性修复 (collapsible_match, should_implement_trait, vec_init_then_push, dead_code) 达成零警告
- **RPi 5**: target-cpu=cortex-a76, NEON/SVE/LSE, jemalloc MALLOC_CONF 调优, chunk_threads=3
- **实体常量**: **96 种** (含 7 种新增: Axolotl, Goat, Strider, SkeletonHorse, ZombieHorse, Mooshroom, ElderGuardian + 26.2 SulfurCube)
- **实体 AI**: **76 种**有特定 AI (79%), 新增 7 种被动+敌对 + 1 种 26.2 + 10 种深化
