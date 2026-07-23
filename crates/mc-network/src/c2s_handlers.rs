//! C2S packet handler functions — extracted from connection.rs play_loop.
//!
//! Each function handles one C2S packet type with minimal parameter sets.
//! The play_loop match dispatches to these functions instead of containing inline code.

use crate::connection::{send_packet, send_chunk_data_cached, ServerRef};
use crate::packet_io::PacketStream;
use mc_protocol::codec::read_varint_enum;
use mc_protocol::packets::play::*;
use mc_protocol::varint::varint_size;
use tracing::{debug, info};

// ═══════════════════════════════════════════════════════════════
// 0x27 — Player Action (block break, start/stop digging)
// ═══════════════════════════════════════════════════════════════

pub async fn handle_player_action(
    io: &mut PacketStream,
    server: &ServerRef,
    uuid: &uuid::Uuid,
    frame: &[u8],
) {
    if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(frame)
        && payload.len() >= 4 {
            let status = payload[0] as i32;
            if let Some(pos_bytes) = payload.get(1..)
                && pos_bytes.len() >= 8 {
                let raw = i64::from_be_bytes(pos_bytes[..8].try_into().unwrap_or([0;8]));
                let x = (raw >> 38) as i32;
                let y = ((raw << 52) >> 52) as i32;
                let z = (raw >> 12) as i32 & 0x3FFFFFF;
                if !(-64..=319).contains(&y) { return; }
                if status == 2 {
                    block_break(io, server, uuid, x, y, z).await;
                }
            }
    }
}

async fn block_break(
    io: &mut PacketStream,
    server: &ServerRef,
    uuid: &uuid::Uuid,
    x: i32, y: i32, z: i32,
) {
    let cp = mc_core::position::ChunkPos::new(x >> 4, z >> 4);
    if let Some(mut chunk) = server.chunk_store.get_mut(&cp) {
        let old_block = chunk.get_block((x & 0xF) as usize, y, (z & 0xF) as usize);
        chunk.set_block((x & 0xF) as usize, y, (z & 0xF) as usize, mc_core::block::BlockState::AIR);
        mc_world::lighting::recalc_sky_light_on_remove(&mut chunk, (x & 0xF) as usize, y, (z & 0xF) as usize);
        mc_world::lighting::propagate_block_light(&mut chunk);
        let _ = send_chunk_data_cached(io, &mut chunk).await;
        server.dirty_chunks_broadcast.write().insert(cp);
        server.dirty_blocks.mark_block(x, y, z, mc_core::block::BlockState::AIR.id);
        mc_world::redstone::register_vibration(x, y, z, 3);
        mc_world::lighting::propagate_lighting_cross_chunk(
            &server.chunk_store, &cp, (x & 0xF) as usize, y, (z & 0xF) as usize, true);
        // Sound
        let break_sound = SoundEffect {
            sound_id: mc_core::sound::SoundIds::break_sound_for_block(old_block.id),
            category: mc_core::sound::SoundCategory::BLOCKS,
            x: x * 8, y: y * 8, z: z * 8, volume: 1.0, pitch: 1.0, seed: fastrand::i64(..),
        };
        let _ = send_packet(io, &break_sound).await;
        // Particles
        let break_particle = Particle {
            particle_id: 0, long_distance: false,
            x: x as f64 + 0.5, y: y as f64 + 0.5, z: z as f64 + 0.5,
            offset_x: 0.2, offset_y: 0.2, offset_z: 0.2, max_speed: 0.1, count: 10,
        };
        let _ = send_packet(io, &break_particle).await;
        // Enchantment handling
        let held_item = server.player_manager.get_held_item(uuid);
        let held_enchants = held_item.as_ref()
            .and_then(|i| i.nbt.as_ref())
            .map(|nbt| mc_player::enchant::parse_item_enchants(&Some(nbt.clone())))
            .unwrap_or_default();
        let unbreaking_level = held_enchants.get("unbreaking").copied().unwrap_or(0);
        let skip_durability = unbreaking_level > 0
            && fastrand::u32(1..=100) <= 100 / (unbreaking_level as u32 + 1);
        if !skip_durability {
            let _ = server.player_manager.damage_held_item(uuid, 1);
        }
        // Drop items
        if !old_block.is_air() {
            let fortune_level = held_enchants.get("fortune").copied().unwrap_or(0);
            let extra_drops = if fortune_level > 0 && is_ore_block(old_block.id) {
                (fastrand::u32(..) % (fortune_level as u32 + 1)) as usize
            } else { 0 };
            for _ in 0..=extra_drops {
                let eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let _ = send_packet(io, &SpawnEntity {
                    entity_id: eid, entity_uuid: uuid::Uuid::new_v4(), entity_type: 54,
                    x: x as f64 + 0.5, y: y as f64 + 0.5, z: z as f64 + 0.5,
                    pitch: 0, yaw: 0, head_yaw: 0, data: 1,
                    vel_x: 0, vel_y: 150, vel_z: 0,
                }).await;
                server.dropped_items.write().insert(eid, (old_block.id, x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5));
            }
        }
    }
}

fn is_ore_block(block_id: u32) -> bool {
    matches!(block_id,
        31 | 29 | 27 | 47 | 300 | 117 | 59 | 303 |
        337 | 338 | 339 | 340 | 341 | 342 | 343
    )
}

// ═══════════════════════════════════════════════════════════════
// 0x3E — Use Item On (block placement + container open)
// ═══════════════════════════════════════════════════════════════

pub async fn handle_use_item_on(
    io: &mut PacketStream,
    server: &ServerRef,
    uuid: &uuid::Uuid,
    username: &str,
    frame: &[u8],
) {
    if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(frame) {
        // Read hand
        let (hand, _) = read_varint_enum(&payload).unwrap_or((0, 0));
        let off = varint_size(hand);
        if off + 8 > payload.len() { return; }
        let raw = i64::from_be_bytes(payload[off..off+8].try_into().unwrap_or([0;8]));
        let x = (raw >> 38) as i32;
        let y = ((raw << 52) >> 52) as i32;
        let z = (raw >> 12) as i32 & 0x3FFFFFF;
        let _ = hand;

        if !(-64..=319).contains(&y) { return; }

        let cp = mc_core::position::ChunkPos::new(x >> 4, z >> 4);
        if let Some(chunk) = server.chunk_store.get(&cp) {
            let target_block = chunk.get_block((x & 0xF) as usize, y, (z & 0xF) as usize);
            if let Some(slot_count) = mc_player::container::container_slot_count(target_block.id) {
                let window_id = server.container_manager.open(uuid, (x, y, z), slot_count);
                let window_type = mc_player::container::container_window_type(target_block.id);
                let _ = send_packet(io, &OpenScreen {
                    window_id: window_id as i32,
                    window_type,
                    title: "{\"text\":\"Container\"}".into(),
                }).await;
                let slots: Vec<Option<SlotData>> = server.container_manager
                    .all_slots(window_id).into_iter()
                    .map(|opt| opt.map(|s| SlotData { item_id: s.item.id as i32, count: s.count, nbt: s.nbt.clone() }))
                    .collect();
                let carried = server.player_manager.get_cursor_item(uuid)
                    .map(|s| SlotData { item_id: s.item.id as i32, count: s.count, nbt: s.nbt.clone() });
                let state = server.container_manager.get_state_id(window_id);
                let _ = send_packet(io, &ContainerSetContent {
                    window_id, state_id: state, items: slots, carried_item: carried,
                }).await;
                debug!("Opened container at ({}, {}, {}) for {}", x, y, z, username);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// 0x0F — Container Close
// ═══════════════════════════════════════════════════════════════

pub fn handle_container_close(
    io: &PacketStream,
    server: &ServerRef,
    uuid: &uuid::Uuid,
    frame: &[u8],
) {
    if let Ok(c) = io.codec().decode::<ContainerClose>(frame) {
        server.container_manager.close(uuid, c.window_id);
    }
}

// ═══════════════════════════════════════════════════════════════
// 0x12 — Rename Item (anvil)
// ═══════════════════════════════════════════════════════════════

pub fn handle_rename_item(
    io: &PacketStream,
    server: &ServerRef,
    uuid: &uuid::Uuid,
    username: &str,
    frame: &[u8],
) {
    if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(frame) {
        let (name_len, off) = read_varint_enum(&payload).unwrap_or((0, 0));
        if name_len > 0 && name_len < 64 && off + name_len as usize <= payload.len() {
            let name = String::from_utf8_lossy(&payload[off..off + name_len as usize]).to_string();
            let _ = server.player_manager.set_held_item_name(uuid, &name);
            info!("{} renamed item to '{}'", username, name);
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// 0x19 — Paddle Boat
// ═══════════════════════════════════════════════════════════════

pub fn handle_paddle_boat(io: &PacketStream, server: &ServerRef, uuid: &uuid::Uuid, frame: &[u8]) {
    if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(frame)
        && payload.len() >= 2 {
            let left = payload[0] != 0;
            let right = payload[1] != 0;
            if left || right {
                let eid = server.player_manager.get_entity_id(uuid).unwrap_or(0);
                server.player_manager.apply_boat_paddle(uuid, eid, left, right);
            }
    }
}

// ═══════════════════════════════════════════════════════════════
// 0x25 — Pick From Block (creative mode)
// ═══════════════════════════════════════════════════════════════

pub fn handle_pick_from_block(io: &PacketStream, server: &ServerRef, uuid: &uuid::Uuid, frame: &[u8]) {
    if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(frame) {
        let (block_id, _) = read_varint_enum(&payload).unwrap_or((0, 0));
        let block = mc_core::block::BlockState::new(block_id as u32);
        let _ = server.player_manager.add_item_to_player(uuid, block, 1);
    }
}

// ═══════════════════════════════════════════════════════════════
// 0x17 — Pick Item (middle-click)
// ═══════════════════════════════════════════════════════════════

pub fn handle_pick_item(io: &PacketStream, server: &ServerRef, uuid: &uuid::Uuid, frame: &[u8]) {
    if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(frame)
        && !payload.is_empty() {
            let slot = payload[0] as i32;
            if (0..=8).contains(&slot) {
                let px = server.player_manager.get(uuid).map(|p| p.position.x).unwrap_or(0.0) as i32;
                let py = server.player_manager.get(uuid).map(|p| p.position.y).unwrap_or(64.0) as i32;
                let pz = server.player_manager.get(uuid).map(|p| p.position.z).unwrap_or(0.0) as i32;
                let cp = mc_core::position::ChunkPos::new(px >> 4, pz >> 4);
                if let Some(chunk) = server.chunk_store.get(&cp) {
                    let block = chunk.get_block((px & 0xF) as usize, py - 1, (pz & 0xF) as usize);
                    if !block.is_air() {
                        let _ = server.player_manager.add_item_to_player(uuid, block, 1);
                    }
                }
            }
    }
}

// ═══════════════════════════════════════════════════════════════
// 0x24 — Resource Pack Response
// ═══════════════════════════════════════════════════════════════

pub fn handle_resource_pack(io: &PacketStream, server: &ServerRef, uuid: &uuid::Uuid, frame: &[u8]) {
    if let Ok(resp) = io.codec().decode::<ResourcePackResponse>(frame) {
        debug!("Resource pack response from {}: status={}", server.player_manager.get(uuid).map(|p| p.username.clone()).unwrap_or_default(), resp.result);
    }
}

// ═══════════════════════════════════════════════════════════════
// 0x13 — Recipe Book Data
// ═══════════════════════════════════════════════════════════════

pub fn handle_recipe_book_data(io: &PacketStream, server: &ServerRef, uuid: &uuid::Uuid, frame: &[u8]) {
    if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(frame)
        && payload.len() >= 2 {
            let recipe_id = u16::from_be_bytes([payload[0], payload[1]]);
            server.player_manager.add_known_recipe(uuid, recipe_id);
    }
}

// ═══════════════════════════════════════════════════════════════
// 0x08 — Command Suggestions (tab-complete)
// ═══════════════════════════════════════════════════════════════

pub async fn handle_command_suggestions(
    io: &mut PacketStream,
    frame: &[u8],
) {
    if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(frame) {
        let (tx_id, off) = read_varint_enum(&payload).unwrap_or((0, 0));
        let _text = String::from_utf8_lossy(&payload[off..]).to_string();
        let empty_resp = CommandSuggestionsResponse {
            transaction_id: tx_id,
            start: 0, length: _text.len() as i32,
            matches: Vec::new(),
        };
        let _ = send_packet(io, &empty_resp).await;
    }
}

// ═══════════════════════════════════════════════════════════════
// 0x23 — Select Trade (villager)
// ═══════════════════════════════════════════════════════════════

/// Handle SelectTrade (0x23). Returns true to continue, false to skip this packet.
pub fn handle_select_trade(io: &PacketStream, _server: &ServerRef, username: &str, frame: &[u8]) -> bool {
    if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(frame) {
        let (slot, _) = read_varint_enum(&payload).unwrap_or((0, 0));
        if !(0..=9).contains(&slot) {
            debug!("{} selected invalid trade slot {}", username, slot);
            return false;
        }
    }
    true
}

// ═══════════════════════════════════════════════════════════════
// 0x10 — Lock Difficulty
// ═══════════════════════════════════════════════════════════════

/// Handle LockDifficulty (0x10). Returns true to continue, false to skip (non-OP).
pub fn handle_lock_difficulty(io: &PacketStream, server: &ServerRef, uuid: &uuid::Uuid, username: &str, frame: &[u8]) -> bool {
    let is_op = server.player_manager.get(uuid).map(|p| p.is_op).unwrap_or(false);
    if !is_op {
        debug!("Non-OP player '{}' attempted to lock difficulty", username);
        return false;
    }
    if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(frame)
        && !payload.is_empty() {
            let locked = payload[0] != 0;
            server.world_state.write().difficulty_locked = locked;
            info!("Difficulty lock: {} (by {})", locked, username);
    }
    true
}

// ═══════════════════════════════════════════════════════════════
// 0x0E — Edit Book
// ═══════════════════════════════════════════════════════════════

pub fn handle_edit_book(io: &PacketStream, server: &ServerRef, uuid: &uuid::Uuid, username: &str, frame: &[u8]) {
    if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(frame) {
        let (slot, off) = read_varint_enum(&payload).unwrap_or((0, 0));
        let text = String::from_utf8_lossy(&payload[off..]).to_string();
        let _ = server.player_manager.update_item_nbt_at_slot(uuid, slot, &text);
        info!("Book edited by {} in slot {} ({} chars)", username, slot, text.len());
    }
}

// ═══════════════════════════════════════════════════════════════
// 0x11 — Advancement Tab
// ═══════════════════════════════════════════════════════════════

pub fn handle_advancement_tab(io: &PacketStream, username: &str, frame: &[u8]) {
    if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(frame) {
        let action = payload.first().copied().unwrap_or(0);
        if action == 0 && payload.len() > 1 {
            let tab_id = String::from_utf8_lossy(&payload[1..]).to_string();
            info!("{} opened advancement tab '{}'", username, tab_id);
        } else {
            debug!("{} advancement tab action={}", username, action);
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// 0x16 — Cookie Response (play phase)
// ═══════════════════════════════════════════════════════════════

pub fn handle_cookie_response(io: &PacketStream, frame: &[u8]) {
    if let Ok(c) = io.codec().decode::<CookieResponse>(frame) {
        debug!("Cookie response: key={}", c.key);
    }
}
