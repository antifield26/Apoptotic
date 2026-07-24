//! 连接处理器 — 管理单个客户端的完整生命周期
//!
//! 每个连接对应一个 tokio task，处理 Handshake → Status/Login → Play。

use mc_protocol::codec::*;
use mc_protocol::packets::handshake::HandshakePacket;
use mc_protocol::packets::login::*;
use mc_protocol::packets::play::*;
use mc_protocol::packets::status::*;
use mc_protocol::state::ConnectionState;
use mc_command::dispatcher::CommandDispatcher;
use mc_player::player::SharedPlayerManager;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::net::TcpStream;
use tracing::{debug, error, info, warn};
use crate::packet_io::PacketStream;
use crate::rate_limiter;

/// 清理失败的玩家加入
fn cleanup_player_join(server: &ServerRef, uuid: &uuid::Uuid, _bevy_entity_id: u32) {
    // Despawn is handled by tick loop when player count drops
    server.player_manager.remove_player(uuid);
}

/// 计算动态视距: 玩家越多，视距越小 (节省内存)
pub(crate) fn effective_view_distance(server: &ServerRef) -> u8 {
    let online = server.player_manager.online_count() as u32;
    let max = server.max_view_distance;
    if online <= 1 { return max; }
    if online <= 3 { return max.saturating_sub(1).max(4); }
    if online <= 6 { return max.saturating_sub(2).max(4); }
    max.saturating_sub(3).max(4)
}

/// Tracks dirty block changes per section for efficient UpdateSectionBlocks broadcast.
/// Uses DashMap for lock-free concurrent access from multiple connection handlers.
pub struct DirtyBlockTracker {
    /// Maps (chunk_x, section_y, chunk_z) → Vec of (local_index, block_state_id)
    /// local_index = y*256 + z*16 + x within a 16×16×16 section
    pending: dashmap::DashMap<(i32, i32, i32), parking_lot::Mutex<Vec<(i16, i32)>>>,
    /// Generation counter incremented when new blocks are marked
    generation: std::sync::atomic::AtomicU64,
    /// Per-player last-seen generation (to avoid duplicate broadcasts)
    #[allow(dead_code)] // reserved for per-player generation tracking
    player_seen: dashmap::DashMap<uuid::Uuid, u64>,
}

impl Default for DirtyBlockTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl DirtyBlockTracker {
    pub fn new() -> Self {
        Self {
            pending: dashmap::DashMap::new(),
            generation: std::sync::atomic::AtomicU64::new(0),
            player_seen: dashmap::DashMap::new(),
        }
    }

    /// Mark a single block as changed at world coordinates.
    /// Converts world coords to section-local index for UpdateSectionBlocks.
    pub fn mark_block(&self, world_x: i32, world_y: i32, world_z: i32, block_state_id: u32) {
        let chunk_x = world_x.div_euclid(16);
        let chunk_z = world_z.div_euclid(16);
        let section_y = world_y.div_euclid(16);
        let local_x = world_x.rem_euclid(16) as i16;
        let local_y = world_y.rem_euclid(16) as i16;
        let local_z = world_z.rem_euclid(16) as i16;
        // Local index within a 16×16×16 section: y*256 + z*16 + x
        let local_index: i16 = local_y * 256 + local_z * 16 + local_x;
        self.pending
            .entry((chunk_x, section_y, chunk_z))
            .or_insert_with(|| parking_lot::Mutex::new(Vec::with_capacity(16)))
            .lock()
            .push((local_index, block_state_id as i32));
        self.generation.fetch_add(1, std::sync::atomic::Ordering::Release);
    }

    /// Mark a full chunk position as dirty (for chunk-level operations like TNT).
    /// Records all non-air blocks in the chunk as pending updates.
    pub fn mark_chunk(&self, _cp: &mc_core::position::ChunkPos) {
        // For now, increment generation so connections resend the full ChunkData.
        // In a future optimization, we'd diff against the previous state.
        self.generation.fetch_add(1, std::sync::atomic::Ordering::Release);
    }

    /// Drain all pending block changes within view distance of a player.
    /// Returns Vec of (chunk_x, section_y, chunk_z, blocks) for constructing
    /// UpdateSectionBlocks packets.
    pub fn drain_nearby(
        &self,
        player_cx: i32,
        player_cz: i32,
        max_dist: i32,
    ) -> Vec<(i32, i32, i32, Vec<(i16, i32)>)> {
        let _current_gen = self.generation.load(std::sync::atomic::Ordering::Acquire);
        let mut results = Vec::new();
        // Collect entries within range WITHOUT removing (multiple players may need them)
        for entry in self.pending.iter() {
            let (cx, _sy, cz) = *entry.key();
            if (cx - player_cx).abs() <= max_dist && (cz - player_cz).abs() <= max_dist {
                let blocks = entry.value().lock().clone();
                if !blocks.is_empty() {
                    results.push((cx, _sy, cz, blocks));
                }
            }
        }
        results
    }

    /// Remove entries older than `max_age_ticks` (cleanup for stale entries).
    pub fn cleanup_stale(&self, _current_tick: u64) {
        // Entries are removed by drain_nearby; any left are for disconnected players
        // or out-of-range. Clean them periodically to prevent unbounded growth.
        if self.pending.len() > 1024 {
            self.pending.clear();
            tracing::warn!("DirtyBlockTracker: cleared {} stale entries (overflow protection)", self.pending.len());
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(std::sync::atomic::Ordering::Acquire)
    }
}

/// 服务器引用 — 连接需要访问的共享状态
#[derive(Clone)]
pub struct ServerRef {
    pub motd: String,
    pub max_players: u32,
    pub protocol_version: i32,
    pub version_name: String,
    pub online_mode: bool,
    pub compression_threshold: u32,
    pub world_seed: u64,
    pub generator_name: String,
    pub view_distance: u8,
    pub max_view_distance: u8,
    pub player_manager: SharedPlayerManager,
    pub command_dispatcher: Arc<parking_lot::Mutex<CommandDispatcher>>,
    pub shutdown_tx: broadcast::Sender<()>,
    pub chunk_store: mc_world::chunk_store::ChunkStore,
    pub world_state: mc_core::world_state::SharedWorldState,
    /// 世界数据目录 (绝对路径: server_root/data/world/region)
    pub world_dir: std::path::PathBuf,
    /// 从 DB 预加载的玩家存档数据 (UUID → 上次退出时的状态)
    pub saved_player_data: Arc<parking_lot::RwLock<std::collections::HashMap<uuid::Uuid, mc_persistence::player_data::PlayerRow>>>,
    /// 全局非玩家实体 ID 计数 (item entities, etc.)
    pub next_entity_id: Arc<std::sync::atomic::AtomicI32>,
    /// 手动保存触发器 (广播通道)
    pub save_trigger: broadcast::Sender<()>,
    /// 地形生成器 (缓存, 复用于所有连接)
    pub generator: std::sync::Arc<dyn mc_world::generator::TerrainGenerator + Send + Sync>,
    /// 生物管理器 (追踪所有非玩家实体)
    pub mob_manager: std::sync::Arc<mc_player::mob::MobManager>,
    /// 容器管理器 (追踪打开的容器 GUI)
    pub container_manager: std::sync::Arc<mc_player::container::ContainerManager>,
    /// 袭击管理器 (村庄袭击事件)
    pub raid_manager: std::sync::Arc<mc_player::raid::RaidManager>,
    /// 配方注册表
    pub recipe_registry: std::sync::Arc<mc_player::recipe::RecipeRegistry>,
    /// 钓鱼管理器
    pub fishing_manager: std::sync::Arc<parking_lot::RwLock<mc_player::fishing::FishingManager>>,
    /// 酿造台管理器
    pub brewing_manager: std::sync::Arc<parking_lot::RwLock<mc_player::brewing::BrewingStandManager>>,
    /// 信标管理器
    pub beacon_manager: std::sync::Arc<parking_lot::RwLock<mc_player::beacon::BeaconManager>>,
    /// 掉落物品追踪 (entity_id → (item_block_id, x, y, z))
    pub dropped_items: crate::SharedDroppedItems,
    /// 唱片机追踪 ((x,y,z) → disc_id)
    pub jukebox_discs: crate::SharedJukeboxDiscs,
    /// 熔炉管理器
    pub furnace_manager: std::sync::Arc<parking_lot::RwLock<mc_player::furnace::FurnaceManager>>,
    /// 成就追踪器
    pub advancement_tracker: std::sync::Arc<parking_lot::RwLock<mc_player::advancement::AdvancementTracker>>,
    /// 成就注册表 (共享, 用于触发检测)
    pub advancement_registry: std::sync::Arc<mc_player::advancement::AdvancementRegistry>,
    /// 需要跨玩家广播的脏区块 (legacy — kept for full-chunk rebroadcast on chunk-level ops)
    pub dirty_chunks_broadcast: std::sync::Arc<parking_lot::RwLock<std::collections::HashSet<mc_core::position::ChunkPos>>>,
    /// Per-section dirty block tracker for immediate UpdateSectionBlocks (0x47) broadcast.
    /// Replaces the 20-tick ChunkData rebroadcast delay with per-tick section updates.
    pub dirty_blocks: std::sync::Arc<DirtyBlockTracker>,
    /// Configurable server links (shown in pause menu via 0x4F packet)
    pub server_links: Vec<(String, String)>,
    /// Plugin manager (notify player join/leave, tick)
    pub plugin_manager: std::sync::Arc<mc_plugin::plugin::PluginManager>,
    /// Plugin context (shared for notifications)
    pub plugin_ctx: mc_plugin::plugin::PluginContext,
    /// Entity broadcast radius (configurable, default 64.0 blocks)
    /// Used as cap for per-entity-type tracking ranges (A6).
    pub entity_broadcast_radius: f64,
}

/// 处理一个客户端连接
pub async fn handle_connection(
    stream: TcpStream,
    server: ServerRef,
) {
    let peer_addr = stream
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| "unknown".into());
    let peer_socket = stream.peer_addr().ok();

    // Rate limit: max 5 connections per minute per IP
    if let Some(addr) = peer_socket
        && !rate_limiter::allow_connection(addr) {
            warn!("Connection rejected by rate limiter: {}", peer_addr);
            return;
        }

    info!("New connection from {}", peer_addr);

    let (read, write) = stream.into_split();
    let mut io = PacketStream::new(read, write, 0); // start uncompressed

    // ── 1. Handshake ──
    let handshake = match read_handshake(&mut io).await {
        Ok(h) => h,
        Err(e) => {
            error!("Handshake error from {}: {}", peer_addr, e);
            return;
        }
    };

    // Protocol version compatibility check
    // Supported: 26.0 (766) through 26.2 (776) — the 26.x family
    const MIN_PROTOCOL: i32 = 766;
    const MAX_PROTOCOL: i32 = 776;

    let version_ok = handshake.protocol_version >= MIN_PROTOCOL
        && handshake.protocol_version <= MAX_PROTOCOL;

    debug!(
        "Handshake from {}: protocol={} (compatible: {}), next_state={:?}",
        peer_addr, handshake.protocol_version, version_ok, handshake.next_state
    );

    if !version_ok && !matches!(handshake.next_state, ConnectionState::Status) {
        // For Login attempts with incompatible versions, reject with helpful message
        if let ConnectionState::Login = handshake.next_state {
            warn!(
                "Incompatible protocol version {} from {} (supported: {}-{})",
                handshake.protocol_version, peer_addr, MIN_PROTOCOL, MAX_PROTOCOL
            );
            // Send LoginDisconnect before the client fully enters login state
            // The client will display the disconnect reason in the multiplayer menu
            let dc = mc_protocol::packets::login::LoginDisconnect {
                reason: format!(
                    "{{\"text\":\"Unsupported version!\\nServer: 26.x (protocol {}-{})\\nYour client: protocol {}\"}}",
                    MIN_PROTOCOL, MAX_PROTOCOL, handshake.protocol_version
                ),
            };
            let _ = send_packet(&mut io, &dc).await;
            return;
        }
    }

    match handshake.next_state {
        ConnectionState::Status => {
            handle_status(&mut io, &server).await;
        }
        ConnectionState::Login => {
            handle_login(&mut io, &server, peer_socket).await;
        }
        _ => {
            warn!("Unknown next_state from {}", peer_addr);
        }
    }

    info!("Connection closed: {}", peer_addr);
}

/// 读取 Handshake 数据包
async fn read_handshake(io: &mut PacketStream) -> Result<HandshakePacket, CodecError> {
    let frame = io.read_frame().await?;
    tracing::debug!("Handshake frame: {} bytes: {:02X?}", frame.len(), &frame[..frame.len().min(32)]);
    let result = io.codec().decode::<HandshakePacket>(&frame);
    if let Err(ref e) = result {
        tracing::error!("Handshake decode error: {:?}, frame bytes: {:02X?}", e, &frame[..frame.len().min(32)]);
    }
    result
}

/// 处理 Status 阶段 (Server List Ping)
async fn handle_status(io: &mut PacketStream, server: &ServerRef) {
    loop {
        let frame = match io.read_frame().await {
            Ok(f) => f,
            Err(e) => {
                debug!("Status read error: {}", e);
                return;
            }
        };

        let (packet_id, _payload) = match io.codec().parse_packet_id_and_payload(&frame) {
            Ok(v) => v,
            Err(e) => {
                error!("Status decode error: {}", e);
                return;
            }
        };

        match packet_id {
            0x00 => {
                // Status Request → Send Status Response
                debug!("Status request from client");
                let response = StatusResponse {
                    version: mc_protocol::packets::status::VersionInfo {
                        name: server.version_name.clone(),
                        protocol: server.protocol_version,
                    },
                    players: PlayersInfo {
                        max: server.max_players,
                        online: server.player_manager.online_count() as u32,
                        sample: Vec::new(),
                    },
                    description: Description {
                        text: server.motd.clone(),
                    },
                    favicon: None,
                    enforces_secure_chat: false,
                    previews_chat: Some(false),
                };

                match io.codec().encode(&response) {
                    Ok(data) => {
                        if let Err(e) = io.write_frame(&data).await {
                            error!("Failed to send Status Response: {}", e);
                            return;
                        }
                    }
                    Err(e) => {
                        error!("Failed to encode Status Response: {}", e);
                        return;
                    }
                }
            }
            0x01 => {
                // Ping Request → Send Pong
                match io.codec().decode::<PingRequest>(&frame) {
                    Ok(ping) => {
                        let pong = PongResponse {
                            payload: ping.payload,
                        };
                        match io.codec().encode(&pong) {
                            Ok(data) => {
                                if let Err(e) = io.write_frame(&data).await {
                                    error!("Failed to send Pong: {}", e);
                                    return;
                                }
                            }
                            Err(e) => {
                                error!("Failed to encode Pong: {}", e);
                                return;
                            }
                        }
                        // Status ping-pong complete, close connection
                        return;
                    }
                    Err(e) => {
                        error!("Ping decode error: {}", e);
                        return;
                    }
                }
            }
            _ => {
                debug!("Unknown Status packet ID: 0x{:02X}", packet_id);
            }
        }
    }
}

/// 处理 Login 阶段
async fn handle_login(io: &mut PacketStream, server: &ServerRef, peer_socket: Option<std::net::SocketAddr>) {
    let frame = match io.read_frame().await {
        Ok(f) => f,
        Err(e) => {
            error!("Login read error: {}", e);
            return;
        }
    };

    let (packet_id, _) = match io.codec().parse_packet_id_and_payload(&frame) {
        Ok(v) => v,
        Err(e) => {
            error!("Login frame decode error: {}", e);
            return;
        }
    };

    if packet_id != 0x00 {
        // Not Login Start
        error!("Expected Login Start (0x00), got 0x{:02X}", packet_id);
        return;
    }

    let login_start = match io.codec().decode::<LoginStart>(&frame) {
        Ok(ls) => ls,
        Err(e) => {
            error!("Login Start decode error: {}", e);
            return;
        }
    };

    info!("Login request: username='{}'", login_start.username);

    let (player_uuid, properties) = if server.online_mode {
        // ═══ 在线模式: 加密握手 + Mojang 验证 ═══
        let enc_keys = match crate::encryption::generate_keys() {
            Ok(k) => k,
            Err(e) => {
                error!("Failed to generate encryption keys: {}", e);
                let _ = send_packet(io, &LoginDisconnect {
                    reason: "{\"text\":\"Server error during encryption setup\"}".into(),
                }).await;
                return;
            }
        };

        // Send Encryption Request
        let enc_req = EncryptionRequest {
            server_id: String::new(), // modern Minecraft uses ""
            public_key: enc_keys.public_key_der.clone(),
            verify_token: enc_keys.verify_token.to_vec(),
        };
        if let Err(e) = send_packet(io, &enc_req).await {
            error!("Failed to send EncryptionRequest: {}", e);
            return;
        }

        // Read Encryption Response (with 30s timeout — client may disconnect during handshake)
        let enc_resp_frame = match tokio::time::timeout(
            tokio::time::Duration::from_secs(30),
            io.read_frame(),
        ).await {
            Ok(Ok(f)) => f,
            Ok(Err(e)) => {
                error!("Encryption response read error: {}", e);
                return;
            }
            Err(_) => {
                error!("Encryption response timed out for '{}'", login_start.username);
                return;
            }
        };
        let enc_response = match io.codec().decode::<EncryptionResponse>(&enc_resp_frame) {
            Ok(r) => r,
            Err(e) => {
                error!("Encryption response decode error: {}", e);
                return;
            }
        };

        // Decrypt shared secret and verify token
        let (shared_secret, token) = match crate::encryption::decrypt_client_secrets(
            &enc_keys.private_key,
            &enc_response.shared_secret,
            &enc_response.verify_token,
        ) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to decrypt client secrets: {}", e);
                let _ = send_packet(io, &LoginDisconnect {
                    reason: "{\"text\":\"Encryption verification failed\"}".into(),
                }).await;
                return;
            }
        };

        // Verify token
        if token != enc_keys.verify_token {
            error!("Verify token mismatch for '{}'", login_start.username);
            let _ = send_packet(io, &LoginDisconnect {
                reason: "{\"text\":\"Token verification failed\"}".into(),
            }).await;
            return;
        }

        // Enable AES-CFB8 encryption
        io.enable_encryption(&shared_secret);
        debug!("AES-CFB8 encryption enabled for '{}'", login_start.username);

        // Compute Mojang auth hash
        let server_hash = crate::encryption::compute_server_hash(
            "",
            &shared_secret,
            &enc_keys.public_key_der,
        );

        // Verify with Mojang (with retry for network issues)
        let mut profile = None;
        let mut last_err = String::new();
        for attempt in 0..3 {
            match crate::encryption::verify_mojang_session(
                &login_start.username,
                &server_hash,
            ).await {
                Ok(p) => { profile = Some(p); break; }
                Err(e) => {
                    last_err = e;
                    if attempt < 2 {
                        tracing::warn!("Mojang API attempt {} failed, retrying...", attempt + 1);
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                }
            }
        }
        let profile = match profile {
            Some(p) => p,
            None => {
                error!("Mojang verification failed for '{}': {}", login_start.username, last_err);
                let _ = send_packet(io, &LoginDisconnect {
                    reason: "{\"text\":\"Mojang authentication failed. Are you logged in?\"}".into(),
                }).await;
                return;
            }
        };

        let uuid = uuid::Uuid::parse_str(&profile.id).unwrap_or_else(|_| {
            mc_core::auth::offline_uuid(&login_start.username)
        });

        // Convert Mojang properties to login properties (skin, cape)
        let props: Vec<mc_protocol::packets::login::Property> = profile.properties.iter().map(|p| {
            mc_protocol::packets::login::Property {
                name: p.name.clone(),
                value: p.value.clone(),
                signature: p.signature.clone(),
            }
        }).collect();

        info!("Mojang verified: '{}' (uuid={})", profile.name, uuid);
        (uuid, props)
    } else {
        // 离线模式 — 直接使用 offline UUID
        let uuid = login_start.player_uuid.unwrap_or_else(|| {
            mc_core::auth::offline_uuid(&login_start.username)
        });
        (uuid, Vec::new())
    };

    // Ban check (shared between online and offline)
    if server.player_manager.is_banned(&player_uuid) {
        let dc = LoginDisconnect {
            reason: "{\"text\":\"You are banned from this server\"}".into(),
        };
        let _ = send_packet(io, &dc).await;
        info!("Banned player '{}' tried to join", login_start.username);
        return;
    }

    // Whitelist check
    if !server.player_manager.is_whitelisted(&player_uuid) {
        let dc = LoginDisconnect {
            reason: "{\"text\":\"You are not whitelisted on this server\"}".into(),
        };
        let _ = send_packet(io, &dc).await;
        info!("Non-whitelisted player '{}' tried to join", login_start.username);
        return;
    }

    let success = LoginSuccess {
        uuid: player_uuid,
        username: login_start.username.clone(),
        properties,
    };

    match io.codec().encode(&success) {
        Ok(data) => {
            if let Err(e) = io.write_frame(&data).await {
                error!("Failed to send Login Success: {}", e);
                return;
            }
        }
        Err(e) => {
            error!("Failed to encode Login Success: {}", e);
            return;
        }
    }

    info!(
        "Player '{}' logged in (uuid={}, {})",
        login_start.username, player_uuid,
        if server.online_mode { "online" } else { "offline" }
    );

    // 如果配置了压缩，发送 Set Compression 并启用压缩
    if server.compression_threshold > 0 {
        let set_comp = SetCompression {
            threshold: server.compression_threshold as i32,
        };
        match io.codec().encode(&set_comp) {
            Ok(data) => {
                if let Err(e) = io.write_frame(&data).await {
                    error!("Failed to send Set Compression: {}", e);
                    return;
                }
            }
            Err(e) => {
                error!("Failed to encode Set Compression: {}", e);
                return;
            }
        }
        // 启用压缩
        *io.codec_mut() = MinecraftCodec::new(server.compression_threshold);
    }

    // 等待 Login Acknowledged (1.20.2+ 要求)
    // 在启用压缩后读取，所以使用新的 codec
    match io.read_frame().await {
        Ok(frame) => {
            match io.codec().decode::<LoginAcknowledged>(&frame) {
                Ok(_) => {
                    debug!("Login acknowledged by '{}'", login_start.username);
                }
                Err(e) => {
                    // Some older clients might send 0x03 with different format
                    // or skip LoginAcknowledged entirely. Log and continue.
                    debug!("Login Acknowledged decode: {} — continuing anyway", e);
                }
            }
        }
        Err(e) => {
            debug!("Failed to read Login Acknowledged: {} — continuing anyway", e);
        }
    }

    // ── 进入 Configuration 阶段 (protocol 764+ 必需) ──
    // 如果客户端不支持 Config 阶段，直接进入 Play
    if server.protocol_version >= 764 {
        handle_configuration(io, &login_start.username, server).await;
    }

    // ── 进入 Play 阶段 ──
    handle_play(io, &login_start.username, player_uuid, server, peer_socket).await;
}

/// 处理 Configuration 阶段 — 同步注册表、功能标志，完成客户端配置
async fn handle_configuration(
    io: &mut PacketStream,
    username: &str,
    _server: &ServerRef,
) {
    use mc_protocol::packets::config::*;

    debug!("Entering Configuration phase for '{}'", username);

    // 1. Send brand plugin message
    let brand = ConfigPluginMessage {
        channel: "minecraft:brand".into(),
        data: b"Minecraft LAN Server\0".to_vec(),
    };
    let _ = send_packet(io, &brand).await;

    // 2. Send Known Packs (empty = use default data pack)
    let _ = send_packet(io, &KnownPacks { known_packs: vec![] }).await;

    // 3. Send Registry Data — entries with data:None = client uses built-in defaults
    let _ = send_packet(io, &RegistryData {
        registry_id: "minecraft:dimension_type".into(),
        entries: vec![
            RegistryEntry { key: "minecraft:overworld".into(), data: None },
            RegistryEntry { key: "minecraft:the_nether".into(), data: None },
            RegistryEntry { key: "minecraft:the_end".into(), data: None },
        ],
    }).await;
    let _ = send_packet(io, &RegistryData {
        registry_id: "minecraft:worldgen/biome".into(),
        entries: vec![
            RegistryEntry { key: "minecraft:plains".into(), data: None },
            RegistryEntry { key: "minecraft:the_void".into(), data: None },
            RegistryEntry { key: "minecraft:forest".into(), data: None },
            RegistryEntry { key: "minecraft:ocean".into(), data: None },
            RegistryEntry { key: "minecraft:desert".into(), data: None },
            RegistryEntry { key: "minecraft:nether_wastes".into(), data: None },
            RegistryEntry { key: "minecraft:soul_sand_valley".into(), data: None },
            RegistryEntry { key: "minecraft:crimson_forest".into(), data: None },
            RegistryEntry { key: "minecraft:warped_forest".into(), data: None },
            RegistryEntry { key: "minecraft:basalt_deltas".into(), data: None },
            RegistryEntry { key: "minecraft:the_end".into(), data: None },
            RegistryEntry { key: "minecraft:end_highlands".into(), data: None },
            RegistryEntry { key: "minecraft:end_midlands".into(), data: None },
            RegistryEntry { key: "minecraft:small_end_islands".into(), data: None },
            RegistryEntry { key: "minecraft:end_barrens".into(), data: None },
        ],
    }).await;

    // 4. Send Feature Flags (empty = vanilla)
    let _ = send_packet(io, &FeatureFlags { flags: vec![] }).await;

    // 5. Read client packets until all required config packets are received
    let config_start = tokio::time::Instant::now();
    let config_timeout = tokio::time::Duration::from_secs(30);
    let mut got_client_info = false;
    let mut got_known_packs = false;
    loop {
        if config_start.elapsed() > config_timeout {
            warn!("Configuration timed out for '{}'", username);
            break;
        }

        let frame = match tokio::time::timeout(
            tokio::time::Duration::from_secs(5),
            io.read_frame(),
        ).await {
            Ok(Ok(f)) => f,
            Ok(Err(e)) => {
                debug!("Config read error for {}: {}", username, e);
                break;
            }
            Err(_) => {
                // Send keep-alive to keep the connection alive
                let _ = send_packet(io, &ConfigKeepAlive { id: 42 }).await;
                continue;
            }
        };

        let (packet_id, _payload) = match io.codec().parse_packet_id_and_payload(&frame) {
            Ok(v) => v,
            Err(e) => {
                error!("Config decode error: {}", e);
                continue;
            }
        };

        match packet_id {
            0x00 => {
                match io.codec().decode::<ConfigClientInformation>(&frame) {
                    Ok(info) => {
                        debug!("Config client info: locale={}, vd={}", info.locale, info.view_distance);
                        got_client_info = true;
                    }
                    Err(_) => debug!("Config client info decode failed — ignoring"),
                }
            }
            0x04 => {
                debug!("Config pong from {}", username);
            }
            0x07 => {
                match io.codec().decode::<ServerboundKnownPacks>(&frame) {
                    Ok(packs) => {
                        debug!("Client known packs: {} packs", packs.known_packs.len());
                        got_known_packs = true;
                    }
                    Err(_) => debug!("Known packs decode failed — continuing"),
                }
            }
            0x02 => {
                // CookieResponse — client responds to server's CookieRequest
                match io.codec().decode::<mc_protocol::packets::play::CookieResponse>(&frame) {
                    Ok(cookie) => {
                        debug!("Cookie from {}: key={}, has_payload={}",
                            username, cookie.key, cookie.payload.is_some());
                    }
                    Err(_) => debug!("Cookie response decode failed — continuing"),
                }
            }
            _ => {
                debug!("Unknown Config packet 0x{:02X} from {}", packet_id, username);
            }
        }

        // All required client packets received → proceed to Play
        if got_client_info && got_known_packs {
            break;
        }
    }

    // 6. Send Finish Configuration
    let _ = send_packet(io, &FinishConfiguration).await;
    debug!("Sent FinishConfiguration to '{}'", username);

    // 7. Wait for client AckFinishConfig
    match tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        io.read_frame(),
    ).await {
        Ok(Ok(frame)) => {
            if let Ok((packet_id, _)) = io.codec().parse_packet_id_and_payload(&frame)
                && packet_id == 0x03 {
                    let _ = io.codec().decode::<AckFinishConfig>(&frame);
                    debug!("Configuration finished for '{}'", username);
                }
        }
        Ok(Err(e)) => debug!("Config ack read error: {}", e),
        Err(_) => debug!("Config ack timeout — continuing to Play"),
    }
}

/// 处理 Play 阶段
async fn handle_play(
    io: &mut PacketStream,
    username: &str,
    _uuid: uuid::Uuid,
    server: &ServerRef,
    peer_socket: Option<std::net::SocketAddr>,
) {
    use mc_protocol::packets::play::*;

    // Generate spawn area chunks using cached generator
    let view_radius = effective_view_distance(server) as i32;
    let mut spawn_chunks = Vec::new();
    for dx in -view_radius..=view_radius {
        for dz in -view_radius..=view_radius {
            let pos = mc_core::position::ChunkPos::new(dx, dz);
            let chunk = server.generator.generate_chunk(pos, server.world_seed);
            server.chunk_store.insert(pos, chunk.clone());
            spawn_chunks.push(chunk);
        }
    }
    let chunk_count = spawn_chunks.len();
    info!("Generated {} spawn chunks using '{}'", chunk_count, server.generator.name());

    // Max players hard cap — reject if server is full
    {
        let online = server.player_manager.online_count() as u32;
        if online >= server.max_players {
            let disconnect = LoginDisconnect {
                reason: format!("{{\"text\":\"Server is full ({} / {})\",\"color\":\"red\"}}", online, server.max_players),
            };
            let _ = send_packet(io, &disconnect).await;
            warn!("Rejected '{}': server full ({} / {})", username, online, server.max_players);
            return;
        }
    }

    // 注册玩家到 PlayerManager
    let player = server.player_manager.add_player(_uuid, username.to_string());
    server.plugin_manager.notify_player_join(&server.plugin_ctx, &_uuid, username);
    let entity_id = player.entity_id;

    // Restore saved player data (position, health, gamemode, OP)
    let saved_pos = {
        let data = server.saved_player_data.read();
        data.get(&_uuid).cloned()
    };
    if let Some(ref row) = saved_pos {
        info!("Restoring saved state for '{}': pos=({:.0},{:.0},{:.0}), health={}, gm={}, op={}",
            username, row.pos_x, row.pos_y, row.pos_z, row.health, row.gamemode, row.is_op);
        let _ = server.player_manager.update_position_full(&_uuid, row.pos_x, row.pos_y, row.pos_z, row.yaw, row.pitch);
        let _ = server.player_manager.set_health(&_uuid, row.health);
        // Restore hunger
        if row.food > 0 {
            let _ = server.player_manager.set_food(&_uuid, row.food, row.saturation);
        }
        if let Some(gm) = mc_core::types::GameMode::from_id(row.gamemode) {
            let _ = server.player_manager.set_gamemode(&_uuid, gm);
        }
        if row.is_op {
            let _ = server.player_manager.set_op(&_uuid, true);
        }
        // Restore inventory from BLOB
        if let Some(ref blob) = row.inventory_blob
            && !blob.is_empty()
                && let Some(inv) = mc_player::inventory::Inventory::deserialize(blob) {
                    let filled = inv.items.iter().filter(|s| s.is_some()).count();
                    let _ = server.player_manager.set_inventory(&_uuid, inv);
                    debug!("Restored inventory for '{}' ({} slots filled)", username, filled);
                }
    }

    // Protocol 776: JoinGame removed — dimension data from Configuration phase.
    // Set up chunk streaming cache for the player's view distance.
    let cache_center = SetChunkCacheCenter { chunk_x: 0, chunk_z: 0 };
    let _ = send_packet(io, &cache_center).await;
    let cache_radius = SetChunkCacheRadius { radius: server.view_distance as i32 };
    let _ = send_packet(io, &cache_radius).await;

    // Send player abilities (survival defaults — updated on gamemode change)
    let _ = send_packet(io, &PlayerAbilities::survival()).await;

    // Send recipe book
    let recipe_pkt = build_update_recipes(&server.recipe_registry);
    let _ = send_packet(io, &recipe_pkt).await;

    // Send server links (shown in pause menu, configurable via server_links in config.toml)
    let links: Vec<ServerLink> = if server.server_links.is_empty() {
        vec![ServerLink {
            label: "Project".into(),
            url: "https://github.com/antifield26/MinecraftServer".into(),
        }]
    } else {
        server.server_links.iter().map(|(label, url)| ServerLink {
            label: label.clone(), url: url.clone(),
        }).collect()
    };
    let _ = send_packet(io, &ServerLinks { links }).await;

    // 1.21.5: UpdateEnabledFeatures — tell client about supported features
    let _ = send_packet(io, &UpdateEnabledFeatures { features: vec!["minecraft:update_1_21".into()] }).await;

    // Send entity attributes (health, armor, etc.)
    let _ = send_packet(io, &UpdateAttributes {
        entity_id,
        attributes: vec![
            Attribute {
                key: "minecraft:generic.max_health".into(),
                value: 20.0,
                modifiers: vec![],
            },
            Attribute {
                key: "minecraft:generic.armor".into(),
                value: 0.0,
                modifiers: vec![],
            },
            Attribute {
                key: "minecraft:generic.armor_toughness".into(),
                value: 0.0,
                modifiers: vec![],
            },
            Attribute {
                key: "minecraft:generic.attack_damage".into(),
                value: 1.0,
                modifiers: vec![],
            },
            Attribute {
                key: "minecraft:generic.movement_speed".into(),
                value: 0.1,
                modifiers: vec![],
            },
            Attribute {
                key: "minecraft:generic.attack_speed".into(),
                value: 4.0,
                modifiers: vec![],
            },
            Attribute {
                key: "minecraft:generic.knockback_resistance".into(),
                value: 0.0,
                modifiers: vec![],
            },
            Attribute {
                key: "minecraft:generic.luck".into(),
                value: 0.0,
                modifiers: vec![],
            },
            Attribute {
                key: "minecraft:player.block_interaction_range".into(),
                value: 4.5,
                modifiers: vec![],
            },
            Attribute {
                key: "minecraft:player.entity_interaction_range".into(),
                value: 3.0,
                modifiers: vec![],
            },
            // ── 26.2 Chaos Cubed: new entity attributes ──
            Attribute {
                key: "minecraft:generic.bounciness".into(),
                value: 0.0,
                modifiers: vec![],
            },
            Attribute {
                key: "minecraft:generic.friction_modifier".into(),
                value: 1.0,
                modifiers: vec![],
            },
            Attribute {
                key: "minecraft:generic.air_drag_modifier".into(),
                value: 1.0,
                modifiers: vec![],
            },
            Attribute {
                key: "minecraft:generic.nameplate_distance".into(),
                value: 64.0,
                modifiers: vec![],
            },
            Attribute {
                key: "minecraft:generic.below_name_distance".into(),
                value: 10.0,
                modifiers: vec![],
            },
        ],
    }).await;

    // Send default spawn position (compass direction — extract values before await)
    {
        let (sx, sy, sz) = {
            let ws = server.world_state.read();
            (ws.spawn_x as i32, ws.spawn_y as i32, ws.spawn_z as i32)
        };
        let _ = send_packet(io, &SetDefaultSpawnPosition {
            x: sx, y: sy, z: sz, angle: 0.0,
        }).await;
    }

    // Send world border initialization
    {
        let wb = {
            let ws = server.world_state.read();
            ws.world_border.clone()
        };
        let _ = send_packet(io, &InitializeWorldBorder {
            x: wb.center_x,
            z: wb.center_z,
            old_diameter: wb.size,
            new_diameter: wb.target_size,
            speed: if wb.lerp_time_ticks > 0 {
                ((wb.size - wb.target_size).abs() / wb.lerp_time_ticks as f64 * 1000.0) as i64
            } else { 0 },
            portal_teleport_boundary: (wb.size as i32).min(29999984),
            warning_blocks: wb.warning_blocks,
            warning_time: wb.warning_time,
        }).await;

        // Send UpdateAdvancements — reset=true tells client server doesn't manage advancements
        let _ = send_packet(io, &mc_protocol::packets::play::UpdateAdvancements {
            reset: true,
            advancement_ids: vec![],
            progress_map: vec![],
        }).await;
    }

    // 发送 Player Position (restore saved position or use spawn default)
    let (player_x, player_y, player_z, player_yaw, player_pitch) = if let Some(ref row) = saved_pos {
        (row.pos_x, row.pos_y, row.pos_z, row.yaw, row.pitch)
    } else {
        (0.0, 64.0, 0.0, 0.0f32, 0.0f32)
    };
    let pos_packet = PlayerPosition {
        x: player_x,
        y: player_y,
        z: player_z,
        yaw: player_yaw,
        pitch: player_pitch,
        flags: 0,
        teleport_id: 42,
    };

    if let Err(e) = send_packet(io, &pos_packet).await {
        error!("Failed to send Player Position: {}", e);
        cleanup_player_join(server, &_uuid, 0);
        return;
    }

    // Send DeclareCommands for tab completion
    let cmd_names: Vec<String> = {
        let disp = server.command_dispatcher.lock();
        disp.list_commands().iter().map(|s| s.to_string()).collect()
    };
    let names: Vec<&str> = cmd_names.iter().map(|s| s.as_str()).collect();
    let declare = mc_protocol::packets::play::DeclareCommands::from_command_names(&names);
    let _ = send_packet(io, &declare).await;

    // 发送所有区块
    for chunk in &spawn_chunks {
        let chunk_data = chunk.to_chunk_data();
        if let Err(e) = send_packet(io, &chunk_data).await {
            error!("Failed to send chunk ({}, {}): {}", chunk.position.x, chunk.position.z, e);
            cleanup_player_join(server, &_uuid, 0);
            return;
        }
        // Spawn passive mobs in initial chunks (0-1 per chunk)
        let mob_count = fastrand::u32(0..2) as usize;
        let passive_types: [i32; 6] = [11, 12, 13, 14, 15, 16]; // cow, pig, chicken, sheep, rabbit, bat
        for _ in 0..mob_count {
            let mob_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let mob_type = passive_types[fastrand::usize(0..4)];
            let mx = (chunk.position.x * 16 + fastrand::i32(0..16)) as f64 + 0.5;
            let mz = (chunk.position.z * 16 + fastrand::i32(0..16)) as f64 + 0.5;
            let my = find_surface_y(chunk, (mx as i32 & 0xF) as usize, (mz as i32 & 0xF) as usize);
            let spawn_pkt = SpawnEntity {
                entity_id: mob_eid, entity_uuid: uuid::Uuid::new_v4(),
                entity_type: mob_type,
                x: mx, y: my, z: mz,
                pitch: 0, yaw: (fastrand::u32(0..256) as u8), head_yaw: 0,
                data: 0,
                vel_x: 0, vel_y: 0, vel_z: 0,
            };
            let _ = send_packet(io, &spawn_pkt).await;
            let _ = send_packet(io, &mc_protocol::packets::play::SetEntityMetadata::mob_defaults(mob_eid)).await;
            // Register mob server-side for tracking
            let tracked = mc_player::mob::TrackedMob {
                entity_id: mob_eid,
                uuid: spawn_pkt.entity_uuid,
                mob_type,
                position: mc_core::position::Position::new(mx, my, mz),
                health: mc_player::mob::mob_max_health(mob_type),
                max_health: mc_player::mob::mob_max_health(mob_type),
                age_ticks: 0,
                ai_timer: 40 + fastrand::u64(..) % 61,
                ai_state: mc_player::mob::MobAiState::Idle,
                attack_cooldown: 0,
                last_sync_tick: 0,
                owner_uuid: None,
                is_tamed: false,
                is_sitting: false,
                tame_attempts: 0,
                is_baby: false,
                in_love_ticks: 0,
                breed_cooldown: 0,
                is_sheared: false, is_on_fire: false, is_in_water: false, path: Vec::new(), path_last_tick: 0, sulfur_cube_archetype: None, absorbed_block_id: None, is_small_cube: false, is_dormant: false, dirty_flags: 3,
            };
            server.mob_manager.register(tracked);
        }
    }

    // 发送 Tab List 更新（添加玩家自己）
    // C→S Player Info Update (0x3E) would go here

    info!(
        "Player '{}' entered the game (entity_id={}, {} chunks) — {} online",
        username, entity_id, chunk_count,
        server.player_manager.online_count()
    );

    // ── Sync new player with existing players ──
    server.player_manager.broadcast_join(username);

    // 1. Tell all existing players about the new player (via entity broadcast → their play_loop)
    server.player_manager.broadcast_entity_spawn(entity_id, _uuid, username, 0.0, 64.0, 0.0, 0.0, 0.0);

    // 2. Tell the new player about all existing players (send directly before play_loop)
    for existing in server.player_manager.others(&_uuid) {
        let info = mc_protocol::packets::play::PlayerInfoUpdate {
            actions: 0x01 | 0x02 | 0x04 | 0x08,
            entries: vec![mc_protocol::packets::play::PlayerInfoEntry {
                uuid: existing.uuid,
                username: existing.username.clone(),
                gamemode: existing.gamemode.id() as i32,
                ping: 0, // ping not yet measured during login handshake
                listed: true,
            }],
        };
        let _ = send_packet(io, &info).await;
        let spawn = mc_protocol::packets::play::SpawnPlayer {
            entity_id: existing.entity_id,
            player_uuid: existing.uuid,
            x: existing.position.x,
            y: existing.position.y,
            z: existing.position.z,
            yaw: existing.position.yaw,
            pitch: existing.position.pitch,
        };
        let _ = send_packet(io, &spawn).await;
        let meta = mc_protocol::packets::play::SetEntityMetadata::player_defaults(existing.entity_id);
        let _ = send_packet(io, &meta).await;
    }

    // Sync existing scoreboard, team, bossbar state to new player
    {
        // Scoreboard objectives - clone data and drop lock before await (B8 fix)
        let sb_data = {
            let sb = mc_command::commands::scoreboard::global_scoreboard();
            let objectives: Vec<_> = sb.list_objectives().iter()
                .map(|obj| (obj.name.clone(), obj.display_name.clone()))
                .collect();
            let scores: Vec<_> = sb.scores.iter()
                .flat_map(|(obj_name, score_map)| {
                    score_map.iter().map(move |(player_name, value)| {
                        (obj_name.clone(), player_name.clone(), *value)
                    })
                })
                .collect();
            (objectives, scores)
        }; // MutexGuard dropped here
        for (name, display_name) in &sb_data.0 {
            let _ = send_packet(io, &mc_protocol::packets::play::ScoreboardObjective {
                name: name.clone(),
                mode: 0, // create
                objective_value: format!("{{\"text\":\"{}\"}}", display_name),
                objective_type: 0, // integer/dummy
                number_format: 0,
            }).await;
        }
        for (obj_name, player_name, value) in &sb_data.1 {
            let _ = send_packet(io, &mc_protocol::packets::play::UpdateScore {
                entity_name: player_name.clone(),
                objective_name: obj_name.clone(),
                value: *value,
                display_name: Some(player_name.clone()),
                number_format: 0,
            }).await;
        }
        // BossBar: send all active bossbars
        let bar_list: Vec<(String, String, f32, i32, i32, u8)> = {
            let bars = mc_command::commands::advanced::bossbar_registry();
            bars.list().iter()
                .map(|(id, d)| (id.clone(), d.title.clone(), d.health, d.color, d.division, d.flags))
                .collect()
        }; // drop MutexGuard before await (B8 fix)
        for (id, title, health, color, division, flags) in &bar_list {
            let bar_pkt = mc_protocol::packets::play::BossBar {
                uuid: uuid::Uuid::parse_str(id).unwrap_or(uuid::Uuid::nil()),
                action: 0, // add
                title: Some(title.clone()),
                health: Some(*health),
                color: Some(*color),
                division: Some(*division),
                flags: Some(*flags),
            };
            let _ = send_packet(io, &bar_pkt).await;
        }
    }

    // Build initial set of loaded chunks for streaming tracker
    let initial_loaded: std::collections::HashSet<mc_core::position::ChunkPos> = spawn_chunks
        .iter()
        .map(|c| c.position)
        .collect();

    // ── Play 主循环 ──
    crate::play_loop::play_loop(io, username, _uuid, server, entity_id, initial_loaded, peer_socket).await;

    // Broadcast leave + entity despawn to all players
    server.player_manager.broadcast_leave(username);
    server.player_manager.broadcast_entity_despawn(entity_id, _uuid);

    // 玩家断开连接 — 清理 PlayerManager
    server.plugin_manager.notify_player_leave(&server.plugin_ctx, &_uuid);
    server.player_manager.remove_player(&_uuid);
    info!(
        "Player '{}' disconnected — {} online",
        username,
        server.player_manager.online_count()
    );
}

/// 当玩家跨越区块边界时，流式加载新可见区块
pub(crate) async fn stream_new_chunks(
    io: &mut PacketStream,
    server: &ServerRef,
    player_chunk: &mut mc_core::position::ChunkPos,
    loaded_chunks: &mut std::collections::HashSet<mc_core::position::ChunkPos>,
    new_chunk: mc_core::position::ChunkPos,
    view_radius: i32,
) {
    let old_chunk = *player_chunk;
    *player_chunk = new_chunk;

    if view_radius <= 0 { return; }

    let mut visible: std::collections::HashSet<mc_core::position::ChunkPos> =
        std::collections::HashSet::new();
    for dx in -view_radius..=view_radius {
        for dz in -view_radius..=view_radius {
            visible.insert(mc_core::position::ChunkPos::new(new_chunk.x + dx, new_chunk.z + dz));
        }
    }

    let new_chunks: Vec<mc_core::position::ChunkPos> = visible
        .iter()
        .filter(|cp| !loaded_chunks.contains(cp))
        .copied()
        .collect();

    // ── Chunk pre-send: predict forward movement ──
    // Pre-generate chunks in the direction of movement (1 chunk ahead of current border)
    // so they're ready when the player crosses the boundary.
    if !new_chunks.is_empty() {
        let move_dir = (
            (new_chunk.x - old_chunk.x).signum(),
            (new_chunk.z - old_chunk.z).signum(),
        );
        if move_dir.0 != 0 || move_dir.1 != 0 {
            // Predict the next chunk in movement direction and preload it
            let predicted_x = new_chunk.x + move_dir.0 * (view_radius + 1);
            let predicted_z = new_chunk.z + move_dir.1 * (view_radius + 1);
            let predicted_cp = mc_core::position::ChunkPos::new(predicted_x, predicted_z);
            if !loaded_chunks.contains(&predicted_cp) {
                // Pre-generate in background — chunk will be ready when needed
                let _pre_chunk = server.generator.generate_chunk(predicted_cp, server.world_seed);
                server.chunk_store.insert(predicted_cp, _pre_chunk);
                debug!("Chunk pre-send: predicted ({}, {}) in direction ({}, {})",
                    predicted_x, predicted_z, move_dir.0, move_dir.1);
            }
        }
    }

    if !new_chunks.is_empty() {
        // E7: Chunk send throttling — limit per-tick chunk sends to avoid
        // network flood when player moves fast or teleports. Prioritize
        // chunks closest to the player (Chebyshev distance).
        const MAX_CHUNKS_PER_SEND: usize = 6;
        let mut sorted: Vec<(i32, mc_core::position::ChunkPos)> = new_chunks
            .iter()
            .map(|cp| {
                let dist = (cp.x - new_chunk.x).abs().max((cp.z - new_chunk.z).abs());
                (dist, *cp)
            })
            .collect();
        sorted.sort_by_key(|(dist, _)| *dist); // closest first
        let to_send: Vec<mc_core::position::ChunkPos> = sorted.iter()
            .take(MAX_CHUNKS_PER_SEND)
            .map(|(_, cp)| *cp)
            .collect();
        let deferred = new_chunks.len().saturating_sub(to_send.len());
        debug!(
            "Chunk streaming: ({},{}) → ({},{}) ({} new, sending {} now, {} deferred)",
            old_chunk.x, old_chunk.z, new_chunk.x, new_chunk.z,
            new_chunks.len(), to_send.len(), deferred,
        );

        // Protocol 776: chunk batch protocol
        let _ = send_packet(io, &ChunkBatchStart).await;

        for cp in &to_send {
            let chunk = server.generator.generate_chunk(*cp, server.world_seed);
            let chunk_data = chunk.to_chunk_data();
            if let Err(e) = send_packet(io, &chunk_data).await {
                tracing::warn!("Failed to send chunk ({}, {}): {}", cp.x, cp.z, e);
            }

            // Spawn passive mobs in newly generated chunks (0-2 per chunk for RPi5 perf)
            // Must be done before chunk_store.insert to borrow chunk
            use mc_protocol::packets::play::SpawnEntity;
            let mob_count = fastrand::u32(0..3) as usize;
            let passive_types: [i32; 6] = [11, 12, 13, 14, 15, 16]; // cow, pig, chicken, sheep, rabbit, bat
            for _ in 0..mob_count {
                let mob_eid = server.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let mob_type = passive_types[fastrand::usize(0..4)];
                let mx = (cp.x * 16 + fastrand::i32(0..16)) as f64 + 0.5;
                let mz = (cp.z * 16 + fastrand::i32(0..16)) as f64 + 0.5;
                let my = find_surface_y(&chunk, (mx as i32 & 0xF) as usize, (mz as i32 & 0xF) as usize);
                let spawn_pkt = SpawnEntity {
                    entity_id: mob_eid,
                    entity_uuid: uuid::Uuid::new_v4(),
                    entity_type: mob_type,
                    x: mx, y: my, z: mz,
                    pitch: 0, yaw: (fastrand::u32(0..256) as u8),
                    head_yaw: 0,
                    data: 0,
                    vel_x: 0, vel_y: 0, vel_z: 0,
                };
                let _ = send_packet(io, &spawn_pkt).await;
                // Send entity metadata for the mob
                let mob_meta = mc_protocol::packets::play::SetEntityMetadata::mob_defaults(mob_eid);
                let _ = send_packet(io, &mob_meta).await;
                // Register mob server-side for tracking
                let tracked = mc_player::mob::TrackedMob {
                    entity_id: mob_eid,
                    uuid: spawn_pkt.entity_uuid,
                    mob_type,
                    position: mc_core::position::Position::new(mx, my, mz),
                    health: mc_player::mob::mob_max_health(mob_type),
                    max_health: mc_player::mob::mob_max_health(mob_type),
                    age_ticks: 0,
                    ai_timer: 40 + fastrand::u64(..) % 61,
                    ai_state: mc_player::mob::MobAiState::Idle,
                    attack_cooldown: 0,
                    last_sync_tick: 0,
                    owner_uuid: None,
                    is_tamed: false,
                    is_sitting: false,
                    tame_attempts: 0,
                    is_baby: false,
                    in_love_ticks: 0,
                    breed_cooldown: 0,
                    is_sheared: false, is_on_fire: false, is_in_water: false, path: Vec::new(), path_last_tick: 0, sulfur_cube_archetype: None, absorbed_block_id: None, is_small_cube: false, is_dormant: false, dirty_flags: 3,
                };
                server.mob_manager.register(tracked);
            }

            server.chunk_store.insert(*cp, chunk);
        }
        // Protocol 776: signal end of chunk batch
        let _ = send_packet(io, &ChunkBatchFinished { batch_size: to_send.len() as i32 }).await;
    }

    // Evict chunks that left view distance (save dirty ones first, send forget packets)
    let evicted: Vec<_> = loaded_chunks
        .iter()
        .filter(|cp| !visible.contains(cp))
        .copied()
        .collect();
    let dirty_evicted: Vec<mc_world::chunk::Chunk> = evicted
        .iter()
        .filter_map(|cp| server.chunk_store.get(cp))
        .filter(|c| c.dirty)
        .map(|c| c.clone())
        .collect();
    if !dirty_evicted.is_empty() {
        let writer = mc_world::anvil::AnvilWriter::new();
        if let Err(e) = writer.write_chunks(&server.world_dir, &dirty_evicted) {
            tracing::warn!("Failed to save evicted dirty chunks: {}", e);
        }
    }
    for cp in &evicted {
        // Protocol 776: send forget chunk before removing
        let _ = send_packet(io, &ForgetLevelChunk { chunk_x: cp.x, chunk_z: cp.z }).await;
        server.chunk_store.remove(cp);
    }
    loaded_chunks.retain(|cp| visible.contains(cp));
    for cp in new_chunks {
        loaded_chunks.insert(cp);
    }
    if !evicted.is_empty() {
        debug!("Unloaded {} chunks ({} dirty saved)", evicted.len(), dirty_evicted.len());
    }
}

/// Play 阶段主循环
/// Resolve a sound name to a protocol sound ID (simple fallback mapping)
pub fn resolve_sound_id(name: &str) -> i32 {
    match name {
        "minecraft:block.note_block.pling" | "block.note_block.pling" => 400,
        "minecraft:block.note_block.harp" | "block.note_block.harp" => 401,
        "minecraft:entity.player.levelup" | "entity.player.levelup" => 500,
        "minecraft:entity.experience_orb.pickup" | "entity.experience_orb.pickup" => 501,
        "minecraft:entity.generic.explode" | "entity.generic.explode" => mc_core::sound::SoundIds::ENTITY_GENERIC_EXPLODE,
        "minecraft:entity.player.hurt" | "entity.player.hurt" => mc_core::sound::SoundIds::ENTITY_PLAYER_HURT,
        "minecraft:entity.generic.hurt" | "entity.generic.hurt" => mc_core::sound::SoundIds::ENTITY_GENERIC_HURT,
        _ => mc_core::sound::SoundIds::BLOCK_STONE_BREAK, // fallback
    }
}

/// Build UpdateRecipes packet from RecipeRegistry
fn build_update_recipes(reg: &mc_player::recipe::RecipeRegistry) -> mc_protocol::packets::play::UpdateRecipes {
    use mc_protocol::packets::play::{NetworkRecipe, UpdateRecipes};
    use mc_protocol::codec::*;
    let mut recipes = Vec::new();
    for recipe in &reg.recipes {
        let mut data = Vec::new();
        data.extend_from_slice(&write_string(&recipe.group));
        data.extend_from_slice(&write_varint_bytes(recipe.category));
        data.extend_from_slice(&write_varint_bytes(recipe.width as i32));
        data.extend_from_slice(&write_varint_bytes(recipe.height as i32));
        let ingr_count = recipe.ingredients.len();
        data.extend_from_slice(&write_varint_bytes(ingr_count as i32));
        for slot_ingredients in &recipe.ingredients {
            data.extend_from_slice(&write_varint_bytes(slot_ingredients.len() as i32));
            for &item_id in slot_ingredients {
                data.extend_from_slice(&write_bool(true));
                data.extend_from_slice(&write_varint_bytes(item_id as i32));
                data.push(1);
                data.extend_from_slice(&write_varint_bytes(0));
            }
        }
        data.extend_from_slice(&write_bool(true));
        data.extend_from_slice(&write_varint_bytes(recipe.result_item as i32));
        data.push(recipe.result_count);
        data.extend_from_slice(&write_varint_bytes(0));
        let recipe_type = if recipe.is_shapeless { "minecraft:crafting_shapeless" } else { "minecraft:crafting_shaped" };
        recipes.push(NetworkRecipe { recipe_type: recipe_type.into(), recipe_id: recipe.id.clone(), data });
    }
    UpdateRecipes { recipes }
}

/// Look up base weapon damage from held item protocol ID
pub(crate) fn weapon_damage(item_id: u32) -> f32 {
    match item_id {
        // Swords
        686 => 4.0,   // WOODEN_SWORD
        712 => 5.0,   // STONE_SWORD
        723 => 6.0,   // IRON_SWORD
        743 => 7.0,   // DIAMOND_SWORD
        755 => 8.0,   // NETHERITE_SWORD
        // Axes (higher base damage, slower attack speed — speed not implemented)
        691 => 7.0,   // WOODEN_AXE
        716 => 9.0,   // STONE_AXE
        727 => 9.0,   // IRON_AXE
        747 => 10.0,  // DIAMOND_AXE
        756 => 11.0,  // NETHERITE_AXE
        _ => 1.0,     // Fist/other items
    }
}

/// Get the surface Y level at a world position from a loaded chunk
fn find_surface_y(chunk: &mc_world::chunk::Chunk, local_x: usize, local_z: usize) -> f64 {
    // Search top-down through sections for the first non-air block
    for section_idx in (0..chunk.sections.len()).rev() {
        if let Some(ref section) = chunk.sections[section_idx] {
            let section_base_y = (section_idx as i32 - 4) * 16; // MIN_SECTION_Y = -4
            for sub_y in (0..16).rev() {
                let block = section.blocks.get(local_x, sub_y, local_z);
                if !block.is_air() && block.id != 267 { // not air and not water
                    return (section_base_y + sub_y as i32 + 1) as f64;
                }
            }
        }
    }
    64.0 // fallback: world spawn height
}


/// 编码并发送一个数据包
pub async fn send_packet(
    io: &mut PacketStream,
    encoder: &(dyn PacketEncoder + Sync),
) -> Result<(), CodecError> {
    let data = io.codec().encode(encoder)?;
    io.write_frame(&data)
        .await
        .map_err(CodecError::Io)
}

/// Send pre-encoded packet bytes directly — skips encoding for cached ChunkData.
async fn send_cached_packet(
    io: &mut PacketStream,
    data: &[u8],
) -> Result<(), CodecError> {
    io.write_frame(data)
        .await
        .map_err(CodecError::Io)
}

/// Send ChunkData to a player, using cached bytes when available.
/// Caches the encoded bytes for subsequent broadcasts (zero-allocation rebroadcast).
pub(crate) async fn send_chunk_data_cached(
    io: &mut PacketStream,
    chunk: &mut mc_world::chunk::Chunk,
) -> Result<(), CodecError> {
    // Check cache first — avoid re-encoding on rebroadcast
    if let Some(cached) = chunk.cached_chunk_bytes() {
        return send_cached_packet(io, &cached).await;
    }
    // Encode (borrows io immutably), drop borrow, then write
    let encoded = {
        let chunk_packet = chunk.to_chunk_data();
        io.codec().encode(&chunk_packet)?
    };
    let cached: std::sync::Arc<Vec<u8>> = std::sync::Arc::new(encoded);
    chunk.set_cached_bytes(std::sync::Arc::clone(&cached));
    send_cached_packet(io, &cached).await
}

/// Check an advancement criterion for a player and send UpdateAdvancements if completed.
/// Uses the shared registry from ServerRef (avoids re-creating AdvancementRegistry per call).
pub(crate) async fn fire_advancement(
    server: &ServerRef,
    io: &mut PacketStream,
    uuid: &uuid::Uuid,
    criterion: &mc_player::advancement::Criterion,
) {
    let newly_completed = server.advancement_tracker.write().check_criterion(
        uuid, criterion, &server.advancement_registry,
    );
    if !newly_completed.is_empty() {
        // Build criteria strings for progress mapping
        let progress_map: Vec<(String, Vec<String>)> = newly_completed
            .iter()
            .map(|id| {
                let criteria_strs: Vec<String> = server.advancement_registry
                    .get(id)
                    .map(|adv| adv.criteria.iter().map(|c| format!("{:?}", c)).collect())
                    .unwrap_or_default();
                (id.clone(), criteria_strs)
            })
            .collect();

        let update = mc_protocol::packets::play::UpdateAdvancements {
            reset: false,
            advancement_ids: newly_completed.clone(),
            progress_map,
        };
        let _ = send_packet(io, &update).await;
    }
}
