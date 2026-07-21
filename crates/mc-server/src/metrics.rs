//! Prometheus 指标端点 — 通过 HTTP 暴露服务器指标
//!
//! 指标:
//! - mc_players_online: 在线玩家数 (gauge)
//! - mc_chunks_loaded: 已加载区块数 (gauge)
//! - mc_uptime_seconds: 运行时间 (counter)
//! - mc_ticks_total: 总 tick 数 (counter)
//! - mc_memory_bytes: 预估内存使用 (gauge)
//! - mc_tps_current: 当前 TPS (gauge)
//! - mc_tps_p50/p95/p99: TPS 百分位 (gauge, 滑动窗口 100 tick)
//! - mc_tick_stage_duration_us: 各阶段耗时 (gauge, per-stage)

use mc_player::player::SharedPlayerManager;
use mc_world::chunk_store::ChunkStore;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{info, warn};

// ═══════════════════════════════════════════════════════════════
// TPS sliding window (最近 100 tick 的耗时，用于计算百分位)
// ═══════════════════════════════════════════════════════════════

static TICK_DURATIONS: Mutex<VecDeque<u64>> = Mutex::new(VecDeque::new());
const TPS_WINDOW_SIZE: usize = 100;

/// 每 tick 调用一次，记录 tick 耗时 (微秒)
pub fn record_tick(duration_us: u64) {
    let mut window = TICK_DURATIONS.lock();
    window.push_back(duration_us);
    if window.len() > TPS_WINDOW_SIZE {
        window.pop_front();
    }
}

/// 计算滑动窗口内 TPS 的 p50/p95/p99 百分位
fn compute_tps_percentiles() -> (u64, u64, u64) {
    let window = TICK_DURATIONS.lock();
    if window.is_empty() {
        return (20, 20, 20);
    }
    let mut sorted: Vec<u64> = window.iter().copied().collect();
    sorted.sort_unstable();
    let len = sorted.len();
    let p50_idx = (len * 50 / 100).min(len - 1);
    let p95_idx = (len * 95 / 100).min(len - 1);
    let p99_idx = (len * 99 / 100).min(len - 1);

    let us_to_tps = |us: u64| -> u64 {
        1_000_000u64.checked_div(us).map(|tps| tps.min(20)).unwrap_or(20)
    };

    (us_to_tps(sorted[p50_idx]), us_to_tps(sorted[p95_idx]), us_to_tps(sorted[p99_idx]))
}

// ═══════════════════════════════════════════════════════════════
// Tick stage timing (per-stage 最近一次耗时)
// ═══════════════════════════════════════════════════════════════

static TICK_STAGE_TIMES: std::sync::LazyLock<dashmap::DashMap<&'static str, u64>> = std::sync::LazyLock::new(dashmap::DashMap::new);

/// 记录某个 tick 阶段的耗时 (微秒)
pub fn record_stage_time(stage: &'static str, duration_us: u64) {
    TICK_STAGE_TIMES.insert(stage, duration_us);
}

// ═══════════════════════════════════════════════════════════════
// HTTP 服务器
// ═══════════════════════════════════════════════════════════════

/// 启动一个轻量级的 HTTP 服务器，在 `/metrics` 端点暴露 Prometheus 格式的指标
pub async fn serve_metrics(
    bind_addr: &str,
    player_manager: SharedPlayerManager,
    chunk_store: ChunkStore,
    start_time: std::time::Instant,
    tick_count: Arc<std::sync::atomic::AtomicU64>,
) {
    let listener = match TcpListener::bind(bind_addr).await {
        Ok(l) => {
            info!("Prometheus metrics listening on http://{}", bind_addr);
            l
        }
        Err(e) => {
            warn!("Failed to bind metrics endpoint {}: {}", bind_addr, e);
            return;
        }
    };

    loop {
        let (mut stream, _) = match listener.accept().await {
            Ok(c) => c,
            Err(_) => continue,
        };

        let pm = player_manager.clone();
        let cs = chunk_store.clone();
        let start = start_time;
        let tc = tick_count.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 1024];
            let n = match stream.read(&mut buf).await {
                Ok(n) if n > 0 => n,
                _ => return,
            };

            let request = String::from_utf8_lossy(&buf[..n]);

            // Health check endpoint
            if request.contains("GET /health") {
                let online = pm.online_count();
                let cs_count = cs.count();
                let status = if cs_count > 0 { "ok" } else { "degraded" };
                let resp = format!(
                    "HTTP/1.1 {code} OK\r\nContent-Type: application/json\r\n\r\n\
                     {{\"status\":\"{status}\",\"players\":{online},\"chunks\":{cs_count}}}",
                    code = if status == "ok" { "200" } else { "503" }
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                return;
            }

            // GET /status — JSON status page
            if request.contains("GET /status") {
                let memory = estimate_memory_mb();
                let online = pm.online_count();
                let cs_count = cs.count();
                let uptime_secs = start.elapsed().as_secs();
                let (_p50, tps_p95, _p99) = compute_tps_percentiles();
                let tps_str = if tps_p95 > 0 { format!("{}", 1_000_000 / tps_p95.max(1)) } else { "N/A".to_string() };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n\
                     {{\"server\":\"Apoptotic\",\"version\":\"26.2\",\
                     \"players\":{0},\"chunks\":{1},\
                     \"uptime_seconds\":{2},\"tps_p95\":{3},\
                     \"memory_mb\":{4},\"status\":\"running\"}}",
                    online, cs_count, uptime_secs, tps_str, memory
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                return;
            }

            // E1: Admin dashboard — live HTML panel
            if request.contains("GET /admin") || request.contains("GET / ") {
                let online = pm.online_count();
                let cs_count = cs.count();
                let uptime_secs = start.elapsed().as_secs();
                let (_p50, _p95, _p99) = compute_tps_percentiles();
                let memory = estimate_memory_mb();
                let tick_val = tc.load(std::sync::atomic::Ordering::Relaxed);
                let mins = uptime_secs / 60;
                let hours = mins / 60;
                let time_str = if hours > 0 {
                    format!("{}h {}m", hours, mins % 60)
                } else {
                    format!("{}m {}s", mins, uptime_secs % 60)
                };
                let players = pm.all_players();
                let player_rows: String = players.iter()
                    .map(|p| format!(
                        "<tr><td>{}</td><td>{:.0} HP</td><td>({:.0},{:.0},{:.0})</td><td>{}</td></tr>",
                        p.username, p.health,
                        p.position.x, p.position.y, p.position.z,
                        p.dimension
                    )).collect::<Vec<_>>().join("\n");
                let html = format!(r#"HTTP/1.1 200 OK
Content-Type: text/html; charset=utf-8

<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Apoptotic — Admin Panel</title>
<style>
  body{{font-family:system-ui,sans-serif;background:#0d1117;color:#c9d1d9;margin:0;padding:20px}}
  h1{{color:#58a6ff;margin:0 0 5px}} .sub{{color:#8b949e;font-size:14px}}
  .grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:12px;margin:20px 0}}
  .card{{background:#161b22;border:1px solid#30363d;border-radius:8px;padding:16px}}
  .card .label{{color:#8b949e;font-size:12px;text-transform:uppercase;letter-spacing:0.5px}}
  .card .value{{font-size:28px;font-weight:700;color:#58a6ff;margin:4px 0}}
  .card .value.good{{color:#3fb950}} .card .value.warn{{color:#d2991d}} .card .value.bad{{color:#f85149}}
  table{{width:100%;border-collapse:collapse;margin-top:20px}}
  th,td{{text-align:left;padding:8px 12px;border-bottom:1px solid#21262d}}
  th{{color:#8b949e;font-size:12px;text-transform:uppercase}} tr:hover{{background:#161b22}}
  .footer{{color:#484f58;font-size:11px;margin-top:30px;text-align:center}}
  .refresh{{color:#8b949e;font-size:12px}}
</style>
</head>
<body>
<h1>⚡ Apoptotic</h1>
<div class="sub">Minecraft 26.2 Server — Admin Dashboard <span class="refresh">(auto-refresh 5s)</span></div>
<div class="grid">
  <div class="card"><div class="label">Players</div><div class="value good">{online}</div></div>
  <div class="card"><div class="label">Chunks Loaded</div><div class="value">{cs_count}</div></div>
  <div class="card"><div class="label">Uptime</div><div class="value">{time_str}</div></div>
  <div class="card"><div class="label">Memory</div><div class="value">{memory} MB</div></div>
  <div class="card"><div class="label">Total Ticks</div><div class="value">{tick_val}</div></div>
  <div class="card"><div class="label">TPS (p95)</div><div class="value good">{tps_p95}</div></div>
</div>
<h3>Online Players</h3>
<table>
<tr><th>Name</th><th>Health</th><th>Position</th><th>Dimension</th></tr>
{player_rows}
</table>
<div class="footer">Apoptotic v0.1.0 · Rust · Raspberry Pi 5 · MIT</div>
<meta http-equiv="refresh" content="5">
</body>
</html>"#,
                    online=online, cs_count=cs_count,
                    time_str=time_str, memory=memory, tick_val=tick_val,
                    tps_p95=_p95,
                    player_rows=player_rows,
                );
                let _ = stream.write_all(html.as_bytes()).await;
                return;
            }

            // Only respond to GET /metrics
            if !request.contains("GET /metrics") {
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").await;
                return;
            }

            let uptime_secs = start.elapsed().as_secs();
            let online = pm.online_count();
            let loaded_chunks = cs.count();
            let tick_val = tc.load(std::sync::atomic::Ordering::Relaxed);
            let (tps_p50, tps_p95, tps_p99) = compute_tps_percentiles();

            // Build stage timing lines
            let mut stage_lines = String::new();
            for entry in TICK_STAGE_TIMES.iter() {
                stage_lines.push_str(&format!(
                    "# HELP mc_tick_stage_duration_us Tick stage duration in microseconds\n\
                     # TYPE mc_tick_stage_duration_us gauge\n\
                     mc_tick_stage_duration_us{{stage=\"{name}\"}} {value}\n",
                    name = entry.key(),
                    value = entry.value(),
                ));
            }

            let response = format!(
                "# HELP mc_players_online Number of players currently online\n\
                 # TYPE mc_players_online gauge\n\
                 mc_players_online {online}\n\
                 # HELP mc_chunks_loaded Number of chunks currently loaded\n\
                 # TYPE mc_chunks_loaded gauge\n\
                 mc_chunks_loaded {loaded_chunks}\n\
                 # HELP mc_uptime_seconds Server uptime in seconds\n\
                 # TYPE mc_uptime_seconds counter\n\
                 mc_uptime_seconds {uptime_secs}\n\
                 # HELP mc_ticks_total Total game ticks processed\n\
                 # TYPE mc_ticks_total counter\n\
                 mc_ticks_total {tick_val}\n\
                 # HELP mc_memory_bytes Estimated memory usage\n\
                 # TYPE mc_memory_bytes gauge\n\
                 mc_memory_bytes {memory}\n\
                 # HELP mc_tps_current Current ticks per second\n\
                 # TYPE mc_tps_current gauge\n\
                 mc_tps_current {tps}\n\
                 # HELP mc_tps_p50 TPS 50th percentile (median, last 100 ticks)\n\
                 # TYPE mc_tps_p50 gauge\n\
                 mc_tps_p50 {tps_p50}\n\
                 # HELP mc_tps_p95 TPS 95th percentile (last 100 ticks)\n\
                 # TYPE mc_tps_p95 gauge\n\
                 mc_tps_p95 {tps_p95}\n\
                 # HELP mc_tps_p99 TPS 99th percentile (last 100 ticks)\n\
                 # TYPE mc_tps_p99 gauge\n\
                 mc_tps_p99 {tps_p99}\n\
                 {stage_lines}",
                online = online,
                loaded_chunks = loaded_chunks,
                uptime_secs = uptime_secs,
                tick_val = tick_val,
                memory = estimate_memory_mb() * 1024 * 1024,
                tps = tick_val.checked_div(uptime_secs).unwrap_or(20),
                tps_p50 = tps_p50,
                tps_p95 = tps_p95,
                tps_p99 = tps_p99,
                stage_lines = stage_lines,
            );

            let http = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\n\r\n{}",
                response.len(),
                response,
            );
            let _ = stream.write_all(http.as_bytes()).await;
        });
    }
}

/// 估计当前内存使用 (MB)
fn estimate_memory_mb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
            let parts: Vec<&str> = statm.split_whitespace().collect();
            if let Some(res) = parts.get(1)
                && let Ok(pages) = res.parse::<u64>() {
                    return pages * 4 / 1024; // 4KB pages → MB
            }
        }
    }
    32 // 32 MB default estimate
}
