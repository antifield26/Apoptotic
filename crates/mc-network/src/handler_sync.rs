//! 玩家状态事件广播处理器
//! 处理 PlayerStateEventKind 的 ~20 种事件类型，向客户端发送对应的协议包。
//! 从 connection.rs play_loop 提取。

use crate::connection::{self, ServerRef};
use crate::packet_io::PacketStream;
use mc_protocol::packets::play::*;
use tracing::info;

/// 处理玩家状态事件队列中的所有待处理事件。
/// 在 play_loop 的每次迭代中调用。
pub async fn handle_player_state_events(
    player_state_rx: &mut tokio::sync::broadcast::Receiver<mc_player::player::PlayerStateEvent>,
    io: &mut PacketStream,
    _uuid: uuid::Uuid,
    username: &str,
    server: &ServerRef,
) {
    while let Ok(ev) = player_state_rx.try_recv() {
        // Only process events for this player, or global (nil UUID) events
        if ev.uuid != _uuid && ev.uuid != uuid::Uuid::nil() {
            continue;
        }
        match &ev.kind {
            mc_player::player::PlayerStateEventKind::HealthUpdate(health) => {
                let (food, saturation) = server.player_manager.get(&_uuid)
                    .map(|p| (p.food_level, p.food_saturation))
                    .unwrap_or((20, 5.0));
                let set_health = SetHealth { health: *health, food, saturation };
                let _ = connection::send_packet(io, &set_health).await;
                if *health <= 0.0 {
                    info!("Player '{}' died — spawning XP orbs", username);
                    let (px, py, pz) = {
                        let pm = server.player_manager.get(&_uuid);
                        pm.map(|p| (p.position.x, p.position.y, p.position.z))
                            .unwrap_or((0.0, 64.0, 0.0))
                    };
                    let orb_count = 3 + (fastrand::u32(..) % 5) as i32;
                    for i in 0..orb_count {
                        let orb_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let offset_x = (fastrand::f64() - 0.5) * 1.5;
                        let offset_z = (fastrand::f64() - 0.5) * 1.5;
                        let spawn = SpawnEntity {
                            entity_id: orb_eid, entity_uuid: uuid::Uuid::new_v4(),
                            entity_type: 53, x: px + offset_x, y: py + 0.5, z: pz + offset_z,
                            pitch: 0, yaw: 0, head_yaw: 0, data: 1 + (i % 3),
                            vel_x: 0, vel_y: 0, vel_z: 0,
                        };
                        let _ = connection::send_packet(io, &spawn).await;
                    }
                }
            }
            mc_player::player::PlayerStateEventKind::GamemodeUpdate(gm) => {
                let event = GameEvent { event: 3, value: gm.id() as f32 };
                let _ = connection::send_packet(io, &event).await;
                let abilities = match gm {
                    mc_core::types::GameMode::Creative => PlayerAbilities::creative(),
                    _ => PlayerAbilities::survival(),
                };
                let _ = connection::send_packet(io, &abilities).await;
            }
            mc_player::player::PlayerStateEventKind::Teleport(x, y, z, yaw, pitch, teleport_id) => {
                let pos = PlayerPosition { teleport_id: *teleport_id, x: *x, y: *y, z: *z, dx: 0.0, dy: 0.0, dz: 0.0, yaw: *yaw, pitch: *pitch, flags: 0 };
                let _ = connection::send_packet(io, &pos).await;
            }
            mc_player::player::PlayerStateEventKind::FoodUpdate(food, saturation) => {
                let health = server.player_manager.get(&_uuid).map(|p| p.health).unwrap_or(20.0);
                let set_health = SetHealth { health, food: *food, saturation: *saturation };
                let _ = connection::send_packet(io, &set_health).await;
            }
            mc_player::player::PlayerStateEventKind::XpUpdate(bar, level, total) => {
                let xp_pkt = SetExperience { experience_bar: *bar, level: *level, total_experience: *total };
                let _ = connection::send_packet(io, &xp_pkt).await;
            }
            mc_player::player::PlayerStateEventKind::Title(action, text) => {
                // 1.21.5 split title packets
                match *action {
                    0 => { let _ = connection::send_packet(io, &SetTitleText { text: text.clone() }).await; }
                    1 => { let _ = connection::send_packet(io, &SetTitleSubtitle { text: text.clone() }).await; }
                    2 => { let _ = connection::send_packet(io, &SetActionBarText { text: text.clone() }).await; }
                    _ => { let _ = connection::send_packet(io, &SetTitleText { text: text.clone() }).await; }
                }
            }
            mc_player::player::PlayerStateEventKind::PlaySound(name, category, volume, pitch) => {
                let (px, py, pz) = server.player_manager.get(&_uuid)
                    .map(|p| (p.position.x, p.position.y, p.position.z))
                    .unwrap_or((0.0, 64.0, 0.0));
                let sound_id = crate::connection::resolve_sound_id(name);
                let snd = SoundEffect { sound_id, category: *category, x: (px * 8.0) as i32, y: (py * 8.0) as i32, z: (pz * 8.0) as i32, volume: *volume, pitch: *pitch, seed: fastrand::i64(..) };
                let _ = connection::send_packet(io, &snd).await;
            }
            mc_player::player::PlayerStateEventKind::StopSound => {
                let stop = StopSound { flags: 0x00, category: None, sound_name: None };
                let _ = connection::send_packet(io, &stop).await;
            }
            mc_player::player::PlayerStateEventKind::ClearInventory => {
                let inv_pkt = ContainerSetContent { window_id: 0, state_id: 0, items: vec![None; 46], carried_item: None };
                let _ = connection::send_packet(io, &inv_pkt).await;
            }
            mc_player::player::PlayerStateEventKind::EnchantHeld(enchant_name, level) => {
                let msg = format!("{{\"text\":\"Enchanted with {} level {}\",\"color\":\"aqua\"}}", enchant_name, level);
                let chat = SystemChatMessage { content: msg, overlay: false };
                let _ = connection::send_packet(io, &chat).await;
            }
            mc_player::player::PlayerStateEventKind::EffectAdd(entity_id, effect_id, amplifier, duration, flags) => {
                let eff = EntityEffect { entity_id: *entity_id, effect_id: *effect_id, amplifier: *amplifier, duration: *duration, flags: *flags };
                let _ = connection::send_packet(io, &eff).await;
            }
            mc_player::player::PlayerStateEventKind::EffectRemove(entity_id, effect_id) => {
                let rem = RemoveEntityEffect { entity_id: *entity_id, effect_id: *effect_id };
                let _ = connection::send_packet(io, &rem).await;
            }
            mc_player::player::PlayerStateEventKind::VillagerTrade(profession_id, entity_id) => {
                let trades = mc_player::villager::get_trade_offers(*profession_id);
                let title = format!("{{\"text\":\"Villager — {} ({})\"}}",
                    mc_player::villager::Profession::from_id(*profession_id).name(), trades.len());
                let open_pkt = OpenScreen { window_id: *entity_id, window_type: 14, title };
                let _ = connection::send_packet(io, &open_pkt).await;
            }
            mc_player::player::PlayerStateEventKind::SwitchDimension(dim_name, x, y, z) => {
                let respawn = Respawn {
                    dimension_type: dim_name.clone(), dimension_name: dim_name.clone(),
                    hashed_seed: server.world_seed as i64,
                    gamemode: server.player_manager.get(&_uuid).map(|p| p.gamemode.id()).unwrap_or(0),
                    previous_gamemode: -1, is_debug: false, is_flat: false,
                    death_location: None, portal_cooldown: 0, data_kept: 0,
                };
                let _ = connection::send_packet(io, &respawn).await;
                let pos = PlayerPosition { teleport_id: 42, x: *x, y: *y, z: *z, dx: 0.0, dy: 0.0, dz: 0.0, yaw: 0.0, pitch: 0.0, flags: 0 };
                let _ = connection::send_packet(io, &pos).await;
            }
            mc_player::player::PlayerStateEventKind::ScoreboardObjective(name, action, display, _criteria) => {
                let objective = ScoreboardObjective { name: name.clone(), mode: *action, objective_value: format!("{{\"text\":\"{}\"}}", display), objective_type: 0, number_format: 0 };
                let _ = connection::send_packet(io, &objective).await;
            }
            mc_player::player::PlayerStateEventKind::ScoreboardScore(entity, obj, score, action) => {
                let update = UpdateScore { entity_name: entity.clone(), objective_name: obj.clone(), value: *score, display_name: if *action == 0 { Some(entity.clone()) } else { None }, number_format: 0 };
                let _ = connection::send_packet(io, &update).await;
            }
            mc_player::player::PlayerStateEventKind::ScoreboardDisplay(position, obj_name) => {
                let display = ScoreboardDisplay { position: *position, score_name: obj_name.clone() };
                let _ = connection::send_packet(io, &display).await;
            }
            mc_player::player::PlayerStateEventKind::TeamUpdate(name, action, display, prefix, suffix, color, friendly_fire, players) => {
                let team = Teams { name: name.clone(), mode: *action, display_name: format!("{{\"text\":\"{}\"}}", display), friendly_fire: if *friendly_fire { 1 } else { 0 }, nametag_visibility: "always".into(), collision_rule: "always".into(), color: color.parse::<i32>().unwrap_or(0), prefix: format!("{{\"text\":\"{}\"}}", prefix), suffix: format!("{{\"text\":\"{}\"}}", suffix), entities: players.clone() };
                let _ = connection::send_packet(io, &team).await;
            }
            mc_player::player::PlayerStateEventKind::BossBarUpdate(id, action, title, health, color, division, flags) => {
                let bar = BossBar { uuid: uuid::Uuid::parse_str(id).unwrap_or(uuid::Uuid::nil()), action: *action as i32, title: if title.is_empty() { None } else { Some(title.clone()) }, health: Some(*health), color: Some(*color), division: Some(*division), flags: Some(*flags) };
                let _ = connection::send_packet(io, &bar).await;
            }
            mc_player::player::PlayerStateEventKind::GameEventGlobal(event_type, value) => {
                let event = GameEvent { event: *event_type, value: *value };
                let _ = connection::send_packet(io, &event).await;
            }
            mc_player::player::PlayerStateEventKind::TransferPlayer(host, port) => {
                let transfer = Transfer { host: host.clone(), port: *port };
                let _ = connection::send_packet(io, &transfer).await;
                info!("Sent transfer to {}:{} — player '{}'", host, port, username);
            }
        }
    }
}
