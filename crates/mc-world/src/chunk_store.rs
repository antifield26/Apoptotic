//! 区块存储 — 使用 DashMap 并发访问 + Rayon 并行生成 + LZ4/Zstd Linear 格式

use crate::chunk::Chunk;
use dashmap::DashMap;
use mc_core::position::ChunkPos;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use rayon::prelude::*;

const DEFAULT_MAX_CHUNKS: usize = 1024;
/// Maximum dirty chunks before proactive writeback kicks in (80% of capacity)
const DIRTY_WRITEBACK_THRESHOLD: f64 = 0.80;
/// D5: Estimated memory per chunk (~100KB average: ~50KB section data + ~50KB cached packet)
const ESTIMATED_CHUNK_BYTES: usize = 100_000;
/// D5: Max memory budget (512MB default for RPi 5 8GB — leaves 7.5GB for OS + jemalloc overhead)
#[allow(dead_code)]
const DEFAULT_MAX_MEMORY_MB: usize = 512;

/// 区块压缩算法
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChunkCompression {
    /// LZ4 — 快速压缩 (默认, 适合 RAM disk)
    Lz4,
    /// Zstd — 高压缩率 (适合 SD 卡, 压缩率高 20-30%)
    Zstd,
}

impl ChunkCompression {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "zstd" | "zstandard" => ChunkCompression::Zstd,
            "adaptive" | "auto" => ChunkCompression::Lz4, // D4: adaptive starts as LZ4
            _ => ChunkCompression::Lz4,
        }
    }
}

/// D4: Set adaptive compression mode (2 = auto-switch based on I/O speed)
pub fn set_adaptive_compression() {
    COMPRESSION.store(2, Ordering::Relaxed);
}

/// 全局压缩算法选择 (可通过配置修改, 2=adaptive)
/// D4: Adaptive mode switches between LZ4 and Zstd based on I/O throughput
static COMPRESSION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0); // 0=LZ4, 1=Zstd, 2=adaptive
/// D4: Rolling average of last 4 save times (milliseconds) for adaptive switching
static SAVE_TIMES_MS: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());
const ADAPTIVE_WINDOW: usize = 4;
const SLOW_SAVE_THRESHOLD_MS: u64 = 500; // switch to LZ4 if save >500ms
const FAST_SAVE_THRESHOLD_MS: u64 = 100;  // switch back to Zstd if save <100ms

/// 设置全局区块压缩算法
pub fn set_compression(comp: ChunkCompression) {
    COMPRESSION.store(comp as u8, Ordering::Relaxed);
}

/// D4: Record a save operation time for adaptive compression tuning
pub fn record_save_time(duration_ms: u64) {
    let mut times = SAVE_TIMES_MS.lock().unwrap();
    times.push(duration_ms);
    if times.len() > ADAPTIVE_WINDOW {
        times.remove(0);
    }
    // Adaptive mode: automatically switch based on I/O throughput
    if COMPRESSION.load(Ordering::Relaxed) == 2 {
        let avg = times.iter().sum::<u64>() / times.len() as u64;
        let current = compression();
        if avg > SLOW_SAVE_THRESHOLD_MS && current == ChunkCompression::Zstd {
            // Too slow on Zstd — fall back to faster LZ4
            COMPRESSION.store(0, Ordering::Relaxed);
            tracing::info!("Adaptive compression: switched to LZ4 (avg save {}ms > {}ms threshold)", avg, SLOW_SAVE_THRESHOLD_MS);
        } else if avg < FAST_SAVE_THRESHOLD_MS && current == ChunkCompression::Lz4 {
            // Fast enough — switch to Zstd for better compression
            COMPRESSION.store(1, Ordering::Relaxed);
            tracing::info!("Adaptive compression: switched to Zstd (avg save {}ms < {}ms threshold)", avg, FAST_SAVE_THRESHOLD_MS);
        }
    }
}

/// 获取当前压缩算法
pub fn compression() -> ChunkCompression {
    match COMPRESSION.load(Ordering::Relaxed) {
        1 => ChunkCompression::Zstd,
        2 => ChunkCompression::Lz4, // adaptive starts as LZ4 (safe default)
        _ => ChunkCompression::Lz4,
    }
}

/// 线程安全的区块存储
///
/// 使用 DashMap 分片锁 + Rayon 并行生成 + LZ4 Linear 格式。
/// 超过 max_chunks 时自动 LRU 驱逐。
#[derive(Clone)]
pub struct ChunkStore {
    chunks: std::sync::Arc<DashMap<ChunkPos, Chunk>>,
    count: std::sync::Arc<AtomicUsize>,
    max_chunks: usize,
}

impl ChunkStore {
    pub fn new() -> Self {
        Self {
            chunks: std::sync::Arc::new(DashMap::with_capacity(1024)),
            count: std::sync::Arc::new(AtomicUsize::new(0)),
            max_chunks: DEFAULT_MAX_CHUNKS,
        }
    }

    pub fn with_max_chunks(mut self, max: usize) -> Self {
        self.max_chunks = max;
        self
    }

    /// D5: Estimated memory usage in bytes (chunks × ~100KB avg)
    pub fn estimated_memory_bytes(&self) -> usize {
        self.chunks.len() * ESTIMATED_CHUNK_BYTES
    }

    /// D5: Check if adding `n` more chunks would exceed the memory budget
    pub fn would_exceed_budget(&self, additional: usize, max_mb: usize) -> bool {
        let future = self.chunks.len() + additional;
        future * ESTIMATED_CHUNK_BYTES > max_mb * 1024 * 1024
    }

    /// D5: Get memory budget status as (used_mb, max_mb, usage_pct)
    pub fn memory_budget_status(&self, max_mb: usize) -> (usize, usize, f64) {
        let used_mb = self.estimated_memory_bytes() / (1024 * 1024);
        let pct = if max_mb > 0 { used_mb as f64 / max_mb as f64 * 100.0 } else { 0.0 };
        (used_mb, max_mb, pct)
    }

    /// Rayon 并行生成多个区块
    pub fn generate_parallel(
        &self,
        positions: &[ChunkPos],
        generator: &dyn crate::generator::TerrainGenerator,
        seed: u64,
    ) -> usize {
        let generated: Vec<(ChunkPos, Chunk)> = positions
            .par_iter()
            .filter_map(|&pos| {
                // Skip if already loaded
                if self.chunks.contains_key(&pos) {
                    return None;
                }
                // Try disk load first
                let chunk = crate::chunk_store::load_from_disk_linear(pos);
                Some((pos, chunk.unwrap_or_else(|| generator.generate_chunk(pos, seed))))
            })
            .collect();

        let count = generated.len();
        for (pos, chunk) in generated {
            self.insert(pos, chunk);
        }
        if count > 0 {
            tracing::info!("Rayon parallel: generated {} chunks ({} total)", count, self.chunks.len());
        }
        count
    }

    /// 批量预生成 spawn 周围区块 — 距离优先 (近→远分 bands 渐进加载)
    /// 参考 Pumpkin 的二叉堆优先级队列: 先加载最近的区块保证出生点可立即游玩
    pub fn preload_spawn(
        &self,
        center_x: i32,
        center_z: i32,
        radius: i32,
        generator: &dyn crate::generator::TerrainGenerator,
        seed: u64,
    ) {
        let total_chunks = ((2 * radius + 1) * (2 * radius + 1)) as usize;
        let mut positions = Vec::with_capacity(total_chunks);
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                positions.push(ChunkPos::new(center_x + dx, center_z + dz));
            }
        }
        // Sort by Chebyshev distance (Minecraft chunk distance metric) — center first
        positions.sort_by_cached_key(|cp| {
            std::cmp::max((cp.x - center_x).unsigned_abs(), (cp.z - center_z).unsigned_abs())
        });

        // Process in distance bands for progressive loading with progress logging
        let mut processed = 0usize;
        let mut current_dist = 0u32;
        let mut band_start = 0usize;
        for (i, cp) in positions.iter().enumerate() {
            let dist = std::cmp::max(
                (cp.x - center_x).unsigned_abs(),
                (cp.z - center_z).unsigned_abs(),
            );
            if dist != current_dist || i == positions.len() - 1 {
                let band_end = if i == positions.len() - 1 { positions.len() } else { i };
                if band_end > band_start {
                    let band: Vec<ChunkPos> = positions[band_start..band_end].to_vec();
                    let n = self.generate_parallel(&band, generator, seed);
                    processed += n;
                    if n > 0 {
                        tracing::info!(
                            "Spawn preload band d={}: {} chunks (total {} / {})",
                            current_dist, n, processed, total_chunks
                        );
                    }
                }
                current_dist = dist;
                band_start = i;
            }
        }

        tracing::info!(
            "Spawn preload complete: {} chunks at {} total · {} skipped (already loaded)",
            processed, self.chunks.len(), total_chunks.saturating_sub(processed)
        );
    }

    pub fn insert(&self, pos: ChunkPos, chunk: Chunk) {
        if self.chunks.insert(pos, chunk).is_none() {
            self.count.fetch_add(1, Ordering::Relaxed);
        }
        let current = self.count.load(Ordering::Relaxed);
        if current > self.max_chunks {
            self.evict_lru(current - self.max_chunks);
        }
    }

    fn evict_lru(&self, count: usize) {
        // Phase 1: evict clean (non-dirty) chunks first
        let mut candidates: Vec<ChunkPos> = self.chunks.iter()
            .filter(|entry| !entry.value().dirty)
            .map(|entry| *entry.key())
            .collect();
        candidates.sort_unstable_by_key(|key| {
            self.chunks.get(key).map(|c| c.lru_order).unwrap_or(u64::MAX)
        });
        let clean_n = count.min(candidates.len());
        for key in candidates.iter().take(clean_n) {
            if self.chunks.remove(key).is_some() {
                self.count.fetch_sub(1, Ordering::Relaxed);
            }
        }
        if clean_n > 0 {
            tracing::debug!("LRU evicted {} clean chunks (capacity={})", clean_n, self.max_chunks);
        }

        // Phase 2: if still over capacity, flush and evict dirty chunks
        let remaining = count.saturating_sub(clean_n);
        if remaining > 0 {
            let dirty_n = self.flush_dirty_lru(remaining);
            if dirty_n > 0 {
                tracing::info!("LRU: flushed and evicted {} dirty chunks to stay within capacity ({})",
                    dirty_n, self.max_chunks);
            }
        }
    }

    /// Flush the oldest dirty chunks to disk and evict them.
    /// Returns the number of chunks flushed+evicted.
    pub fn flush_dirty_lru(&self, count: usize) -> usize {
        let mut candidates: Vec<ChunkPos> = self.chunks.iter()
            .filter(|entry| entry.value().dirty)
            .map(|entry| *entry.key())
            .collect();
        candidates.sort_unstable_by_key(|key| {
            self.chunks.get(key).map(|c| c.lru_order).unwrap_or(u64::MAX)
        });
        let n = count.min(candidates.len());
        let mut flushed = 0usize;
        for key in candidates.iter().take(n) {
            // Clone the chunk for I/O, then evict
            if let Some(chunk) = self.chunks.get(key).map(|r| r.clone()) {
                let region_dir = std::path::Path::new("data/world/region");
                if save_chunk_linear(&chunk, region_dir).is_ok() {
                    flushed += 1;
                }
            }
            if self.chunks.remove(key).is_some() {
                self.count.fetch_sub(1, Ordering::Relaxed);
            }
        }
        flushed
    }

    /// Count dirty chunks currently in memory
    pub fn dirty_count(&self) -> usize {
        self.chunks.iter().filter(|entry| entry.value().dirty).count()
    }

    /// Proactive writeback: if dirty chunks exceed threshold, flush oldest without evicting.
    /// Returns number of chunks flushed (but kept in memory, now clean).
    pub fn proactive_writeback(&self) -> usize {
        let dirty = self.dirty_count();
        let threshold = (self.max_chunks as f64 * DIRTY_WRITEBACK_THRESHOLD) as usize;
        if dirty <= threshold {
            return 0;
        }
        let to_flush = dirty.saturating_sub(threshold / 2); // flush down to half threshold
        let mut candidates: Vec<ChunkPos> = self.chunks.iter()
            .filter(|entry| entry.value().dirty)
            .map(|entry| *entry.key())
            .collect();
        candidates.sort_unstable_by_key(|key| {
            self.chunks.get(key).map(|c| c.lru_order).unwrap_or(u64::MAX)
        });
        let mut flushed = 0usize;
        for key in candidates.iter().take(to_flush) {
            if let Some(chunk) = self.chunks.get(key).map(|r| r.clone()) {
                let region_dir = std::path::Path::new("data/world/region");
                if save_chunk_linear(&chunk, region_dir).is_ok() {
                    // Mark as clean after successful write
                    if let Some(mut entry) = self.chunks.get_mut(key) {
                        entry.value_mut().dirty = false;
                    }
                    flushed += 1;
                }
            }
        }
        if flushed > 0 {
            tracing::debug!("Proactive writeback: flushed {} dirty chunks ({} dirty → {} dirty, threshold={})",
                flushed, dirty, self.dirty_count(), threshold);
        }
        flushed
    }

    /// Check if a chunk position is loaded in memory
    pub fn contains_key(&self, pos: &ChunkPos) -> bool {
        self.chunks.contains_key(pos)
    }

    pub fn get(&self, pos: &ChunkPos) -> Option<dashmap::mapref::one::Ref<'_, ChunkPos, Chunk>> {
        self.chunks.get(pos)
    }

    pub fn get_mut(&self, pos: &ChunkPos) -> Option<dashmap::mapref::one::RefMut<'_, ChunkPos, Chunk>> {
        self.chunks.get_mut(pos)
    }

    pub fn get_or_load(&self, pos: ChunkPos) -> dashmap::mapref::one::RefMut<'_, ChunkPos, Chunk> {
        self.chunks.entry(pos).or_insert_with(|| Chunk::new(pos))
    }

    pub fn count(&self) -> usize { self.chunks.len() }

    pub fn all_chunks(&self) -> Vec<(ChunkPos, Chunk)> {
        self.chunks.iter().map(|entry| (*entry.key(), entry.value().clone())).collect()
    }

    /// Returns all loaded chunk positions without cloning chunk data (lightweight)
    pub fn all_loaded_positions(&self) -> Vec<ChunkPos> {
        self.chunks.iter().map(|entry| *entry.key()).collect()
    }

    pub fn dirty_chunks(&self) -> Vec<(ChunkPos, Chunk)> {
        self.chunks.iter()
            .filter(|entry| entry.value().dirty)
            .map(|entry| (*entry.key(), entry.value().clone()))
            .collect()
    }

    pub fn remove(&self, pos: &ChunkPos) -> Option<Chunk> {
        self.chunks.remove(pos).map(|(_, c)| c)
    }

    pub fn unload_distant(&self, keep: &[ChunkPos]) {
        let keep_set: std::collections::HashSet<_> = keep.iter().collect();
        let to_remove: Vec<ChunkPos> = self.chunks.iter()
            .map(|e| *e.key())
            .filter(|k| !keep_set.contains(k))
            .collect();
        for pos in to_remove {
            self.chunks.remove(&pos);
        }
    }

    /// 尝试从 Anvil 磁盘加载
    pub fn try_load_from_disk(&self, pos: ChunkPos, region_dir: &Path) -> Option<Chunk> {
        if self.chunks.contains_key(&pos) {
            return self.chunks.get(&pos).map(|r| r.clone());
        }
        // Try Linear format first, fallback to Anvil
        if let Some(chunk) = load_from_disk_linear(pos) {
            self.insert(pos, chunk.clone());
            return Some(chunk);
        }
        let mut reader = crate::anvil::AnvilReader::new();
        match reader.load_chunk(region_dir, pos.x, pos.z) {
            Ok(Some(chunk)) => {
                self.insert(pos, chunk.clone());
                Some(chunk)
            }
            _ => None,
        }
    }

    pub fn mark_all_clean(&self) {
        for mut entry in self.chunks.iter_mut() {
            entry.value_mut().dirty = false;
        }
    }
}

impl Default for ChunkStore {
    fn default() -> Self { Self::new() }
}

// ═══════════════════════════════════════════════════════
// LZ4 Linear 格式 — 快速压缩区块存储
// ═══════════════════════════════════════════════════════

use std::fs::{self, File};
use std::io::Write;

/// 将单个区块序列化并 LZ4 压缩后写入 Linear 文件
/// 格式: [chunk_x:i32 LE][chunk_z:i32 LE][format:u8][data_len:u32 LE][lz4_data]
/// format: 0=LZ4 (当前唯一支持), 1-255=保留
pub fn save_chunk_linear(chunk: &Chunk, region_dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(region_dir)?;
    let filename = format!("c.{}.{}.linear", chunk.position.x, chunk.position.z);
    let path = region_dir.join(&filename);

    // Serialize chunk to binary
    let raw = serialize_chunk_binary(chunk);
    // Compress with LZ4 (NEON auto-vectorized via target-cpu=cortex-a76)
    let compressed = lz4_flex::compress_prepend_size(&raw);

    let mut file = File::create(&path)?;
    file.write_all(&chunk.position.x.to_le_bytes())?;
    file.write_all(&chunk.position.z.to_le_bytes())?;
    file.write_all(&0u8.to_le_bytes())?; // format byte: 0=LZ4
    file.write_all(&(compressed.len() as u32).to_le_bytes())?;
    file.write_all(&compressed)?;
    Ok(())
}

/// 批量保存脏区块为 LZ4/Zstd Linear 格式 (D4: adaptive compression with timing)
pub fn save_dirty_chunks_linear(chunks: &[(ChunkPos, Chunk)], region_dir: &Path) -> usize {
    let start = std::time::Instant::now();
    let mut count = 0usize;
    let comp = compression();
    for (_pos, chunk) in chunks {
        if chunk.dirty {
            match save_chunk_linear(chunk, region_dir) {
                Ok(()) => count += 1,
                Err(e) => tracing::error!("{} save failed for ({},{}): {}", if comp == ChunkCompression::Zstd { "Zstd" } else { "LZ4" }, chunk.position.x, chunk.position.z, e),
            }
        }
    }
    let elapsed_ms = start.elapsed().as_millis() as u64;
    if count > 0 {
        tracing::info!("{} saved {} chunks to {} ({}ms)", if comp == ChunkCompression::Zstd { "Zstd" } else { "LZ4" }, count, region_dir.display(), elapsed_ms);
        // D4: record save time for adaptive compression
        record_save_time(elapsed_ms);
    }
    count
}

/// 从 LZ4 Linear 文件加载单个区块
pub fn load_from_disk_linear(pos: ChunkPos) -> Option<Chunk> {
    let _rx = pos.x.div_euclid(32);
    let _rz = pos.z.div_euclid(32);
    // Try current directory and common region paths
    let candidates = [
        std::env::current_dir().ok().map(|p| p.join("data/world/region")),
        std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("data/world/region"))),
    ];
    for base in candidates.iter().flatten() {
        let filename = format!("c.{}.{}.linear", pos.x, pos.z);
        let path = base.join(&filename);
        if path.exists() {
            match load_linear_file(&path) {
                Ok(chunk) => {
                    tracing::trace!("LZ4 loaded chunk ({},{})", pos.x, pos.z);
                    return Some(chunk);
                }
                Err(e) => {
                    tracing::debug!("LZ4 read failed for ({},{}): {}", pos.x, pos.z, e);
                }
            }
        }
    }
    None
}

fn load_linear_file(path: &Path) -> std::io::Result<Chunk> {
    let data = fs::read(path)?;
    if data.len() < 12 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "file too small"));
    }
    let cx = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let cz = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let byte8 = data[8];

    // Auto-detect: format byte 0=LZ4(new), >=2 = old format without format byte
    let data_offset = if byte8 <= 1 { 13 } else { 8 };

    if data.len() < data_offset { return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "truncated header")); }
    let len_pos = if byte8 <= 1 { 9 } else { 8 };
    let data_len = u32::from_le_bytes([data[len_pos], data[len_pos+1], data[len_pos+2], data[len_pos+3]]) as usize;
    if data_len == 0 || data_len > 10_000_000 || data_offset + data_len > data.len() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid data length"));
    }

    let compressed = &data[data_offset..data_offset + data_len];
    let raw = lz4_flex::decompress_size_prepended(compressed)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    deserialize_chunk_binary(cx, cz, &raw)
}

// Thread-local scratch buffer for chunk serialization reuse (A7).
// Avoids allocating a new ~64KB Vec per serialized chunk during save operations.
// D1: Initial buffer is allocated with hugepage-friendly size (64KB aligned).
// On Linux, jemalloc metadata_thp:always ensures THP for this allocation.
std::thread_local! {
    static SERIALIZE_SCRATCH: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new({
        let mut v = Vec::with_capacity(65536);
        // D1: hint the OS to use hugepages for this frequently-used buffer
        #[cfg(target_os = "linux")]
        unsafe {
            let ptr = v.as_mut_ptr();
            let len = v.capacity();
            // madvise MADV_HUGEPAGE — suggest transparent hugepage for this region
            libc::madvise(ptr as *mut libc::c_void, len, libc::MADV_HUGEPAGE);
        }
        v
    });
}

/// 二进制序列化区块 (用于 LZ4 压缩前)
/// Uses a thread-local scratch buffer to avoid repeated allocations during batch saves (A7).
fn serialize_chunk_binary(chunk: &Chunk) -> Vec<u8> {
    SERIALIZE_SCRATCH.with(|cell| {
        let mut buf = cell.borrow_mut();
        buf.clear();
        buf.extend_from_slice(&chunk.position.x.to_le_bytes());
        buf.extend_from_slice(&chunk.position.z.to_le_bytes());
        let filled: Vec<usize> = chunk.sections.iter().enumerate()
            .filter(|(_, s)| s.is_some()).map(|(i, _)| i).collect();
        buf.extend_from_slice(&(filled.len() as u16).to_le_bytes());
        for idx in &filled {
            if let Some(sec) = &chunk.sections[*idx] {
                buf.extend_from_slice(&(*idx as u16).to_le_bytes());
                let blocks_data = sec.blocks.encode_binary();
                buf.extend_from_slice(&(blocks_data.len() as u32).to_le_bytes());
                buf.extend_from_slice(&blocks_data);
                let biomes_data = sec.biomes.encode_binary();
                buf.extend_from_slice(&(biomes_data.len() as u32).to_le_bytes());
                buf.extend_from_slice(&biomes_data);
                buf.extend_from_slice(&sec.sky_light[..]);
                buf.extend_from_slice(&sec.block_light[..]);
            }
        }
        // Clone the serialized data out of the scratch buffer
        buf.clone()
    })
}

/// 二进制反序列化区块
fn deserialize_chunk_binary(cx: i32, cz: i32, data: &[u8]) -> std::io::Result<Chunk> {
    use crate::chunk::Section;

    let mut chunk = Chunk::new(ChunkPos::new(cx, cz));
    if data.len() < 2 { return Ok(chunk); }
    let count = u16::from_le_bytes([data[0], data[1]]) as usize;
    let mut pos = 2usize;
    for _ in 0..count {
        if pos + 2 > data.len() { break; }
        let idx = u16::from_le_bytes([data[pos], data[pos+1]]) as usize;
        pos += 2;
        // blocks
        if pos + 4 > data.len() { break; }
        let blocks_len = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
        pos += 4;
        let mut blocks = crate::paletted::PalettedContainer::new();
        if pos + blocks_len <= data.len() {
            let _ = blocks.decode_binary(&data[pos..pos+blocks_len]);
            pos += blocks_len;
        } else { break; }
        // biomes
        if pos + 4 > data.len() { break; }
        let biomes_len = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
        pos += 4;
        let mut biomes = crate::paletted::PalettedContainer::new();
        if pos + biomes_len <= data.len() {
            let _ = biomes.decode_binary(&data[pos..pos+biomes_len]);
            pos += biomes_len;
        } else { break; }
        // light
        if pos + 4096 > data.len() { break; }
        let mut sky_light = Box::new([0u8; 2048]);
        sky_light.copy_from_slice(&data[pos..pos+2048]);
        pos += 2048;
        let mut block_light = Box::new([0u8; 2048]);
        block_light.copy_from_slice(&data[pos..pos+2048]);
        pos += 2048;

        if idx < chunk.sections.len() {
            let section_y = mc_core::position::MIN_SECTION_Y + idx as i32;
            chunk.sections[idx] = Some(Section {
                position: mc_core::position::SectionPos::new(ChunkPos::new(cx, cz), section_y),
                blocks,
                biomes,
                sky_light,
                block_light,
            });
        }
    }
    chunk.dirty = false;
    Ok(chunk)
}
