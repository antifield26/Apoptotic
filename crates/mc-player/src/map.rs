//! Map manager — creates and updates map data from chunk terrain.
//!
//! Each map covers 128×128 blocks (8×8 chunks). Color is sampled from
//! the top non-air block at each position using a simplified palette.

use dashmap::DashMap;
use mc_core::position::ChunkPos;
use mc_world::chunk_store::ChunkStore;
use std::sync::atomic::{AtomicI32, Ordering};

/// Map data ready for packet construction (avoids mc-protocol dependency)
pub struct MapUpdate {
    pub map_id: i32,
    pub scale: u8,
    pub columns: u8,
    pub rows: u8,
    pub pixels: Vec<u8>,
}

/// Minecraft map color palette — maps block types to base colors.
/// Reference: https://minecraft.wiki/w/Map_item_format#Color_table
const COLOR_AIR: u8 = 0;
const COLOR_GRASS: u8 = 12;    // green
const COLOR_SAND: u8 = 22;     // sand
const COLOR_WATER: u8 = 34;    // water blue
const COLOR_STONE: u8 = 54;    // stone gray
const COLOR_WOOD: u8 = 30;     // brown
const COLOR_LEAVES: u8 = 44;   // dark green
const COLOR_SNOW: u8 = 18;     // white
const COLOR_LAVA: u8 = 50;     // orange-red

/// Map a block ID to its map color (simplified 8-color palette)
fn block_to_map_color(block_id: u32) -> u8 {
    match block_id {
        0 => COLOR_AIR,
        // Grass-like: grass_block, dirt variants, podzol, mycelium, farmland
        2 | 3 | 8 | 9 | 10 | 11 | 251 | 252 => COLOR_GRASS,
        // Sand
        12 => COLOR_SAND,
        // Water + Lava
        267 => COLOR_WATER,
        268 => COLOR_LAVA,
        // Snow/ice
        79 | 80 | 174 | 250 => COLOR_SNOW,
        // Wood/planks/logs
        13..=19 | 41 | 47 | 50 | 54 | 58 | 61 | 62 | 63 | 65 |
        68 | 69 | 72 | 84 | 85 | 86 | 91 | 96 | 101 | 102 | 103 |
        107 | 113 | 116 | 120 | 125 | 126 | 134 | 135 | 136 |
        143 | 163 | 164 | 183..=187 | 191..=194 => COLOR_WOOD,
        // Leaves
        161 | 162 => COLOR_LEAVES,
        // Everything else maps to stone gray
        _ => COLOR_STONE,
    }
}

/// Map manager — tracks active maps and their center positions.
pub struct MapManager {
    next_map_id: AtomicI32,
    maps: DashMap<i32, (i32, i32, u8)>, // map_id → (center_x, center_z, scale)
}

impl Default for MapManager {
    fn default() -> Self { Self::new() }
}

impl MapManager {
    pub fn new() -> Self {
        Self {
            next_map_id: AtomicI32::new(0),
            maps: DashMap::new(),
        }
    }

    /// Create a new map centered at the player's position.
    pub fn create_map(&self, center_x: i32, center_z: i32, scale: u8) -> i32 {
        let map_id = self.next_map_id.fetch_add(1, Ordering::Relaxed);
        self.maps.insert(map_id, (center_x, center_z, scale));
        map_id
    }

    /// Generate MapData packet content for a given map ID.
    /// Samples a 128×128 area centered on the map's center position.
    pub fn generate_map_data(
        &self,
        map_id: i32,
        chunk_store: &ChunkStore,
    ) -> Option<MapUpdate> {
        let (cx, cz, scale) = self.maps.get(&map_id).map(|r| *r)?;
        let size = 128i32;
        let half = size / 2;
        let mut pixels = Vec::with_capacity((size * size) as usize);

        for z in 0..size {
            for x in 0..size {
                let world_x = cx - half + x;
                let world_z = cz - half + z;
                let color = sample_top_color(chunk_store, world_x, world_z);
                pixels.push(color);
            }
        }

        Some(MapUpdate {
            map_id,
            scale,
            columns: size as u8,
            rows: size as u8,
            pixels,
        })
    }
}

/// Sample the top non-air block color at a world column.
fn sample_top_color(cs: &ChunkStore, world_x: i32, world_z: i32) -> u8 {
    let cp = ChunkPos::new(world_x >> 4, world_z >> 4);
    if let Some(chunk) = cs.get(&cp) {
        let lx = (world_x & 0xF) as usize;
        let lz = (world_z & 0xF) as usize;
        for y in (0..=319).rev() {
            let block = chunk.get_block(lx, y, lz);
            if !block.is_air() {
                return block_to_map_color(block.id);
            }
        }
    }
    COLOR_AIR
}
