//! Rate limiter — per-IP connection limits + per-connection packet limits.
//! Uses DashMap for lock-free concurrent access.

use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Rate limiter for connections and packets.
pub struct RateLimiter {
    /// Per-IP connection attempt timestamps (for connection rate limiting)
    connections: DashMap<SocketAddr, Vec<Instant>>,
    /// Per-connection packet counter + last reset timestamp
    packets: DashMap<SocketAddr, (AtomicU64, AtomicU64)>, // (count, last_reset_secs)
    /// Reference instant for computing elapsed seconds (fixes B1: Instant::now().elapsed() always 0)
    start: Instant,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            connections: DashMap::new(),
            packets: DashMap::new(),
            start: Instant::now(),
        }
    }

    /// Get seconds since rate limiter was created.
    fn now_secs(&self) -> u64 {
        self.start.elapsed().as_secs()
    }

    /// Check if a new connection from this IP is allowed.
    /// Max 5 connections per minute per IP. Returns true if allowed.
    pub fn check_connection(&self, addr: SocketAddr) -> bool {
        let now = Instant::now();
        let mut entry = self.connections.entry(addr).or_default();
        // Purge timestamps older than 60 seconds
        entry.retain(|t| now.duration_since(*t).as_secs() < 60);
        if entry.len() >= 5 {
            tracing::warn!("Rate limit: {} exceeded 5 connections/minute", addr.ip());
            return false;
        }
        entry.push(now);
        true
    }

    /// Clean up connection tracking for a disconnected peer.
    pub fn cleanup_connection(&self, addr: SocketAddr) {
        self.packets.remove(&addr);
    }

    /// Check if this connection can process another packet.
    /// Max 20 packets per second. Returns true if allowed.
    pub fn check_packet(&self, addr: SocketAddr) -> bool {
        let now_secs = self.now_secs();
        let entry = self.packets.entry(addr).or_insert_with(|| {
            (AtomicU64::new(0), AtomicU64::new(now_secs))
        });
        let val = entry.value();

        let last = val.1.load(Ordering::Relaxed);
        if now_secs - last >= 1 {
            // New second — reset counter
            val.1.store(now_secs, Ordering::Relaxed);
            val.0.store(0, Ordering::Relaxed);
        }

        let current = val.0.fetch_add(1, Ordering::Relaxed);
        if current >= 20 {
            tracing::warn!("Rate limit: {} exceeded 20 packets/second", addr);
            false
        } else {
            true
        }
    }

    /// Periodic cleanup: remove stale packet entries for connections inactive > 60 seconds.
    /// Called from the server tick loop to prevent unbounded DashMap growth.
    pub fn cleanup_stale(&self) {
        let now_secs = self.now_secs();
        self.packets.retain(|_, (_, last_reset)| {
            let last = last_reset.load(Ordering::Relaxed);
            now_secs - last < 60
        });
    }
}

// Global rate limiter instance (lazy initialized)
use std::sync::LazyLock;
static GLOBAL_RATE_LIMITER: LazyLock<RateLimiter> = LazyLock::new(RateLimiter::new);

/// Global check: can this IP establish a new connection?
pub fn allow_connection(addr: SocketAddr) -> bool {
    GLOBAL_RATE_LIMITER.check_connection(addr)
}

/// Global check: can this connection process another packet?
pub fn allow_packet(addr: SocketAddr) -> bool {
    GLOBAL_RATE_LIMITER.check_packet(addr)
}

/// Clean up tracking for a disconnected peer.
pub fn cleanup_addr(addr: SocketAddr) {
    GLOBAL_RATE_LIMITER.cleanup_connection(addr);
}

/// Periodic cleanup of stale rate-limiter entries. Safe to call from tick loop.
pub fn cleanup_stale_rate_limits() {
    GLOBAL_RATE_LIMITER.cleanup_stale();
}
