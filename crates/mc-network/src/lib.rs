#![allow(clippy::type_complexity)]

pub mod c2s_handlers;
pub mod connection;
pub mod encryption;
pub mod handler_sync;
pub mod lan_broadcast;
pub mod listener;
pub mod packet_io;
pub mod play_loop;
pub mod rate_limiter;

// Type aliases for complex shared types
use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashMap;
/// Shared dropped items tracker: entity_id → (item_block_id, x, y, z)
pub type SharedDroppedItems = Arc<RwLock<HashMap<i32, (u32, f64, f64, f64)>>>;
/// Shared jukebox disc tracker: (x, y, z) → disc_id
pub type SharedJukeboxDiscs = Arc<RwLock<HashMap<(i32, i32, i32), u32>>>;
