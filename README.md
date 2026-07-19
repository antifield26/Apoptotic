# Apoptotic

Rust 实现的 Minecraft Java Edition 26.2 局域网联机服务器，针对 Raspberry Pi 5 优化，支持 2~8 人。

[![CI](https://github.com/antifield26/Apoptotic/actions/workflows/ci.yml/badge.svg)](https://github.com/antifield26/Apoptotic/actions/workflows/ci.yml)

## 特性

- **协议**: Minecraft 26.2 (protocol 776)，73 S2C + 37 C2S Play 包处理器
- **世界**: 54 种群系（含 26.2 Sulfur Caves），7 种生成器，13 种结构，9 种树木
- **实体**: 96 种实体类型，~67 种独特 AI，含弹射物系统、驯服、繁殖、骑乘
- **生存**: 完整合成（1534 配方）、熔炉、附魔（42 种）、酿造（50+）、村民交易、钓鱼、战斗
- **红石**: 35 组件，含比较器减法模式、观察者、压力板、拌线钩
- **命令**: 63 个命令，@a/@p/@r/@s 选择器，/execute 支持
- **安全**: 速率限制 + 路径防护 + Mojang 在线认证 + 封包大小限制 + RCON
- **插件**: NativePlugin trait + WASM 运行时（extism，可选）
- **运维**: Docker 多架构、Prometheus 监控、systemd watchdog、自动备份

## 快速开始

### 本地运行

```bash
# 编译
cargo build --release

# 启动
cargo run --release

# 客户端连接 localhost:25565
```

### Docker

```bash
# 构建 ARM64 镜像（RPi 5）
docker build --platform linux/arm64 -t apopototic .

# 使用 Docker Compose 启动
docker compose up -d

# 带监控（Prometheus + Grafana）
docker compose --profile monitoring up -d
```

### Raspberry Pi 5 部署

```bash
# 安装依赖
sudo apt install gcc-aarch64-linux-gnu

# 交叉编译
rustup target add aarch64-unknown-linux-gnu
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
  cargo build --release --target aarch64-unknown-linux-gnu

# 安装 systemd 服务
sudo cp scripts/minecraft-server.service /etc/systemd/system/
sudo systemctl enable --now minecraft-server
```

## 架构

```
crates/
├── mc-server/        # 入口，tick(15子系统)，自动保存，插件
├── mc-core/          # BlockState，ItemRegistry(1038)，Effect(33)，Biome(54)，EntityType(96)
├── mc-protocol/      # VarInt，Codec，73 S2C/37 C2S，Registry NBT(62 biomes)
├── mc-network/       # TCP，LAN广播，状态机，GUI dispatch(21容器)，rate_limiter
├── mc-world/         # PalettedContainer，Chunk，7 Generator，LZ4，Lighting，Redstone，Fluid
├── mc-player/        # PlayerManager，Inventory，Container，Recipe(1534)，Mob(~67 AI)，Enchant 等
├── mc-persistence/   # SQLite PlayerDB，WorldSaver(NBT)，LZ4 Linear
├── mc-command/       # 63 commands，/execute，/scoreboard，/bossbar，/team
├── mc-admin/         # Console，RCON(TCP 25575)
└── mc-plugin/        # NativePlugin trait，WASM(extism)，DatapackLoader
```

**技术栈**: Tokio async I/O，DashMap 无锁并发，parking_lot，jemalloc，Rayon 并行，LZ4 压缩

## RPi 5 性能优化

- `target-cpu=cortex-a76`，NEON/SVE/LSE 指令集
- jemalloc 全局分配器 + MALLOC_CONF 调优
- thread_local 缓存（PermutationTable，bitset）
- ChunkData `Arc<Vec<u8>>` 缓存，修改自动失效
- Rayon par_iter spawn chunk 预生成
- spawn_blocking 异步 I/O
- LTO + strip + panic=abort 最小化二进制
- PGO/BOLT 调优脚本就绪（`scripts/optimize-profile.sh`，`scripts/bolt-optimize.sh`）

## 数据管道

从 PrismarineJS minecraft-data 自动生成物品注册表：

```bash
# 下载官方数据并生成 diff 报告
./scripts/update-minecraft-data.sh 26.2

# 应用更改（覆盖 item.rs）
./scripts/update-minecraft-data.sh 26.2 --apply
```

## 插件开发

### NativePlugin

```rust
use mc_plugin::plugin::{NativePlugin, PluginContext};

struct MyPlugin;
impl NativePlugin for MyPlugin {
    fn name(&self) -> &str { "my_plugin" }
    fn on_enable(&mut self, ctx: &PluginContext) { /* ... */ }
    fn on_tick(&mut self, ctx: &PluginContext, tick: u64) { /* ... */ }
}
```

### WASM 插件

```bash
cargo build --features mc-plugin/wasm-runtime
```

将 `.wasm` 文件放入 `plugins/` 目录，服务器启动时自动发现。

## 已知限制

| 类别 | 覆盖率 | 备注 |
|------|--------|------|
| 物品注册 | 1038/~2200 (47%) | 较多装饰/特殊方块未注册 |
| 配方 | 1534/~1700 (90%) | 旗帜图案、烟花、锻造纹饰未实现 |
| C2S 处理器 | 37/54 (69%) | 31 完整 + 6 stub |
| 状态效果 | 24/33 (73%) | 纯客户端视觉未连线 |
| 实体 AI | ~67/96 (70%) | ~15 种仅通用漫游 |

## 开发命令

```bash
cargo build                     # 调试构建
cargo build --release           # 发布构建
cargo test                      # 运行测试
cargo clippy                    # Lint 检查
cargo run                       # 启动服务器

# WASM 插件
cargo build --features mc-plugin/wasm-runtime
```

## License

MIT
