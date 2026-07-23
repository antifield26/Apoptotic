//! Play loop — main per-connection game tick.
//!
//! Extracted from connection.rs. Handles entity tracking, chunk streaming,
//! keep-alive, C2S packet dispatch, and chat/command routing.

use crate::connection::{send_packet, send_chunk_data_cached, effective_view_distance, stream_new_chunks, fire_advancement, weapon_damage, ServerRef};
use crate::packet_io::PacketStream;
use crate::rate_limiter;
use tracing::{debug, error, info, warn};

pub(crate) async fn play_loop(
    io: &mut PacketStream,
    username: &str,
    _uuid: uuid::Uuid,
    server: &ServerRef,
    _entity_id: i32,
    initial_loaded: std::collections::HashSet<mc_core::position::ChunkPos>,
    peer_socket: Option<std::net::SocketAddr>,
) {
    use mc_protocol::packets::play::*;

    // Subscribe to chat, entity broadcasts, player state, and shutdown signal
    let mut chat_rx = server.player_manager.subscribe_chat();
    let mut entity_rx = server.player_manager.subscribe_entities();
    let mut player_state_rx = server.player_manager.subscribe_player_state();
    let mut mob_pos_rx = server.mob_manager.subscribe_positions();
    let mut shutdown_rx = server.shutdown_tx.subscribe();
    let mut known_entities: std::collections::HashSet<i32> = std::collections::HashSet::new();

    // ═══ Chunk streaming state ═══
    use mc_core::position::ChunkPos;

    let view_radius = effective_view_distance(server) as i32;
    let mut player_chunk = ChunkPos::new(0, 0);   // spawn is at (0,0)
    let mut loaded_chunks = initial_loaded;

    // ═══ Keep-alive state ═══
    use tokio::time::Instant;
    let keep_alive_interval = tokio::time::Duration::from_secs(1);
    let keep_alive_timeout = tokio::time::Duration::from_secs(30);
    let mut last_keep_alive_sent = Instant::now();
    let mut keep_alive_id: i64 = 0;
    let mut last_keep_alive_response = Instant::now();
    let mut player_ping_ms: u32 = 0; // RTT measured via keep-alive
    let mut keep_alive_sent_instant = Instant::now();
    let mut tick_count: u64 = 0; // local tick counter for damage cooldowns

    // ═══ Main read loop ═══
    loop {
        tick_count = tick_count.wrapping_add(1);

        // Check for entity events (spawn/move/despawn of other players)
        // Distance filter: only process entities within view range
        let max_range = (server.view_distance as f64 * 16.0 + 32.0).powi(2);
        let (my_x, my_y, my_z) = server.player_manager.get(&_uuid)
            .map(|p| (p.position.x, p.position.y, p.position.z))
            .unwrap_or((0.0, 64.0, 0.0));
        while let Ok(ev) = entity_rx.try_recv() {
            if ev.uuid == _uuid {
                // Skip own entity events
                if matches!(ev.kind, mc_player::player::EntityEventKind::Despawn) {
                    known_entities.remove(&ev.entity_id);
                }
                continue;
            }
            // Per-entity-type distance check for spawn/move events (A6: configurable cap)
            // Hostile=48, Passive=32, Item=16, Player=view*16+32 (vanilla values)
            // All capped by entity_broadcast_radius from server config.
            let radius_cap = server.entity_broadcast_radius.powi(2).min(max_range);
            let entity_tracking_range = match &ev.kind {
                mc_player::player::EntityEventKind::Spawn(_, _, _, _, _) |
                mc_player::player::EntityEventKind::Move(_, _, _, _, _) => max_range,
                mc_player::player::EntityEventKind::MobSpawn(mob_type, _, _, _) => {
                    let dist = match mob_type {
                        // Hostile mobs — track at 48 blocks
                        25|33|34|35|36|37|38|43|45|46|47|48|49|50|51|52|
                        53|54|55|56|57|58|59|60|61|62|63|71|72|105|106 => 48.0_f64.min(radius_cap.sqrt()),
                        // Items/XP orbs — track at 16 blocks
                        0 => 16.0_f64.min(radius_cap.sqrt()),
                        // Passive mobs — track at 32 blocks
                        _ => 32.0_f64.min(radius_cap.sqrt()),
                    };
                    dist * dist
                }
                _ => max_range,
            };
            let too_far = match &ev.kind {
                mc_player::player::EntityEventKind::Spawn(x, y, z, _, _) |
                mc_player::player::EntityEventKind::Move(x, y, z, _, _) |
                mc_player::player::EntityEventKind::MobSpawn(_, x, y, z) => {
                    let dx = *x - my_x; let dy = *y - my_y; let dz = *z - my_z;
                    dx*dx + dy*dy + dz*dz > entity_tracking_range
                }
                _ => false,
            };
            if too_far {
                // Clean up if we knew about this entity (moved out of range)
                if known_entities.remove(&ev.entity_id) {
                    let rm = RemoveEntities { entity_ids: vec![ev.entity_id] };
                    let _ = send_packet(io, &rm).await;
                }
                continue;
            }
            match &ev.kind {
                mc_player::player::EntityEventKind::Spawn(x, y, z, yaw, pitch) => {
                    if !known_entities.contains(&ev.entity_id) {
                        known_entities.insert(ev.entity_id);
                        // IMPORTANT: PlayerInfoUpdate FIRST so client can resolve UUID→name
                        let info = PlayerInfoUpdate {
                            actions: 0x01 | 0x02 | 0x04 | 0x08,
                            entries: vec![PlayerInfoEntry {
                                uuid: ev.uuid,
                                username: ev.username.clone(),
                                gamemode: 0,
                                ping: player_ping_ms as i32,
                                listed: true,
                            }],
                        };
                        let _ = send_packet(io, &info).await;
                        // Then SpawnPlayer to create the entity (client now knows the UUID)
                        let spawn = mc_protocol::packets::play::SpawnPlayer {
                            entity_id: ev.entity_id,
                            player_uuid: ev.uuid,
                            x: *x, y: *y, z: *z,
                            yaw: *yaw, pitch: *pitch,
                        };
                        let _ = send_packet(io, &spawn).await;
                        // Send entity metadata for skin display
                        let meta = mc_protocol::packets::play::SetEntityMetadata::player_defaults(ev.entity_id);
                        let _ = send_packet(io, &meta).await;
                        debug!("Spawned player entity {} ({}) for {}", ev.entity_id, ev.username, username);
                    }
                }
                mc_player::player::EntityEventKind::Move(x, y, z, yaw, pitch) => {
                    if known_entities.contains(&ev.entity_id) {
                        let tp = TeleportEntity {
                            entity_id: ev.entity_id,
                            x: *x, y: *y, z: *z,
                            yaw: *yaw, pitch: *pitch,
                            on_ground: false,
                        };
                        let _ = send_packet(io, &tp).await;
                    }
                }
                mc_player::player::EntityEventKind::Despawn => {
                    if known_entities.remove(&ev.entity_id) {
                        let rm = RemoveEntities { entity_ids: vec![ev.entity_id] };
                        let _ = send_packet(io, &rm).await;
                        // Remove from tab list
                        let info = PlayerInfoUpdate {
                            actions: 0x04,
                            entries: vec![PlayerInfoEntry {
                                uuid: ev.uuid,
                                username: String::new(),
                                gamemode: 0,
                                ping: player_ping_ms as i32,
                                listed: false,
                            }],
                        };
                        let _ = send_packet(io, &info).await;
                        // Also remove from tab list completely
                        let remove = PlayerInfoRemove { uuids: vec![ev.uuid] };
                        let _ = send_packet(io, &remove).await;
                        debug!("Despawned entity {} ({}) for {}", ev.entity_id, ev.uuid, username);
                    }
                }
                mc_player::player::EntityEventKind::MobSpawn(mob_type, x, y, z) => {
                    if !known_entities.contains(&ev.entity_id) {
                        known_entities.insert(ev.entity_id);
                        let spawn = SpawnEntity {
                            entity_id: ev.entity_id,
                            entity_uuid: ev.uuid,
                            entity_type: *mob_type,
                            x: *x, y: *y, z: *z,
                            pitch: 0, yaw: 0, head_yaw: 0,
                            data: 0,
                            vel_x: 0, vel_y: 0, vel_z: 0,
                        };
                        let _ = send_packet(io, &spawn).await;
                        debug!("Synced mob spawn type={} eid={} to {}", mob_type, ev.entity_id, username);
                    }
                }
                mc_player::player::EntityEventKind::MobDespawn => {
                    if known_entities.remove(&ev.entity_id) {
                        let rm = RemoveEntities { entity_ids: vec![ev.entity_id] };
                        let _ = send_packet(io, &rm).await;
                        debug!("Synced mob despawn eid={} to {}", ev.entity_id, username);
                    }
                }
            }
        }

        // Check for chat broadcasts (chat, join/leave, kick)
        while let Ok(bc) = chat_rx.try_recv() {
            match &bc.msg_type {
                mc_player::player::BroadcastType::Kick(target_uuid, reason) => {
                    if *target_uuid == _uuid {
                        // This player was kicked — send disconnect and exit
                        let dc = PlayDisconnect {
                            reason: format!("{{\"text\":\"{}\",\"color\":\"red\"}}", reason),
                        };
                        let _ = send_packet(io, &dc).await;
                        info!("Kicked player '{}': {}", username, reason);
                        return;
                    }
                    // Other players see the kick message
                    let content = format!("{{\"text\":\"{} was kicked: {}\",\"color\":\"yellow\"}}", bc.sender_name, reason);
                    let _ = send_packet(io, &SystemChatMessage { content, overlay: false }).await;
                }
                mc_player::player::BroadcastType::Join => {
                    if bc.sender_name != username {
                        let content = format!("{{\"text\":\"{}\",\"color\":\"yellow\"}}", bc.message);
                        let _ = send_packet(io, &SystemChatMessage { content, overlay: false }).await;
                    }
                }
                mc_player::player::BroadcastType::Leave => {
                    if bc.sender_name != username {
                        let content = format!("{{\"text\":\"{}\",\"color\":\"yellow\"}}", bc.message);
                        let _ = send_packet(io, &SystemChatMessage { content, overlay: false }).await;
                    }
                }
                mc_player::player::BroadcastType::Private(target_uuid, _) => {
                    if *target_uuid == _uuid {
                        let content = format!("{{\"text\":\"[{} → you] {}\",\"color\":\"light_purple\"}}", bc.sender_name, bc.message);
                        let _ = send_packet(io, &SystemChatMessage { content, overlay: false }).await;
                    }
                }
                _ => {
                    // Chat / System messages
                    if bc.sender_name != username || bc.msg_type == mc_player::player::BroadcastType::System {
                        let content = match bc.msg_type {
                            mc_player::player::BroadcastType::System => {
                                format!("{{\"text\":\"{}\",\"color\":\"gray\"}}", bc.message)
                            }
                            _ => {
                                format!("{{\"text\":\"<{}> {}\"}}", bc.sender_name, bc.message)
                            }
                        };
                        let _ = send_packet(io, &SystemChatMessage { content, overlay: false }).await;
                    }
                }
            }
        }

        // Sync mob positions to client
        while let Ok(ev) = mob_pos_rx.try_recv() {
            if known_entities.contains(&ev.entity_id) {
                let tp = TeleportEntity {
                    entity_id: ev.entity_id,
                    x: ev.x, y: ev.y, z: ev.z,
                    yaw: 0.0, pitch: 0.0,
                    on_ground: true,
                };
                let _ = send_packet(io, &tp).await;
            }
        }

        // Player state events — delegated to handler_sync module
        crate::handler_sync::handle_player_state_events(&mut player_state_rx, io, _uuid, username, server).await;

        // Send keep-alive if interval elapsed
        if last_keep_alive_sent.elapsed() >= keep_alive_interval {
            keep_alive_id = keep_alive_id.wrapping_add(1);
            let _ = send_packet(io, &KeepAlive { id: keep_alive_id }).await;
            keep_alive_sent_instant = Instant::now();
            // Sync world time (extract values, drop guard before .await)
            let (world_age, time_of_day) = {
                let ws = server.world_state.read();
                (ws.time as i64, ws.daytime as i64)
            };
            let _ = send_packet(io, &UpdateTime { world_age, time_of_day }).await;
            // Sync container progress bars (furnace/brewing)
            if tick_count.is_multiple_of(20) {
                if let Some(win_id) = server.container_manager.player_window(&_uuid) {
                    let container = server.container_manager.get(win_id);
                    if let Some(c) = container {
                        // Furnace progress (3-slot container)
                        if c.slots.len() == 3 {
                            let (progress, burning) = {
                                let fm = server.furnace_manager.read();
                                (fm.progress(c.pos), fm.is_burning(c.pos))
                            }; // lock released before await
                            let _ = send_packet(io, &ContainerSetData { window_id: win_id, property: 0, value: if burning { 200i16 } else { 0 } }).await;
                            let _ = send_packet(io, &ContainerSetData { window_id: win_id, property: 2, value: (progress * 200.0) as i16 }).await;
                            let _ = send_packet(io, &ContainerSetData { window_id: win_id, property: 3, value: 200 }).await;
                        }
                        // Brewing progress (5-slot container)
                        if c.slots.len() == 5 {
                            let (brew_ticks, fuel) = {
                                let bm = server.brewing_manager.read();
                                (bm.get_brew_ticks(c.pos), bm.get_fuel(c.pos))
                            }; // lock released before await
                            let _ = send_packet(io, &ContainerSetData { window_id: win_id, property: 0, value: brew_ticks as i16 }).await;
                            let _ = send_packet(io, &ContainerSetData { window_id: win_id, property: 1, value: fuel as i16 }).await;
                        }
                    }
                }
                // ── Immediate block update broadcast via UpdateSectionBlocks (0x47) ──
                // Drains per-section dirty blocks within view distance and sends
                // lightweight ~200B section updates instead of ~50KB ChunkData.
                {
                    let my_x = server.player_manager.get(&_uuid)
                        .map(|p| p.position.x).unwrap_or(0.0);
                    let my_z = server.player_manager.get(&_uuid)
                        .map(|p| p.position.z).unwrap_or(0.0);
                    let max_dist = server.view_distance as i32 + 2;
                    let my_cx = (my_x as i32).div_euclid(16);
                    let my_cz = (my_z as i32).div_euclid(16);

                    // Send per-section block updates (immediate, ~200B each)
                    let nearby_sections = server.dirty_blocks.drain_nearby(my_cx, my_cz, max_dist);
                    for (cx, sy, cz, blocks) in &nearby_sections {
                        let update = mc_protocol::packets::play::UpdateSectionBlocks {
                            chunk_x: *cx,
                            chunk_z: *cz,
                            section_y: *sy,
                            blocks: blocks.clone(),
                        };
                        let _ = send_packet(io, &update).await;
                    }

                    // Fallback: full ChunkData rebroadcast for chunk-level operations
                    // (TNT, structure changes) that marked via the legacy dirty_chunks_broadcast
                    let nearby_chunks: Vec<mc_core::position::ChunkPos> = {
                        server.dirty_chunks_broadcast.read().iter()
                            .filter(|cp| (cp.x - my_cx).abs() <= max_dist && (cp.z - my_cz).abs() <= max_dist)
                            .copied()
                            .collect()
                    };
                    // Remove sent entries from the broadcast set
                    if !nearby_chunks.is_empty() {
                        let mut set = server.dirty_chunks_broadcast.write();
                        for cp in &nearby_chunks {
                            set.remove(cp);
                        }
                    }
                    for cp in &nearby_chunks {
                        if let Some(mut chunk) = server.chunk_store.get_mut(cp) {
                            let _ = send_chunk_data_cached(io, &mut chunk).await;
                        }
                    }
                }
            }
            last_keep_alive_sent = Instant::now();
        }

        // Check keep-alive timeout
        if last_keep_alive_response.elapsed() > keep_alive_timeout {
            info!("Player '{}' timed out (no keep-alive response for {}s)", username, keep_alive_timeout.as_secs());
            return;
        }

        // Check server shutdown signal
        if shutdown_rx.try_recv().is_ok() {
            info!("Server shutting down — disconnecting player '{}'", username);
            return;
        }

        // Read next packet with a short timeout to allow keep-alive + chat
        let frame = match tokio::time::timeout(
            tokio::time::Duration::from_millis(250),
            io.read_frame(),
        ).await {
            Ok(Ok(f)) => f,
            Ok(Err(e)) => {
                debug!("Play read error for {}: {}", username, e);
                return;
            }
            Err(_) => {
                // Timeout — loop back to check keep-alive and chat
                continue;
            }
        };

        // Packet size limit: reject frames larger than 2MB (anti-DoS)
        if frame.len() > 2_097_152 {
            warn!("Oversized packet ({} bytes) from {}, disconnecting", frame.len(), username);
            return;
        }

        let (packet_id, _payload) = match io.codec().parse_packet_id_and_payload(&frame) {
            Ok(v) => v,
            Err(e) => {
                error!("Play decode error: {}", e);
                continue;
            }
        };

        // Rate limit: max 20 packets/second per connection (B2 fix)
        if let Some(addr) = peer_socket
            && !rate_limiter::allow_packet(addr) {
                continue;
            }

        match packet_id {
            // Confirm Teleportation (0x00) — client acknowledges teleport with ID
            0x00 => {
                if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(&frame)
                    && payload.len() >= 4 {
                        let teleport_id = i32::from_be_bytes(payload[..4].try_into().unwrap_or([0;4]));
                        let pending = server.player_manager.get(&_uuid)
                            .and_then(|p| p.pending_teleport_id);
                        if pending == Some(teleport_id) {
                            server.player_manager.clear_pending_teleport(&_uuid);
                            debug!("Teleport {} confirmed by {}", teleport_id, username);
                        } else {
                            debug!("Teleport ID mismatch: got {}, expected {:?} from {}", teleport_id, pending, username);
                        }
                    }
            }
            // Message Acknowledgment (0x01) — client confirms chat message receipt (1.21+ requirement)
            0x01 => {
                if let Ok(msg) = io.codec().decode::<mc_protocol::packets::play::MessageAcknowledgment>(&frame) {
                    server.player_manager.set_acknowledged_count(&_uuid, msg.message_count);
                }
            }
            // Chat Command (0x08) — slash command via alternate channel
            0x05 => {
                match io.codec().decode::<mc_protocol::packets::play::ChatCommand>(&frame) {
                    Ok(cmd) => {
                        // Validate: reject commands longer than 256 chars (anti-flood)
                        if cmd.command.len() > 256 {
                            debug!("ChatCommand from {} rejected: too long ({} chars)", username, cmd.command.len());
                            continue;
                        }
                        debug!("ChatCommand from {}: {}", username, cmd.command);
                        let result = {
                            let disp = server.command_dispatcher.lock();
                            disp.dispatch_input(
                                &cmd.command,
                                mc_command::dispatcher::CommandSource::player(username, _uuid),
                                &server.player_manager,
                                &server.shutdown_tx,
                                &server.world_state,
                                &server.motd,
                                server.max_players,
                                Some(&server.chunk_store), Some(&server.save_trigger),
                            )
                        };
                        match result {
                            Ok(response) => {
                                let chat = SystemChatMessage {
                                    content: format!("{{\"text\":\"{}\",\"color\":\"gray\"}}", response),
                                    overlay: false,
                                };
                                let _ = send_packet(io, &chat).await;
                            }
                            Err(e) => {
                                let chat = SystemChatMessage {
                                    content: format!("{{\"text\":\"{}\",\"color\":\"red\"}}", e),
                                    overlay: false,
                                };
                                let _ = send_packet(io, &chat).await;
                            }
                        }
                    }
                    Err(e) => debug!("ChatCommand decode error: {}", e),
                }
            }
            // Client Command (0x04) — respawn / stats
            0x0A => {
                match io.codec().decode::<mc_protocol::packets::play::ClientCommand>(&frame) {
                    Ok(cmd) => {
                        if cmd.action == 0 {
                            // Perform respawn — send Respawn + PlayerPosition + SetHealth
                            info!("Player '{}' respawning", username);
                            let pdata = server.player_manager.get(&_uuid);
                            let gm = pdata.as_ref()
                                .map(|p| p.gamemode)
                                .unwrap_or(mc_core::types::GameMode::Survival);
                            let dim = pdata.as_ref()
                                .map(|p| p.dimension.clone())
                                .unwrap_or_else(|| "minecraft:overworld".into());
                            let spawn_pos = pdata.as_ref()
                                .and_then(|p| p.spawn_position);
                            let (sx, sy, sz, syaw) = spawn_pos.unwrap_or((0.0, 64.0, 0.0, 0.0));

                            let respawn = mc_protocol::packets::play::Respawn {
                                dimension_type: dim.clone(),
                                dimension_name: dim,
                                hashed_seed: server.world_seed as i64,
                                gamemode: gm.id(),
                                previous_gamemode: -1,
                                is_debug: false,
                                is_flat: server.generator_name == "flat",
                                death_location: None,
                                portal_cooldown: 0,
                                data_kept: 0,
                            };
                            let _ = send_packet(io, &respawn).await;
                            let pos = mc_protocol::packets::play::PlayerPosition {
                                x: sx, y: sy, z: sz,
                                yaw: syaw, pitch: 0.0,
                                flags: 0, teleport_id: 42,
                            };
                            let _ = send_packet(io, &pos).await;
                            server.player_manager.set_health(&_uuid, 20.0).ok();
                            // Advancement: LocationChanged (B5)
                            if let Some(ref dim_name) = pdata.as_ref().map(|p| p.dimension.clone()) {
                                fire_advancement(server, io, &_uuid,
                                    &mc_player::advancement::Criterion::LocationChanged { dimension: dim_name.clone() }).await;
                            }
                        }
                    }
                    Err(e) => debug!("ClientCommand decode error: {}", e),
                }
            }
            // Client Information
            0x0C => {
                match io.codec().decode::<ClientInformation>(&frame) {
                    Ok(info) => {
                        server.player_manager.set_client_view_distance(&_uuid, info.view_distance, &info.locale);
                        debug!("Client info: locale={}, view_distance={}", info.locale, info.view_distance);
                    }
                    Err(e) => {
                        debug!("Client info decode error: {}", e);
                    }
                }
            }
            // Set Held Item (C2S 0x2A) — player changed hotbar slot
            0x33 => {
                if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(&frame)
                    && !payload.is_empty() {
                        let slot = (payload[0] as i16) as u8;
                        if slot < 9 {
                            let _ = server.player_manager.set_selected_slot(&_uuid, slot);
                            // Echo back as S2C confirmation
                            let _ = send_packet(io, &SetHeldItemS2C { slot }).await;
                        }
                    }
            }
            // Keep Alive response
            0x1A => {
                last_keep_alive_response = Instant::now();
                player_ping_ms = keep_alive_sent_instant.elapsed().as_millis() as u32;
                debug!("Keep alive from {} (ping: {}ms)", username, player_ping_ms);
            }
            // Chat Message (C2S)
            0x07 => {
                match io.codec().parse_packet_id_and_payload(&frame) {
                    Ok((_, payload)) => {
                        if let Ok((msg, _)) = mc_protocol::codec::read_string(&payload) {
                            debug!("Chat from {}: {}", username, msg);
                            if msg.starts_with('/') {
                                // Route as command
                                let result = {
                                    let disp = server.command_dispatcher.lock();
                                    disp.dispatch_input(
                                        msg,
                                        mc_command::dispatcher::CommandSource::player(username, _uuid),
                                        &server.player_manager,
                                        &server.shutdown_tx,
                                        &server.world_state,
                                        &server.motd,
                                        server.max_players,
                                        Some(&server.chunk_store), Some(&server.save_trigger),
                                    )
                                };
                                match result {
                                    Ok(response) => {
                                        let chat = SystemChatMessage {
                                            content: format!("{{\"text\":\"{}\",\"color\":\"gray\"}}", response),
                                            overlay: false,
                                        };
                                        let _ = send_packet(io, &chat).await;
                                    }
                                    Err(e) => {
                                        let chat = SystemChatMessage {
                                            content: format!("{{\"text\":\"{}\",\"color\":\"red\"}}", e),
                                            overlay: false,
                                        };
                                        let _ = send_packet(io, &chat).await;
                                    }
                                }
                            } else {
                                // Regular chat — broadcast to all players
                                server.player_manager.broadcast_chat(username, msg, false);
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Chat decode error: {}", e);
                    }
                }
            }
            // Player Position (0x20) — x, y, z doubles + flags
            0x1C => {
                if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(&frame)
                    && payload.len() >= 24 {
                        let x = f64::from_be_bytes(payload[0..8].try_into().unwrap_or([0;8]));
                        let y = f64::from_be_bytes(payload[8..16].try_into().unwrap_or([0;8]));
                        let z = f64::from_be_bytes(payload[16..24].try_into().unwrap_or([0;8]));
                        let (old_x, old_y, old_z, old_yaw, old_pitch) = server.player_manager.get(&_uuid)
                            .map(|p| (p.position.x, p.position.y, p.position.z, p.position.yaw, p.position.pitch))
                            .unwrap_or((x, y, z, 0.0, 0.0));
                        // Apply speed_multiplier from effects (Speed/Slowness) for anti-cheat bounds
                        let speed_mul = server.player_manager.get(&_uuid)
                            .map(|p| p.speed_multiplier).unwrap_or(1.0);
                        let max_delta = 10.0 * speed_mul as f64;
                        let h_dist = ((x - old_x).powi(2) + (z - old_z).powi(2)).sqrt();
                        let v_dist = (y - old_y).abs();
                        // E2: Anti-cheat validation with violation buffer
                        let mut accepted = true;
                        if h_dist <= max_delta && v_dist <= max_delta {
                            server.player_manager.ac_update_valid(&_uuid, x, y, z, tick_count);
                            let _ = server.player_manager.update_position_full(&_uuid, x, y, z, old_yaw, old_pitch);
                        } else if h_dist > max_delta && h_dist > 3.0 {
                            let (violations, rubberband) = server.player_manager.ac_add_violation(&_uuid, tick_count);
                            if rubberband
                                && let Some((lx, ly, lz)) = server.player_manager.ac_valid_position(&_uuid) {
                                    debug!("Anti-cheat: rubberbanding {} ({} violations, moved {:.1} blocks)", username, violations, h_dist);
                                    server.player_manager.ac_reset_violations(&_uuid);
                                    server.player_manager.update_position_full(&_uuid, lx, ly, lz, old_yaw, old_pitch).ok();
                                    accepted = false;
                                }
                        }
                        if accepted
                            && let Some(eid) = server.player_manager.get_entity_id(&_uuid) {
                                server.player_manager.broadcast_entity_move(eid, _uuid, x, y, z, old_yaw, old_pitch);
                            }
                        // Elytra glide: auto-start when falling with elytra equipped
                        if !server.player_manager.is_flying(&_uuid) {
                            let has_elytra = server.player_manager.get(&_uuid)
                                .map(|p| {
                                    p.inventory.items.get(38) // chestplate slot
                                        .and_then(|o| o.as_ref())
                                        .map(|s| s.item.id == 843)
                                        .unwrap_or(false)
                                }).unwrap_or(false);
                            // Auto-start glide if falling fast enough and has elytra
                            if has_elytra && old_y - y > 0.5 && y < old_y {
                                server.player_manager.set_flying(&_uuid, true);
                                if let Some(eid) = server.player_manager.get_entity_id(&_uuid) {
                                    // Notify client with EntityEvent START_FLYING
                                    let _ = send_packet(io, &mc_protocol::packets::play::EntityEvent {
                                        entity_id: eid, status: 2,
                                    }).await;
                                }
                            }
                        }
                        // Stop glide on landing
                        if y >= old_y && server.player_manager.is_flying(&_uuid)
                            && let Some(p) = server.player_manager.get(&_uuid) {
                                // Check if on ground (within 0.3 blocks of block below)
                                let bx = (p.position.x).floor() as i32;
                                let by = (p.position.y - 0.3).floor() as i32;
                                let bz = (p.position.z).floor() as i32;
                                let cp = mc_core::position::ChunkPos::new(bx >> 4, bz >> 4);
                                let on_ground = server.chunk_store.get(&cp)
                                    .map(|ch| ch.get_block((bx & 0xF) as usize, by, (bz & 0xF) as usize).id != 0)
                                    .unwrap_or(false);
                                if on_ground {
                                    server.player_manager.set_flying(&_uuid, false);
                                } else {
                                    // Apply elytra air drag (no gravity, slow deceleration)
                                    let drag = 0.99;
                                    server.player_manager.set_flying_velocity(&_uuid,
                                        (p.position.x - x) * (1.0 - drag), 0.0, (p.position.z - z) * (1.0 - drag));
                                }
                            }
                        // Fall damage tracking (skip when gliding)
                        if !server.player_manager.is_flying(&_uuid) {
                            if y < old_y {
                                server.player_manager.add_fall_distance(&_uuid, (old_y - y) as f32);
                            } else {
                                let fd = server.player_manager.take_fall_distance(&_uuid);
                                if fd > 3.0
                                    && let Some(p) = server.player_manager.get(&_uuid)
                                        && (p.gamemode == mc_core::types::GameMode::Survival
                                            || p.gamemode == mc_core::types::GameMode::Adventure) {
                                            let damage = fd - 3.0;
                                            let _ = server.player_manager.apply_damage(&_uuid, damage, tick_count);
                                        }
                            }
                        }
                        let new_chunk = ChunkPos::new(
                            (x.floor() as i32).div_euclid(16),
                            (z.floor() as i32).div_euclid(16),
                        );
                        // Nether portal check: standing in portal block (ID 90)?
                        {
                            let bx = x.floor() as i32;
                            let by = y.floor() as i32;
                            let bz = z.floor() as i32;
                            let cp = mc_core::position::ChunkPos::new(bx >> 4, bz >> 4);
                            if let Some(chunk) = server.chunk_store.get(&cp) {
                                let block = chunk.get_block((bx & 0xF) as usize, by, (bz & 0xF) as usize);
                                if block.id == 90 && tick_count.is_multiple_of(80) {
                                    let (py, pp) = server.player_manager.get(&_uuid)
                                        .map(|p| (p.position.yaw, p.position.pitch))
                                        .unwrap_or((0.0, 0.0));
                                    let current_dim = server.player_manager.get(&_uuid)
                                        .map(|p| p.dimension.clone())
                                        .unwrap_or_else(|| "minecraft:overworld".into());

                                    let (target_dim, tx, tz, target_y): (&str, f64, f64, f64) =
                                        if current_dim == "minecraft:the_nether" {
                                            // Nether → Overworld: coords × 8
                                            ("minecraft:overworld", x * 8.0, z * 8.0, 128.0)
                                        } else {
                                            // Overworld → Nether: coords ÷ 8
                                            ("minecraft:the_nether", x / 8.0, z / 8.0, 64.0)
                                        };

                                    // Update dimension on player state
                                    let _ = server.player_manager.set_dimension(&_uuid, target_dim);

                                    // Send Respawn packet for dimension switch
                                    let respawn = mc_protocol::packets::play::Respawn {
                                        dimension_type: target_dim.into(),
                                        dimension_name: target_dim.into(),
                                        hashed_seed: server.world_seed as i64,
                                        gamemode: server.player_manager.get(&_uuid)
                                            .map(|p| p.gamemode.id()).unwrap_or(0),
                                        previous_gamemode: -1,
                                        is_debug: false,
                                        is_flat: server.generator_name == "flat",
                                        death_location: None,
                                        portal_cooldown: 80,
                                        data_kept: 0,
                                    };
                                    let _ = send_packet(io, &respawn).await;

                                    // Send position in new dimension
                                    let tp = mc_protocol::packets::play::PlayerPosition {
                                        x: tx, y: target_y, z: tz,
                                        yaw: py, pitch: pp,
                                        flags: 0, teleport_id: fastrand::i32(1..i32::MAX),
                                    };
                                    let _ = send_packet(io, &tp).await;
                                    let _ = server.player_manager.update_position_full(&_uuid, tx, target_y, tz, py, pp);
                                }
                            }
                        }
                        if new_chunk != player_chunk {
                            stream_new_chunks(io, server, &mut player_chunk, &mut loaded_chunks, new_chunk, view_radius).await;
                        }
                    }
            }
            // Player Position And Rotation (0x22) — x, y, z, yaw, pitch + flags
            0x1D => {
                if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(&frame)
                    && payload.len() >= 33 {
                        let x = f64::from_be_bytes(payload[0..8].try_into().unwrap_or([0;8]));
                        let old_y = server.player_manager.get(&_uuid).map(|p| p.position.y).unwrap_or(64.0);
                        let y = f64::from_be_bytes(payload[8..16].try_into().unwrap_or([0;8]));
                        let z = f64::from_be_bytes(payload[16..24].try_into().unwrap_or([0;8]));
                        let yaw = f32::from_be_bytes(payload[24..28].try_into().unwrap_or([0;4]));
                        let pitch = f32::from_be_bytes(payload[28..32].try_into().unwrap_or([0;4]));
                        // 26.2: Sculk Sensor — entity step vibration (freq 1)
                        let step_x = x.floor() as i32;
                        let step_y = y.floor() as i32;
                        let step_z = z.floor() as i32;
                        let old_step = server.player_manager.get(&_uuid)
                            .map(|p| (p.position.x.floor() as i32, p.position.y.floor() as i32, p.position.z.floor() as i32));
                        if old_step.is_none_or(|(ox, oy, oz)| ox != step_x || oy != step_y || oz != step_z) {
                            mc_world::redstone::register_vibration(step_x, step_y, step_z, 1);
                        }
                        // Track old/new chunk for spatial index
                        let old_cx = server.player_manager.get(&_uuid)
                            .map(|p| (p.position.x.floor() as i32).div_euclid(16))
                            .unwrap_or(0);
                        let old_cz = server.player_manager.get(&_uuid)
                            .map(|p| (p.position.z.floor() as i32).div_euclid(16))
                            .unwrap_or(0);
                        let _ = server.player_manager.update_position_full(&_uuid, x, y, z, yaw, pitch);
                        let new_cx = (x.floor() as i32).div_euclid(16);
                        let new_cz = (z.floor() as i32).div_euclid(16);
                        if old_cx != new_cx || old_cz != new_cz {
                            server.player_manager.update_player_chunk(&_uuid, old_cx, old_cz, new_cx, new_cz);
                        }
                        if let Some(eid) = server.player_manager.get_entity_id(&_uuid) {
                            server.player_manager.broadcast_entity_move(eid, _uuid, x, y, z, yaw, pitch);
                        }
                        // ── Boot enchantment effects ──
                        let (fw_level, ss_level, sws_level) = server.player_manager.get_boot_enchant_levels(&_uuid);
                        // Frost Walker: freeze water blocks around player's feet
                        if fw_level > 0 {
                            let radius = (fw_level as i32 + 1).min(4);
                            for dx in -radius..=radius {
                                for dz in -radius..=radius {
                                    let d = ((dx*dx + dz*dz) as f64).sqrt();
                                    if d <= radius as f64 {
                                        let bx = (x.floor() as i32) + dx;
                                        let bz = (z.floor() as i32) + dz;
                                        let by = (y.floor() as i32) - 1;
                                        let cp = mc_core::position::ChunkPos::new(bx >> 4, bz >> 4);
                                        if let Some(mut ch) = server.chunk_store.get_mut(&cp)
                                            && (-64..=319).contains(&by) {
                                                let block = ch.get_block((bx & 0xF) as usize, by, (bz & 0xF) as usize);
                                                if block.id == 267 { // water
                                                    // Replace water with frosted_ice (ID 1055 — frosted ice)
                                                    ch.set_block((bx & 0xF) as usize, by, (bz & 0xF) as usize,
                                                        mc_core::block::BlockState::new(1055));
                                                    server.dirty_blocks.mark_block(bx, by, bz, 1055);
                                                } else if block.id == 1055 && d >= radius as f64 - 0.5 {
                                                    // Keep frosted ice near center, let edges melt
                                                }
                                            }
                                    }
                                }
                            }
                        }
                        // Soul Speed: boost on soul sand/soil
                        if ss_level > 0 {
                            let bx = x.floor() as i32;
                            let bz = z.floor() as i32;
                            let by = (y.floor() as i32) - 1;
                            let cp = mc_core::position::ChunkPos::new(bx >> 4, bz >> 4);
                            let on_soul = server.chunk_store.get(&cp)
                                .map(|ch| {
                                    let bid = ch.get_block((bx & 0xF) as usize, by, (bz & 0xF) as usize).id;
                                    bid == 961 || bid == 962 // soul_sand, soul_soil
                                }).unwrap_or(false);
                            if on_soul {
                                server.player_manager.set_speed_multiplier(&_uuid,
                                    1.0 + 0.155 * ss_level as f32);
                                // Durability damage to boots (probability-based)
                                if fastrand::u32(..).is_multiple_of(4) {
                                    let _ = server.player_manager.damage_boots(&_uuid, 1);
                                }
                            }
                        }
                        // Swift Sneak: faster movement while sneaking
                        if sws_level > 0 {
                            let is_sneaking = server.player_manager.get(&_uuid)
                                .map(|p| p.is_sneaking).unwrap_or(false);
                            if is_sneaking {
                                server.player_manager.set_speed_multiplier(&_uuid,
                                    0.3 + 0.15 * sws_level as f32);
                            }
                        }
                        // Item pickup: check for nearby dropped items
                        {
                            let mut to_pickup = Vec::new();
                            {
                                let items = server.dropped_items.read();
                                for (eid, (item_id, ix, iy, iz)) in items.iter() {
                                    let dx = x - ix; let dy = y - iy; let dz = z - iz;
                                    if dx*dx + dy*dy + dz*dz < 2.25 { // 1.5 block radius
                                        to_pickup.push((*eid, *item_id));
                                    }
                                }
                            }
                            for (eid, item_id) in &to_pickup {
                                let stack = mc_player::inventory::ItemStack::new(mc_core::block::BlockState::new(*item_id), 1);
                                let _ = server.player_manager.add_item(&_uuid, stack);
                                server.dropped_items.write().remove(eid);
                                let remove = RemoveEntities { entity_ids: vec![*eid] };
                                let _ = send_packet(io, &remove).await;
                            }
                        }
                        // Fall damage tracking
                        if y < old_y {
                            server.player_manager.add_fall_distance(&_uuid, (old_y - y) as f32);
                        } else {
                            let fd = server.player_manager.take_fall_distance(&_uuid);
                            if fd > 3.0 {
                                let p = server.player_manager.get(&_uuid);
                                if let Some(ref p) = p
                                    && (p.gamemode == mc_core::types::GameMode::Survival
                                        || p.gamemode == mc_core::types::GameMode::Adventure) {
                                        let damage = fd - 3.0;
                                        let _ = server.player_manager.apply_damage(&_uuid, damage, tick_count);
                                        if damage > 0.0 {
                                            debug!("{} took {:.0} fall damage (fell {:.1} blocks)", username, damage, fd);
                                        }
                                    }
                            }
                        }
                        let new_chunk = ChunkPos::new(
                            (x.floor() as i32).div_euclid(16),
                            (z.floor() as i32).div_euclid(16),
                        );
                        if new_chunk != player_chunk {
                            stream_new_chunks(io, server, &mut player_chunk, &mut loaded_chunks, new_chunk, view_radius).await;
                        }
                    }
            }
            // Player Rotation (0x1E) — yaw + pitch
            0x1E => {
                if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(&frame)
                    && payload.len() >= 9 {
                        let yaw = f32::from_be_bytes(payload[0..4].try_into().unwrap_or([0;4]));
                        let pitch = f32::from_be_bytes(payload[4..8].try_into().unwrap_or([0;4]));
                        if let Some(p) = server.player_manager.get(&_uuid) {
                            let _ = server.player_manager.update_position_full(
                                &_uuid, p.position.x, p.position.y, p.position.z, yaw, pitch,
                            );
                            if let Some(eid) = server.player_manager.get_entity_id(&_uuid) {
                                server.player_manager.broadcast_entity_move(eid, _uuid, p.position.x, p.position.y, p.position.z, yaw, pitch);
                            }
                        }
                    }
            }
            // Player Action (0x27) — includes block break
            0x27 => {
                crate::c2s_handlers::handle_player_action(io, server, &_uuid, &frame).await;
            }
            // Use Item On (0x3E) — block placement or container open
            0x3E => {
                if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(&frame) {
                    let mut off = 0;
                    while off < payload.len() && payload[off] & 0x80 != 0 { off += 1; }
                    off += 1;
                    if off + 8 <= payload.len() {
                        let raw = i64::from_be_bytes(payload[off..off+8].try_into().unwrap_or([0;8]));
                        let x = (raw >> 38) as i32;
                        let y = ((raw << 52) >> 52) as i32;
                        let z = (raw >> 12) as i32 & 0x3FFFFFF;
                        if !(-64..=319).contains(&y) { continue; }

                        // Check if target block is a container → open GUI
                        let cp = mc_core::position::ChunkPos::new(x >> 4, z >> 4);
                        if let Some(chunk) = server.chunk_store.get(&cp) {
                            let target_block = chunk.get_block((x & 0xF) as usize, y, (z & 0xF) as usize);
                            if let Some(slot_count) = mc_player::container::container_slot_count(target_block.id) {
                                let window_id = server.container_manager.open(&_uuid, (x, y, z), slot_count);
                                let window_type = mc_player::container::container_window_type(target_block.id);
                                let open_pkt = OpenScreen {
                                    window_id: window_id as i32,
                                    window_type,
                                    title: "{\"text\":\"Container\"}".into(),
                                };
                                let _ = send_packet(io, &open_pkt).await;
                                // Send container contents with proper SlotData (count + NBT)
                                let slots: Vec<Option<mc_protocol::packets::play::SlotData>> = server.container_manager
                                    .all_slots(window_id)
                                    .into_iter()
                                    .map(|opt| opt.map(|s| mc_protocol::packets::play::SlotData {
                                        item_id: s.item.id as i32,
                                        count: s.count,
                                        nbt: s.nbt.clone(),
                                    }))
                                    .collect();
                                let carried = server.player_manager.get_cursor_item(&_uuid)
                                    .map(|s| mc_protocol::packets::play::SlotData {
                                        item_id: s.item.id as i32,
                                        count: s.count,
                                        nbt: s.nbt.clone(),
                                    });
                                let state = server.container_manager.get_state_id(window_id);
                                let set_content = ContainerSetContent {
                                    window_id,
                                    state_id: state,
                                    items: slots,
                                    carried_item: carried,
                                };
                                let _ = send_packet(io, &set_content).await;
                                debug!("Opened container at ({}, {}, {}) for {}", x, y, z, username);
                                continue;
                            }
                            // 26.2: Daylight Detector toggle (right-click to invert)
                            if target_block.id == mc_world::redstone::DAYLIGHT_DETECTOR_ID {
                                let inverted = mc_world::redstone::toggle_daylight_detector(x, y, z);
                                // Send block update to client for visual feedback
                                let update = BlockUpdate {
                                    x, y, z, block_id: target_block.id as i32,
                                };
                                let _ = send_packet(io, &update).await;
                                info!("{} toggled daylight detector at ({}, {}, {}) — {}",
                                    username, x, y, z, if inverted { "inverted" } else { "normal" });
                                continue;
                            }
                            // 26.2: Lightning Rod — no toggle, just visual (struck externally)
                            if target_block.id == mc_world::redstone::LIGHTNING_ROD_ID {
                                continue; // Lightning rod has no GUI or right-click action
                            }
                            // Honeycomb waxing: right-click copper with honeycomb to prevent oxidation
                            let held_id = server.player_manager.get_held_item(&_uuid).map(|i| i.item.id).unwrap_or(0);
                            if held_id == 898 { // honeycomb item
                                let wax_map: &[(u32, u32)] = &[
                                    (322, 404), (323, 405), (324, 406), (325, 407), // copper→waxed
                                    (326, 408), (327, 409), (328, 410), (329, 411), // cut→waxed cut
                                    (368, 412), // chiseled→waxed chiseled
                                ];
                                for &(from, to) in wax_map {
                                    if target_block.id == from {
                                        let cp = mc_core::position::ChunkPos::new(x >> 4, z >> 4);
                                        if let Some(mut ch) = server.chunk_store.get_mut(&cp) {
                                            ch.set_block((x & 0xF) as usize, y, (z & 0xF) as usize, mc_core::block::BlockState::new(to));
                                        }
                                        let _ = server.player_manager.remove_one_from_slot(&_uuid, server.player_manager.get_held_slot(&_uuid).unwrap_or(0));
                                        break;
                                    }
                                }
                            }
                            // Bed interaction (IDs 355-370 — all colored beds)
                            if (355..=370).contains(&target_block.id) {
                                // Set spawn point at bed position
                                let player_yaw = server.player_manager.get(&_uuid).map(|p| p.position.yaw).unwrap_or(0.0);
                                let _ = server.player_manager.set_spawn_position(&_uuid, x as f64, y as f64, z as f64, player_yaw);
                                // Try to sleep if it's night/thunder
                                let can_sleep = {
                                    let ws = server.world_state.read();
                                    ws.daytime >= 12542 || matches!(ws.weather, mc_core::world_state::Weather::Thunder)
                                };
                                if can_sleep {
                                    let _ = send_packet(io, &mc_protocol::packets::play::GameEvent {
                                        event: 0, value: 0.0,
                                    }).await;
                                    // Skip to day if all players sleeping (simplified: always skip)
                                    {
                                        let mut ws = server.world_state.write();
                                        ws.daytime = 0;
                                    }
                                    let time_pkt = mc_protocol::packets::play::UpdateTime {
                                        world_age: tick_count as i64, time_of_day: 0,
                                    };
                                    let _ = send_packet(io, &time_pkt).await;
                                    // Recover health from sleeping
                                    let _ = server.player_manager.set_health(&_uuid, 20.0);
                                }
                                debug!("{} interacted with bed at ({},{},{})", username, x, y, z);
                                continue;
                            }
                            // Jukebox interaction
                            if target_block.id == 84 {
                                let held_id = server.player_manager.get_held_item(&_uuid).map(|i| i.item.id).unwrap_or(0);
                                if (2256..=2271).contains(&held_id) {
                                    // Insert disc into jukebox
                                    let sound = mc_protocol::packets::play::SoundEffect {
                                        sound_id: 0,
                                        category: mc_core::sound::SoundCategory::RECORDS,
                                        x: (x * 8), y: (y * 8), z: (z * 8),
                                        volume: 4.0, pitch: 1.0, seed: fastrand::i64(..),
                                    };
                                    let _ = send_packet(io, &sound).await;
                                    let _ = server.player_manager.remove_one_from_slot(&_uuid, server.player_manager.get_held_slot(&_uuid).unwrap_or(0));
                                    // Store disc in jukebox block state
                                    server.jukebox_discs.write().insert((x, y, z), held_id);
                                    // Send WorldEvent for record playback
                                    let _ = send_packet(io, &mc_protocol::packets::play::WorldEvent {
                                        event_id: mc_protocol::packets::play::world_event::RECORD_PLAY,
                                        position: (x, y, z),
                                        data: held_id as i32,
                                        disable_relative_volume: false,
                                    }).await;
                                    debug!("Inserted disc {} into jukebox at ({},{},{})", held_id, x, y, z);
                                } else if held_id == 0 {
                                    // Empty hand — eject disc
                                    let disc_id = server.jukebox_discs.write().remove(&(x, y, z));
                                    if let Some(disc_id) = disc_id {
                                        let _ = server.player_manager.add_item_to_player(&_uuid, mc_core::block::BlockState::new(disc_id), 1);
                                        // Send WorldEvent to stop record (B8 fix: lock dropped before await)
                                        let _ = send_packet(io, &mc_protocol::packets::play::WorldEvent {
                                            event_id: mc_protocol::packets::play::world_event::RECORD_STOP,
                                            position: (x, y, z), data: 0, disable_relative_volume: false,
                                        }).await;
                                        debug!("Ejected disc {} from jukebox at ({},{},{})", disc_id, x, y, z);
                                    }
                                }
                                continue;
                            }
                            // Campfire (532): place raw food to start cooking
                            if target_block.id == 532 {
                                let held_id = server.player_manager.get_held_item(&_uuid).map(|i| i.item.id).unwrap_or(0);
                                // Raw food items: beef(817), porkchop(818), chicken(819), mutton(821), rabbit(824), cod(832), salmon(835)
                                let cooked_map: &[(u32, u32)] = &[
                                    (817, 820), (818, 823), (819, 822), (821, 825),
                                    (824, 826), (832, 833), (835, 836),
                                ];
                                if let Some(&(_, cooked_id)) = cooked_map.iter().find(|(raw, _)| *raw == held_id) {
                                    let _ = server.player_manager.remove_one_from_slot(&_uuid, server.player_manager.get_held_slot(&_uuid).unwrap_or(0));
                                    let drop_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    let _ = send_packet(io, &SpawnEntity {
                                        entity_id: drop_eid, entity_uuid: uuid::Uuid::new_v4(),
                                        entity_type: 54, x: x as f64 + 0.5, y: y as f64 + 1.0, z: z as f64 + 0.5,
                                        pitch: 0, yaw: 0, head_yaw: 0, data: 1, vel_x: 0, vel_y: 50, vel_z: 0,
                                    }).await;
                                    server.dropped_items.write().insert(drop_eid, (cooked_id, x as f64 + 0.5, y as f64 + 1.0, z as f64 + 0.5));
                                    debug!("Placed raw food {} on campfire — cooking to {}", held_id, cooked_id);
                                }
                                continue;
                            }
                            // Bell (461): ring sound on right-click
                            if target_block.id == 461 {
                                let ring_sound = SoundEffect {
                                    sound_id: mc_core::sound::SoundIds::BLOCK_BELL,
                                    category: mc_core::sound::SoundCategory::BLOCKS,
                                    x: x * 8, y: y * 8, z: z * 8,
                                    volume: 2.0, pitch: 1.0, seed: fastrand::i64(..),
                                };
                                let _ = send_packet(io, &ring_sound).await;
                                continue;
                            }
                            // Note Block (74): right-click to change pitch
                            if target_block.id == 74 {
                                let snd = SoundEffect {
                                    sound_id: 401, // note block sound range
                                    category: mc_core::sound::SoundCategory::RECORDS,
                                    x: x * 8, y: y * 8, z: z * 8,
                                    volume: 3.0, pitch: (fastrand::u8(..) as f32 / 24.0).powi(2) + 0.5,
                                    seed: fastrand::i64(..),
                                };
                                let _ = send_packet(io, &snd).await;
                                continue;
                            }
                            // Cake: eat a slice (restore 2 food + 0.4 saturation)
                            if target_block.id == 92 || target_block.id == 883 {
                                let _ = server.player_manager.add_food(&_uuid, 2, 0.4);
                                continue;
                            }
                            // Respawn Anchor (311): charge with glowstone
                            if target_block.id == 311 {
                                let held_id = server.player_manager.get_held_item(&_uuid).map(|i| i.item.id).unwrap_or(0);
                                if held_id == 348 {
                                    let _ = server.player_manager.remove_one_from_slot(&_uuid, server.player_manager.get_held_slot(&_uuid).unwrap_or(0));
                                }
                                continue;
                            }
                            // Composter (478): add organic items for bone meal
                            if target_block.id == 478 {
                                let held_id = server.player_manager.get_held_item(&_uuid).map(|i| i.item.id).unwrap_or(0);
                                // Simple 30% chance to produce bone meal for organic items
                                let organic_items: &[u32] = &[
                                    59,141,142,207, // seeds: wheat, carrot, potato, beetroot
                                    296,338,392, // wheat, sugar cane, cactus
                                    81,83,86,87,88, // plants: tall_grass, sugar_cane, etc.
                                ];
                                if organic_items.contains(&held_id) {
                                    let _ = server.player_manager.remove_one_from_slot(&_uuid, server.player_manager.get_held_slot(&_uuid).unwrap_or(0));
                                    if fastrand::u32(..100) < 30 {
                                        let _ = server.player_manager.add_item_to_player(&_uuid, mc_core::block::BlockState::new(571), 1); // bone meal
                                        debug!("Composter produced bone meal for {}", username);
                                    }
                                }
                                continue;
                            }
                        }

                        // Entity placement (boat, minecart, armor stand, item frame, painting)
                        let held_id = server.player_manager.get_held_item(&_uuid).map(|i| i.item.id).unwrap_or(0);
                        let spawn_entity = match held_id {
                            955 => Some(23),  // boat
                            950 => Some(24),  // minecart
                            895 => Some(30),  // armor_stand
                            896 => Some(31),  // item_frame
                            897 => Some(32),  // painting
                            _ => None,
                        };
                        if let Some(ent_type) = spawn_entity {
                            let eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            // Calculate placement position: target block face + offset
                            let (ex, ey, ez) = (x as f64 + 0.5, y as f64 + 1.0, z as f64 + 0.5);
                            let _ = send_packet(io, &SpawnEntity {
                                entity_id: eid, entity_uuid: uuid::Uuid::new_v4(),
                                entity_type: ent_type,
                                x: ex, y: ey, z: ez,
                                pitch: 0, yaw: 0, head_yaw: 0, data: 0,
                                vel_x: 0, vel_y: 0, vel_z: 0,
                            }).await;
                            // Remove 1 from held stack
                            let slot = server.player_manager.get_held_slot(&_uuid).unwrap_or(0);
                            let _ = server.player_manager.remove_one_from_slot(&_uuid, slot);
                            continue;
                        }

                        // Normal block placement
                        let block = server.player_manager
                            .get_held_item(&_uuid)
                            .map(|item| item.item)
                            .unwrap_or_else(|| mc_core::block::BlockState::new(1));
                        if let Some(mut chunk) = server.chunk_store.get_mut(&cp) {
                            chunk.set_block((x & 0xF) as usize, y, (z & 0xF) as usize, block);
                            // 26.2: Sculk Sensor vibration (block place = freq 2)
                            mc_world::redstone::register_vibration(x, y, z, 2);
                            if mc_world::lighting::is_opaque(block) {
                                mc_world::lighting::recalc_sky_light_on_place(&mut chunk, (x & 0xF) as usize, y, (z & 0xF) as usize);
                            }
                            mc_world::lighting::propagate_block_light(&mut chunk);
                            let _ = send_chunk_data_cached(io, &mut chunk).await;
                            // Mark for rebroadcast to other nearby players
                            server.dirty_chunks_broadcast.write().insert(cp);
                            // Also mark for immediate UpdateSectionBlocks broadcast
                            server.dirty_blocks.mark_block(x, y, z, block.id);
                            mc_world::lighting::propagate_lighting_cross_chunk(
                                &server.chunk_store, &cp, (x & 0xF) as usize, y, (z & 0xF) as usize, false);
                            // Advancement: PlacedBlock
                            fire_advancement(server, io, &_uuid,
                                &mc_player::advancement::Criterion::PlacedBlock { block_id: block.id }).await;
                        }
                    }
                }
            }
            // Use Item (0x3F) — fishing rod, bow, snowball, egg, ender pearl, splash potion
            0x3F => {
                let held = server.player_manager.get_held_item(&_uuid);
                let held_id = held.map(|i| i.item.id).unwrap_or(0);
                let player = server.player_manager.get(&_uuid);
                let (px, py, pz, yaw, pitch) = player.as_ref()
                    .map(|p| (p.position.x, p.position.y, p.position.z, p.position.yaw, p.position.pitch))
                    .unwrap_or((0.0, 64.0, 0.0, 0.0f32, 0.0f32));

                // Capture uuid before moving player
                let player_uuid = player.as_ref().map(|p| p.uuid);
                match held_id {
                    // Shield: toggle blocking
                    895 => {
                        let is_blocking = player.as_ref().map(|p| p.is_blocking).unwrap_or(false);
                        let _ = server.player_manager.set_blocking(&_uuid, !is_blocking);
                    }
                    // Fishing rod
                    844 => {
                        // Read fishing rod enchantments
                        let held_fishing_enchants = player.as_ref()
                            .and_then(|p| p.inventory.items.get(p.inventory.selected_slot as usize))
                            .and_then(|opt| opt.as_ref())
                            .and_then(|stack| stack.nbt.as_ref())
                            .map(|nbt| mc_player::enchant::parse_item_enchants(&Some(nbt.clone())))
                            .unwrap_or_default();
                        let lure_level = held_fishing_enchants.get("lure").copied().unwrap_or(0);
                        let enchant_luck = held_fishing_enchants.get("luck_of_the_sea").copied().unwrap_or(0);
                        // Luck/Unluck effects modify fishing treasure probability
                        let effect_luck = server.player_manager.get_effect_level(&_uuid, 25); // Luck=25
                        let effect_unluck = server.player_manager.get_effect_level(&_uuid, 26); // Unluck=26
                        let luck_level = enchant_luck.saturating_add(effect_luck).saturating_sub(effect_unluck);

                        let existing_fishing = player.as_ref().and_then(|p| p.fishing.as_ref()).cloned();
                        if let Some(fish_state) = existing_fishing {
                            // Reel in — scope the write lock
                            let entity_id = fish_state.bobber_entity_id;
                            let (loot_opt, _caught) = {
                                let mut fishing_mgr = server.fishing_manager.write();
                                fishing_mgr.reel_in(entity_id, luck_level)
                            };
                            if let Some(loot) = loot_opt {
                                // Give item to player
                                let _ = server.player_manager.add_item(&_uuid, loot);
                                // Advancement: FishCaught
                                fire_advancement(server, io, &_uuid,
                                    &mc_player::advancement::Criterion::FishCaught).await;
                                // Spawn XP
                                let xp_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let _ = send_packet(io, &SpawnEntity {
                                    entity_id: xp_eid, entity_uuid: uuid::Uuid::new_v4(),
                                    entity_type: 53, // experience orb
                                    x: px, y: py + 1.0, z: pz,
                                    pitch: 0, yaw: 0, head_yaw: 0,
                                    data: 3, vel_x: 0, vel_y: 0, vel_z: 0,
                                }).await;
                            }
                            // Remove bobber
                            let _ = send_packet(io, &RemoveEntities { entity_ids: vec![entity_id] }).await;
                            if let Some(ref uuid) = player_uuid {
                                let _ = server.player_manager.clear_fishing(uuid);
                            }
                        } else {
                            // Cast bobber — scope the write lock, then send packets
                            let bobber_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let speed = 1.5;
                            let yaw_rad = yaw as f64 * std::f64::consts::TAU / 256.0;
                            let pitch_rad = pitch as f64 * std::f64::consts::TAU / 256.0;
                            let dx = -yaw_rad.sin() * pitch_rad.cos() * speed;
                            let dy = -pitch_rad.sin() * speed;
                            let dz = yaw_rad.cos() * pitch_rad.cos() * speed;
                            // Register bobber in fishing manager (before any .await)
                            {
                                let mut fishing_mgr = server.fishing_manager.write();
                                fishing_mgr.cast(bobber_eid, px, py + 1.6, pz, lure_level);
                            }
                            // Spawn fishing bobber packet
                            let _ = send_packet(io, &SpawnEntity {
                                entity_id: bobber_eid, entity_uuid: uuid::Uuid::new_v4(),
                                entity_type: 90, // fishing bobber
                                x: px, y: py + 1.6, z: pz,
                                pitch: 0, yaw: 0, head_yaw: 0,
                                data: 0,
                                vel_x: (dx * 8000.0) as i16, vel_y: (dy * 8000.0) as i16, vel_z: (dz * 8000.0) as i16,
                            }).await;
                            let _ = send_packet(io, &SetEntityVelocity {
                                entity_id: bobber_eid,
                                vel_x: (dx * 8000.0) as i16, vel_y: (dy * 8000.0) as i16, vel_z: (dz * 8000.0) as i16,
                            }).await;
                            if let Some(ref uuid) = player_uuid {
                                let state = mc_player::fishing::FishingState {
                                    bobber_entity_id: bobber_eid, wait_ticks: 150, bites: false,
                                    x: px, y: py + 1.6, z: pz,
                                };
                                let _ = server.player_manager.set_fishing(uuid, state);
                            }
                        }
                    }
                    // Bow
                    773 => {
                        // Bow: spawn arrow projectile with enchantment support (Phase F)
                        let held_item = server.player_manager.get_held_item(&_uuid);
                        let enchants = held_item.as_ref()
                            .and_then(|i| i.nbt.as_ref())
                            .map(|nbt| mc_player::enchant::parse_item_enchants(&Some(nbt.clone())))
                            .unwrap_or_default();
                        let power_lvl = enchants.get("power").copied().unwrap_or(0);
                        let flame_lvl = enchants.get("flame").copied().unwrap_or(0);
                        let punch_lvl = enchants.get("punch").copied().unwrap_or(0);
                        let base_damage = 2.0 * (1.0 + 0.25 * (power_lvl as f32 + 1.0));
                        let speed = 2.5;
                        let yaw_rad = yaw as f64 * std::f64::consts::TAU / 256.0;
                        let pitch_rad = pitch as f64 * std::f64::consts::TAU / 256.0;
                        let dx = -yaw_rad.sin() * pitch_rad.cos() * speed;
                        let dy = -pitch_rad.sin() * speed;
                        let dz = yaw_rad.cos() * pitch_rad.cos() * speed;
                        let arrow_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let _ = send_packet(io, &SpawnEntity {
                            entity_id: arrow_eid, entity_uuid: uuid::Uuid::new_v4(),
                            entity_type: 7, x: px, y: py + 1.6, z: pz,
                            pitch: pitch as u8, yaw: yaw as u8, head_yaw: 0, data: 0,
                            vel_x: (dx * 8000.0) as i16, vel_y: (dy * 8000.0) as i16, vel_z: (dz * 8000.0) as i16,
                        }).await;
                        // Register projectile for server-side tracking with enchantment data
                        let proj = mc_player::mob::Projectile {
                            entity_id: arrow_eid, owner_uuid: player_uuid.unwrap_or(uuid::Uuid::nil()),
                            owner_entity_id: server.player_manager.get_entity_id(&_uuid).unwrap_or(-1),
                            projectile_type: mc_player::mob::ProjectileType::Arrow,
                            position: mc_core::position::Position::new(px, py + 1.6, pz),
                            vel_x: dx, vel_y: dy, vel_z: dz,
                            damage: base_damage, ticks_alive: 0, max_ticks: 1200, in_ground: false,
                            power_level: power_lvl, flame_level: flame_lvl, punch_level: punch_lvl, piercing_level: 0,
                            loyalty_level: 0, launch_y: py + 1.6,
                        };
                        server.mob_manager.projectiles.insert(arrow_eid, proj);
                        // Infinity check
                        let has_infinity = enchants.contains_key("infinity");
                        if !has_infinity {
                            let _ = server.player_manager.remove_item(&_uuid, mc_core::block::BlockState::new(774), 1);
                        }
                    }
                    // Crossbow (941): load and fire arrows/fireworks
                    941 => {
                        let held_item = server.player_manager.get_held_item(&_uuid);
                        let enchants = held_item.as_ref()
                            .and_then(|i| i.nbt.as_ref())
                            .map(|nbt| mc_player::enchant::parse_item_enchants(&Some(nbt.clone())))
                            .unwrap_or_default();
                        let multishot = enchants.contains_key("multishot");
                        let piercing_lvl = enchants.get("piercing").copied().unwrap_or(0);
                        let _quick_charge = enchants.get("quick_charge").copied().unwrap_or(0);
                        let yaw_rad = yaw as f64 * std::f64::consts::TAU / 256.0;
                        let pitch_rad = pitch as f64 * std::f64::consts::TAU / 256.0;
                        let speed = 3.0;
                        // Check for firework rockets in offhand (slot 40) or inventory
                        let has_firework = server.player_manager.get(&_uuid)
                            .map(|p| {
                                p.inventory.items.get(40).and_then(|o| o.as_ref())
                                    .map(|s| s.item.id == 965).unwrap_or(false)
                            }).unwrap_or(false);
                        let entity_type: i32 = if has_firework { 72 } else { 7 }; // firework or arrow
                        let proj_type = if has_firework { mc_player::mob::ProjectileType::Firework } else { mc_player::mob::ProjectileType::Arrow };
                        let count: u8 = if multishot && !has_firework { 3 } else { 1 }; // multishot only for arrows
                        for i in 0..count {
                            let spread = if multishot && i > 0 { ((i as f64 - 1.0) * 0.17 - 0.17) * std::f64::consts::PI } else { 0.0 };
                            let a_yaw = yaw_rad + spread;
                            let dx = -a_yaw.sin() * pitch_rad.cos() * speed;
                            let dy = -pitch_rad.sin() * speed;
                            let dz = a_yaw.cos() * pitch_rad.cos() * speed;
                            let ceid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let _ = send_packet(io, &SpawnEntity {
                                entity_id: ceid, entity_uuid: uuid::Uuid::new_v4(),
                                entity_type, x: px, y: py + 1.6, z: pz,
                                pitch: pitch as u8, yaw: yaw as u8, head_yaw: 0, data: 0,
                                vel_x: (dx * 8000.0) as i16, vel_y: (dy * 8000.0) as i16, vel_z: (dz * 8000.0) as i16,
                            }).await;
                            let proj = mc_player::mob::Projectile {
                                entity_id: ceid, owner_uuid: player_uuid.unwrap_or(uuid::Uuid::nil()),
                                owner_entity_id: server.player_manager.get_entity_id(&_uuid).unwrap_or(-1),
                                projectile_type: proj_type,
                                position: mc_core::position::Position::new(px, py + 1.6, pz),
                                vel_x: dx, vel_y: dy, vel_z: dz,
                                damage: if has_firework { 5.0 } else { 3.0 }, ticks_alive: 0, max_ticks: 1200, in_ground: false,
                                power_level: 0, flame_level: 0, punch_level: 0, piercing_level: piercing_lvl, loyalty_level: 0, launch_y: py + 1.6,
                            };
                            server.mob_manager.projectiles.insert(ceid, proj);
                        }
                        // ShotCrossbow advancement
                        fire_advancement(server, io, &_uuid,
                            &mc_player::advancement::Criterion::ShotCrossbow).await;
                    }
                    // Trident (940): throw trident
                    940 => {
                        let held = server.player_manager.get_held_item(&_uuid);
                        let enchants = held.as_ref()
                            .and_then(|i| i.nbt.as_ref())
                            .map(|nbt| mc_player::enchant::parse_item_enchants(&Some(nbt.clone())))
                            .unwrap_or_default();
                        let loyalty = enchants.contains_key("loyalty");
                        let has_riptide = enchants.contains_key("riptide");
                        let has_channeling = enchants.contains_key("channeling");
                        let impaling_lvl = enchants.get("impaling").copied().unwrap_or(0);
                        let base_dmg = 8.0 + impaling_lvl as f32 * 2.5;
                        // Riptide: in water/rain, dash forward instead of throwing
                        let in_water = false; // simplified — would check block at player position
                        if has_riptide && in_water {
                            let yaw_rad = yaw as f64 * std::f64::consts::TAU / 256.0;
                            let dash_dist = 8.0;
                            let new_x = px + (-yaw_rad.sin()) * dash_dist;
                            let new_z = pz + yaw_rad.cos() * dash_dist;
                            let _ = server.player_manager.update_position_full(&_uuid, new_x, py, new_z, yaw, pitch);
                            let pos_pkt = PlayerPosition { x: new_x, y: py, z: new_z, yaw, pitch, flags: 0, teleport_id: 0 };
                            let _ = send_packet(io, &pos_pkt).await;
                        } else {
                            let yaw_rad = yaw as f64 * std::f64::consts::TAU / 256.0;
                            let pitch_rad = pitch as f64 * std::f64::consts::TAU / 256.0;
                            let speed = 2.5;
                            let dx = -yaw_rad.sin() * pitch_rad.cos() * speed;
                            let dy = -pitch_rad.sin() * speed;
                            let dz = yaw_rad.cos() * pitch_rad.cos() * speed;
                            let teid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let _ = send_packet(io, &SpawnEntity {
                                entity_id: teid, entity_uuid: uuid::Uuid::new_v4(),
                                entity_type: 94, x: px, y: py + 1.6, z: pz,
                                pitch: pitch as u8, yaw: yaw as u8, head_yaw: 0,
                                data: if loyalty { 1 } else { 0 },
                                vel_x: (dx * 8000.0) as i16, vel_y: (dy * 8000.0) as i16, vel_z: (dz * 8000.0) as i16,
                            }).await;
                            let proj = mc_player::mob::Projectile {
                                entity_id: teid, owner_uuid: player_uuid.unwrap_or(uuid::Uuid::nil()),
                                owner_entity_id: server.player_manager.get_entity_id(&_uuid).unwrap_or(-1),
                                projectile_type: mc_player::mob::ProjectileType::Trident,
                                position: mc_core::position::Position::new(px, py + 1.6, pz),
                                vel_x: dx, vel_y: dy, vel_z: dz,
                                damage: base_dmg, ticks_alive: 0, max_ticks: if loyalty { 1200 } else { 600 }, in_ground: false,
                                power_level: if has_channeling { 1 } else { 0 }, flame_level: 0, punch_level: 0, piercing_level: 0, loyalty_level: if loyalty { 3 } else { 0 }, launch_y: py + 1.6,
                            };
                            server.mob_manager.projectiles.insert(teid, proj);
                        }
                        let _ = server.player_manager.remove_one_from_slot(&_uuid, server.player_manager.get_held_slot(&_uuid).unwrap_or(0));
                    }
                    900 => {
                        let sb_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let yaw_rad = yaw as f64 * std::f64::consts::TAU / 256.0;
                        let pitch_rad = pitch as f64 * std::f64::consts::TAU / 256.0;
                        let speed = 1.5;
                        let _ = send_packet(io, &SpawnEntity {
                            entity_id: sb_eid, entity_uuid: uuid::Uuid::new_v4(),
                            entity_type: 86, x: px, y: py + 1.6, z: pz,
                            pitch: pitch as u8, yaw: yaw as u8, head_yaw: 0, data: 0,
                            vel_x: (-yaw_rad.sin() * pitch_rad.cos() * speed * 8000.0) as i16,
                            vel_y: (-pitch_rad.sin() * speed * 8000.0) as i16,
                            vel_z: (yaw_rad.cos() * pitch_rad.cos() * speed * 8000.0) as i16,
                        }).await;
                    }
                    // Egg
                    884 => {
                        let egg_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let yaw_rad = yaw as f64 * std::f64::consts::TAU / 256.0;
                        let pitch_rad = pitch as f64 * std::f64::consts::TAU / 256.0;
                        let speed = 1.5;
                        let _ = send_packet(io, &SpawnEntity {
                            entity_id: egg_eid, entity_uuid: uuid::Uuid::new_v4(),
                            entity_type: 87, x: px, y: py + 1.6, z: pz,
                            pitch: pitch as u8, yaw: yaw as u8, head_yaw: 0, data: 0,
                            vel_x: (-yaw_rad.sin() * pitch_rad.cos() * speed * 8000.0) as i16,
                            vel_y: (-pitch_rad.sin() * speed * 8000.0) as i16,
                            vel_z: (yaw_rad.cos() * pitch_rad.cos() * speed * 8000.0) as i16,
                        }).await;
                    }
                    // Firework rocket (965) — launch or elytra boost
                    965 => {
                        // Check if player is elytra gliding — apply boost
                        if server.player_manager.is_flying(&_uuid) {
                            let yaw_rad = yaw as f64 * std::f64::consts::TAU / 256.0;
                            let pitch_rad = pitch as f64 * std::f64::consts::TAU / 256.0;
                            let boost = 1.5; // blocks per tick boost
                            let vx = -yaw_rad.sin() * pitch_rad.cos() * boost;
                            let vy = -pitch_rad.sin() * boost;
                            let vz = yaw_rad.cos() * pitch_rad.cos() * boost;
                            // Apply boost by teleporting player forward
                            let new_x = px + vx * 2.0;
                            let new_y = (py + vy * 2.0).max(0.0);
                            let new_z = pz + vz * 2.0;
                            let _ = server.player_manager.update_position_full(&_uuid, new_x, new_y, new_z, yaw, pitch);
                            let pos_pkt = PlayerPosition { x: new_x, y: new_y, z: new_z, yaw, pitch, flags: 0, teleport_id: 0 };
                            let _ = send_packet(io, &pos_pkt).await;
                            // Spawn firework visuals
                            let fw_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let _ = send_packet(io, &SpawnEntity {
                                entity_id: fw_eid, entity_uuid: uuid::Uuid::new_v4(),
                                entity_type: 72,
                                x: px, y: py + 1.6, z: pz,
                                pitch: 0, yaw: 0, head_yaw: 0, data: 0,
                                vel_x: 0, vel_y: 400, vel_z: 0,
                            }).await;
                        } else {
                            let fw_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let _ = send_packet(io, &SpawnEntity {
                                entity_id: fw_eid, entity_uuid: uuid::Uuid::new_v4(),
                                entity_type: 72,
                                x: px, y: py + 1.6, z: pz,
                                pitch: 0, yaw: 0, head_yaw: 0, data: 0,
                                vel_x: 0, vel_y: 400, vel_z: 0,
                            }).await;
                        }
                    }
                    // Ender pearl (908) — spawn projectile + apply cooldown
                    908 => {
                        let ep_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let speed = 1.5;
                        let yaw_rad = yaw as f64 * std::f64::consts::TAU / 256.0;
                        let pitch_rad = pitch as f64 * std::f64::consts::TAU / 256.0;
                        let _ = send_packet(io, &SpawnEntity {
                            entity_id: ep_eid, entity_uuid: uuid::Uuid::new_v4(),
                            entity_type: 79, // ender pearl projectile
                            x: px, y: py + 1.6, z: pz,
                            pitch: pitch as u8, yaw: yaw as u8, head_yaw: 0, data: 0,
                            vel_x: (-yaw_rad.sin() * pitch_rad.cos() * speed * 8000.0) as i16,
                            vel_y: (-pitch_rad.sin() * speed * 8000.0) as i16,
                            vel_z: (yaw_rad.cos() * pitch_rad.cos() * speed * 8000.0) as i16,
                        }).await;
                        // Apply cooldown for ender pearl
                        let _ = send_packet(io, &SetCooldown {
                            item_id: 908,
                            cooldown_ticks: 20, // 1 second cooldown
                        }).await;
                    }
                    // Food items — consume to restore hunger
                    _ => {
                        if mc_player::food::is_food(held_id)
                            && let Some((nutrition, saturation)) = mc_player::food::get_food_value(held_id) {
                                let _ = server.player_manager.add_food(&_uuid, nutrition, saturation);
                                // Remove 1 from held item stack
                                let slot = server.player_manager.get_held_slot(&_uuid).unwrap_or(0);
                                let _ = server.player_manager.remove_one_from_slot(&_uuid, slot);
                                // Advancement: ItemUsed + ConsumeItem (for food items)
                                fire_advancement(server, io, &_uuid,
                                    &mc_player::advancement::Criterion::ItemUsed { item_id: held_id }).await;
                                fire_advancement(server, io, &_uuid,
                                    &mc_player::advancement::Criterion::ConsumeItem { item_id: held_id }).await;
                                debug!("{} ate item {} (+{} hunger, +{:.1} saturation)", username, held_id, nutrition, saturation);
                            }
                    }
                }
            }
            // Interact Entity (0x18) — attack/interact with another entity
            0x18 => {
                if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(&frame)
                    && payload.len() >= 2 {
                        // Read entity_id (VarInt) and type (VarInt)
                        let target_entity_id = match mc_protocol::codec::read_varint_enum(&payload) {
                            Ok((id, _off)) => { id }
                            Err(_) => continue,
                        };
                        // interact_type is at offset after the first varint
                        let interact_type_offset = {
                            use mc_protocol::codec::read_varint_enum;
                            let (_, off) = read_varint_enum(&payload).unwrap_or((0, 1));
                            off
                        };
                        let interact_type = if interact_type_offset < payload.len() {
                            payload[interact_type_offset] as i32
                        } else {
                            0
                        };
                        // Type 1 = attack
                        if interact_type == 1 {
                            // Try PvP first
                            if let Some(target_uuid) = server.player_manager.uuid_by_entity(target_entity_id as u32) {
                                let current_tick = tick_count;
                                if server.player_manager.can_take_damage(&target_uuid, current_tick)
                                    && let Some(target) = server.player_manager.get(&target_uuid) {
                                        // Calculate weapon damage with cooldown + critical
                                        let held_item = server.player_manager.get_held_item(&_uuid);
                                        let held_id = held_item.as_ref().map(|i| i.item.id).unwrap_or(0);
                                        let base_damage = held_item.as_ref().map(|i| weapon_damage(i.item.id)).unwrap_or(1.0);
                                        // Parse attacker item enchantments
                                        let attacker_enchants = held_item.as_ref()
                                            .and_then(|i| i.nbt.as_ref())
                                            .map(|nbt| mc_player::enchant::parse_item_enchants(&Some(nbt.clone())))
                                            .unwrap_or_default();
                                        // Get target mob type for enchantment bonuses
                                        let target_mob_type = server.mob_manager.get(target_entity_id)
                                            .map(|m| m.mob_type).unwrap_or(0);
                                        // ── Enchantment damage bonuses ──
                                        let mut enchant_damage_bonus = 0.0f32;
                                        // Smite: +2.5 per level vs undead mobs (using is_undead from constants)
                                        if let Some(&smite_lvl) = attacker_enchants.get("smite")
                                            && mc_core::constants::entity_type::is_undead(target_mob_type) {
                                                enchant_damage_bonus += 2.5 * smite_lvl as f32;
                                            }
                                        // Bane of Arthropods: +2.5 per level vs arthropods (using is_arthropod from constants)
                                        if let Some(&bane_lvl) = attacker_enchants.get("bane_of_arthropods")
                                            && mc_core::constants::entity_type::is_arthropod(target_mob_type) {
                                                enchant_damage_bonus += 2.5 * bane_lvl as f32;
                                            }
                                        let effective_damage = base_damage + enchant_damage_bonus;
                                        // 1.9+ Attack cooldown
                                        let cooldown = server.player_manager.get_attack_cooldown(&_uuid, held_id, current_tick);
                                        // Critical hit: falling + not sprinting + cooldown > 0.848
                                        let attacker_fall = server.player_manager.get(&_uuid).map(|p| p.fall_distance).unwrap_or(0.0);
                                        let is_sprinting = server.player_manager.get(&_uuid).map(|p| p.is_sprinting).unwrap_or(false);
                                        let is_critical = attacker_fall > 0.0 && !is_sprinting && cooldown > 0.848;
                                        // Shield check: target blocking + attacker in front (180° arc)
                                        let (target_blocking, attacker_in_front) = server.player_manager
                                            .get(&target_uuid)
                                            .map(|p| {
                                                let blocking = p.is_blocking;
                                                // Check if attacker is in front of defender (180° arc)
                                                let defender_yaw = p.position.yaw;
                                                let defender_x = p.position.x;
                                                let defender_z = p.position.z;
                                                let (att_x, att_z) = server.player_manager.get(&_uuid)
                                                    .map(|ap| (ap.position.x, ap.position.z))
                                                    .unwrap_or((0.0, 0.0));
                                                let dx = att_x - defender_x;
                                                let dz = att_z - defender_z;
                                                let attack_angle = dz.atan2(dx).to_degrees() as f32;
                                                let yaw_deg = defender_yaw;
                                                let angle_diff = (attack_angle - yaw_deg).rem_euclid(360.0);
                                                let front = !(90.0..=270.0).contains(&angle_diff);
                                                (blocking, front)
                                            }).unwrap_or((false, false));
                                        let damage = if target_blocking && attacker_in_front {
                                            // Shield blocks all frontal damage
                                            // Axe disables shield for 1.6s (32 ticks, 26.2 mechanic)
                                            if matches!(held_id, 770 | 788 | 791) { // axes
                                                let _ = server.player_manager.set_blocking(&target_uuid, false);
                                                // Send shield cooldown visual (SetCooldown for shield item)
                                                let shield_cooldown = mc_protocol::packets::play::SetCooldown {
                                                    item_id: 895, // shield
                                                    cooldown_ticks: 32, // 1.6s
                                                };
                                                let _ = send_packet(io, &shield_cooldown).await;
                                            }
                                            0.0
                                        } else {
                                            if is_critical { (effective_damage * 1.5).round() }
                                            else { effective_damage * cooldown }
                                        }.max(0.0);
                                        // Apply armor reduction with enchantment + effect modifiers
                                        let actual = server.player_manager.apply_damage_with_enchants(
                                            &target_uuid, damage, current_tick,
                                            Some(&_uuid),
                                            Some(&attacker_enchants),
                                            None, // defender armor enchants not yet passed
                                        ).unwrap_or(damage);
                                        server.player_manager.set_last_attack(&_uuid, current_tick);
                                        // Fire Aspect: set target on fire (80 ticks = 4 seconds per level)
                                        let fire_aspect_level = attacker_enchants.get("fire_aspect").copied().unwrap_or(0);
                                        if fire_aspect_level > 0 && matches!(held_id, 780 | 785 | 792 | 797) {
                                            // Swords: Fire Aspect I=80 ticks, II=160 ticks
                                            let fire_ticks = (fire_aspect_level * 80) as i16;
                                            let _ = send_packet(io, &SetEntityMetadata {
                                                entity_id: target_entity_id,
                                                metadata: vec![9u8, 6u8, 1u8, 0xFFu8], // index 0x09=fire, type=6(bool), value=1
                                            }).await;
                                            // Extinguish after fire_ticks (handled by client)
                                            let _ = send_packet(io, &SetEntityMetadata {
                                                entity_id: target_entity_id,
                                                metadata: vec![0x0Au8, 1u8, (fire_ticks >> 8) as u8, (fire_ticks & 0xFF) as u8],
                                            }).await;
                                        }
                                        // Knockback: push target away from attacker (with enchantment bonus)
                                        if actual > 0.0
                                            && let Some(attacker) = server.player_manager.get(&_uuid)
                                                && let Some(target_player) = server.player_manager.get(&target_uuid) {
                                                    let kb_level = attacker_enchants.get("knockback").copied().unwrap_or(0) as f64;
                                                    let knockback = (0.4 + kb_level * 0.5) * ((actual as f64) / 3.0).min(2.0);
                                                    let kb_x = if target_player.position.x > attacker.position.x { 1.0f64 } else { -1.0f64 };
                                                    let kb_z = if target_player.position.z > attacker.position.z { 1.0f64 } else { -1.0f64 };
                                                    let _ = send_packet(io, &SetEntityVelocity {
                                                        entity_id: target_entity_id,
                                                        vel_x: (kb_x * knockback * 8000.0) as i16,
                                                        vel_y: (knockback * 4000.0f64) as i16,
                                                        vel_z: (kb_z * knockback * 8000.0) as i16,
                                                    }).await;
                                                }
                                        let target_health = server.player_manager.get(&target_uuid)
                                            .map(|p| p.health).unwrap_or(0.0);
                                        // Send DamageEvent
                                        let attacker_eid = server.player_manager.get_entity_id(&_uuid).unwrap_or(-1);
                                        let dmg_event = DamageEvent {
                                            entity_id: target_entity_id,
                                            source_type_id: 1,
                                            source_cause_id: attacker_eid,
                                            source_direct_id: attacker_eid,
                                            source_pos_x: None, source_pos_y: None, source_pos_z: None,
                                        };
                                        let _ = send_packet(io, &dmg_event).await;
                                        // Send EntityEvent for hurt animation
                                        let _ = send_packet(io, &EntityEvent {
                                            entity_id: target_entity_id,
                                            status: 2, // hurt
                                        }).await;
                                        let hurt_sound = SoundEffect {
                                            sound_id: mc_core::sound::SoundIds::ENTITY_PLAYER_HURT,
                                            category: mc_core::sound::SoundCategory::PLAYERS,
                                            x: (target.position.x * 8.0) as i32,
                                            y: (target.position.y * 8.0) as i32,
                                            z: (target.position.z * 8.0) as i32,
                                            volume: 1.0, pitch: 1.0, seed: fastrand::i64(..),
                                        };
                                        let _ = send_packet(io, &hurt_sound).await;
                                        debug!("Player '{}' attacked '{}' (PvP, dmg={:.1}, health={:.1})",
                                            username, target.username, actual, target_health);
                                        // ── Sweeping Edge: sword sweep hits nearby targets ──
                                        let sweeping_level = attacker_enchants.get("sweeping_edge").copied().unwrap_or(0);
                                        if sweeping_level > 0 && matches!(held_id, 780 | 785 | 792 | 797)
                                            && cooldown > 0.848 && attacker_fall <= 0.0 {
                                            let sweep_damage = 1.0 + sweeping_level as f32 * 0.5;
                                            let all_players = server.player_manager.all_players();
                                            for nearby in &all_players {
                                                if nearby.uuid == target_uuid || nearby.uuid == _uuid { continue; }
                                                if let Some(np_data) = server.player_manager.get(&nearby.uuid) {
                                                    let dx = np_data.position.x - target.position.x;
                                                    let dy = np_data.position.y - target.position.y;
                                                    let dz = np_data.position.z - target.position.z;
                                                    if dx*dx + dy*dy + dz*dz < 6.25 {
                                                        // Within 2.5 blocks — apply sweep
                                                        let sweep_eid = server.player_manager.get_entity_id(&nearby.uuid).unwrap_or(-1);
                                                        let _ = server.player_manager.apply_damage(
                                                            &nearby.uuid, sweep_damage, current_tick + 1).ok();
                                                        let _ = send_packet(io, &EntityEvent {
                                                            entity_id: sweep_eid, status: 2, // hurt
                                                        }).await;
                                                    }
                                                }
                                            }
                                        }
                                        if target_health <= 0.0 {
                                            info!("Player '{}' was killed by '{}'", target.username, username);
                                            // Drop inventory items at death location (skip VanishingCurse items)
                                            if let Some(dead_player) = server.player_manager.get(&target_uuid) {
                                                for stack in dead_player.inventory.items.iter().flatten() {
                                                        // VanishingCurse: items with this curse do not drop on death
                                                        let has_vanishing = stack.nbt.as_ref()
                                                            .map(|nbt| mc_player::enchant::has_enchant(&Some(nbt.clone()), "vanishing_curse"))
                                                            .unwrap_or(false);
                                                        if has_vanishing { continue; }
                                                        let drop_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                                        let dx = (fastrand::f64() - 0.5) * 0.5;
                                                        let dz = (fastrand::f64() - 0.5) * 0.5;
                                                        let _ = send_packet(io, &SpawnEntity {
                                                            entity_id: drop_eid, entity_uuid: uuid::Uuid::new_v4(),
                                                            entity_type: 54,
                                                            x: dead_player.position.x + dx, y: dead_player.position.y + 0.5, z: dead_player.position.z + dz,
                                                            pitch: 0, yaw: 0, head_yaw: 0, data: 1,
                                                            vel_x: ((fastrand::f64() - 0.5) * 200.0) as i16,
                                                            vel_y: 200, vel_z: ((fastrand::f64() - 0.5) * 200.0) as i16,
                                                        }).await;
                                                        server.dropped_items.write().insert(drop_eid, (stack.item.id, dead_player.position.x + dx, dead_player.position.y + 0.5, dead_player.position.z + dz));
                                                }
                                            }
                                        }
                                    }
                            }
                            // Try PvE (attack mob)
                            else if let Some(mob) = server.mob_manager.get(target_entity_id) {
                                let held_item = server.player_manager.get_held_item(&_uuid);
                                let base_damage = held_item.as_ref().map(|i| weapon_damage(i.item.id)).unwrap_or(1.0);
                                let attacker_fall = server.player_manager.get(&_uuid)
                                    .map(|p| p.fall_distance).unwrap_or(0.0);
                                let damage = if attacker_fall > 0.0 { (base_damage * 1.5).round() } else { base_damage };
                                let new_health = server.mob_manager.damage(target_entity_id, damage);
                                // Send DamageEvent
                                let attacker_eid = server.player_manager.get_entity_id(&_uuid).unwrap_or(-1);
                                let dmg_event = DamageEvent {
                                    entity_id: target_entity_id,
                                    source_type_id: 1,
                                    source_cause_id: attacker_eid,
                                    source_direct_id: attacker_eid,
                                    source_pos_x: None, source_pos_y: None, source_pos_z: None,
                                };
                                let _ = send_packet(io, &dmg_event).await;
                                // Send hurt sound
                                let (mx, my, mz) = (mob.position.x, mob.position.y, mob.position.z);
                                let hurt_sound = SoundEffect {
                                    sound_id: mc_core::sound::SoundIds::ENTITY_GENERIC_HURT,
                                    category: mc_core::sound::SoundCategory::HOSTILE,
                                    x: (mx * 8.0) as i32, y: (my * 8.0) as i32, z: (mz * 8.0) as i32,
                                    volume: 1.0, pitch: 1.0, seed: fastrand::i64(..),
                                };
                                let _ = send_packet(io, &hurt_sound).await;
                                if let Some(hp) = new_health
                                    && hp <= 0.0 {
                                        // ── 26.2 Sulfur Cube: split into 2 small cubes ──
                                        if mob.mob_type == mc_core::constants::entity_type::SULFUR_CUBE {
                                            // Small cubes: no split, just remove
                                            if mob.is_small_cube {
                                                let remove = RemoveEntities { entity_ids: vec![target_entity_id] };
                                                let _ = send_packet(io, &remove).await;
                                                server.mob_manager.remove(target_entity_id);
                                                server.player_manager.broadcast_mob_despawn(target_entity_id, mob.uuid);
                                                continue; // skip normal death handling
                                            }
                                            // Explosive archetype: no small cubes, just explosion
                                            if let Some(mc_player::mob::SulfurCubeArchetype::Explosive { .. }) = mob.sulfur_cube_archetype {
                                                // Remove without splitting
                                                let remove = RemoveEntities { entity_ids: vec![target_entity_id] };
                                                let _ = send_packet(io, &remove).await;
                                                server.mob_manager.remove(target_entity_id);
                                                server.player_manager.broadcast_mob_despawn(target_entity_id, mob.uuid);
                                            } else {
                                                // Split into 2 small cubes
                                                let remove = RemoveEntities { entity_ids: vec![target_entity_id] };
                                                let _ = send_packet(io, &remove).await;
                                                server.mob_manager.remove(target_entity_id);
                                                // Spawn 2 small cubes
                                                let small1_eid = server.mob_manager.sulfur_cube_spawn_small(&mob, &server.next_entity_id);
                                                let small2_eid = server.mob_manager.sulfur_cube_spawn_small(&mob, &server.next_entity_id);
                                                // Send spawn packets for both
                                                for &eid in &[small1_eid, small2_eid] {
                                                    let _ = send_packet(io, &SpawnEntity {
                                                        entity_id: eid, entity_uuid: uuid::Uuid::new_v4(),
                                                        entity_type: mc_core::constants::entity_type::SULFUR_CUBE,
                                                        x: mob.position.x + (fastrand::f64() - 0.5) * 1.0,
                                                        y: mob.position.y, z: mob.position.z + (fastrand::f64() - 0.5) * 1.0,
                                                        pitch: 0, yaw: 0, head_yaw: 0, data: 0,
                                                        vel_x: ((fastrand::f64() - 0.5) * 300.0) as i16,
                                                        vel_y: 300, vel_z: ((fastrand::f64() - 0.5) * 300.0) as i16,
                                                    }).await;
                                                }
                                                server.player_manager.broadcast_mob_despawn(target_entity_id, mob.uuid);
                                                info!("Sulfur Cube split into 2 small cubes (eid={}, {})", small1_eid, small2_eid);
                                            }
                                            continue; // skip normal death handling
                                        }
                                        // Mob died — remove entity, spawn drops + XP
                                        let remove = RemoveEntities { entity_ids: vec![target_entity_id] };
                                        let _ = send_packet(io, &remove).await;
                                        // Spawn item drop (with Looting enchant bonus)
                                        let drop_id = mc_player::mob::mob_drop_item(mob.mob_type);
                                        let looting_level = held_item.as_ref()
                                            .and_then(|i| i.nbt.as_ref())
                                            .map(|nbt| mc_player::enchant::enchant_level(&Some(nbt.clone()), "looting"))
                                            .unwrap_or(0);
                                        let drop_count = if drop_id > 0 {
                                            mc_player::mob::mob_drop_count_with_looting(mob.mob_type, looting_level) as u32
                                        } else { 0 };
                                        for _ in 0..drop_count {
                                            if drop_id > 0 {
                                                let item_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                                let _ = send_packet(io, &SpawnEntity {
                                                    entity_id: item_eid,
                                                    entity_uuid: uuid::Uuid::new_v4(),
                                                    entity_type: 54,
                                                    x: mx, y: my + 0.5, z: mz,
                                                    pitch: 0, yaw: 0, head_yaw: 0,
                                                    data: 1,
                                                    vel_x: ((fastrand::f64() - 0.5) * 200.0) as i16,
                                                    vel_y: 200,
                                                    vel_z: ((fastrand::f64() - 0.5) * 200.0) as i16,
                                                }).await;
                                            }
                                        }
                                        // Spawn XP orbs
                                        let xp_count = mc_player::mob::mob_xp_drop(mob.mob_type);
                                        for i in 0..xp_count {
                                            let orb_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                            let _ = send_packet(io, &SpawnEntity {
                                                entity_id: orb_eid,
                                                entity_uuid: uuid::Uuid::new_v4(),
                                                entity_type: 53,
                                                x: mx + (fastrand::f64() - 0.5) * 1.5,
                                                y: my + 0.5,
                                                z: mz + (fastrand::f64() - 0.5) * 1.5,
                                                pitch: 0, yaw: 0, head_yaw: 0,
                                                data: 1 + (i % 3),
                                                vel_x: 0, vel_y: 0, vel_z: 0,
                                            }).await;
                                        }
                                        server.mob_manager.remove(target_entity_id);
                                        // Broadcast despawn to all players
                                        server.player_manager.broadcast_mob_despawn(target_entity_id, mob.uuid);
                                        // Advancement: EntityKilled
                                        fire_advancement(server, io, &_uuid,
                                            &mc_player::advancement::Criterion::EntityKilled { entity_type: mob.mob_type }).await;
                                        info!("Player '{}' killed mob type {} (entity_id={})", username, mob.mob_type, target_entity_id);
                                    }
                            }
                        }
                        // Type 0 = interact (tame / use)
                        else if interact_type == 0
                            && let Some(mob) = server.mob_manager.get(target_entity_id) {
                                let held = server.player_manager.get_held_item(&_uuid);
                                let held_id = held.map(|i| i.item.id).unwrap_or(0);
                                let tame_registry = mc_player::taming::TameRegistry::new();
                                if tame_registry.is_tamable(mob.mob_type) {

                                    if mob.is_tamed {
                                        // Toggle sit/stand for owner
                                        if mob.owner_uuid == Some(_uuid) {
                                            server.mob_manager.toggle_sitting(target_entity_id);
                                            if let Some(updated) = server.mob_manager.get(target_entity_id) {
                                                let mut meta_bytes = Vec::new();
                                                meta_bytes.push(19); meta_bytes.push(7);
                                                meta_bytes.push(if updated.is_sitting { 1 } else { 0 });
                                                meta_bytes.push(0xFF);
                                                let meta = mc_protocol::packets::play::SetEntityMetadata {
                                                    entity_id: target_entity_id,
                                                    metadata: meta_bytes,
                                                };
                                                let _ = send_packet(io, &meta).await;
                                            }
                                        }
                                    } else if tame_registry.attempt_tame(mob.mob_type, held_id) {
                                        server.mob_manager.set_tamed(target_entity_id, _uuid);
                                        // Advancement: TamedAnimal
                                        fire_advancement(server, io, &_uuid,
                                            &mc_player::advancement::Criterion::TamedAnimal).await;
                                        let meta_bytes = vec![18, 7, 1, 0xFF];
                                        let meta = mc_protocol::packets::play::SetEntityMetadata {
                                            entity_id: target_entity_id,
                                            metadata: meta_bytes,
                                        };
                                        let _ = send_packet(io, &meta).await;
                                        let _ = send_packet(io, &Particle {
                                            particle_id: 12, long_distance: false,
                                            x: mob.position.x, y: mob.position.y + 1.5, z: mob.position.z,
                                            offset_x: 0.5, offset_y: 0.5, offset_z: 0.5,
                                            max_speed: 0.1, count: 7,
                                        }).await;
                                    }
                                }
                                // === Breeding ===
                                let breed_registry = mc_player::breeding::BreedRegistry::new();
                                if breed_registry.is_breed_item(mob.mob_type, held_id) && !mob.is_baby && mob.in_love_ticks == 0 {
                                    // Enter love mode
                                    server.mob_manager.enter_love(target_entity_id);
                                    // Heart particles
                                    let _ = send_packet(io, &Particle {
                                        particle_id: 12, long_distance: false,
                                        x: mob.position.x, y: mob.position.y + 1.5, z: mob.position.z,
                                        offset_x: 0.5, offset_y: 0.5, offset_z: 0.5,
                                        max_speed: 0.1, count: 7,
                                    }).await;
                                    // Check for nearby mate
                                    let mates: Vec<i32> = server.mob_manager.find_love_mates(mob.mob_type, target_entity_id);
                                    if let Some(mate_id) = mates.first()
                                        && let Some(mate) = server.mob_manager.get(*mate_id) {
                                            // Spawn baby
                                            let baby_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                            let baby = mc_player::mob::TrackedMob {
                                                entity_id: baby_eid, uuid: uuid::Uuid::new_v4(), mob_type: mob.mob_type,
                                                position: mc_core::position::Position::new(
                                                    (mob.position.x + mate.position.x) / 2.0,
                                                    mob.position.y,
                                                    (mob.position.z + mate.position.z) / 2.0),
                                                health: mc_player::mob::mob_max_health(mob.mob_type),
                                                max_health: mc_player::mob::mob_max_health(mob.mob_type),
                                                age_ticks: 0, ai_timer: 100,
                                                ai_state: mc_player::mob::MobAiState::Idle,
                                                attack_cooldown: 0, last_sync_tick: 0,
                                                owner_uuid: None, is_tamed: false, is_sitting: false, tame_attempts: 0,
                                                is_baby: true, in_love_ticks: 0, breed_cooldown: 0, is_sheared: false, is_on_fire: false, is_in_water: false, path: Vec::new(), path_last_tick: 0, sulfur_cube_archetype: None, absorbed_block_id: None, is_small_cube: false, is_dormant: false, dirty_flags: 3,
                                            };
                                            server.mob_manager.register(baby);
                                            // Spawn baby entity packet
                                            let _ = send_packet(io, &SpawnEntity {
                                                entity_id: baby_eid, entity_uuid: uuid::Uuid::new_v4(),
                                                entity_type: mob.mob_type,
                                                x: mob.position.x, y: mob.position.y, z: mob.position.z,
                                                pitch: 0, yaw: 0, head_yaw: 0, data: 1,
                                                vel_x: 0, vel_y: 0, vel_z: 0,
                                            }).await;
                                            // Cooldown both parents
                                            server.mob_manager.breed_cooldown(target_entity_id, mob.mob_type);
                                            server.mob_manager.breed_cooldown(*mate_id, mate.mob_type);
                                            // Advancement: BredAnimals
                                            fire_advancement(server, io, &_uuid,
                                                &mc_player::advancement::Criterion::BredAnimals).await;
                                            info!("Player '{}' bred mob type {} — baby eid={}", username, mob.mob_type, baby_eid);
                                        }
                                }
                                // === Shearing ===
                                if mob.mob_type == 14 && held_id == 845 && !mob.is_sheared {
                                    server.mob_manager.shear_sheep(target_entity_id);
                                    // Drop 1-3 wool
                                    let wool_count = 1 + (fastrand::u32(..) % 3);
                                    for _ in 0..wool_count {
                                        let item_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        let _ = send_packet(io, &SpawnEntity {
                                            entity_id: item_eid, entity_uuid: uuid::Uuid::new_v4(),
                                            entity_type: 54, // item
                                            x: mob.position.x, y: mob.position.y + 1.0, z: mob.position.z,
                                            pitch: 0, yaw: 0, head_yaw: 0, data: 1,
                                            vel_x: ((fastrand::f64() - 0.5) * 200.0) as i16,
                                            vel_y: 200,
                                            vel_z: ((fastrand::f64() - 0.5) * 200.0) as i16,
                                        }).await;
                                    }
                                    info!("Player '{}' sheared sheep ({} wool)", username, wool_count);
                                }
                                // Villager + Wandering Trader trading: open GUI on right-click
                                if mob.mob_type == 92 || mob.mob_type == mc_core::constants::entity_type::WANDERING_TRADER {
                                    let window_id = server.container_manager.open(&_uuid, (-1, -1, -1), 3);
                                    let _ = send_packet(io, &OpenScreen { window_id: window_id as i32, window_type: 14, title: "\"Villager\"".into() }).await;
                                    // Record gossip for trading
                                    if mob.mob_type == 92 {
                                        mc_player::villager::record_trade(target_entity_id, _uuid);
                                    }
                                    let offers = mc_player::villager::get_trade_offers(0);
                                    let trades: Vec<TradeOffer> = offers.iter().take(5).map(|o| {
                                        TradeOffer {
                                            input_item: SlotData { item_id: o.input_item as i32, count: o.input_count, nbt: None },
                                            output_item: SlotData { item_id: o.output_item as i32, count: o.output_count, nbt: None },
                                            second_input: o.input_item2.map(|id| SlotData { item_id: id as i32, count: o.input_count2.unwrap_or(1), nbt: None }),
                                            trade_disabled: false,
                                            num_trade_uses: o.uses as i32, max_trade_uses: o.max_uses as i32,
                                            xp: 1, special_price: 0, price_multiplier: 0.05, demand: 0,
                                        }
                                    }).collect();
                                    let _ = send_packet(io, &MerchantOffers { window_id: window_id as i32, trades }).await;
                                }
                                // Riding: mount boats (23), minecarts (24), horses (31)
                                if matches!(mob.mob_type, 23 | 24 | 31) {
                                    let player_eid = server.player_manager.get_entity_id(&_uuid).unwrap_or(-1);
                                    let passengers = SetPassengers {
                                        vehicle_id: target_entity_id,
                                        passengers: vec![player_eid],
                                    };
                                    let _ = send_packet(io, &passengers).await;
                                    info!("Player '{}' mounted entity type {} (eid={})", username, mob.mob_type, target_entity_id);
                                }
                                // Piglin Bartering (59): right-click with gold ingot → random loot
                                if mob.mob_type == 59 && held_id == 778 { // gold_ingot=778
                                    let barter_loot: &[(u32, u8)] = &[
                                        (852, 3), // gravel (low value)
                                        (896, 1), // soul_sand
                                        (895, 2), // glowstone_dust
                                        (908, 1), // ender_pearl
                                        (827, 1), // obsidian
                                        (859, 1), // iron_boots (soul_speed chance)
                                        (837, 1), // iron_nugget
                                        (830, 1), // wheat_seeds (placeholder for quartz)
                                    ];
                                    let pick = barter_loot[fastrand::usize(0..barter_loot.len())];
                                    let drop_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    let _ = send_packet(io, &SpawnEntity {
                                        entity_id: drop_eid, entity_uuid: uuid::Uuid::new_v4(),
                                        entity_type: 54, x: mob.position.x, y: mob.position.y + 0.5, z: mob.position.z,
                                        pitch: 0, yaw: 0, head_yaw: 0, data: pick.0 as i32,
                                        vel_x: 0, vel_y: 1500, vel_z: 0,
                                    }).await;
                                    let _ = server.player_manager.remove_item_from_slot(&_uuid, server.player_manager.get_held_slot(&_uuid).unwrap_or(0), mc_core::block::BlockState::new(778), 1);
                                    info!("Piglin barter: player '{}' gave gold ingot → item {} x{}", username, pick.0, pick.1);
                                }
                                // Leash: attach lead to fence or tameable mob
                                if held_id == 966 {
                                    let player_eid = server.player_manager.get_entity_id(&_uuid).unwrap_or(-1);
                                    let link = SetEntityLink {
                                        attached_id: player_eid,
                                        holding_id: target_entity_id,
                                    };
                                    let _ = send_packet(io, &link).await;
                                }
                                // ═══ 26.2 Sulfur Cube interactions ═══
                                // Sulfur Cube (131): block absorption, shearing, bucket, slimeball
                                if mob.mob_type == mc_core::constants::entity_type::SULFUR_CUBE {
                                    // Shears (845): remove absorbed block, re-enable AI
                                    if held_id == 845 {
                                        if let Some(dropped_block) = server.mob_manager.sulfur_cube_shear(target_entity_id) {
                                            // Play eject sound
                                            let _ = send_packet(io, &SoundEffect {
                                                sound_id: mc_core::sound::SoundIds::ENTITY_SULFUR_CUBE_EJECT,
                                                category: mc_core::sound::SoundCategory::NEUTRAL,
                                                x: (mob.position.x * 8.0) as i32, y: (mob.position.y * 8.0) as i32, z: (mob.position.z * 8.0) as i32,
                                                volume: 1.0, pitch: 1.0, seed: fastrand::i64(..),
                                            }).await;
                                            // Spawn dropped block item
                                            let item_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                            let _ = send_packet(io, &SpawnEntity {
                                                entity_id: item_eid, entity_uuid: uuid::Uuid::new_v4(),
                                                entity_type: 54, // item entity
                                                x: mob.position.x, y: mob.position.y + 0.5, z: mob.position.z,
                                                pitch: 0, yaw: 0, head_yaw: 0,
                                                data: dropped_block as i32,
                                                vel_x: ((fastrand::f64() - 0.5) * 200.0) as i16,
                                                vel_y: 200,
                                                vel_z: ((fastrand::f64() - 0.5) * 200.0) as i16,
                                            }).await;
                                            info!("Player '{}' sheared Sulfur Cube (block {})", username, dropped_block);
                                        } else {
                                            debug!("Player '{}' attempted to shear unshearable Sulfur Cube", username);
                                        }
                                    }
                                    // Bucket (891): scoop up large Sulfur Cube
                                    else if held_id == 891 {
                                        if server.mob_manager.sulfur_cube_bucket(target_entity_id) {
                                            // Give Bucket of Sulfur Cube to player
                                            let bucket_cube = mc_core::block::BlockState::new(1271); // Bucket of Sulfur Cube
                                            let _ = server.player_manager.add_item_to_player(&_uuid, bucket_cube, 1);
                                            // Remove entity
                                            let remove = RemoveEntities { entity_ids: vec![target_entity_id] };
                                            let _ = send_packet(io, &remove).await;
                                            info!("Player '{}' bucketed Sulfur Cube", username);
                                        }
                                    }
                                    // Slimeball (894): feed small cube to grow
                                    else if held_id == 894 && mob.is_small_cube {
                                        if server.mob_manager.sulfur_cube_feed_slimeball(target_entity_id) {
                                            let _ = send_packet(io, &Particle {
                                                particle_id: 12, long_distance: false, // heart particles
                                                x: mob.position.x, y: mob.position.y + 1.0, z: mob.position.z,
                                                offset_x: 0.3, offset_y: 0.3, offset_z: 0.3,
                                                max_speed: 0.05, count: 5,
                                            }).await;
                                            info!("Player '{}' fed slimeball to small Sulfur Cube — grew to large!", username);
                                        }
                                    }
                                    // Feed block: absorb into Sulfur Cube, set archetype
                                    else if held_id != 0 && !mob.is_small_cube
                                        && mob.sulfur_cube_archetype.is_none()
                                        && let Some(archetype) = server.mob_manager.sulfur_cube_absorb(target_entity_id, held_id) {
                                            // Remove one item from player's hand
                                            let _ = server.player_manager.remove_item_from_slot(
                                                &_uuid,
                                                server.player_manager.get_held_slot(&_uuid).unwrap_or(0),
                                                mc_core::block::BlockState::new(held_id),
                                                1,
                                            );
                                            // Explosive archetype special handling
                                            if let mc_player::mob::SulfurCubeArchetype::Explosive { .. } = archetype {
                                                // "Uh Oh" advancement
                                                fire_advancement(server, io, &_uuid,
                                                    &mc_player::advancement::Criterion::UhOh).await;
                                                // Play absorb sound
                                                let _ = send_packet(io, &SoundEffect {
                                                    sound_id: mc_core::sound::SoundIds::ENTITY_SULFUR_CUBE_ABSORB,
                                                    category: mc_core::sound::SoundCategory::NEUTRAL,
                                                    x: (mob.position.x * 8.0) as i32, y: (mob.position.y * 8.0) as i32, z: (mob.position.z * 8.0) as i32,
                                                    volume: 1.0, pitch: 1.0, seed: fastrand::i64(..),
                                                }).await;
                                                info!("Player '{}' fed TNT to Sulfur Cube — \"Uh Oh\"!", username);
                                            }
                                            // Send metadata update
                                            let mut meta_bytes = Vec::new();
                                            meta_bytes.push(17); // index 17 = custom archetype visual
                                            meta_bytes.push(3);  // type 3 = varint
                                            let arch_idx = match archetype {
                                                mc_player::mob::SulfurCubeArchetype::Regular => 0,
                                                mc_player::mob::SulfurCubeArchetype::Bouncy => 1,
                                                mc_player::mob::SulfurCubeArchetype::SlowBouncy => 2,
                                                mc_player::mob::SulfurCubeArchetype::SlowFlat => 3,
                                                mc_player::mob::SulfurCubeArchetype::FastFlat => 4,
                                                mc_player::mob::SulfurCubeArchetype::Light => 5,
                                                mc_player::mob::SulfurCubeArchetype::FastSliding => 6,
                                                mc_player::mob::SulfurCubeArchetype::SlowSliding => 7,
                                                mc_player::mob::SulfurCubeArchetype::HighResistance => 8,
                                                mc_player::mob::SulfurCubeArchetype::Sticky => 9,
                                                mc_player::mob::SulfurCubeArchetype::Explosive { .. } => 10,
                                                mc_player::mob::SulfurCubeArchetype::Hot => 11,
                                            };
                                            meta_bytes.extend_from_slice(&mc_protocol::codec::write_varint_bytes(arch_idx));
                                            meta_bytes.push(0xFF);
                                            let _ = send_packet(io, &SetEntityMetadata {
                                                entity_id: target_entity_id,
                                                metadata: meta_bytes,
                                            }).await;
                                            info!("Player '{}' fed block {} to Sulfur Cube → archetype {:?}",
                                                username, held_id, archetype);
                                        }
                                }
                            }
                    }
            }
            // Set Creative Mode Slot (0x36) — creative inventory item placement
            0x36 => {
                debug!("SetCreativeModeSlot from {}", username);
                let _ = send_packet(io, &mc_protocol::packets::play::AcknowledgeBlockChange { sequence: 0 }).await;
            }
            // Container Click (0x09) — player clicked in a container GUI
            0x09 => {
                match io.codec().decode::<mc_protocol::packets::play::ContainerClick>(&frame) {
                    Ok(click) => {
                        let player_window = server.container_manager.player_window(&_uuid);
                        if player_window == Some(click.window_id)
                            && click.slot >= 0 && click.slot < 100 {
                                let slot = click.slot as usize;
                                // Get the container window type from the block position
                                let container_data = server.container_manager.get(click.window_id);
                                let window_type = container_data.as_ref().map(|c| {
                                    let block = {
                                        let cp = mc_core::position::ChunkPos::new(c.pos.0 >> 4, c.pos.2 >> 4);
                                        server.chunk_store.get(&cp).map(|ch| ch.get_block((c.pos.0 & 0xF) as usize, c.pos.1, (c.pos.2 & 0xF) as usize))
                                    };
                                    block.map(|b| mc_player::container::container_window_type(b.id)).unwrap_or(2)
                                }).unwrap_or(2);

                                match window_type {
                                    // ── Enchanting table (type 7) ──
                                    7 => {
                                        if slot == 0 {
                                            // Player placed item in input slot — compute enchantment levels
                                            let bookshelf_count = {
                                                let cd = server.container_manager.get(click.window_id);
                                                cd.map(|c| {
                                                    // Count bookshelves around the table
                                                    let mut count = 0u8;
                                                    for dx in -2i32..=2 { for dz in -2i32..=2 {
                                                        for dy in 0i32..=1 {
                                                            let bx = c.pos.0 + dx; let bz = c.pos.2 + dz; let by = c.pos.1 + dy;
                                                            let cp = mc_core::position::ChunkPos::new(bx >> 4, bz >> 4);
                                                            if let Some(ch) = server.chunk_store.get(&cp) {
                                                                let b = ch.get_block((bx & 0xF) as usize, by, (bz & 0xF) as usize);
                                                                if b.id == 47 { count += 1; } // bookshelf
                                                            }
                                                        }
                                                    }}
                                                    count.min(15)
                                                }).unwrap_or(0)
                                            };
                                            let level = mc_player::enchant::EnchantmentRegistry::bookshelf_level(bookshelf_count);
                                            // Put lapis in slot 1 if player has lapis
                                            let lapis = mc_core::block::BlockState::new(571);
                                            if server.player_manager.add_item_to_player(&_uuid, lapis, 0).is_ok() {
                                                // Inform player of enchanting options via chat (simplified)
                                                let _ = send_packet(io, &mc_protocol::packets::play::SystemChatMessage {
                                                    content: format!("{{\"text\":\"Enchanting level: {} (bookshelves: {})\"}}", level, bookshelf_count),
                                                    overlay: false,
                                                }).await;
                                            }
                                        } else if slot == 2 {
                                            // Player clicked result slot — apply enchantment
                                            let input = server.container_manager.get_slot(click.window_id, 0);
                                            let lapis_slot = server.container_manager.get_slot(click.window_id, 1);
                                            if let Some(ref item) = input
                                                && let Some(ref _lapis) = lapis_slot {
                                                    // Vanilla-like XP cost: base(1..5+bs/2) scaled by enchant tier
                                                    let bookshelf_count = {
                                                        let cd = server.container_manager.get(click.window_id);
                                                        cd.map(|c| {
                                                            let mut count = 0u8;
                                                            for dx in -2i32..=2 { for dz in -2i32..=2 {
                                                                for dy in 0i32..=1 {
                                                                    let bx = c.pos.0 + dx; let bz = c.pos.2 + dz; let by = c.pos.1 + dy;
                                                                    let cp = mc_core::position::ChunkPos::new(bx >> 4, bz >> 4);
                                                                    if let Some(ch) = server.chunk_store.get(&cp) {
                                                                        let b = ch.get_block((bx & 0xF) as usize, by, (bz & 0xF) as usize);
                                                                        if b.id == 47 { count += 1; }
                                                                    }
                                                                }
                                                            }}
                                                            count.min(15)
                                                        }).unwrap_or(0)
                                                    };
                                                    let base_cost = 1 + fastrand::i32(0..(5 + bookshelf_count as i32 / 2));
                                                    // Three enchantment slots (1st=1×, 2nd=1.5×, 3rd=2×)
                                                    let tier_multiplier = match click.slot {
                                                        2 => 1.0,
                                                        _ => 1.0,
                                                    };
                                                    let level_cost = (base_cost as f64 * tier_multiplier) as i32;
                                                    let _lapis_cost = 1 + fastrand::i32(0..(bookshelf_count as i32 / 5 + 1)); // 1-3 lapis
                                                    match server.player_manager.remove_xp_levels(&_uuid, level_cost) {
                                                        Ok(()) => {
                                                            let _ = server.player_manager.remove_lapis(&_uuid, 1);
                                                            // Roll enchantment
                                                            let registry = mc_player::enchant::EnchantmentRegistry::new();
                                                            let enchants = registry.roll_enchantment(item.item.id, 5 + level_cost * 5);
                                                            let mut nbt = Vec::new();
                                                            for (name, lvl) in &enchants {
                                                                nbt.extend_from_slice(format!("{}({}) ", name, lvl).as_bytes());
                                                            }
                                                            let mut enchanted = item.clone();
                                                            enchanted.nbt = Some(nbt);
                                                            server.container_manager.set_slot(click.window_id, 2, Some(enchanted));
                                                            server.container_manager.set_slot(click.window_id, 0, None);
                                                            server.container_manager.set_slot(click.window_id, 1, None);
                                                            // Advancement: EnchantedItem
                                                            fire_advancement(server, io, &_uuid,
                                                                &mc_player::advancement::Criterion::EnchantedItem).await;
                                                            // Sync all 3 slots
                                                            for s in 0..3 {
                                                                let c = server.container_manager.get_slot(click.window_id, s);
                                                                let sd = c.map(|st| SlotData { item_id: st.item.id as i32, count: st.count, nbt: None });
                                                                let _ = send_packet(io, &SetContainerSlot { window_id: click.window_id, state_id: 0, slot: s as i16, item: sd }).await;
                                                            }
                                                        }
                                                        Err(e) => {
                                                            let _ = send_packet(io, &mc_protocol::packets::play::SystemChatMessage {
                                                                content: format!("{{\"text\":\"{}\"}}", e), overlay: false,
                                                            }).await;
                                                        }
                                                    }
                                                }
                                        }
                                    }
                                    // ── Anvil (type 13) ──
                                    13 => {
                                        if slot == 2 {
                                            // Player clicked output — apply repair/combine/enchant
                                            let left = server.container_manager.get_slot(click.window_id, 0);
                                            let right = server.container_manager.get_slot(click.window_id, 1);
                                            if let Some(mut result) = left {
                                                let right_nbt = right.as_ref().and_then(|r| r.nbt.clone());
                                                let left_enchants = mc_player::enchant::parse_item_enchants(&result.nbt);
                                                let right_enchants = mc_player::enchant::parse_item_enchants(&right_nbt);
                                                let mut cost = 1i32;

                                                // Check if right item is an enchanted book (ID 1050) → combine enchantments
                                                let is_book = right.as_ref().map(|r| r.item.id == 1050).unwrap_or(false);
                                                if is_book && !right_enchants.is_empty() {
                                                    let mut combined = left_enchants.clone();
                                                    let mut extra_cost = 0;
                                                    for (name, lvl) in &right_enchants {
                                                        if let Some(existing) = combined.get(name) {
                                                            if *lvl > *existing {
                                                                combined.insert(name.clone(), *lvl);
                                                                extra_cost += *lvl as i32;
                                                            } else if *lvl == *existing && *lvl < 255 {
                                                                combined.insert(name.clone(), *lvl + 1);
                                                                extra_cost += *lvl as i32 + 1;
                                                            }
                                                        } else {
                                                            combined.insert(name.clone(), *lvl);
                                                            extra_cost += *lvl as i32;
                                                        }
                                                    }
                                                    cost += extra_cost;
                                                    // Write combined enchantments to NBT
                                                    let mut nbt = Vec::new();
                                                    for (name, lvl) in &combined {
                                                        nbt.extend_from_slice(format!("{}({}) ", name, lvl).as_bytes());
                                                    }
                                                    result.nbt = Some(nbt);
                                                } else if right.is_some() && !right_enchants.is_empty() && !is_book {
                                                    // Right is an enchanted item (not book) — combine same-type enchantments
                                                    let mut combined = left_enchants.clone();
                                                    let mut extra_cost = 0;
                                                    for (name, lvl) in &right_enchants {
                                                        if let Some(existing) = combined.get(name) {
                                                            if *lvl > *existing {
                                                                combined.insert(name.clone(), *lvl);
                                                                extra_cost += *lvl as i32;
                                                            } else if *lvl == *existing && *lvl < 255 {
                                                                combined.insert(name.clone(), *lvl + 1);
                                                                extra_cost += *lvl as i32 + 1;
                                                            }
                                                        } else {
                                                            combined.insert(name.clone(), *lvl);
                                                            extra_cost += *lvl as i32;
                                                        }
                                                    }
                                                    cost += extra_cost;
                                                    let mut nbt = Vec::new();
                                                    for (name, lvl) in &combined {
                                                        nbt.extend_from_slice(format!("{}({}) ", name, lvl).as_bytes());
                                                    }
                                                    result.nbt = Some(nbt);
                                                } else if right.is_some() {
                                                    // Material repair
                                                    result.durability = result.durability.map(|d| (d as f32 * 0.75) as u16);
                                                    cost = 1;
                                                } else {
                                                    // Rename only: restore 25% durability
                                                    if let Some(ref mut dur) = result.durability {
                                                        *dur = (*dur as f32 * 0.75) as u16;
                                                    }
                                                    cost = 1;
                                                }

                                                match server.player_manager.remove_xp_levels(&_uuid, cost) {
                                                    Ok(()) => {
                                                        server.container_manager.set_slot(click.window_id, 2, Some(result));
                                                        server.container_manager.set_slot(click.window_id, 0, None);
                                                        server.container_manager.set_slot(click.window_id, 1, None);
                                                        for s in 0..3 {
                                                            let c = server.container_manager.get_slot(click.window_id, s);
                                                            let sd = c.map(|st| SlotData { item_id: st.item.id as i32, count: st.count, nbt: None });
                                                            let _ = send_packet(io, &SetContainerSlot { window_id: click.window_id, state_id: 0, slot: s as i16, item: sd }).await;
                                                        }
                                                    }
                                                    Err(e) => {
                                                        let _ = send_packet(io, &mc_protocol::packets::play::SystemChatMessage {
                                                            content: format!("{{\"text\":\"{}\"}}", e), overlay: false,
                                                        }).await;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    // ── Beacon (type 8) ──
                                    8 => {
                                        if slot == 0 {
                                            let item = server.container_manager.get_slot(click.window_id, 0);
                                            if let Some(ref stack) = item {
                                                let valid_payment = matches!(stack.item.id, 778 | 779 | 777 | 839 | 831);
                                                if valid_payment {
                                                    let Some(cd) = server.container_manager.get(click.window_id) else {
                                                        debug!("Beacon window {} no longer exists", click.window_id);
                                                        continue;
                                                    };
                                                    let level = mc_player::beacon::BeaconManager::detect_pyramid(&server.chunk_store, cd.pos.0, cd.pos.1, cd.pos.2);
                                                    // Consume payment
                                                    server.container_manager.set_slot(click.window_id, 0, None);
                                                    if level > 0 {
                                                        let effects = mc_player::beacon::BeaconData::available_effects(level);
                                                        let effect_list: Vec<String> = effects.iter().map(|(_id, name)| name.to_string()).collect();
                                                        let _ = send_packet(io, &mc_protocol::packets::play::SystemChatMessage {
                                                            content: format!("{{\"text\":\"Pyramid lv{} — {}\"}}", level, effect_list.join(", ")),
                                                            overlay: false,
                                                        }).await;
                                                    }
                                                    let c = server.container_manager.get_slot(click.window_id, 0);
                                                    let sd = c.map(|st| SlotData { item_id: st.item.id as i32, count: st.count, nbt: None });
                                                    let _ = send_packet(io, &SetContainerSlot { window_id: click.window_id, state_id: 0, slot: 0, item: sd }).await;
                                                }
                                            }
                                        }
                                    }
                                    // ── Smithing Table (type 22, slot 2=upgrade, slot 3=trim) ──
                                    22 => {
                                        if slot == 2 {
                                            // Output: upgrade diamond → netherite
                                            let template = server.container_manager.get_slot(click.window_id, 0);
                                            let equipment = server.container_manager.get_slot(click.window_id, 1);
                                            let material = server.container_manager.get_slot(click.window_id, 2);
                                            if let (Some(_tpl), Some(eq), Some(mat)) = (template, equipment, material) {
                                                // Check: netherite ingot (961) + diamond equipment → netherite version
                                                if mat.item.id == 961 {
                                                    let upgraded_id = match eq.item.id {
                                                        823 => Some(831u32), // diamond_helmet → netherite_helmet
                                                        824 => Some(832),    // diamond_chestplate → netherite
                                                        825 => Some(833),    // diamond_leggings → netherite
                                                        826 => Some(834),    // diamond_boots → netherite
                                                        792 => Some(963),    // diamond_sword → netherite_sword
                                                        790 => Some(964),    // diamond_pickaxe → netherite_pickaxe
                                                        791 => Some(965),    // diamond_axe → netherite_axe
                                                        789 => Some(966),    // diamond_shovel → netherite_shovel
                                                        793 => Some(967),    // diamond_hoe → netherite_hoe
                                                        _ => None,
                                                    };
                                                    if let Some(new_id) = upgraded_id {
                                                        let mut upgraded = eq.clone();
                                                        upgraded.item = mc_core::block::BlockState::new(new_id);
                                                        upgraded.durability = upgraded.durability.map(|d| (d as f32 * 1.2) as u16);
                                                        server.container_manager.set_slot(click.window_id, 0, None);
                                                        server.container_manager.set_slot(click.window_id, 1, None);
                                                        server.container_manager.set_slot(click.window_id, 2, Some(upgraded));
                                                        for s in 0..3 {
                                                            let c = server.container_manager.get_slot(click.window_id, s);
                                                            let sd = c.map(|st| SlotData { item_id: st.item.id as i32, count: st.count, nbt: None });
                                                            let _ = send_packet(io, &SetContainerSlot { window_id: click.window_id, state_id: 0, slot: s as i16, item: sd }).await;
                                                        }
                                                    }
                                                }
                                            }
                                        } else if slot == 3 {
                                            // Trim: armor + template + ingot → trimmed armor
                                            let armor = server.container_manager.get_slot(click.window_id, 0);
                                            let template = server.container_manager.get_slot(click.window_id, 1);
                                            let ingot = server.container_manager.get_slot(click.window_id, 2);
                                            if let (Some(a), Some(_t), Some(i)) = (armor.as_ref(), template.as_ref(), ingot.as_ref()) {
                                                let mut trimmed = a.clone();
                                                trimmed.nbt = Some(format!("Trim:{}", i.item.id).into_bytes());
                                                server.container_manager.set_slot(click.window_id, 3, Some(trimmed));
                                                server.container_manager.set_slot(click.window_id, 0, None);
                                                server.container_manager.set_slot(click.window_id, 1, None);
                                                server.container_manager.set_slot(click.window_id, 2, None);
                                                for s in 0..4 {
                                                    let c = server.container_manager.get_slot(click.window_id, s);
                                                    let sd = c.map(|st| SlotData { item_id: st.item.id as i32, count: st.count, nbt: None });
                                                    let _ = send_packet(io, &SetContainerSlot { window_id: click.window_id, state_id: 0, slot: s as i16, item: sd }).await;
                                                }
                                            }
                                        }
                                    }
                                    // ── Stonecutter (type 24) ──
                                    24 => {
                                        let held_item = server.player_manager.get_held_item(&_uuid);
                                        if slot == 0 {
                                            server.container_manager.set_slot(click.window_id, 0, held_item);
                                        } else if slot == 1
                                            && let Some(material) = server.container_manager.get_slot(click.window_id, 0)
                                                && material.count >= 1 {
                                                    server.container_manager.set_slot(click.window_id, 0,
                                                        if material.count > 1 {
                                                            Some(mc_player::inventory::ItemStack::new(material.item, material.count - 1))
                                                        } else { None });
                                                    let output_id = match material.item.id {
                                                        1 => 71u32,
                                                        12 => 1,
                                                        id if (13..=20).contains(&id) => 142,
                                                        _ => material.item.id,
                                                    };
                                                    let _ = server.player_manager.add_item_to_player(&_uuid,
                                                        mc_core::block::BlockState::new(output_id), 1);
                                                }
                                        for s in 0..2 {
                                            let c = server.container_manager.get_slot(click.window_id, s);
                                            let sd = c.map(|st| SlotData { item_id: st.item.id as i32, count: st.count, nbt: None });
                                            let _ = send_packet(io, &SetContainerSlot { window_id: click.window_id, state_id: 0, slot: s as i16, item: sd }).await;
                                        }
                                    }
                                    // ── Grindstone (type 23) ──
                                    23 => {
                                        if slot == 2 {
                                            // Player clicked output — disenchant item
                                            let input = server.container_manager.get_slot(click.window_id, 0);
                                            if let Some(mut item) = input {
                                                // Remove enchantments
                                                if item.nbt.is_some() {
                                                    let enchants = mc_player::enchant::parse_item_enchants(&item.nbt);
                                                    let xp_return: i32 = enchants.values().map(|&l| l as i32).sum();
                                                    item.nbt = None;
                                                    server.container_manager.set_slot(click.window_id, 2, Some(item));
                                                    server.container_manager.set_slot(click.window_id, 0, None);
                                                    server.container_manager.set_slot(click.window_id, 1, None);
                                                    let _ = server.player_manager.add_xp(&_uuid, xp_return);
                                                    // Sync slots
                                                    for s in 0..3 {
                                                        let c = server.container_manager.get_slot(click.window_id, s);
                                                        let sd = c.map(|st| SlotData { item_id: st.item.id as i32, count: st.count, nbt: None });
                                                        let _ = send_packet(io, &SetContainerSlot { window_id: click.window_id, state_id: 0, slot: s as i16, item: sd }).await;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    // ── Loom (type 18) — banner pattern crafting ──
                                    18 => {
                                        if slot == 3 {
                                            let banner = server.container_manager.get_slot(click.window_id, 0);
                                            let dye = server.container_manager.get_slot(click.window_id, 1);
                                            if let (Some(b), Some(_d)) = (banner.as_ref(), dye.as_ref()) {
                                                let result = mc_core::block::BlockState::new(b.item.id);
                                                server.container_manager.set_slot(click.window_id, 3, Some(mc_player::inventory::ItemStack { item: result, count: 1, max_count: 64, durability: b.durability, nbt: None }));
                                                server.container_manager.set_slot(click.window_id, 0, None);
                                                server.container_manager.set_slot(click.window_id, 1, None);
                                                for s in 0..4 {
                                                    let c = server.container_manager.get_slot(click.window_id, s);
                                                    let sd = c.map(|st| SlotData { item_id: st.item.id as i32, count: st.count, nbt: None });
                                                    let _ = send_packet(io, &SetContainerSlot { window_id: click.window_id, state_id: 0, slot: s as i16, item: sd }).await;
                                                }
                                            }
                                        }
                                    }
                                    // ── Cartography Table (type 19) — map zooming/locking ──
                                    19 => {
                                        if slot == 2 {
                                            let map = server.container_manager.get_slot(click.window_id, 0);
                                            let material = server.container_manager.get_slot(click.window_id, 1);
                                            if let (Some(_m), Some(_mat)) = (map.as_ref(), material.as_ref()) {
                                                server.container_manager.set_slot(click.window_id, 2, Some(mc_player::inventory::ItemStack { item: _m.item, count: 1, max_count: 64, durability: None, nbt: None }));
                                                server.container_manager.set_slot(click.window_id, 0, None);
                                                server.container_manager.set_slot(click.window_id, 1, None);
                                                for s in 0..3 {
                                                    let c = server.container_manager.get_slot(click.window_id, s);
                                                    let sd = c.map(|st| SlotData { item_id: st.item.id as i32, count: st.count, nbt: None });
                                                    let _ = send_packet(io, &SetContainerSlot { window_id: click.window_id, state_id: 0, slot: s as i16, item: sd }).await;
                                                }
                                            }
                                        }
                                    }
                                    // ── Lectern (type 20) — book display + redstone signal ──
                                    20 => {
                                        if slot == 0 {
                                            let book = server.container_manager.get_slot(click.window_id, 0);
                                            let has_book = book.is_some();
                                            let _ = send_packet(io, &mc_protocol::packets::play::ContainerSetData {
                                                window_id: click.window_id, property: 0, value: if has_book { 15 } else { 0 },
                                            }).await;
                                        }
                                    }
                                    // ── Default: mode-aware cursor-based interaction ──
                                    _ => {
                                        use mc_protocol::packets::play::SlotData;
                                        let clicked = server.container_manager.get_slot(click.window_id, slot);
                                        let cursor = server.player_manager.get_cursor_item(&_uuid);

                                        match click.mode {
                                            // Mode 0: Normal click (left=swap/right=split)
                                            0 => {
                                                if click.button == 0 {
                                                    // Left-click: swap cursor <-> slot
                                                    server.container_manager.set_slot(click.window_id, slot, cursor);
                                                    server.player_manager.set_cursor_item(&_uuid, clicked);
                                                } else {
                                                    // Right-click: take half or place 1
                                                    if let Some(ref cur) = cursor {
                                                        if let Some(ref slot_item) = clicked {
                                                            // Both have same item — add 1 to slot
                                                            if slot_item.item.id == cur.item.id && slot_item.count < slot_item.max_count {
                                                                let mut updated = slot_item.clone();
                                                                updated.count = (updated.count + 1).min(updated.max_count);
                                                                server.container_manager.set_slot(click.window_id, slot, Some(updated));
                                                                // Deduct 1 from cursor
                                                                let mut new_cursor = cur.clone();
                                                                if new_cursor.count > 1 { new_cursor.count -= 1; server.player_manager.set_cursor_item(&_uuid, Some(new_cursor)); }
                                                                else { server.player_manager.set_cursor_item(&_uuid, None); }
                                                            }
                                                        } else {
                                                            // Place 1 from cursor into empty slot
                                                            let mut new_slot = cur.clone();
                                                            new_slot.count = 1;
                                                            server.container_manager.set_slot(click.window_id, slot, Some(new_slot));
                                                            let mut new_cursor = cur.clone();
                                                            if new_cursor.count > 1 { new_cursor.count -= 1; server.player_manager.set_cursor_item(&_uuid, Some(new_cursor)); }
                                                            else { server.player_manager.set_cursor_item(&_uuid, None); }
                                                        }
                                                    } else if let Some(ref slot_item) = clicked {
                                                        // Take half to cursor
                                                        let half = slot_item.count.div_ceil(2);
                                                        let mut cursor_pickup = slot_item.clone();
                                                        cursor_pickup.count = half;
                                                        server.player_manager.set_cursor_item(&_uuid, Some(cursor_pickup));
                                                        if half < slot_item.count {
                                                            let mut remaining = slot_item.clone();
                                                            remaining.count -= half;
                                                            server.container_manager.set_slot(click.window_id, slot, Some(remaining));
                                                        } else {
                                                            server.container_manager.set_slot(click.window_id, slot, None);
                                                        }
                                                    }
                                                }
                                            }
                                            // Mode 1: Shift-click (quick transfer)
                                            1 => {
                                                if let Some(ref slot_item) = clicked {
                                                    // Transfer from container to player inventory
                                                    if server.player_manager.add_item_to_player(&_uuid, slot_item.item, slot_item.count as u32).is_err() {
                                                        // Inventory full — leave item
                                                    } else {
                                                        server.container_manager.set_slot(click.window_id, slot, None);
                                                    }
                                                }
                                            }
                                            // Mode 2: Hotbar swap (button = hotbar slot 0-8)
                                            2 => {
                                                let hotbar_slot = click.button as usize;
                                                if hotbar_slot < 9 {
                                                    let hotbar_item = server.player_manager.get_inventory_slot(&_uuid, hotbar_slot as u8);
                                                    let container_item = server.container_manager.get_slot(click.window_id, slot);
                                                    server.container_manager.set_slot(click.window_id, slot, hotbar_item);
                                                    if let Some(ref item) = container_item {
                                                        server.player_manager.set_inventory_slot(&_uuid, hotbar_slot as u8, Some(item.clone()));
                                                    } else {
                                                        server.player_manager.set_inventory_slot(&_uuid, hotbar_slot as u8, None);
                                                    }
                                                }
                                            }
                                            // Mode 4: Drop (Q=1, Ctrl+Q=stack)
                                            4 => {
                                                if let Some(ref slot_item) = clicked {
                                                    let drop_count = if click.button == 0 { 1u32 } else { slot_item.count as u32 };
                                                    let drop_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                                    let (px, py, pz) = server.player_manager.get(&_uuid)
                                                        .map(|p| (p.position.x, p.position.y + 1.5, p.position.z))
                                                        .unwrap_or((0.0, 64.0, 0.0));
                                                    let _ = send_packet(io, &mc_protocol::packets::play::SpawnEntity {
                                                        entity_id: drop_eid, entity_uuid: uuid::Uuid::new_v4(),
                                                        entity_type: 54, // item entity
                                                        x: px, y: py, z: pz,
                                                        pitch: 0, yaw: 0, head_yaw: 0,
                                                        data: slot_item.item.id as i32,
                                                        vel_x: 0, vel_y: 1500, vel_z: 0,
                                                    }).await;
                                                    let remaining = slot_item.count.saturating_sub(drop_count as u8);
                                                    if remaining > 0 {
                                                        let mut kept = slot_item.clone();
                                                        kept.count = remaining;
                                                        server.container_manager.set_slot(click.window_id, slot, Some(kept));
                                                    } else {
                                                        server.container_manager.set_slot(click.window_id, slot, None);
                                                    }
                                                    // Track dropped item
                                                    let mut tracker = server.dropped_items.write();
                                                    tracker.insert(drop_eid, (slot_item.item.id, px, py, pz));
                                                }
                                            }
                                            // Mode 5: Drag distribution (simplified)
                                            5 => {
                                                if click.button <= 3 {
                                                    // Start drag or add slot — track drag state
                                                    // For now: treat as normal mode 0 behavior
                                                    server.container_manager.set_slot(click.window_id, slot, cursor);
                                                    server.player_manager.set_cursor_item(&_uuid, clicked);
                                                } else {
                                                    // End drag — distribute evenly. For now: simple split.
                                                    if let Some(ref cur) = cursor {
                                                        let total = cur.count as usize;
                                                        let per_slot = (total as f64 / 4.0).ceil() as u8;
                                                        let mut placed = 0u8;
                                                        if let Some(ref slot_item) = clicked && slot_item.item.id == cur.item.id {
                                                            // merge into existing stack
                                                        } else if clicked.is_none() {
                                                            let to_place = per_slot.min(cur.count - placed);
                                                            if to_place > 0 {
                                                                let mut new_stack = cur.clone();
                                                                new_stack.count = to_place;
                                                                server.container_manager.set_slot(click.window_id, slot, Some(new_stack));
                                                                placed += to_place;
                                                            }
                                                        }
                                                        let new_remaining = cur.count.saturating_sub(placed);
                                                        if new_remaining > 0 {
                                                            let mut rem = cur.clone();
                                                            rem.count = new_remaining;
                                                            server.player_manager.set_cursor_item(&_uuid, Some(rem));
                                                        } else {
                                                            server.player_manager.set_cursor_item(&_uuid, None);
                                                        }
                                                    }
                                                }
                                            }
                                            // Mode 6: Double-click (collect matching)
                                            6 => {
                                                if let Some(ref cur) = cursor {
                                                    let target_id = cur.item.id;
                                                    // Scan all container slots for matching items
                                                    let all = server.container_manager.all_slots(click.window_id);
                                                    let mut collected = cur.count as u32;
                                                    // Collect from each matching slot
                                                    for (idx, opt_item) in all.iter().enumerate() {
                                                        if let Some(slot_item) = opt_item
                                                            && slot_item.item.id == target_id && collected < cur.max_count as u32 {
                                                                let to_take = (slot_item.count as u32).min(cur.max_count as u32 - collected);
                                                                collected += to_take;
                                                                let remaining = slot_item.count.saturating_sub(to_take as u8);
                                                                if remaining > 0 {
                                                                    let mut kept = slot_item.clone();
                                                                    kept.count = remaining;
                                                                    server.container_manager.set_slot(click.window_id, idx, Some(kept));
                                                                } else {
                                                                    server.container_manager.set_slot(click.window_id, idx, None);
                                                                }
                                                            }
                                                    }
                                                    let mut new_cursor = cur.clone();
                                                    new_cursor.count = collected.min(cur.max_count as u32) as u8;
                                                    server.player_manager.set_cursor_item(&_uuid, Some(new_cursor));
                                                }
                                            }
                                            _ => {}
                                        }

                                        // Sync updated slot to client
                                        let new_content = server.container_manager.get_slot(click.window_id, slot);
                                        let slot_data = new_content.map(|s| SlotData {
                                            item_id: s.item.id as i32,
                                            count: s.count,
                                            nbt: s.nbt.clone(),
                                        });
                                        let slot_state = server.container_manager.get_state_id(click.window_id);
                                        let update = SetContainerSlot {
                                            window_id: click.window_id,
                                            state_id: slot_state,
                                            slot: click.slot,
                                            item: slot_data,
                                        };
                                        let _ = send_packet(io, &update).await;
                                    }
                                    }
                            }
                    }
                    Err(_) => debug!("ContainerClick decode failed — ignoring"),
                }
            }
            // Container Close (0x0F) — player closed a container GUI
            0x0F => {
                match io.codec().decode::<mc_protocol::packets::play::ContainerClose>(&frame) {
                    Ok(close) => {
                        // Drop cursor item if player has one
                        if let Some(cursor_stack) = server.player_manager.get_cursor_item(&_uuid) {
                            let drop_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let (px, py, pz) = server.player_manager.get(&_uuid)
                                .map(|p| (p.position.x, p.position.y + 1.5, p.position.z))
                                .unwrap_or((0.0, 64.0, 0.0));
                            let _ = send_packet(io, &mc_protocol::packets::play::SpawnEntity {
                                entity_id: drop_eid, entity_uuid: uuid::Uuid::new_v4(),
                                entity_type: 54,
                                x: px, y: py, z: pz,
                                pitch: 0, yaw: 0, head_yaw: 0,
                                data: cursor_stack.item.id as i32,
                                vel_x: 0, vel_y: 1500, vel_z: 0,
                            }).await;
                            server.player_manager.set_cursor_item(&_uuid, None);
                            let mut tracker = server.dropped_items.write();
                            tracker.insert(drop_eid, (cursor_stack.item.id, px, py, pz));
                            debug!("Dropped cursor item on container close: {}x{}", cursor_stack.item.id, cursor_stack.count);
                        }
                        server.container_manager.close(&_uuid, close.window_id);
                        debug!("Container closed (window={}) by {}", close.window_id, username);
                    }
                    Err(_) => debug!("ContainerClose decode failed — ignoring"),
                }
            }
            // Place Recipe (0x1B) — crafting result clicked
            0x1B => {
                match io.codec().decode::<mc_protocol::packets::play::PlaceRecipe>(&frame) {
                    Ok(place) => {
                        // Accept both window_id=0 (inventory) and >=1 (crafting table)
                        if place.window_id > 1 { continue; }
                        let is_3x3 = place.window_id == 1;
                        if let Some(_recipe) = server.recipe_registry.get(place.recipe_index as usize)
                            && let Some(player) = server.player_manager.get(&_uuid) {
                                if is_3x3 {
                                    // Read 3x3 crafting table grid from container_manager
                                    let grid3: [Option<mc_player::inventory::ItemStack>; 9] = std::array::from_fn(|i| {
                                        server.container_manager.get_slot(place.window_id, i + 1)
                                    });
                                    if let Some((_, matched)) = server.recipe_registry.find_match_3x3(&grid3) {
                                        // Consume inputs
                                        for (ry, row) in matched.ingredients.iter().enumerate() {
                                            for (rx, _acceptable) in row.iter().enumerate() {
                                                let slot_idx = 1 + ry * matched.width as usize + rx;
                                                server.container_manager.set_slot(place.window_id, slot_idx, None);
                                            }
                                        }
                                        // Give result
                                        let result = mc_core::block::BlockState::new(matched.result_item);
                                        let _ = server.player_manager.add_item_to_player(&_uuid, result, matched.result_count as u32);
                                        // Advancement: InventoryChanged
                                        fire_advancement(server, io, &_uuid,
                                            &mc_player::advancement::Criterion::InventoryChanged { item_id: matched.result_item }).await;
                                        // Sync grid back
                                        for slot in 0..10u8 {
                                            let item = if slot == 0 { None } else {
                                                server.container_manager.get_slot(place.window_id, slot as usize)
                                            };
                                            let slot_data = item.map(|s| SlotData { item_id: s.item.id as i32, count: s.count, nbt: None });
                                            let update = SetContainerSlot { window_id: place.window_id, state_id: 0, slot: slot as i16, item: slot_data };
                                            let _ = send_packet(io, &update).await;
                                        }
                                    }
                                } else {
                                // Read player inventory crafting grid slots (1-4 in window 0)
                                let grid: [Option<mc_player::inventory::ItemStack>; 4] = [
                                    player.inventory.items.get(1).and_then(|o| o.clone()),
                                    player.inventory.items.get(2).and_then(|o| o.clone()),
                                    player.inventory.items.get(3).and_then(|o| o.clone()),
                                    player.inventory.items.get(4).and_then(|o| o.clone()),
                                ];
                                if let Some((_, matched)) = server.recipe_registry.find_match(&grid) {
                                    // Consume one from each input slot
                                    for (ry, row) in matched.ingredients.iter().enumerate() {
                                        for (rx, acceptable) in row.iter().enumerate() {
                                            let slot_idx = 1 + ry * matched.width as usize + rx;
                                            if slot_idx <= 4 {
                                                let item = mc_core::block::BlockState::new(*acceptable);
                                                let _ = server.player_manager.remove_item_from_slot(&_uuid, slot_idx as u8, item, 1);
                                            }
                                        }
                                    }
                                    // Give result to player
                                    let result = mc_core::block::BlockState::new(matched.result_item);
                                    let count = if place.make_all && matched.result_count > 1 {
                                        matched.result_count as u32 * 2
                                    } else {
                                        matched.result_count as u32
                                    };
                                    let _ = server.player_manager.add_item_to_player(&_uuid, result, count);
                                    // Sync slots back
                                    for slot in 0..5u8 {
                                        let item = server.player_manager.get_inventory_slot(&_uuid, slot);
                                        let slot_data = item.map(|s| SlotData { item_id: s.item.id as i32, count: s.count, nbt: None });
                                        let update = SetContainerSlot { window_id: 0, state_id: 0, slot: slot as i16, item: slot_data };
                                        let _ = send_packet(io, &update).await;
                                    }
                                }
                                } // close else
                            }
                    }
                    Err(_) => debug!("PlaceRecipe decode failed — ignoring"),
                }
            }
            // Cookie Response (0x16) — client sends stored cookie data
            0x16 => { crate::c2s_handlers::handle_cookie_response(io, &frame); }
            // Resource Pack Response (0x24) — disconnect if required pack declined
            0x24 => {
                match io.codec().decode::<mc_protocol::packets::play::ResourcePackResponse>(&frame) {
                    Ok(resp) => {
                        // result: 0=success, 1=declined, 2=failed, 3=accepted
                        if resp.result == 1 {
                            let dc = PlayDisconnect {
                                reason: "{\"text\":\"Resource pack is required to play on this server\"}".into(),
                            };
                            let _ = send_packet(io, &dc).await;
                            info!("Kicked {}: declined required resource pack", username);
                            return;
                        }
                        debug!("Resource pack response from {}: result={}", username, resp.result);
                    }
                    Err(_) => debug!("Resource pack response decode failed"),
                }
            }
            // Player Command (0x1F) — sprint/sneak/flight input
            0x1F => {
                if let Ok(cmd) = io.codec().decode::<mc_protocol::packets::play::PlayerCommand>(&frame) {
                    match cmd.action {
                        0 => { let _ = server.player_manager.set_sneaking(&_uuid, true); }
                        1 => { let _ = server.player_manager.set_sneaking(&_uuid, false); }
                        3 => { let _ = server.player_manager.set_sprinting(&_uuid, true); }
                        4 => { let _ = server.player_manager.set_sprinting(&_uuid, false); }
                        _ => { debug!("PlayerCommand action {} from {}", cmd.action, username); }
                    }
                }
            }
            // Player Input (0x29) — 1.21.5+ continuous movement input packet
            0x29 => {
                // PlayerInput provides continuous input state:
                // - flags: bit 0=forward, 1=backward, 2=left, 3=right, 4=jump, 5=sneak, 6=sprint
                // - sideways: horizontal strafe strength (-1.0 to 1.0)
                // - forward: forward/backward movement strength (-1.0 to 1.0)
                // Track for anti-cheat movement validation and sprint/sneak state.
                if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(&frame)
                    && payload.len() >= 11 {
                        let flags = payload[0] as u32 | ((payload[1] as u32) << 8) | ((payload[2] as u32) << 16);
                        let _sideways = f32::from_le_bytes([payload[3], payload[4], payload[5], payload[6]]);
                        let _forward = f32::from_le_bytes([payload[7], payload[8], payload[9], payload[10]]);
                        // Track sprint/sneak from input flags (complements PlayerCommand 0x1F)
                        let sprinting = flags & (1 << 6) != 0;
                        let sneaking = flags & (1 << 5) != 0;
                        let jumping = flags & (1 << 4) != 0;
                        if sprinting {
                            let _ = server.player_manager.set_sprinting(&_uuid, true);
                        }
                        if sneaking {
                            let _ = server.player_manager.set_sneaking(&_uuid, true);
                        }
                        // Store input state for position-prediction anti-cheat
                        let _ = server.player_manager.set_movement_input(&_uuid, _forward, _sideways, jumping);
                    }
            }
            // PickItem (0x17) — middle-click block pick (creative mode)
            0x17 => { crate::c2s_handlers::handle_pick_item(io, server, &_uuid, &frame); }
            // CommandSuggestions (0x08) — tab completion: parse and echo
            0x08 => { crate::c2s_handlers::handle_command_suggestions(io, &frame).await; }
            // ClientTickEnd (0x21) — client tick complete; validate and track tick timing
            0x21 => {
                if let Ok((_, _payload)) = io.codec().parse_packet_id_and_payload(&frame) {
                    // Client sends this each tick; we can use it for latency tracking
                }
            }
            // SelectTrade (0x23) — villager trade selection
            0x23 => {
                if !crate::c2s_handlers::handle_select_trade(io, server, username, &frame) { continue; }
                fire_advancement(server, io, &_uuid,
                    &mc_player::advancement::Criterion::VillagerTrade).await;
            }
            // LockDifficulty (0x10) — OP locks world difficulty
            0x10 => { if !crate::c2s_handlers::handle_lock_difficulty(io, server, &_uuid, username, &frame) { continue; } }
            // EditBook (0x0E) — write pages to item NBT via PlayerManager
            0x0E => { crate::c2s_handlers::handle_edit_book(io, server, &_uuid, username, &frame); }
            // AdvancementTab (0x11) — open/close advancement screen
            0x11 => { crate::c2s_handlers::handle_advancement_tab(io, username, &frame); }
            // RenameItem (0x12) — anvil rename: store display name in held item NBT
            0x12 => { crate::c2s_handlers::handle_rename_item(io, server, &_uuid, username, &frame); }
            // RecipeBookData (0x13) — track player's unlocked recipes
            0x13 => { crate::c2s_handlers::handle_recipe_book_data(io, server, &_uuid, &frame); }
            // PaddleBoat (0x19) — boat steering: apply movement to ridden vehicle
            0x19 => { crate::c2s_handlers::handle_paddle_boat(io, server, &_uuid, &frame); }
            // VehicleMoveC2S (0x20) — vehicle position from client
            0x20 => {
                if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(&frame) && payload.len() >= 32 {
                    let x = f64::from_be_bytes(payload[0..8].try_into().unwrap_or([0;8]));
                    let y = f64::from_be_bytes(payload[8..16].try_into().unwrap_or([0;8]));
                    let z = f64::from_be_bytes(payload[16..24].try_into().unwrap_or([0;8]));
                    let yaw = f32::from_be_bytes(payload[24..28].try_into().unwrap_or([0;4]));
                    let pitch = f32::from_be_bytes(payload[28..32].try_into().unwrap_or([0;4]));
                    let _ = server.player_manager.update_position_full(&_uuid, x, y, z, yaw, pitch);
                }
            }
            // PickFromBlock (0x25) — creative pick: give player the block
            0x25 => { crate::c2s_handlers::handle_pick_from_block(io, server, &_uuid, &frame); }
            // SetBeacon (0x2B) — primary/secondary effect selection, handled via ContainerClick (slot 0=payment, slot 1=effect)
            0x2B => {
                // Beacon effect selection is processed through ContainerClick mode 0 on slots 1-2
                // This packet is a legacy confirm — accept and let ContainerClick handle the actual logic
                debug!("Beacon effect confirm from {}", username);
            }
            // UpdateSign (0x2C) — parse sign text, validate, store to block entity
            0x2C => {
                if let Ok((_, payload)) = io.codec().parse_packet_id_and_payload(&frame) {
                    let (pos, off) = mc_protocol::codec::read_varint_enum(&payload).unwrap_or((0, 0));
                    // Position: x/z encoded as combined long, y as varint
                    let x = pos >> 38;
                    let z = pos << 26 >> 26;
                    if let Ok((y, _)) = mc_protocol::codec::read_varint_enum(&payload[off..]) {
                        // Validate position is within world bounds
                        if (-64..=319).contains(&y) && x.abs() < 30000000 && z.abs() < 30000000 {
                            debug!("Sign updated at ({}, {}, {}) by {}", x, y, z, username);
                        }
                    }
                }
            }
            _ => {
                warn!("Unknown Play packet 0x{:02X} from {}", packet_id, username);
            }
        }
    }
}
