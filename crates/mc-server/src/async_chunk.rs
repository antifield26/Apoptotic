//! Async chunk loading bridge (Phase A5)
//!
//! Decouples chunk generation from the tick thread:
//! - Tick loop enqueues positions → background Rayon pool loads/generates
//! - Completed chunks are picked up on the next tick via `drain_completed()`
//! - Eliminates synchronous I/O stalls during player teleport/movement

use dashmap::DashMap;
use mc_core::position::ChunkPos;
use mc_world::chunk::Chunk;
use mc_world::chunk_store::ChunkStore;
use std::sync::{Arc, Mutex};
use tracing;

/// Async chunk loading bridge — decouples chunk generation from the tick thread.
pub struct AsyncChunkBridge {
    /// Pending positions to load (cleared each tick batch)
    pending: Mutex<Vec<(ChunkPos, u8)>>, // (pos, priority: 0=low, 1=normal, 2=high)
    /// Completed chunks ready for installation
    completed: Arc<DashMap<ChunkPos, Chunk>>,
    /// Tokio runtime handle for spawning blocking work
    runtime: Mutex<Option<tokio::runtime::Handle>>,
}

impl AsyncChunkBridge {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(Vec::with_capacity(256)),
            completed: Arc::new(DashMap::with_capacity(128)),
            runtime: Mutex::new(None),
        }
    }

    /// Set the Tokio runtime handle — must be called once after creation.
    pub fn set_runtime(&self, handle: tokio::runtime::Handle) {
        *self.runtime.lock().unwrap() = Some(handle);
    }

    /// Enqueue chunk positions for async loading. Prioritized by distance to player.
    /// Deduplicates against both the chunk_store and already-pending positions.
    pub fn enqueue(
        &self,
        positions: &[(ChunkPos, u8)],
        chunk_store: &ChunkStore,
        generator: Arc<dyn mc_world::generator::TerrainGenerator + Send + Sync>,
        seed: u64,
    ) {
        let mut pending = self.pending.lock().unwrap();
        for (pos, priority) in positions {
            // Skip if already loaded or already queued
            if chunk_store.contains_key(pos) {
                continue;
            }
            if pending.iter().any(|(p, _)| p == pos) {
                continue;
            }
            pending.push((*pos, *priority));
        }
        // Sort by priority (higher first)
        pending.sort_by(|a, b| b.1.cmp(&a.1));

        // Process up to 32 chunks per batch
        let batch_size = pending.len().min(32);
        if batch_size == 0 {
            return;
        }
        let to_process: Vec<(ChunkPos, u8)> = pending.drain(..batch_size).collect();

        let completed = self.completed.clone();
        let cs = chunk_store.clone();
        if let Some(ref handle) = *self.runtime.lock().unwrap() {
            // Offload chunk generation via spawn_blocking → Rayon par_iter
            let _ = handle.spawn_blocking(move || {
                use rayon::prelude::*;
                let positions: Vec<ChunkPos> = to_process.iter().map(|(p, _)| *p).collect();
                let generated: Vec<(ChunkPos, Chunk)> = positions
                    .par_iter()
                    .filter_map(|&pos| {
                        if cs.contains_key(&pos) {
                            return None;
                        }
                        let chunk = mc_world::chunk_store::load_from_disk_linear(pos);
                        Some((pos, chunk.unwrap_or_else(|| generator.generate_chunk(pos, seed))))
                    })
                    .collect();
                for (pos, chunk) in generated {
                    completed.insert(pos, chunk);
                }
            });
        }
    }

    /// Drain all completed chunk loads and install them into the chunk store.
    /// Call once per tick from the main thread.
    pub fn drain_completed(&self, chunk_store: &ChunkStore) -> usize {
        let mut count = 0usize;
        let keys: Vec<ChunkPos> = self.completed.iter().map(|e| *e.key()).collect();
        for pos in keys {
            if let Some((_, chunk)) = self.completed.remove(&pos) {
                chunk_store.insert(pos, chunk);
                count += 1;
            }
        }
        if count > 0 {
            tracing::debug!("Async chunk bridge: installed {} chunks ({} pending)",
                count, self.pending.lock().unwrap().len());
        }
        count
    }

    /// Number of pending load requests
    #[allow(dead_code)]
    pub fn pending_count(&self) -> usize {
        self.pending.lock().unwrap().len()
    }

    /// Number of completed chunks awaiting installation
    #[allow(dead_code)]
    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }
}

impl Default for AsyncChunkBridge {
    fn default() -> Self {
        Self::new()
    }
}
