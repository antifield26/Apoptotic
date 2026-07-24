\# Apoptotic Server — 网络问题汇总



> 测试环境: Raspberry Pi 5 (Debian 13 ARM64) · Rust 2024 Edition

> 测试方法: Python 协议测试脚本 + 源码审查 + 与 PaperMC 对比

> 测试时间: 2026-07-24



\---



\## 🔴 Issue #1 (CRITICAL): `write\_frame` 加密了 VarInt 长度前缀 — 在线模式不可用



\*\*文件\*\*: `crates/mc-network/src/packet\_io.rs` — `write\_frame()` 方法（约第 107-117 行）



\*\*问题描述\*\*:



Minecraft 协议规定：启用 AES 加密后，TCP 帧格式为:

```

\[packet\_length: VarInt — 明文，不加密]\[packet\_body — AES 加密]

```



当前代码流程:

1\. `MinecraftCodec::encode()` 返回完整帧: `\[length VarInt]\[packet body]`（正确）

2\. `write\_frame()` 将\*\*整个 data\*\*（含 length VarInt）一起加密后写入 TCP（\*\*错误\*\*）



```rust

// 当前代码 (crates/mc-network/src/packet\_io.rs):

pub async fn write\_frame(\&mut self, data: \&\[u8]) -> std::io::Result<()> {

&#x20;   if let Some(ref mut cipher) = self.encryption {

&#x20;       let mut encrypted = data.to\_vec();

&#x20;       cipher.encrypt(\&mut encrypted);  // ← BUG: 把长度前缀也加密了

&#x20;       self.write.write\_all(\&encrypted).await?;

&#x20;   } else {

&#x20;       self.write.write\_all(data).await?;

&#x20;   }

&#x20;   Ok(())

}

```



\*\*后果\*\*:

\- 客户端读到第一字节就是 AES 加密后的垃圾数据

\- VarInt 解析失败，帧长度错误

\- 所有在线模式（`online\_mode = true`）连接在加密握手完成后立刻断开

\- 当前 `online\_mode = false` 未触发此 bug



\*\*修复方向\*\*:

`encode()` 需要返回分离的长度前缀和包体（或 `write\_frame` 自己分离），然后只加密包体:

```

\[明文 length VarInt] + encrypt(\[packet body])

```



\*\*影响范围\*\*: 在线模式 100% 不可用



\---



\## 🟡 Issue #2 (PERFORMANCE): 每个连接创建一个 OS 线程



\*\*文件\*\*: `crates/mc-server/src/main.rs` — `accept\_loop` 函数



\*\*问题描述\*\*:



```rust

// 当前代码:

let handle = tokio::runtime::Handle::current();

std::thread::spawn(move || {         // ← 每个连接创建一个 OS 线程

&#x20;   handle.block\_on(async move {

&#x20;       let \_permit = permit.acquire().await;

&#x20;       connection::handle\_connection(stream, srv).await;

&#x20;   });

});

```



\*\*问题\*\*:

\- `std::thread::spawn` 创建完整操作系统线程（Linux 默认 \~2MB 栈空间）

\- `block\_on` 阻塞该线程等待异步任务完成

\- 10 个并发玩家 = 10 个额外 OS 线程 = \~20MB 额外虚拟内存

\- Tokio 设计理念是用少量工作线程处理大量异步任务，这里完全绕过了 Tokio 的调度



\*\*修复方向\*\*:

```rust

tokio::spawn(async move {

&#x20;   let \_permit = permit.acquire().await;

&#x20;   connection::handle\_connection(stream, srv).await;

});

```



\*\*影响范围\*\*: 连接扩展性、线程资源浪费



\---



\## 🟡 Issue #3 (PERFORMANCE): 空闲服务器 CPU 占用 66.8%



\*\*现象\*\*:

\- 运行 10 小时，0 玩家，0 活跃连接

\- `user time: 24161s / uptime: \~36000s` = \*\*67% 持续 CPU\*\*

\- 10 个线程（4 tokio workers + 主 tick loop + 其他）

\- `strace` 显示极少 syscall → 纯计算密集型（非 I/O 等待）

\- VmRSS 仅 24MB（内存正常）



\*\*可能的热点区域\*\*（需要 perf 确认）:



| 疑似热点 | 执行频率 | 文件 |

|---------|---------|------|

| 红石 BFS 信号传播 | 每 2 tick (10Hz) | `crates/mc-world/src/redstone.rs` |

| 熔炉 tick 遍历 | \*\*每 tick (20Hz)\*\* | `crates/mc-player/src/furnace.rs` |

| 流体物理 tick | 每 5 tick (4Hz) | `crates/mc-world/src/fluid.rs` |

| 矿车轨道物理 | 每 10 tick (2Hz) | `crates/mc-server/src/main.rs` |

| TNT 爆炸处理 | 每 tick (20Hz) | `crates/mc-server/src/main.rs` |

| 漏斗物品传输 | 每 8 tick (2.5Hz) | `crates/mc-server/src/main.rs` |

| mob\_ai tick | 每 tick (20Hz) | `crates/mc-server/src/tick.rs` |

| physics tick (下落方块等) | 每 20 tick (1Hz) | `crates/mc-world/src/physics.rs` |



\*\*调试建议\*\*:

```bash

\# 在开发机上:

cargo build --release

perf record -p <pid> --call-graph dwarf -- sleep 10

perf report

\# 或者用 flamegraph:

perf script | stackcollapse-perf.pl | flamegraph.pl > flame.svg

```



\*\*影响范围\*\*: RPi 5 功耗/散热、其他服务（PaperMC）的性能受到影响



\---



\## 🔵 Issue #4 (MINOR): `write\_frame` 加密路径中没有做长度前缀分离



\*\*文件\*\*: `crates/mc-network/src/packet\_io.rs`



同上 Issue #1，但这里单独列出是因为修复需要同时修改 `MinecraftCodec::encode()` 的返回值和 `write\_frame()` 的逻辑。两种修复方案:



\*\*方案 A\*\*: `encode()` 只返回包体（不带长度前缀），`write\_frame()` 负责添加长度前缀 + 选择性加密包体



\*\*方案 B\*\*: `encode()` 返回 `(length\_prefix: Vec<u8>, body: Vec<u8>)`，`write\_frame()` 分别处理



推荐方案 A，改动更小，且与现有代码结构更一致。



\---



\## 🔵 Issue #5 (MINOR): 测试脚本协议版本过时



\*\*文件\*\*: `scripts/test\_ping.py:45`



```python

handshake += write\_varint(767)  # 应改为 776 或从配置读取

```



\*\*影响\*\*: 测试可用但版本不完全匹配



\---



\## 🟢 已验证正确的部分（无需修改）



| 组件 | 状态 | 验证方式 |

|------|------|---------|

| Handshake 解析 | ✅ | 协议版本 766-776 兼容性检查正确 |

| Login Start 解码 | ✅ | UUID 16 字节 + 用户名格式正确 |

| Login Success 编码 | ✅ | UUID + username + properties 格式正确 |

| Set Compression (0x03) | ✅ | threshold=256，login\_compression 在 login\_finished 之前发送 |

| ConfigPluginMessage (brand) | ✅ | Channel + String payload 编码正确 |

| RegistryData 编码 | ✅ | Registry ID + entries 格式正确 |

| FeatureFlags 编码 | ✅ | VarInt count + String list 正确 |

| FinishConfiguration | ✅ | 空 payload，等待客户端 AckFinishConfig |

| JoinGame (0x2B) | ✅ | Hexdump 逐字节验证格式正确 |

| ChunkData 发送 | ✅ | 区块数据正常广播 |

| `read\_frame` 解密 | ✅ | 长度前缀明文读取 → body 解密，流程正确 |

| 压缩/解压 zlib | ✅ | data\_length VarInt 判断压缩/未压缩正确 |

| Config Keep-Alive | ✅ | 每隔 5s 发送防止超时 |

| 速率限制 | ✅ | 5 连接/分钟/IP + 20 包/秒/连接 |

| 协议版本拒绝 | ✅ | LoginDisconnect 带友好提示消息 |

| Banned / Whitelist | ✅ | 检查逻辑正确 |

| 最大玩家数限制 | ✅ | 达到上限时正确拒绝 |



\---



\## 📊 修复优先级



| 优先级 | Issue | 影响 |

|--------|-------|------|

| \*\*P0\*\* | #1 write\_frame 加密错误 | 在线模式 100% 不可用 |

| \*\*P1\*\* | #3 空闲高 CPU | RPi 功耗/散热，影响同机 PaperMC |

| \*\*P2\*\* | #2 thread-per-connection | 连接扩展性 |

| \*\*P3\*\* | #4 encode/write\_frame 重构 | 与 #1 一起修 |

| \*\*P4\*\* | #5 测试脚本版本号 | 测试准确性 |

