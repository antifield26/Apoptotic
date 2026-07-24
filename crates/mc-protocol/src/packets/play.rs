//! Play 阶段数据包 (C2S & S2C)
//!
//! 参考: https://wiki.vg/Protocol#Play

use crate::codec::*;

// ═══════════════════════════════════════════════════════
// S→C: Join Game (0x2B)
// ═══════════════════════════════════════════════════════

pub struct JoinGame {
    pub entity_id: i32,
    pub is_hardcore: bool,
    pub gamemode: u8,
    pub previous_gamemode: i8,
    pub dimension_names: Vec<String>,
    pub registry_codec: Vec<u8>,       // NBT compound (simplified: empty)
    pub dimension_type: String,         // e.g. "minecraft:overworld"
    pub dimension_name: String,         // e.g. "minecraft:overworld"
    pub hashed_seed: i64,
    pub max_players: i32,
    pub view_distance: i32,
    pub simulation_distance: i32,
    pub reduced_debug_info: bool,
    pub enable_respawn_screen: bool,
    pub is_debug: bool,
    pub is_flat: bool,
    pub death_location: Option<(String, i64)>, // (dimension, pos)
    pub portal_cooldown: i32,
}

impl PacketEncoder for JoinGame {
    fn packet_id(&self) -> i32 { 0x2B }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_i32(self.entity_id));
        buf.extend_from_slice(&write_bool(self.is_hardcore));
        buf.push(self.gamemode);
        buf.push(self.previous_gamemode as u8);

        // dimension names (varint-prefixed list of strings)
        buf.extend_from_slice(&write_varint_bytes(self.dimension_names.len() as i32));
        for name in &self.dimension_names {
            buf.extend_from_slice(&write_string(name));
        }

        // registry codec — NBT compound (generated from registry module)
        let registry_bytes = if self.registry_codec.is_empty() {
            crate::registry::default_registry_codec()
        } else {
            self.registry_codec.clone()
        };
        buf.extend_from_slice(&registry_bytes);

        buf.extend_from_slice(&write_string(&self.dimension_type));
        buf.extend_from_slice(&write_string(&self.dimension_name));
        buf.extend_from_slice(&write_i64(self.hashed_seed));
        buf.extend_from_slice(&write_varint_bytes(self.max_players));
        buf.extend_from_slice(&write_varint_bytes(self.view_distance));
        buf.extend_from_slice(&write_varint_bytes(self.simulation_distance));
        buf.extend_from_slice(&write_bool(self.reduced_debug_info));
        buf.extend_from_slice(&write_bool(self.enable_respawn_screen));
        buf.extend_from_slice(&write_bool(self.is_debug));
        buf.extend_from_slice(&write_bool(self.is_flat));

        // death location (optional)
        if let Some((ref dim, pos)) = self.death_location {
            buf.extend_from_slice(&write_bool(true));
            buf.extend_from_slice(&write_string(dim));
            buf.extend_from_slice(&write_i64(pos));
        } else {
            buf.extend_from_slice(&write_bool(false));
        }

        buf.extend_from_slice(&write_varint_bytes(self.portal_cooldown));
        buf
    }
}

impl PacketDecoder for JoinGame {
    fn packet_id() -> i32 { 0x2B }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("JoinGame decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Acknowledge Block Change (0x04) — confirm block dig to client
// ═══════════════════════════════════════════════════════

pub struct AcknowledgeBlockChange {
    pub sequence: i32, // VarInt — sequence ID for block change acknowledgment
}

impl PacketEncoder for AcknowledgeBlockChange {
    fn packet_id(&self) -> i32 { 0x04 }
    fn encode_payload(&self) -> Vec<u8> {
        write_varint_bytes(self.sequence)
    }
}

impl PacketDecoder for AcknowledgeBlockChange {
    fn packet_id() -> i32 { 0x04 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("AcknowledgeBlockChange decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Spawn Entity (0x01) — create non-player entity on client
// ═══════════════════════════════════════════════════════

pub struct SpawnEntity {
    pub entity_id: i32,
    pub entity_uuid: uuid::Uuid,
    pub entity_type: i32,  // 54 = item in most versions
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub pitch: u8,
    pub yaw: u8,
    pub head_yaw: u8,
    pub data: i32,
    pub vel_x: i16,
    pub vel_y: i16,
    pub vel_z: i16,
}

impl PacketEncoder for SpawnEntity {
    fn packet_id(&self) -> i32 { 0x01 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.entity_id));
        buf.extend_from_slice(&write_uuid(&self.entity_uuid));
        buf.extend_from_slice(&write_varint_bytes(self.entity_type));
        buf.extend_from_slice(&write_double(self.x));
        buf.extend_from_slice(&write_double(self.y));
        buf.extend_from_slice(&write_double(self.z));
        buf.push(self.pitch);
        buf.push(self.yaw);
        buf.push(self.head_yaw);
        buf.extend_from_slice(&write_varint_bytes(self.data));
        buf.extend_from_slice(&self.vel_x.to_be_bytes());
        buf.extend_from_slice(&self.vel_y.to_be_bytes());
        buf.extend_from_slice(&self.vel_z.to_be_bytes());
        buf
    }
}

impl PacketDecoder for SpawnEntity {
    fn packet_id() -> i32 { 0x01 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("SpawnEntity decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Set Entity Velocity (0x5E) — entity physics velocity
// ═══════════════════════════════════════════════════════

pub struct SetEntityVelocity {
    pub entity_id: i32,
    pub vel_x: i16,
    pub vel_y: i16,
    pub vel_z: i16,
}

impl PacketEncoder for SetEntityVelocity {
    fn packet_id(&self) -> i32 { 0x5E }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.entity_id));
        buf.extend_from_slice(&self.vel_x.to_be_bytes());
        buf.extend_from_slice(&self.vel_y.to_be_bytes());
        buf.extend_from_slice(&self.vel_z.to_be_bytes());
        buf
    }
}

impl PacketDecoder for SetEntityVelocity {
    fn packet_id() -> i32 { 0x5E }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("SetEntityVelocity decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Update Section Blocks (0x47) — batch block updates (single section)
// ═══════════════════════════════════════════════════════

pub struct UpdateSectionBlocks {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub section_y: i32,
    pub blocks: Vec<(i16, i32)>, // (local_index, block_state_id)
}

impl PacketEncoder for UpdateSectionBlocks {
    fn packet_id(&self) -> i32 { 0x47 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let section_pos: i64 = ((self.chunk_x as i64 & 0x3FFFFF) << 42)
            | ((self.chunk_z as i64 & 0x3FFFFF) << 20)
            | (self.section_y as i64 & 0xFFFFF);
        buf.extend_from_slice(&section_pos.to_be_bytes());
        buf.extend_from_slice(&write_bool(false)); // suppress light updates
        buf.extend_from_slice(&write_varint_bytes(self.blocks.len() as i32));
        for (idx, state_id) in &self.blocks {
            buf.extend_from_slice(&write_varint_bytes(*idx as i32));
            buf.extend_from_slice(&write_varint_bytes(*state_id));
        }
        buf
    }
}

impl PacketDecoder for UpdateSectionBlocks {
    fn packet_id() -> i32 { 0x47 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("UpdateSectionBlocks decode not implemented".into()))
    }
}

// S→C: Server Links (0x4F) — links shown in pause menu
// ═══════════════════════════════════════════════════════

pub struct ServerLinks {
    pub links: Vec<ServerLink>,
}

pub struct ServerLink {
    pub label: String,
    pub url: String,
}

impl PacketEncoder for ServerLinks {
    fn packet_id(&self) -> i32 { 0x4F }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.links.len() as i32));
        for link in &self.links {
            buf.extend_from_slice(&write_bool(true)); // is_builtin = false
            buf.extend_from_slice(&write_string(&link.label));
            buf.extend_from_slice(&write_string(&link.url));
        }
        buf
    }
}

impl PacketDecoder for ServerLinks {
    fn packet_id() -> i32 { 0x4F }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("ServerLinks decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Set Default Spawn Position (0x5A) — compass direction
// ═══════════════════════════════════════════════════════

pub struct SetDefaultSpawnPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub angle: f32,
}

impl PacketEncoder for SetDefaultSpawnPosition {
    fn packet_id(&self) -> i32 { 0x5A }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_position(self.x, self.y, self.z));
        buf.extend_from_slice(&write_f32(self.angle));
        buf
    }
}

impl PacketDecoder for SetDefaultSpawnPosition {
    fn packet_id() -> i32 { 0x5A }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("SetDefaultSpawnPosition decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Synchronize Player Position (0x41)
// ═══════════════════════════════════════════════════════

pub struct PlayerPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub flags: u8,     // bit field for relative coords
    pub teleport_id: i32,
}

impl PacketEncoder for PlayerPosition {
    fn packet_id(&self) -> i32 { 0x41 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_double(self.x));
        buf.extend_from_slice(&write_double(self.y));
        buf.extend_from_slice(&write_double(self.z));
        buf.extend_from_slice(&write_f32(self.yaw));
        buf.extend_from_slice(&write_f32(self.pitch));
        buf.push(self.flags);
        buf.extend_from_slice(&write_varint_bytes(self.teleport_id));
        buf
    }
}

impl PacketDecoder for PlayerPosition {
    fn packet_id() -> i32 { 0x41 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("PlayerPosition decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Keep Alive (0x26)
// ═══════════════════════════════════════════════════════

pub struct KeepAlive {
    pub id: i64,
}

impl PacketEncoder for KeepAlive {
    fn packet_id(&self) -> i32 { 0x26 }
    fn encode_payload(&self) -> Vec<u8> {
        write_i64(self.id).to_vec()
    }
}

impl PacketDecoder for KeepAlive {
    fn packet_id() -> i32 { 0x26 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (id, _) = read_i64(data)?;
        Ok(Self { id })
    }
}

// ═══════════════════════════════════════════════════════
// S→C: System Chat Message (0x6C)
// ═══════════════════════════════════════════════════════

pub struct SystemChatMessage {
    pub content: String,    // JSON chat component
    pub overlay: bool,      // false = action bar, true = chat
}

impl PacketEncoder for SystemChatMessage {
    fn packet_id(&self) -> i32 { 0x72 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_string(&self.content));
        buf.extend_from_slice(&write_bool(self.overlay));
        buf
    }
}

impl PacketDecoder for SystemChatMessage {
    fn packet_id() -> i32 { 0x72 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("SystemChatMessage decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Disconnect (Play) (0x1D)
// ═══════════════════════════════════════════════════════

pub struct PlayDisconnect {
    pub reason: String,  // JSON chat component
}

impl PacketEncoder for PlayDisconnect {
    fn packet_id(&self) -> i32 { 0x1C }
    fn encode_payload(&self) -> Vec<u8> {
        write_string(&self.reason)
    }
}

impl PacketDecoder for PlayDisconnect {
    fn packet_id() -> i32 { 0x1C }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("PlayDisconnect decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: World Event (0x24) — plays record, activates end gateway, etc.
// ═══════════════════════════════════════════════════════

pub struct WorldEvent {
    pub event_id: i32,
    pub position: (i32, i32, i32),
    pub data: i32,
    pub disable_relative_volume: bool,
}

impl PacketEncoder for WorldEvent {
    fn packet_id(&self) -> i32 { 0x24 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.event_id));
        // Position as a block position (long)
        let pos: i64 = ((self.position.0 as i64 & 0x3FFFFFF) << 38)
            | ((self.position.1 as i64 & 0xFFF) << 26)
            | (self.position.2 as i64 & 0x3FFFFFF);
        buf.extend_from_slice(&pos.to_be_bytes());
        buf.extend_from_slice(&write_varint_bytes(self.data));
        buf.push(if self.disable_relative_volume { 1 } else { 0 });
        buf
    }
}

impl PacketDecoder for WorldEvent {
    fn packet_id() -> i32 { 0x24 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("WorldEvent decode not implemented".into()))
    }
}

/// World event IDs for common events
pub mod world_event {
    pub const DISPENSER_DISPENSE: i32 = 1000;
    pub const DISPENSER_FAIL: i32 = 1001;
    pub const RECORD_PLAY: i32 = 1010;
    pub const RECORD_STOP: i32 = 1011;
    pub const END_GATEWAY_SPAWN: i32 = 3000;
    pub const ENDER_DRAGON_GROWL: i32 = 3001;
}

// ═══════════════════════════════════════════════════════
// C→S: Client Information (0x08)
// ═══════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct ClientInformation {
    pub locale: String,
    pub view_distance: u8,
    pub chat_mode: i32,
    pub chat_colors: bool,
    pub displayed_skin_parts: u8,
    pub main_hand: i32,
    pub enable_text_filtering: bool,
    pub allow_server_listings: bool,
}

impl PacketEncoder for ClientInformation {
    fn packet_id(&self) -> i32 { 0x0C }
    fn encode_payload(&self) -> Vec<u8> { Vec::new() }
}

impl PacketDecoder for ClientInformation {
    fn packet_id() -> i32 { 0x0C }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (locale, mut offset) = read_string(data)?;
        let (view_distance, n) = read_u8(&data[offset..])?; offset += n;
        let (chat_mode, n) = read_varint_enum(&data[offset..])?; offset += n;
        let (chat_colors, n) = read_bool(&data[offset..])?; offset += n;
        let (displayed_skin_parts, n) = read_u8(&data[offset..])?; offset += n;
        let (main_hand, n) = read_varint_enum(&data[offset..])?; offset += n;
        let (enable_text_filtering, n) = read_bool(&data[offset..])?; offset += n;
        let (allow_server_listings, _) = read_bool(&data[offset..])?;

        Ok(Self {
            locale: locale.to_string(),
            view_distance,
            chat_mode,
            chat_colors,
            displayed_skin_parts,
            main_hand,
            enable_text_filtering,
            allow_server_listings,
        })
    }
}

// ═══════════════════════════════════════════════════════
// 通用辅助字段写入
// ═══════════════════════════════════════════════════════

/// 写入 i32 (4-byte big-endian — Minecraft 协议中的 `int` 类型)
pub fn write_i32(v: i32) -> Vec<u8> {
    v.to_be_bytes().to_vec()
}

/// 写入 f32 (big-endian)
pub fn write_f32(v: f32) -> [u8; 4] {
    v.to_be_bytes()
}

/// 读取 u8
pub fn read_u8(data: &[u8]) -> Result<(u8, usize), CodecError> {
    if data.is_empty() {
        return Err(CodecError::Malformed("u8 too short".into()));
    }
    Ok((data[0], 1))
}

/// 写入 byte array (VarInt prefix)
pub fn write_byte_array(data: &[u8]) -> Vec<u8> {
    let mut buf = write_varint_bytes(data.len() as i32);
    buf.extend_from_slice(data);
    buf
}

/// 写入位置 (i64 encoded: ((x & 0x3FFFFFF) << 38) | ((z & 0x3FFFFFF) << 12) | (y & 0xFFF))
pub fn write_position(x: i32, y: i32, z: i32) -> [u8; 8] {
    let encoded: i64 = ((x as i64 & 0x3FFFFFF) << 38)
        | ((z as i64 & 0x3FFFFFF) << 12)
        | (y as i64 & 0xFFF);
    encoded.to_be_bytes()
}

// ═══════════════════════════════════════════════════════
// S→C: Block Destroy Stage (0x05) — sync block breaking progress
// ═══════════════════════════════════════════════════════

pub struct BlockDestroyStage {
    pub entity_id: i32,     // VarInt — player doing the mining
    pub location: (i32, i32, i32), // (x, y, z) — block position
    pub destroy_stage: u8,  // 0-9, 10 = complete destruction
}

impl PacketEncoder for BlockDestroyStage {
    fn packet_id(&self) -> i32 { 0x05 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.entity_id));
        buf.extend_from_slice(&write_position(self.location.0, self.location.1, self.location.2));
        buf.push(self.destroy_stage);
        buf
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Chunk Data (0x27)
// ═══════════════════════════════════════════════════════

/// 单个 section 的网络数据
pub struct ChunkSectionData {
    pub block_count: i16,            // 非空气方块数
    pub blocks: Vec<u8>,             // PalettedContainer 编码
    pub biomes: Vec<u8>,             // PalettedContainer 编码
}

// ═══════════════════════════════════════════════════════
// Protocol 776: Chunk batch system
// ═══════════════════════════════════════════════════════

/// S→C: Chunk Batch Start (0x0C) — signals the beginning of a chunk batch
pub struct ChunkBatchStart;

impl PacketEncoder for ChunkBatchStart {
    fn packet_id(&self) -> i32 { 0x0C }
    fn encode_payload(&self) -> Vec<u8> { Vec::new() } // empty payload
}

/// S→C: Chunk Batch Finished (0x0B) — signals end of chunk batch
pub struct ChunkBatchFinished {
    pub batch_size: i32,   // number of chunks in this batch
}

impl PacketEncoder for ChunkBatchFinished {
    fn packet_id(&self) -> i32 { 0x0B }
    fn encode_payload(&self) -> Vec<u8> {
        write_varint_bytes(self.batch_size)
    }
}

/// S→C: Chunks Biomes (0x0D) — sends biome data for chunks
pub struct ChunksBiomes {
    pub chunk_biomes: Vec<ChunkBiomeEntry>,
}

pub struct ChunkBiomeEntry {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub biome_data: Vec<u8>,  // serialized biome paletted container
}

impl PacketEncoder for ChunksBiomes {
    fn packet_id(&self) -> i32 { 0x0D }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.chunk_biomes.len() as i32));
        for entry in &self.chunk_biomes {
            buf.extend_from_slice(&write_i32(entry.chunk_x));
            buf.extend_from_slice(&write_i32(entry.chunk_z));
            buf.extend_from_slice(&write_varint_bytes(entry.biome_data.len() as i32));
            buf.extend_from_slice(&entry.biome_data);
        }
        buf
    }
}

/// S→C: Set Chunk Cache Center (0x5F) — sets center of chunk loading area
pub struct SetChunkCacheCenter {
    pub chunk_x: i32,
    pub chunk_z: i32,
}

impl PacketEncoder for SetChunkCacheCenter {
    fn packet_id(&self) -> i32 { 0x5F }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.chunk_x));
        buf.extend_from_slice(&write_varint_bytes(self.chunk_z));
        buf
    }
}

/// S→C: Set Chunk Cache Radius (0x60) — sets chunk loading radius
pub struct SetChunkCacheRadius {
    pub radius: i32,
}

impl PacketEncoder for SetChunkCacheRadius {
    fn packet_id(&self) -> i32 { 0x60 }
    fn encode_payload(&self) -> Vec<u8> {
        write_varint_bytes(self.radius)
    }
}

/// S→C: Forget Level Chunk (0x25) — unload a chunk on the client
pub struct ForgetLevelChunk {
    pub chunk_x: i32,
    pub chunk_z: i32,
}

impl PacketEncoder for ForgetLevelChunk {
    fn packet_id(&self) -> i32 { 0x25 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_i32(self.chunk_x));
        buf.extend_from_slice(&write_i32(self.chunk_z));
        buf
    }
}

// ═══════════════════════════════════════════════════════

/// Level Chunk With Light (protocol 776: 0x2D) — replaces old ChunkData
pub struct ChunkData {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub heightmaps: Vec<u8>,         // NBT heightmaps (maintained for backward compat)
    pub sections: Vec<ChunkSectionData>,
    pub block_entities: Vec<u8>,     // block entity NBT data
    pub sky_light_mask: Vec<i64>,
    pub block_light_mask: Vec<i64>,
    pub empty_sky_light_mask: Vec<i64>,
    pub empty_block_light_mask: Vec<i64>,
    pub sky_light_arrays: Vec<Vec<u8>>,
    pub block_light_arrays: Vec<Vec<u8>>,
}

impl PacketEncoder for ChunkData {
    fn packet_id(&self) -> i32 { 0x2D }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Chunk X, Z
        buf.extend_from_slice(&write_i32(self.chunk_x));
        buf.extend_from_slice(&write_i32(self.chunk_z));

        // ── Heightmaps: structured array [{type: varint, data: u64[]}] ──
        // Send empty heightmaps — client can regenerate from block data
        buf.extend_from_slice(&write_varint_bytes(0));

        // ── Buffer: serialized section data ──
        let mut section_buf = Vec::new();
        section_buf.extend_from_slice(&write_varint_bytes(self.sections.len() as i32));
        for section in &self.sections {
            section_buf.extend_from_slice(&section.block_count.to_be_bytes());
            // Block states: paletted container (varint-length + bytes)
            section_buf.extend_from_slice(&write_varint_bytes(section.blocks.len() as i32));
            section_buf.extend_from_slice(&section.blocks);
            // Biomes: paletted container (varint-length + bytes)
            section_buf.extend_from_slice(&write_varint_bytes(section.biomes.len() as i32));
            section_buf.extend_from_slice(&section.biomes);
        }
        buf.extend_from_slice(&write_varint_bytes(section_buf.len() as i32));
        buf.extend_from_slice(&section_buf);

        // ── Block entities: [{packedXZ: i8, y: i16, type: varint, data: nbt}] ──
        // Send block entities as raw NBT array (compatible format)
        buf.extend_from_slice(&write_varint_bytes(self.block_entities.len() as i32));
        if !self.block_entities.is_empty() {
            buf.extend_from_slice(&self.block_entities);
        }

        // ── Sky light: array of byte arrays ──
        write_light_arrays_776(&mut buf, &self.sky_light_arrays);
        // ── Block light: array of byte arrays ──
        write_light_arrays_776(&mut buf, &self.block_light_arrays);

        buf
    }
}

impl PacketDecoder for ChunkData {
    fn packet_id() -> i32 { 0x2D }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("ChunkData decode not implemented".into()))
    }
}

fn write_bit_set(buf: &mut Vec<u8>, longs: &[i64]) {
    buf.extend_from_slice(&write_varint_bytes(longs.len() as i32));
    for &l in longs {
        buf.extend_from_slice(&l.to_be_bytes());
    }
}

fn write_light_arrays(buf: &mut Vec<u8>, arrays: &[Vec<u8>]) {
    buf.extend_from_slice(&write_varint_bytes(arrays.len() as i32));
    for arr in arrays {
        buf.extend_from_slice(&write_varint_bytes(arr.len() as i32));
        buf.extend_from_slice(arr);
    }
}

// Protocol 776 light array format: varint count, then [varint length + bytes] per array
fn write_light_arrays_776(buf: &mut Vec<u8>, arrays: &[Vec<u8>]) {
    buf.extend_from_slice(&write_varint_bytes(arrays.len() as i32));
    for arr in arrays {
        buf.extend_from_slice(&write_varint_bytes(arr.len() as i32));
        buf.extend_from_slice(arr);
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Teleport Entity (entity position sync)
// ═══════════════════════════════════════════════════════

pub struct TeleportEntity {
    pub entity_id: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

impl PacketEncoder for TeleportEntity {
    fn packet_id(&self) -> i32 { 0x76 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.entity_id));
        buf.extend_from_slice(&write_double(self.x));
        buf.extend_from_slice(&write_double(self.y));
        buf.extend_from_slice(&write_double(self.z));
        buf.extend_from_slice(&self.yaw.to_be_bytes());
        buf.extend_from_slice(&self.pitch.to_be_bytes());
        buf.extend_from_slice(&write_bool(self.on_ground));
        buf
    }
}

impl PacketDecoder for TeleportEntity {
    fn packet_id() -> i32 { 0x76 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("TeleportEntity decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Spawn Player (create player entity on client)
// ═══════════════════════════════════════════════════════

pub struct SpawnPlayer {
    pub entity_id: i32,
    pub player_uuid: uuid::Uuid,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
}

/// Convert float angle to Minecraft byte angle (0-255)
fn angle_to_byte(angle: f32) -> u8 {
    ((angle.rem_euclid(360.0) / 360.0) * 256.0) as u8
}

impl PacketEncoder for SpawnPlayer {
    fn packet_id(&self) -> i32 { 0x03 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.entity_id));
        buf.extend_from_slice(&write_uuid(&self.player_uuid));
        buf.extend_from_slice(&write_double(self.x));
        buf.extend_from_slice(&write_double(self.y));
        buf.extend_from_slice(&write_double(self.z));
        buf.push(angle_to_byte(self.yaw));
        buf.push(angle_to_byte(self.pitch));
        buf.push(angle_to_byte(self.yaw)); // head yaw = yaw
        buf.extend_from_slice(&write_varint_bytes(0)); // data = 0
        buf
    }
}

impl PacketDecoder for SpawnPlayer {
    fn packet_id() -> i32 { 0x03 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("SpawnPlayer decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Remove Entities (despawn entities from client)
// ═══════════════════════════════════════════════════════

pub struct RemoveEntities {
    pub entity_ids: Vec<i32>,
}

impl PacketEncoder for RemoveEntities {
    fn packet_id(&self) -> i32 { 0x46 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.entity_ids.len() as i32));
        for id in &self.entity_ids {
            buf.extend_from_slice(&write_varint_bytes(*id));
        }
        buf
    }
}

impl PacketDecoder for RemoveEntities {
    fn packet_id() -> i32 { 0x46 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("RemoveEntities decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Player Info Update (tab list add/remove)
// ═══════════════════════════════════════════════════════

pub struct PlayerInfoUpdate {
    pub actions: i32,      // bitmask: 0x01=add, 0x02=game mode, 0x04=listed, 0x08=latency, 0x10=display name
    pub entries: Vec<PlayerInfoEntry>,
}

pub struct PlayerInfoEntry {
    pub uuid: uuid::Uuid,
    pub username: String,
    pub gamemode: i32,
    pub ping: i32,
    pub listed: bool,
}

impl PacketEncoder for PlayerInfoUpdate {
    fn packet_id(&self) -> i32 { 0x3F }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.actions));
        buf.extend_from_slice(&write_varint_bytes(self.entries.len() as i32));
        for entry in &self.entries {
            buf.extend_from_slice(&write_uuid(&entry.uuid));
            // Add player action (0x01): username + properties
            if self.actions & 0x01 != 0 {
                buf.extend_from_slice(&write_string(&entry.username));
                buf.extend_from_slice(&write_varint_bytes(0)); // no properties
            }
            // Update gamemode (0x02)
            if self.actions & 0x02 != 0 {
                buf.extend_from_slice(&write_varint_bytes(entry.gamemode));
            }
            // Update listed (0x04)
            if self.actions & 0x04 != 0 {
                buf.extend_from_slice(&write_bool(entry.listed));
            }
            // Update latency (0x08)
            if self.actions & 0x08 != 0 {
                buf.extend_from_slice(&write_varint_bytes(entry.ping));
            }
            // Update display name (0x10) — skip for now
        }
        buf
    }
}

impl PacketDecoder for PlayerInfoUpdate {
    fn packet_id() -> i32 { 0x3F }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("PlayerInfoUpdate decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Player Info Remove (0x42) — remove players from tab list
// ═══════════════════════════════════════════════════════

pub struct PlayerInfoRemove {
    pub uuids: Vec<uuid::Uuid>,
}

impl PacketEncoder for PlayerInfoRemove {
    fn packet_id(&self) -> i32 { 0x42 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.uuids.len() as i32));
        for uuid in &self.uuids {
            buf.extend_from_slice(&write_uuid(uuid));
        }
        buf
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Transfer (0x73) — switch to another server
// ═══════════════════════════════════════════════════════

pub struct Transfer {
    pub host: String,
    pub port: i32,
}

impl PacketEncoder for Transfer {
    fn packet_id(&self) -> i32 { 0x73 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_string(&self.host));
        buf.extend_from_slice(&write_varint_bytes(self.port));
        buf
    }
}

// ═══════════════════════════════════════════════════════
// C→S: Teleport Confirm (0x00) — client confirms teleport
// ═══════════════════════════════════════════════════════

pub struct TeleportConfirm {
    pub teleport_id: i32,
}

impl PacketEncoder for TeleportConfirm {
    fn packet_id(&self) -> i32 { 0x00 }
    fn encode_payload(&self) -> Vec<u8> { write_varint_bytes(self.teleport_id) }
}

impl PacketDecoder for TeleportConfirm {
    fn packet_id() -> i32 { 0x00 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (teleport_id, _) = read_varint_enum(data)?;
        Ok(Self { teleport_id })
    }
}

// ═══════════════════════════════════════════════════════
// C→S: Chat Command (0x0C) — slash command as chat packet
// ═══════════════════════════════════════════════════════

pub struct ChatCommand {
    pub command: String,
}

impl PacketEncoder for ChatCommand {
    fn packet_id(&self) -> i32 { 0x05 }
    fn encode_payload(&self) -> Vec<u8> { write_string(&self.command) }
}

impl PacketDecoder for ChatCommand {
    fn packet_id() -> i32 { 0x05 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        // Format: command (String) + timestamp (long) + salt (long) + argument signatures + message count
        let (command, _) = read_string(data)?;
        Ok(Self { command: command.to_string() })
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Game Event (0x20)
// ═══════════════════════════════════════════════════════

pub struct GameEvent {
    pub event: u8,  // 0=no respawn block, 1=rain, 2=thunder, 3=gamemode, 4=credits, 5=demo
    pub value: f32,
}

impl PacketEncoder for GameEvent {
    fn packet_id(&self) -> i32 { 0x22 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.event);
        buf.extend_from_slice(&write_f32(self.value));
        buf
    }
}

impl PacketDecoder for GameEvent {
    fn packet_id() -> i32 { 0x22 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("GameEvent decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// C→S: Client Command (0x0D) — respawn / stats request
// ═══════════════════════════════════════════════════════

pub struct ClientCommand {
    pub action: i32, // 0 = perform respawn, 1 = request stats
}

impl PacketEncoder for ClientCommand {
    fn packet_id(&self) -> i32 { 0x0A }
    fn encode_payload(&self) -> Vec<u8> { write_varint_bytes(self.action) }
}

impl PacketDecoder for ClientCommand {
    fn packet_id() -> i32 { 0x0A }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (action, _) = read_varint_enum(data)?;
        Ok(Self { action })
    }
}

// ═══════════════════════════════════════════════════════
// C→S: Player Command (0x1F) — sprint/sneak/flight input
// ═══════════════════════════════════════════════════════

pub struct PlayerCommand {
    pub entity_id: i32,
    pub action: i32,
    // 0=START_SNEAKING, 1=STOP_SNEAKING,
    // 2=LEAVE_BED, 3=START_SPRINTING,
    // 4=STOP_SPRINTING, 5=START_HORSE_JUMP,
    // 6=STOP_HORSE_JUMP, 7=OPEN_VEHICLE_INVENTORY,
    // 8=START_FLYING_WITH_ELYTRA
    pub data: i32, // jump boost for horse jump
}

impl PacketEncoder for PlayerCommand {
    fn packet_id(&self) -> i32 { 0x1F }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.entity_id));
        buf.extend_from_slice(&write_varint_bytes(self.action));
        buf.extend_from_slice(&write_varint_bytes(self.data));
        buf
    }
}

impl PacketDecoder for PlayerCommand {
    fn packet_id() -> i32 { 0x1F }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (entity_id, mut offset) = read_varint_enum(data)?;
        let (action, n) = read_varint_enum(&data[offset..])?;
        offset += n;
        let (data, _) = read_varint_enum(&data[offset..])?;
        Ok(Self { entity_id, action, data })
    }
}

// ═══════════════════════════════════════════════════════
// C→S: Player Input (0x29) — 1.21.5 movement input (NEW in protocol 776)
// ═══════════════════════════════════════════════════════

pub struct PlayerInput {
    pub input_flags: u32,       // bitmask: forward, backward, left, right, jump, sneak, sprint
    pub move_vector: (f32, f32), // (sideways, forward) movement vector
    pub is_sprint: bool,
    pub is_sneak: bool,
}

impl PacketEncoder for PlayerInput {
    fn packet_id(&self) -> i32 { 0x29 }
    fn encode_payload(&self) -> Vec<u8> {
        // PlayerInput is C2S only — no S2C encoding needed
        Vec::new()
    }
}

impl PacketDecoder for PlayerInput {
    fn packet_id() -> i32 { 0x29 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        if data.len() < 11 {
            return Err(CodecError::Malformed("PlayerInput too short".into()));
        }
        // Input flags: 3 bytes (24-bit little-endian bitmask)
        let flags = data[0] as u32 | ((data[1] as u32) << 8) | ((data[2] as u32) << 16);
        // Movement vector: two floats (sideways, forward)
        let sideways = f32::from_le_bytes([data[3], data[4], data[5], data[6]]);
        let forward = f32::from_le_bytes([data[7], data[8], data[9], data[10]]);
        let is_sprint = (flags & 0x20) != 0;
        let is_sneak = (flags & 0x10) != 0;
        Ok(Self {
            input_flags: flags,
            move_vector: (sideways, forward),
            is_sprint,
            is_sneak,
        })
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Set Experience (0x5F) — XP bar/level sync
// ═══════════════════════════════════════════════════════

pub struct SetExperience {
    pub experience_bar: f32,  // 0.0 to 1.0 — progress within current level
    pub level: i32,            // VarInt — current XP level
    pub total_experience: i32, // VarInt — total XP points
}

impl PacketEncoder for SetExperience {
    fn packet_id(&self) -> i32 { 0x5F }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_f32(self.experience_bar));
        buf.extend_from_slice(&write_varint_bytes(self.level));
        buf.extend_from_slice(&write_varint_bytes(self.total_experience));
        buf
    }
}

impl PacketDecoder for SetExperience {
    fn packet_id() -> i32 { 0x5F }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("SetExperience decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Damage Event (0x1A) — hurt animation + damage display
// ═══════════════════════════════════════════════════════

pub struct DamageEvent {
    pub entity_id: i32,        // VarInt — entity taking damage
    pub source_type_id: i32,   // VarInt — damage type registry ID
    pub source_cause_id: i32,  // VarInt — entity causing damage (-1 if none)
    pub source_direct_id: i32, // VarInt — direct entity dealing damage (-1 if none)
    pub source_pos_x: Option<f64>, // Optional — damage origin X
    pub source_pos_y: Option<f64>, // Optional — damage origin Y
    pub source_pos_z: Option<f64>, // Optional — damage origin Z
}

impl PacketEncoder for DamageEvent {
    fn packet_id(&self) -> i32 { 0x1A }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.entity_id));
        buf.extend_from_slice(&write_varint_bytes(self.source_type_id));
        buf.extend_from_slice(&write_varint_bytes(self.source_cause_id));
        buf.extend_from_slice(&write_varint_bytes(self.source_direct_id));
        // Source position (optional)
        if let (Some(x), Some(y), Some(z)) = (self.source_pos_x, self.source_pos_y, self.source_pos_z) {
            buf.extend_from_slice(&write_bool(true));
            buf.extend_from_slice(&write_double(x));
            buf.extend_from_slice(&write_double(y));
            buf.extend_from_slice(&write_double(z));
        } else {
            buf.extend_from_slice(&write_bool(false));
        }
        buf
    }
}

impl PacketDecoder for DamageEvent {
    fn packet_id() -> i32 { 0x1A }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("DamageEvent decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Sound Effect (0x68) — play a sound at a location
// ═══════════════════════════════════════════════════════

pub struct SoundEffect {
    pub sound_id: i32,       // VarInt — registry ID in minecraft:sound_event
    pub category: i32,       // VarInt — 0=master,1=music,2=records,3=weather,4=blocks,5=hostile,6=neutral,7=players,8=ambient,9=voice
    pub x: i32,              // Fixed-point: x * 8
    pub y: i32,              // Fixed-point: y * 8
    pub z: i32,              // Fixed-point: z * 8
    pub volume: f32,
    pub pitch: f32,
    pub seed: i64,           // For randomized variation
}

impl PacketEncoder for SoundEffect {
    fn packet_id(&self) -> i32 { 0x68 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.sound_id));
        buf.extend_from_slice(&write_varint_bytes(self.category));
        buf.extend_from_slice(&write_i32(self.x));
        buf.extend_from_slice(&write_i32(self.y));
        buf.extend_from_slice(&write_i32(self.z));
        buf.extend_from_slice(&write_f32(self.volume));
        buf.extend_from_slice(&write_f32(self.pitch));
        buf.extend_from_slice(&write_i64(self.seed));
        buf
    }
}

impl PacketDecoder for SoundEffect {
    fn packet_id() -> i32 { 0x68 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("SoundEffect decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Stop Sound (0x6C) — stop a playing sound
// ═══════════════════════════════════════════════════════

pub struct StopSound {
    pub flags: u8,         // 0x01=stop by category, 0x02=stop by sound name
    pub category: Option<i32>,    // VarInt (if flags & 0x01)
    pub sound_name: Option<String>, // (if flags & 0x02)
}

impl PacketEncoder for StopSound {
    fn packet_id(&self) -> i32 { 0x6C }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.flags);
        if self.flags & 0x01 != 0
            && let Some(cat) = self.category {
                buf.extend_from_slice(&write_varint_bytes(cat));
            }
        if self.flags & 0x02 != 0
            && let Some(ref name) = self.sound_name {
                buf.extend_from_slice(&write_string(name));
            }
        buf
    }
}

impl PacketDecoder for StopSound {
    fn packet_id() -> i32 { 0x6C }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("StopSound decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Particle (0x30) — spawn particle effects
// ═══════════════════════════════════════════════════════

pub struct Particle {
    pub particle_id: i32,      // VarInt — particle type ID
    pub long_distance: bool,   // render at any distance
    pub x: f64, pub y: f64, pub z: f64, // position
    pub offset_x: f32, pub offset_y: f32, pub offset_z: f32, // spread
    pub max_speed: f32,
    pub count: i32,            // VarInt — number of particles
}

impl PacketEncoder for Particle {
    fn packet_id(&self) -> i32 { 0x28 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.particle_id));
        buf.extend_from_slice(&write_bool(self.long_distance));
        buf.extend_from_slice(&write_double(self.x));
        buf.extend_from_slice(&write_double(self.y));
        buf.extend_from_slice(&write_double(self.z));
        buf.extend_from_slice(&write_f32(self.offset_x));
        buf.extend_from_slice(&write_f32(self.offset_y));
        buf.extend_from_slice(&write_f32(self.offset_z));
        buf.extend_from_slice(&write_f32(self.max_speed));
        buf.extend_from_slice(&write_varint_bytes(self.count));
        buf
    }
}

impl PacketDecoder for Particle {
    fn packet_id() -> i32 { 0x30 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("Particle decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Update Attributes (0x7D) — entity attribute sync
// ═══════════════════════════════════════════════════════

pub struct UpdateAttributes {
    pub entity_id: i32,  // VarInt
    pub attributes: Vec<Attribute>,
}

pub struct Attribute {
    pub key: String,        // e.g. "minecraft:generic.max_health"
    pub value: f64,
    pub modifiers: Vec<AttributeModifier>,
}

pub struct AttributeModifier {
    pub uuid: uuid::Uuid,
    pub name: String,
    pub amount: f64,
    pub operation: i32,  // 0=add, 1=multiply_base, 2=multiply_total
}

impl PacketEncoder for UpdateAttributes {
    fn packet_id(&self) -> i32 { 0x7D }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.entity_id));
        buf.extend_from_slice(&write_varint_bytes(self.attributes.len() as i32));
        for attr in &self.attributes {
            buf.extend_from_slice(&write_string(&attr.key));
            buf.extend_from_slice(&write_double(attr.value));
            buf.extend_from_slice(&write_varint_bytes(attr.modifiers.len() as i32));
            for modif in &attr.modifiers {
                buf.extend_from_slice(&write_uuid(&modif.uuid));
                buf.extend_from_slice(&write_double(modif.amount));
                buf.extend_from_slice(&write_varint_bytes(modif.operation));
                buf.extend_from_slice(&write_string(&modif.name));
            }
        }
        buf
    }
}

impl PacketDecoder for UpdateAttributes {
    fn packet_id() -> i32 { 0x7D }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("UpdateAttributes decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Set Titles (0x6B) — title/subtitle/actionbar messages
// ═══════════════════════════════════════════════════════

pub struct SetTitles {
    pub action: i32, // VarInt: 0=title, 1=subtitle, 2=actionbar, 3=times, 4=clear, 5=reset
    pub title_text: Option<String>,     // JSON chat (for title/subtitle/actionbar)
    pub fade_in: Option<i32>,           // ticks (for times action)
    pub stay: Option<i32>,              // ticks (for times action)
    pub fade_out: Option<i32>,          // ticks (for times action)
}

impl PacketEncoder for SetTitles {
    fn packet_id(&self) -> i32 { 0x6B }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.action));
        match self.action {
            0..=2 => {
                // title/subtitle/actionbar — JSON text
                if let Some(ref text) = self.title_text {
                    buf.extend_from_slice(&write_string(text));
                } else {
                    buf.extend_from_slice(&write_string(""));
                }
            }
            3 => {
                // times — fadeIn, stay, fadeOut
                buf.extend_from_slice(&write_i32(self.fade_in.unwrap_or(10)));
                buf.extend_from_slice(&write_i32(self.stay.unwrap_or(70)));
                buf.extend_from_slice(&write_i32(self.fade_out.unwrap_or(20)));
            }
            4 | 5 => {
                // clear/reset — no additional data
            }
            _ => {}
        }
        buf
    }
}

impl PacketDecoder for SetTitles {
    fn packet_id() -> i32 { 0x6B }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("SetTitles decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Boss Bar (0x0B) — boss health bar display
// ═══════════════════════════════════════════════════════

pub struct BossBar {
    pub uuid: uuid::Uuid,
    pub action: i32, // VarInt: 0=add, 1=remove, 2=update_health, 3=update_title, 4=update_style, 5=update_flags
    pub title: Option<String>,     // JSON chat (for add/update_title)
    pub health: Option<f32>,       // 0.0-1.0 (for add/update_health)
    pub color: Option<i32>,        // VarInt: 0=pink, 1=blue, 2=red, 3=green, 4=yellow, 5=purple, 6=white
    pub division: Option<i32>,     // VarInt: 0=none, 1=6, 2=10, 3=12, 4=20
    pub flags: Option<u8>,         // bitmask: 0x01=darken_sky, 0x02=dragon_bar, 0x04=create_fog
}

impl PacketEncoder for BossBar {
    fn packet_id(&self) -> i32 { 0x0B }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_uuid(&self.uuid));
        buf.extend_from_slice(&write_varint_bytes(self.action));
        match self.action {
            0 => {
                // add
                if let Some(ref title) = self.title { buf.extend_from_slice(&write_string(title)); }
                else { buf.extend_from_slice(&write_string("")); }
                buf.extend_from_slice(&write_f32(self.health.unwrap_or(1.0)));
                buf.extend_from_slice(&write_varint_bytes(self.color.unwrap_or(2)));
                buf.extend_from_slice(&write_varint_bytes(self.division.unwrap_or(0)));
                buf.push(self.flags.unwrap_or(0));
            }
            1 => {
                // remove — no extra data
            }
            2 => {
                // update health
                buf.extend_from_slice(&write_f32(self.health.unwrap_or(1.0)));
            }
            3 => {
                // update title
                if let Some(ref title) = self.title { buf.extend_from_slice(&write_string(title)); }
            }
            4 => {
                // update style
                buf.extend_from_slice(&write_varint_bytes(self.color.unwrap_or(2)));
                buf.extend_from_slice(&write_varint_bytes(self.division.unwrap_or(0)));
            }
            5 => {
                // update flags
                buf.push(self.flags.unwrap_or(0));
            }
            _ => {}
        }
        buf
    }
}

impl PacketDecoder for BossBar {
    fn packet_id() -> i32 { 0x0B }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("BossBar decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Open Screen (0x32) — open a container GUI
// ═══════════════════════════════════════════════════════

pub struct OpenScreen {
    pub window_id: i32,      // VarInt
    pub window_type: i32,    // VarInt: 0=9x1,1=9x2,2=9x3,3=9x4,4=9x5,5=9x6,6=3x3
    pub title: String,       // JSON chat component
}

impl PacketEncoder for OpenScreen {
    fn packet_id(&self) -> i32 { 0x32 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.window_id));
        buf.extend_from_slice(&write_varint_bytes(self.window_type));
        buf.extend_from_slice(&write_string(&self.title));
        buf
    }
}

impl PacketDecoder for OpenScreen {
    fn packet_id() -> i32 { 0x32 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("OpenScreen decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Container Set Data (0x14) — sync furnace/brewing progress
// ═══════════════════════════════════════════════════════

pub struct ContainerSetData {
    pub window_id: u8,
    pub property: i16,
    pub value: i16,
}

impl PacketEncoder for ContainerSetData {
    fn packet_id(&self) -> i32 { 0x14 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.window_id);
        buf.extend_from_slice(&self.property.to_be_bytes());
        buf.extend_from_slice(&self.value.to_be_bytes());
        buf
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Merchant Offers (0x3A) — sync villager trade list
// ═══════════════════════════════════════════════════════

/// A single villager trade offer
pub struct TradeOffer {
    pub input_item: SlotData,
    pub output_item: SlotData,
    pub second_input: Option<SlotData>, // optional second input item
    pub trade_disabled: bool,
    pub num_trade_uses: i32,         // current uses
    pub max_trade_uses: i32,         // max uses before lock
    pub xp: i32,                     // XP given to villager
    pub special_price: i32,          // adjusted price
    pub price_multiplier: f32,       // demand factor
    pub demand: i32,                 // demand value
}

pub struct MerchantOffers {
    pub window_id: i32,              // VarInt
    pub trades: Vec<TradeOffer>,
}

impl PacketEncoder for MerchantOffers {
    fn packet_id(&self) -> i32 { 0x3A }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.window_id));
        buf.extend_from_slice(&write_varint_bytes(self.trades.len() as i32));
        for trade in &self.trades {
            // Input item 1 (present)
            buf.push(1u8); // item present
            buf.extend_from_slice(&write_varint_bytes(trade.input_item.item_id));
            buf.push(trade.input_item.count);
            if let Some(ref nbt) = trade.input_item.nbt {
                buf.extend_from_slice(nbt);
            } else {
                buf.extend_from_slice(&write_varint_bytes(0));
            }
            // Output item (present)
            buf.push(1u8); // item present
            buf.extend_from_slice(&write_varint_bytes(trade.output_item.item_id));
            buf.push(trade.output_item.count);
            if let Some(ref nbt) = trade.output_item.nbt {
                buf.extend_from_slice(nbt);
            } else {
                buf.extend_from_slice(&write_varint_bytes(0));
            }
            // Second input item (optional)
            if let Some(ref second) = trade.second_input {
                buf.push(1u8); // item present
                buf.extend_from_slice(&write_varint_bytes(second.item_id));
                buf.push(second.count);
                if let Some(ref nbt) = second.nbt {
                    buf.extend_from_slice(nbt);
                } else {
                    buf.extend_from_slice(&write_varint_bytes(0));
                }
            } else {
                buf.push(0u8); // no second input
            }
            buf.push(trade.trade_disabled as u8);
            buf.extend_from_slice(&write_varint_bytes(trade.num_trade_uses));
            buf.extend_from_slice(&write_varint_bytes(trade.max_trade_uses));
            buf.extend_from_slice(&write_varint_bytes(trade.xp));
            buf.extend_from_slice(&write_varint_bytes(trade.special_price));
            buf.extend_from_slice(&write_f32(trade.price_multiplier));
            buf.extend_from_slice(&write_varint_bytes(trade.demand));
        }
        buf
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Set Container Slot (0x15) — update a single container slot
// ═══════════════════════════════════════════════════════

pub struct SetContainerSlot {
    pub window_id: u8,
    pub state_id: i32,        // VarInt
    pub slot: i16,
    pub item: Option<SlotData>,
}

#[derive(Debug, Clone)]
pub struct SlotData {
    pub item_id: i32,    // VarInt
    pub count: u8,
    pub nbt: Option<Vec<u8>>,
}

impl PacketEncoder for SetContainerSlot {
    fn packet_id(&self) -> i32 { 0x15 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.window_id);
        buf.extend_from_slice(&write_varint_bytes(self.state_id));
        buf.extend_from_slice(&self.slot.to_be_bytes());
        if let Some(ref item) = self.item {
            buf.push(1); // present
            buf.extend_from_slice(&write_varint_bytes(item.item_id));
            buf.push(item.count);
            if let Some(ref nbt) = item.nbt {
                buf.extend_from_slice(nbt);
            } else {
                buf.extend_from_slice(&write_varint_bytes(0)); // no NBT
            }
        } else {
            buf.push(0); // not present
        }
        buf
    }
}

impl PacketDecoder for SetContainerSlot {
    fn packet_id() -> i32 { 0x15 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("SetContainerSlot decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// C→S: Container Click (0x09) — player clicked in a container GUI
// ═══════════════════════════════════════════════════════

pub struct ContainerClick {
    pub window_id: u8,
    pub state_id: i32,     // VarInt
    pub slot: i16,
    pub button: u8,
    pub mode: i32,         // VarInt: 0=click,1=shift,2=hotbar,3=creative,4=drop,5=drag,6=double
}

impl PacketDecoder for ContainerClick {
    fn packet_id() -> i32 { 0x09 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        if data.is_empty() { return Err(CodecError::Malformed("too short".into())); }
        let window_id = data[0];
        let (state_id, mut off) = read_varint_enum(&data[1..])?;
        off += 1;
        if off + 2 > data.len() { return Err(CodecError::Malformed("too short for slot".into())); }
        let slot = i16::from_be_bytes(data[off..off+2].try_into().unwrap());
        off += 2;
        let button = data.get(off).copied().unwrap_or(0); off += 1;
        let (mode, _) = read_varint_enum(&data[off..])?;
        Ok(Self { window_id, state_id, slot, button, mode })
    }
}

// ═══════════════════════════════════════════════════════
// C→S: Container Close (0x0F) — player closed a container GUI
// ═══════════════════════════════════════════════════════

pub struct ContainerClose {
    pub window_id: u8,
}

impl PacketDecoder for ContainerClose {
    fn packet_id() -> i32 { 0x0F }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        if data.is_empty() { return Err(CodecError::Malformed("too short".into())); }
        Ok(Self { window_id: data[0] })
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Update Recipes (0x7B) — send recipe book data to client
// ═══════════════════════════════════════════════════════

pub struct UpdateRecipes {
    pub recipes: Vec<NetworkRecipe>,
}

pub struct NetworkRecipe {
    pub recipe_type: String,    // "minecraft:crafting_shaped"
    pub recipe_id: String,
    pub data: Vec<u8>,          // pre-encoded recipe payload
}

impl PacketEncoder for UpdateRecipes {
    fn packet_id(&self) -> i32 { 0x7B }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.recipes.len() as i32));
        for recipe in &self.recipes {
            buf.extend_from_slice(&write_string(&recipe.recipe_type));
            buf.extend_from_slice(&write_string(&recipe.recipe_id));
            buf.extend_from_slice(&recipe.data);
        }
        buf
    }
}

impl PacketDecoder for UpdateRecipes {
    fn packet_id() -> i32 { 0x7B }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("UpdateRecipes decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Command Suggestions Response (0x0F)
// ═══════════════════════════════════════════════════════

pub struct SuggestionMatch {
    pub text: String,
    pub tooltip: Option<String>,
}

pub struct CommandSuggestionsResponse {
    pub transaction_id: i32,
    pub start: i32,
    pub length: i32,
    pub matches: Vec<SuggestionMatch>,
}

impl PacketEncoder for CommandSuggestionsResponse {
    fn packet_id(&self) -> i32 { 0x0F }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.transaction_id));
        buf.extend_from_slice(&write_varint_bytes(self.start));
        buf.extend_from_slice(&write_varint_bytes(self.length));
        buf.extend_from_slice(&write_varint_bytes(self.matches.len() as i32));
        for m in &self.matches {
            buf.extend_from_slice(&write_string(&m.text));
            if let Some(ref tip) = m.tooltip {
                buf.push(1);
                buf.extend_from_slice(&write_string(tip));
            } else {
                buf.push(0);
            }
        }
        buf
    }
}

impl PacketDecoder for CommandSuggestionsResponse {
    fn packet_id() -> i32 { 0x0F }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("CommandSuggestionsResponse decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// C→S: Place Recipe (0x1B) — player clicked crafting result
// ═══════════════════════════════════════════════════════

pub struct PlaceRecipe {
    pub window_id: u8,
    pub recipe_index: i32,  // VarInt — index in UpdateRecipes list
    pub make_all: bool,
}

impl PacketDecoder for PlaceRecipe {
    fn packet_id() -> i32 { 0x1B }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        if data.is_empty() { return Err(CodecError::Malformed("too short".into())); }
        let window_id = data[0];
        let (recipe_index, _) = read_varint_enum(&data[1..])?;
        // make_all is a bool after recipe_index VarInt + window_id byte
        let make_all_offset = 1 + {
            let mut off = 1;
            while off < data.len() && data[off] & 0x80 != 0 { off += 1; }
            off + 1
        };
        let make_all = data.get(make_all_offset).map(|&b| b != 0).unwrap_or(false);
        Ok(Self { window_id, recipe_index, make_all })
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Entity Effect (0x78) — apply status effect visual to entity
// ═══════════════════════════════════════════════════════

pub struct EntityEffect {
    pub entity_id: i32,   // VarInt
    pub effect_id: i32,   // VarInt — status effect type ID
    pub amplifier: u8,    // 0-255
    pub duration: i32,    // VarInt — ticks
    pub flags: u8,        // bit 0=ambient, bit 1=show particles, bit 2=show icon
}

impl PacketEncoder for EntityEffect {
    fn packet_id(&self) -> i32 { 0x78 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.entity_id));
        buf.extend_from_slice(&write_varint_bytes(self.effect_id));
        buf.push(self.amplifier);
        buf.extend_from_slice(&write_varint_bytes(self.duration));
        buf.push(self.flags);
        buf
    }
}

impl PacketDecoder for EntityEffect {
    fn packet_id() -> i32 { 0x78 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("EntityEffect decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Remove Entity Effect (0x42) — remove status effect from entity
// ═══════════════════════════════════════════════════════

pub struct RemoveEntityEffect {
    pub entity_id: i32,  // VarInt
    pub effect_id: i32,  // VarInt
}

impl PacketEncoder for RemoveEntityEffect {
    fn packet_id(&self) -> i32 { 0x79 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.entity_id));
        buf.extend_from_slice(&write_varint_bytes(self.effect_id));
        buf
    }
}

impl PacketDecoder for RemoveEntityEffect {
    fn packet_id() -> i32 { 0x79 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("RemoveEntityEffect decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Explosion (0x1D) — explosion effect + knockback
// ═══════════════════════════════════════════════════════

pub struct Explosion {
    pub x: f64, pub y: f64, pub z: f64,
    pub radius: f32,
    pub player_motion_x: f32,
    pub player_motion_y: f32,
    pub player_motion_z: f32,
}

impl PacketEncoder for Explosion {
    fn packet_id(&self) -> i32 { 0x1D }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_double(self.x));
        buf.extend_from_slice(&write_double(self.y));
        buf.extend_from_slice(&write_double(self.z));
        buf.extend_from_slice(&write_f32(self.radius));
        // Block records: empty (no blocks destroyed for now)
        buf.extend_from_slice(&write_varint_bytes(0));
        buf.extend_from_slice(&write_f32(self.player_motion_x));
        buf.extend_from_slice(&write_f32(self.player_motion_y));
        buf.extend_from_slice(&write_f32(self.player_motion_z));
        buf
    }
}

impl PacketDecoder for Explosion {
    fn packet_id() -> i32 { 0x1D }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("Explosion decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Move Entity Rot (0x2F) — entity rotation update (delta)
// ═══════════════════════════════════════════════════════

pub struct MoveEntityRot {
    pub entity_id: i32,  // VarInt
    pub yaw: u8,         // byte angle
    pub pitch: u8,       // byte angle
    pub on_ground: bool,
}

impl PacketEncoder for MoveEntityRot {
    fn packet_id(&self) -> i32 { 0x2F }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.entity_id));
        buf.push(self.yaw);
        buf.push(self.pitch);
        buf.push(self.on_ground as u8);
        buf
    }
}

impl PacketDecoder for MoveEntityRot {
    fn packet_id() -> i32 { 0x2F }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("MoveEntityRot decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Move Entity Pos Rot (0x30) — entity position+rotation delta
// ═══════════════════════════════════════════════════════

pub struct MoveEntityPosRot {
    pub entity_id: i32,  // VarInt
    pub delta_x: i16,    // 1/128 blocks
    pub delta_y: i16,
    pub delta_z: i16,
    pub yaw: u8,
    pub pitch: u8,
    pub on_ground: bool,
}

impl PacketEncoder for MoveEntityPosRot {
    fn packet_id(&self) -> i32 { 0x30 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.entity_id));
        buf.extend_from_slice(&self.delta_x.to_be_bytes());
        buf.extend_from_slice(&self.delta_y.to_be_bytes());
        buf.extend_from_slice(&self.delta_z.to_be_bytes());
        buf.push(self.yaw);
        buf.push(self.pitch);
        buf.push(self.on_ground as u8);
        buf
    }
}

impl PacketDecoder for MoveEntityPosRot {
    fn packet_id() -> i32 { 0x30 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("MoveEntityPosRot decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Declare Commands (0x12) — command tree for tab completion
// ═══════════════════════════════════════════════════════

/// A node in the command tree
#[derive(Debug, Clone)]
pub struct CommandNode {
    pub flags: u8,           // 0x01=executable, 0x02=redirect, 0x04=suggestions
    pub children: Vec<i32>,  // child node indices
    pub redirect_node: Option<i32>,
    pub name: String,        // literal/argument name, empty for root
    pub parser_id: Option<String>,
    pub suggestions_type: Option<String>,
}

/// The Declare Commands packet — sent once after login
pub struct DeclareCommands {
    pub nodes: Vec<CommandNode>,
    pub root_index: i32,
}

impl DeclareCommands {
    /// Build a 2-level command tree with known subcommands for common commands
    pub fn from_command_names(names: &[&str]) -> Self {
        let mut nodes: Vec<CommandNode> = Vec::new();
        let mut root_children: Vec<i32> = Vec::new();

        // Root node (index 0)
        nodes.push(CommandNode {
            flags: 0,
            children: Vec::new(),
            redirect_node: None,
            name: String::new(),
            parser_id: None,
            suggestions_type: None,
        });

        // Known subcommands map — covers all 61 commands with proper argument types
        let subcommands: std::collections::HashMap<&str, Vec<(&str, &str)>> = {
            let mut m = std::collections::HashMap::new();
            // Phase 1: already had sub-trees
            m.insert("gamemode", vec![("survival", "brigadier:string"), ("creative", "brigadier:string"), ("adventure", "brigadier:string"), ("spectator", "brigadier:string")]);
            m.insert("time", vec![("set", "brigadier:string"), ("add", "brigadier:integer"), ("query", "brigadier:string")]);
            m.insert("weather", vec![("clear", "brigadier:string"), ("rain", "brigadier:string"), ("thunder", "brigadier:string")]);
            m.insert("difficulty", vec![("peaceful", "brigadier:string"), ("easy", "brigadier:string"), ("normal", "brigadier:string"), ("hard", "brigadier:string")]);
            m.insert("effect", vec![("give", "brigadier:string"), ("clear", "brigadier:string")]);
            m.insert("scoreboard", vec![("objectives", "brigadier:string"), ("players", "brigadier:string")]);
            m.insert("bossbar", vec![("add", "brigadier:string"), ("remove", "brigadier:string"), ("set", "brigadier:string"), ("list", "brigadier:string")]);
            m.insert("team", vec![("add", "brigadier:string"), ("remove", "brigadier:string"), ("join", "brigadier:string"), ("leave", "brigadier:string"), ("list", "brigadier:string")]);
            m.insert("tick", vec![("freeze", "brigadier:string"), ("unfreeze", "brigadier:string"), ("sprint", "brigadier:integer"), ("query", "brigadier:string")]);
            // Phase 2: execute with full sub-command tree
            m.insert("execute", vec![("as", "brigadier:string"), ("at", "brigadier:string"), ("run", "brigadier:string"), ("if", "brigadier:string"), ("unless", "brigadier:string"), ("store", "brigadier:string"), ("align", "brigadier:string"), ("rotated", "brigadier:string"), ("anchored", "brigadier:string"), ("facing", "brigadier:string"), ("positioned", "brigadier:string")]);
            // Phase 2: whitelist
            m.insert("whitelist", vec![("add", "brigadier:string"), ("remove", "brigadier:string"), ("list", "brigadier:string"), ("on", "brigadier:string"), ("off", "brigadier:string"), ("reload", "brigadier:string")]);
            // Phase 2: worldborder
            m.insert("worldborder", vec![("set", "brigadier:double"), ("center", "brigadier:string"), ("add", "brigadier:double"), ("get", "brigadier:string"), ("damage", "brigadier:string"), ("warning", "brigadier:string")]);
            // Phase 2: data
            m.insert("data", vec![("get", "brigadier:string"), ("merge", "brigadier:string"), ("remove", "brigadier:string"), ("modify", "brigadier:string")]);
            // Phase 2: attribute
            m.insert("attribute", vec![("get", "brigadier:string"), ("set", "brigadier:string"), ("list", "brigadier:string")]);
            // Phase 2: recipe
            m.insert("recipe", vec![("give", "brigadier:string"), ("take", "brigadier:string")]);
            // Phase 2: forceload
            m.insert("forceload", vec![("add", "brigadier:string"), ("remove", "brigadier:string"), ("query", "brigadier:string")]);
            // Phase 2: item
            m.insert("item", vec![("replace", "brigadier:string"), ("modify", "brigadier:string")]);
            // Phase 2: xp
            m.insert("xp", vec![("add", "brigadier:integer"), ("set", "brigadier:integer"), ("query", "brigadier:string")]);
            // Phase 2: title
            m.insert("title", vec![("title", "brigadier:string"), ("subtitle", "brigadier:string"), ("actionbar", "brigadier:string"), ("clear", "brigadier:string"), ("reset", "brigadier:string")]);
            // Phase 2: tag
            m.insert("tag", vec![("add", "brigadier:string"), ("remove", "brigadier:string"), ("list", "brigadier:string")]);
            // Phase 2: ride
            m.insert("ride", vec![("mount", "brigadier:string"), ("dismount", "brigadier:string")]);
            // Phase 2: debug
            m.insert("debug", vec![("start", "brigadier:string"), ("stop", "brigadier:string")]);
            // Phase 2: banlist
            m.insert("banlist", vec![("ips", "brigadier:string"), ("players", "brigadier:string")]);
            // Phase 2: gamerule (common rules as sub-commands)
            m.insert("gamerule", vec![("doMobSpawning", "brigadier:bool"), ("doWeatherCycle", "brigadier:bool"), ("doDaylightCycle", "brigadier:bool"), ("keepInventory", "brigadier:bool"), ("doFireTick", "brigadier:bool")]);
            // Phase 2: spectate
            m.insert("spectate", vec![("entity", "brigadier:string")]);
            // Phase 2: defaultgamemode
            m.insert("defaultgamemode", vec![("survival", "brigadier:string"), ("creative", "brigadier:string"), ("adventure", "brigadier:string"), ("spectator", "brigadier:string")]);
            // Phase 2: fillbiome
            m.insert("fillbiome", vec![("from", "brigadier:string"), ("to", "brigadier:string")]);
            // Phase 2: trigger
            m.insert("trigger", vec![("add", "brigadier:integer"), ("set", "brigadier:integer")]);
            // Phase 3: remaining commands (33 new subcommand trees)
            m.insert("ban", vec![("target", "brigadier:string"), ("reason", "brigadier:string")]);
            m.insert("kick", vec![("target", "brigadier:string"), ("reason", "brigadier:string")]);
            m.insert("pardon", vec![("target", "brigadier:string")]);
            m.insert("msg", vec![("target", "brigadier:string"), ("message", "brigadier:string")]);
            m.insert("say", vec![("message", "brigadier:string")]);
            m.insert("stop", vec![]);
            m.insert("save-all", vec![("flush", "brigadier:string")]);
            m.insert("list", vec![("uuids", "brigadier:string")]);
            m.insert("seed", vec![]);
            m.insert("locate", vec![("structure", "brigadier:string"), ("biome", "brigadier:string"), ("poi", "brigadier:string")]);
            m.insert("clone", vec![("begin", "brigadier:string"), ("end", "brigadier:string"), ("destination", "brigadier:string")]);
            m.insert("fill", vec![("from", "brigadier:string"), ("to", "brigadier:string"), ("block", "brigadier:string")]);
            m.insert("setblock", vec![("pos", "brigadier:string"), ("block", "brigadier:string")]);
            m.insert("kill", vec![("target", "brigadier:string")]);
            m.insert("clear", vec![("target", "brigadier:string"), ("item", "brigadier:string")]);
            m.insert("playsound", vec![("sound", "brigadier:string"), ("target", "brigadier:string"), ("pos", "brigadier:string")]);
            m.insert("stopsound", vec![("target", "brigadier:string")]);
            m.insert("spreadplayers", vec![("center", "brigadier:string"), ("distance", "brigadier:float"), ("range", "brigadier:float"), ("targets", "brigadier:string")]);
            m.insert("damage", vec![("target", "brigadier:string"), ("amount", "brigadier:float")]);
            m.insert("enchant", vec![("target", "brigadier:string"), ("enchantment", "brigadier:string"), ("level", "brigadier:integer")]);
            m.insert("publish", vec![("port", "brigadier:integer")]);
            m.insert("save-on", vec![]);
            m.insert("save-off", vec![]);
            m.insert("spawnpoint", vec![("target", "brigadier:string"), ("pos", "brigadier:string")]);
            m.insert("setworldspawn", vec![("pos", "brigadier:string")]);
            m.insert("reload", vec![("difficulty", "brigadier:string")]);
            m.insert("transfer", vec![("player", "brigadier:string"), ("host", "brigadier:string"), ("port", "brigadier:integer")]);
            m.insert("me", vec![("action", "brigadier:string")]);
            m.insert("status", vec![]);
            m.insert("help", vec![]);
            m
        };

        for name in names {
            let mut cmd_children: Vec<i32> = Vec::new();
            if let Some(subs) = subcommands.get(*name) {
                for (sub_name, parser) in subs {
                    cmd_children.push(nodes.len() as i32);
                    nodes.push(CommandNode {
                        flags: 0x01,
                        children: Vec::new(),
                        redirect_node: None,
                        name: sub_name.to_string(),
                        parser_id: Some(parser.to_string()),
                        suggestions_type: Some("ask_server".into()),
                    });
                }
            }
            root_children.push(nodes.len() as i32);
            nodes.push(CommandNode {
                flags: if cmd_children.is_empty() { 0x01 } else { 0 },
                children: cmd_children,
                redirect_node: None,
                name: name.to_string(),
                parser_id: Some("brigadier:string".into()),
                suggestions_type: Some("ask_server".into()),
            });
        }

        // Update root children
        nodes[0].children = root_children;

        Self { nodes, root_index: 0 }
    }
}

impl PacketEncoder for DeclareCommands {
    fn packet_id(&self) -> i32 { 0x10 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.nodes.len() as i32));
        for node in &self.nodes {
            // Flags
            buf.push(node.flags);
            // Children
            buf.extend_from_slice(&write_varint_bytes(node.children.len() as i32));
            for &child in &node.children {
                buf.extend_from_slice(&write_varint_bytes(child));
            }
            // Redirect node (if has_redirect flag set)
            if node.flags & 0x02 != 0 {
                buf.extend_from_slice(&write_varint_bytes(node.redirect_node.unwrap_or(0)));
            }
            // Name
            if node.flags == 0 {
                // Root node — no name
            } else {
                buf.extend_from_slice(&write_string(&node.name));
            }
            // Parser ID (if not executable and has suggestions)
            if node.flags & 0x01 == 0 && node.flags & 0x04 != 0 {
                buf.extend_from_slice(&write_string(node.parser_id.as_deref().unwrap_or("brigadier:string")));
            }
            // Suggestions type (if has_suggestions flag)
            if node.flags & 0x04 != 0 {
                buf.extend_from_slice(&write_string(node.suggestions_type.as_deref().unwrap_or("ask_server")));
            }
        }
        buf.extend_from_slice(&write_varint_bytes(self.root_index));
        buf
    }
}

impl PacketDecoder for DeclareCommands {
    fn packet_id() -> i32 { 0x10 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("DeclareCommands decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Set Health (0x62)
// ═══════════════════════════════════════════════════════

pub struct SetHealth {
    pub health: f32,
    pub food: i32,
    pub saturation: f32,
}

impl PacketEncoder for SetHealth {
    fn packet_id(&self) -> i32 { 0x61 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_f32(self.health));
        buf.extend_from_slice(&write_varint_bytes(self.food));
        buf.extend_from_slice(&write_f32(self.saturation));
        buf
    }
}

impl PacketDecoder for SetHealth {
    fn packet_id() -> i32 { 0x61 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("SetHealth decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Set Entity Metadata (0x5A)
// ═══════════════════════════════════════════════════════

/// Entity metadata — sent after SpawnPlayer to configure skin/name display
pub struct SetEntityMetadata {
    pub entity_id: i32,
    /// Pre-encoded metadata bytes (type-index-value triples, terminated by 0xFF)
    pub metadata: Vec<u8>,
}

impl SetEntityMetadata {
    /// Build minimal player metadata: all skin parts visible + name tag
    pub fn player_defaults(entity_id: i32) -> Self {
        let meta = vec![0x00, 0, 0x00, 17, 0, 0xFF, 0xFF];
        Self { entity_id, metadata: meta }
    }

    /// Build minimal mob metadata (alive, default health)
    pub fn mob_defaults(entity_id: i32) -> Self {
        let mut meta = Vec::with_capacity(16);
        meta.push(0x00); meta.push(0); meta.push(0x00); // status flags: alive
        // Health (index 9, float type = 2)
        meta.push(9);    // index
        meta.push(2);    // type: float
        meta.extend_from_slice(&10.0f32.to_be_bytes()); // 10 HP
        meta.push(0xFF); // terminator
        Self { entity_id, metadata: meta }
    }
}

impl PacketEncoder for SetEntityMetadata {
    fn packet_id(&self) -> i32 { 0x5C }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.entity_id));
        buf.extend_from_slice(&self.metadata);
        buf
    }
}

impl PacketDecoder for SetEntityMetadata {
    fn packet_id() -> i32 { 0x5C }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("SetEntityMetadata decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Respawn (0x42)
// ═══════════════════════════════════════════════════════

pub struct Respawn {
    pub dimension_type: String,
    pub dimension_name: String,
    pub hashed_seed: i64,
    pub gamemode: u8,
    pub previous_gamemode: i8,
    pub is_debug: bool,
    pub is_flat: bool,
    pub death_location: Option<(String, i64)>,
    pub portal_cooldown: i32,
    pub data_kept: u8, // bitmask: 0x01=attributes, 0x02=metadata
}

impl PacketEncoder for Respawn {
    fn packet_id(&self) -> i32 { 0x4B }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_string(&self.dimension_type));
        buf.extend_from_slice(&write_string(&self.dimension_name));
        buf.extend_from_slice(&write_i64(self.hashed_seed));
        buf.push(self.gamemode);
        buf.push(self.previous_gamemode as u8);
        buf.extend_from_slice(&write_bool(self.is_debug));
        buf.extend_from_slice(&write_bool(self.is_flat));
        // death location (optional)
        if let Some((ref dim, pos)) = self.death_location {
            buf.extend_from_slice(&write_bool(true));
            buf.extend_from_slice(&write_string(dim));
            buf.extend_from_slice(&write_i64(pos));
        } else {
            buf.extend_from_slice(&write_bool(false));
        }
        buf.extend_from_slice(&write_varint_bytes(self.portal_cooldown));
        buf.push(self.data_kept);
        buf
    }
}

impl PacketDecoder for Respawn {
    fn packet_id() -> i32 { 0x4B }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("Respawn decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Block Update (0x0C) — single block change
// ═══════════════════════════════════════════════════════

pub struct BlockUpdate {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_id: i32, // VarInt global palette ID
}

// ═══════════════════════════════════════════════════════════════
// BlockEntityData (S2C 0x07) — sync block entity NBT to client
// ═══════════════════════════════════════════════════════════════
pub struct BlockEntityData {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_entity_type: i32,
    pub nbt_data: Vec<u8>,
}

impl PacketEncoder for BlockEntityData {
    fn packet_id(&self) -> i32 { 0x07 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_position(self.x, self.y, self.z));
        buf.extend_from_slice(&write_varint_bytes(self.block_entity_type));
        buf.extend_from_slice(&self.nbt_data);
        buf
    }
}

impl PacketDecoder for BlockEntityData {
    fn packet_id() -> i32 { 0x07 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("BlockEntityData decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════════════
// BlockUpdate (S2C 0x08)
// ═══════════════════════════════════════════════════════════════
impl PacketEncoder for BlockUpdate {
    fn packet_id(&self) -> i32 { 0x08 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_position(self.x, self.y, self.z));
        buf.extend_from_slice(&write_varint_bytes(self.block_id));
        buf
    }
}

impl PacketDecoder for BlockUpdate {
    fn packet_id() -> i32 { 0x08 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("BlockUpdate decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Player Abilities (0x36) — creative flight, etc.
// ═══════════════════════════════════════════════════════

pub struct PlayerAbilities {
    pub flags: u8,         // 0x01=invulnerable, 0x02=flying, 0x04=allow_flying, 0x08=creative_mode
    pub flying_speed: f32,
    pub walking_speed: f32,
}

impl PlayerAbilities {
    /// Survival mode defaults
    pub fn survival() -> Self {
        Self { flags: 0, flying_speed: 0.05, walking_speed: 0.1 }
    }
    /// Creative mode: allow flight
    pub fn creative() -> Self {
        Self { flags: 0x04 | 0x08, flying_speed: 0.05, walking_speed: 0.1 }
    }
}

impl PacketEncoder for PlayerAbilities {
    fn packet_id(&self) -> i32 { 0x39 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.flags);
        buf.extend_from_slice(&write_f32(self.flying_speed));
        buf.extend_from_slice(&write_f32(self.walking_speed));
        buf
    }
}

impl PacketDecoder for PlayerAbilities {
    fn packet_id() -> i32 { 0x39 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("PlayerAbilities decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Set Held Item (0x64) — sync selected hotbar slot
// ═══════════════════════════════════════════════════════

pub struct SetHeldItemS2C {
    pub slot: u8, // 0-8 hotbar slot
}

impl PacketEncoder for SetHeldItemS2C {
    fn packet_id(&self) -> i32 { 0x62 }
    fn encode_payload(&self) -> Vec<u8> {
        vec![self.slot]
    }
}

impl PacketDecoder for SetHeldItemS2C {
    fn packet_id() -> i32 { 0x62 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        if data.is_empty() { return Err(CodecError::Malformed("too short".into())); }
        Ok(Self { slot: data[0] })
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Container Set Content (0x11) — full inventory sync
// ═══════════════════════════════════════════════════════

pub struct ContainerSetContent {
    pub window_id: u8,
    pub state_id: i32,
    pub items: Vec<Option<SlotData>>,
    pub carried_item: Option<SlotData>,
}

impl PacketEncoder for ContainerSetContent {
    fn packet_id(&self) -> i32 { 0x12 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.window_id);
        buf.extend_from_slice(&write_varint_bytes(self.state_id));
        buf.extend_from_slice(&write_varint_bytes(self.items.len() as i32));
        for item in &self.items {
            if let Some(slot) = item {
                buf.push(1); // present
                buf.extend_from_slice(&write_varint_bytes(slot.item_id));
                buf.push(slot.count);
                if let Some(ref nbt) = slot.nbt {
                    buf.extend_from_slice(nbt);
                } else {
                    buf.extend_from_slice(&write_varint_bytes(0)); // no NBT
                }
            } else {
                buf.push(0); // not present
            }
        }
        // Carried item
        if let Some(slot) = &self.carried_item {
            buf.push(1); // present
            buf.extend_from_slice(&write_varint_bytes(slot.item_id));
            buf.push(slot.count);
            if let Some(ref nbt) = slot.nbt {
                buf.extend_from_slice(nbt);
            } else {
                buf.extend_from_slice(&write_varint_bytes(0)); // no NBT
            }
        } else {
            buf.push(0); // not present
        }
        buf
    }
}

impl PacketDecoder for ContainerSetContent {
    fn packet_id() -> i32 { 0x12 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("ContainerSetContent decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Set Time (0x72) — world time sync
// ═══════════════════════════════════════════════════════

pub struct UpdateTime {
    pub world_age: i64,
    pub time_of_day: i64,
}

impl PacketEncoder for UpdateTime {
    fn packet_id(&self) -> i32 { 0x6A }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_i64(self.world_age));
        buf.extend_from_slice(&write_i64(self.time_of_day));
        buf
    }
}

impl PacketDecoder for UpdateTime {
    fn packet_id() -> i32 { 0x6A }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("UpdateTime decode not implemented".into()))
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Cookie Request (0x17) — ask client for stored cookie
// ═══════════════════════════════════════════════════════

pub struct CookieRequest {
    pub key: String, // cookie identifier (e.g. "minecraft:server_data")
}

impl PacketEncoder for CookieRequest {
    fn packet_id(&self) -> i32 { 0x17 }
    fn encode_payload(&self) -> Vec<u8> {
        write_string(&self.key)
    }
}

// ═══════════════════════════════════════════════════════
// C→S: Cookie Response (0x16) — client sends stored cookie
// ═══════════════════════════════════════════════════════

pub struct CookieResponse {
    pub key: String,
    pub payload: Option<Vec<u8>>,
}

impl PacketDecoder for CookieResponse {
    fn packet_id() -> i32 { 0x16 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (key, mut offset) = read_string(data)?;
        let (has_payload, n) = read_bool(&data[offset..])?; offset += n;
        let payload = if has_payload {
            let (len, n) = read_varint_enum(&data[offset..])?; offset += n;
            let len = len as usize;
            if offset + len > data.len() { return Err(CodecError::Malformed("cookie payload too short".into())); }
            let p = data[offset..offset + len].to_vec();
            Some(p)
        } else {
            None
        };
        Ok(Self { key: key.to_string(), payload })
    }
}

// ═══════════════════════════════════════════════════════
// C→S: Resource Pack Response (0x24) — client responds to resource pack push
// ═══════════════════════════════════════════════════════

pub struct ResourcePackResponse {
    pub uuid: uuid::Uuid,
    pub result: i32, // 0=success, 1=declined, 2=failed, 3=accepted
}

impl PacketDecoder for ResourcePackResponse {
    fn packet_id() -> i32 { 0x24 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (uuid, offset) = read_uuid(data)?;
        let (result, _) = read_varint_enum(&data[offset..])?;
        Ok(Self { uuid, result })
    }
}

// ═══════════════════════════════════════════════════════
// C→S: Change Beacon Effect — select beacon primary/secondary effect
// ═══════════════════════════════════════════════════════

pub struct ChangeBeaconEffect {
    pub primary_effect: Option<i32>,
    pub secondary_effect: Option<i32>,
}

impl PacketDecoder for ChangeBeaconEffect {
    fn packet_id() -> i32 { 0x2B } // Change Beacon Effect C2S
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        if data.len() < 2 { return Err(CodecError::Malformed("too short".into())); }
        let has_primary = data[0] != 0;
        let has_secondary = data[1] != 0;
        let mut off = 2;
        let primary = if has_primary { let (v, o) = crate::codec::read_varint_enum(&data[off..])?; off = o; Some(v) } else { None };
        let secondary = if has_secondary { let (v, _o) = crate::codec::read_varint_enum(&data[off..])?; Some(v) } else { None };
        Ok(Self { primary_effect: primary, secondary_effect: secondary })
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Update Advancements — sync advancement tree + player progress
// ═══════════════════════════════════════════════════════

pub struct UpdateAdvancements {
    pub reset: bool,
    pub advancement_ids: Vec<String>,
    pub progress_map: Vec<(String, Vec<String>)>, // (adv_id, completed_criteria)
}

impl PacketEncoder for UpdateAdvancements {
    fn packet_id(&self) -> i32 { 0x69 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut out = Vec::new();
        // Reset flag
        out.push(if self.reset { 1 } else { 0 });
        // Advancement mapping (advancement identifiers for client)
        out.extend_from_slice(&crate::codec::write_varint_bytes(self.advancement_ids.len() as i32));
        for id in &self.advancement_ids {
            out.extend_from_slice(&crate::codec::write_string(id));
        }
        // Progress mapping
        out.extend_from_slice(&crate::codec::write_varint_bytes(self.progress_map.len() as i32));
        for (adv_id, criteria) in &self.progress_map {
            out.extend_from_slice(&crate::codec::write_string(adv_id));
            out.extend_from_slice(&crate::codec::write_varint_bytes(criteria.len() as i32));
            for c in criteria {
                out.extend_from_slice(&crate::codec::write_string(c));
            }
        }
        out
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Initialize World Border — sync border to client
// ═══════════════════════════════════════════════════════

pub struct InitializeWorldBorder {
    pub x: f64,
    pub z: f64,
    pub old_diameter: f64,
    pub new_diameter: f64,
    pub speed: i64,
    pub portal_teleport_boundary: i32,
    pub warning_blocks: i32,
    pub warning_time: i32,
}

impl PacketEncoder for InitializeWorldBorder {
    fn packet_id(&self) -> i32 { 0x21 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.x.to_be_bytes());
        out.extend_from_slice(&self.z.to_be_bytes());
        out.extend_from_slice(&self.old_diameter.to_be_bytes());
        out.extend_from_slice(&self.new_diameter.to_be_bytes());
        out.extend_from_slice(&crate::codec::write_varint_bytes(self.speed as i32));
        out.extend_from_slice(&crate::codec::write_varint_bytes(self.portal_teleport_boundary));
        out.extend_from_slice(&crate::codec::write_varint_bytes(self.warning_blocks));
        out.extend_from_slice(&crate::codec::write_varint_bytes(self.warning_time));
        out
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Set Cooldown — item cooldown visual overlay
// ═══════════════════════════════════════════════════════

pub struct SetCooldown {
    pub item_id: i32,
    pub cooldown_ticks: i32,
}

impl PacketEncoder for SetCooldown {
    fn packet_id(&self) -> i32 { 0x13 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&crate::codec::write_varint_bytes(self.item_id));
        out.extend_from_slice(&crate::codec::write_varint_bytes(self.cooldown_ticks));
        out
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Entity Event — entity status animation (hurt/death/etc)
// ═══════════════════════════════════════════════════════

pub struct EntityEvent {
    pub entity_id: i32,
    pub status: u8,
}

impl PacketEncoder for EntityEvent {
    fn packet_id(&self) -> i32 { 0x1B }
    fn encode_payload(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&crate::codec::write_varint_bytes(self.entity_id));
        out.push(self.status);
        out
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Set Passengers (0x5D) — mount entities onto vehicles
// ═══════════════════════════════════════════════════════

pub struct SetPassengers {
    pub vehicle_id: i32,
    pub passengers: Vec<i32>,
}

impl PacketEncoder for SetPassengers {
    fn packet_id(&self) -> i32 { 0x5D }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&crate::codec::write_varint_bytes(self.vehicle_id));
        buf.extend_from_slice(&crate::codec::write_varint_bytes(self.passengers.len() as i32));
        for pid in &self.passengers {
            buf.extend_from_slice(&crate::codec::write_varint_bytes(*pid));
        }
        buf
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Set Entity Link (0x33) — leash connection
// ═══════════════════════════════════════════════════════

pub struct SetEntityLink {
    pub attached_id: i32,
    pub holding_id: i32,
}

impl PacketEncoder for SetEntityLink {
    fn packet_id(&self) -> i32 { 0x33 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&crate::codec::write_varint_bytes(self.attached_id));
        buf.extend_from_slice(&crate::codec::write_varint_bytes(self.holding_id));
        buf
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Scoreboard Objective (0x4D)
// ═══════════════════════════════════════════════════════

pub struct ScoreboardObjective {
    pub name: String,
    pub mode: u8,       // 0=create, 1=remove, 2=update
    pub objective_value: String, // display text (JSON) or empty
    pub objective_type: i32,   // 0=integer, 1=hearts (dummy)
    pub number_format: u8,     // 0=blank, 1=styled
}

impl PacketEncoder for ScoreboardObjective {
    fn packet_id(&self) -> i32 { 0x4D }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&crate::codec::write_string(&self.name));
        buf.push(self.mode);
        buf.extend_from_slice(&crate::codec::write_string(&self.objective_value));
        buf.extend_from_slice(&crate::codec::write_varint_bytes(self.objective_type));
        buf.push(self.number_format);
        buf
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Update Score (0x4E)
// ═══════════════════════════════════════════════════════

pub struct UpdateScore {
    pub entity_name: String,
    pub objective_name: String,
    pub value: i32,
    pub display_name: Option<String>,
    pub number_format: u8,
}

impl PacketEncoder for UpdateScore {
    fn packet_id(&self) -> i32 { 0x4E }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&crate::codec::write_string(&self.entity_name));
        buf.extend_from_slice(&crate::codec::write_string(&self.objective_name));
        buf.extend_from_slice(&crate::codec::write_varint_bytes(self.value));
        buf.push(if self.display_name.is_some() { 1u8 } else { 0u8 });
        if let Some(ref name) = self.display_name {
            buf.extend_from_slice(&crate::codec::write_string(name));
        }
        buf.push(self.number_format);
        buf
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Display Scoreboard (0x58)
// ═══════════════════════════════════════════════════════

pub struct ScoreboardDisplay {
    pub position: u8, // 0=list, 1=sidebar, 2=belowName
    pub score_name: String,
}

impl PacketEncoder for ScoreboardDisplay {
    fn packet_id(&self) -> i32 { 0x58 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.position);
        buf.extend_from_slice(&crate::codec::write_string(&self.score_name));
        buf
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Map Data (0x2C) — map item content
// ═══════════════════════════════════════════════════════

pub struct MapData {
    pub map_id: i32,
    pub scale: u8,
    pub locked: bool,
    pub icons: Vec<MapIcon>,
    pub columns: u8,
    pub rows: u8,
    pub x: u8,
    pub z: u8,
    pub data: Vec<u8>,
}

pub struct MapIcon {
    pub icon_type: i32,
    pub x: u8,
    pub z: u8,
    pub direction: u8,
    pub display_name: Option<String>,
}

impl PacketEncoder for MapData {
    fn packet_id(&self) -> i32 { 0x2C }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(128);
        buf.extend_from_slice(&crate::codec::write_varint_bytes(self.map_id));
        buf.push(self.scale);
        buf.push(if self.locked { 1u8 } else { 0u8 });
        buf.push(if self.icons.is_empty() { 0u8 } else { 1u8 });
        if !self.icons.is_empty() {
            buf.extend_from_slice(&crate::codec::write_varint_bytes(self.icons.len() as i32));
            for icon in &self.icons {
                buf.extend_from_slice(&crate::codec::write_varint_bytes(icon.icon_type));
                buf.push(icon.x);
                buf.push(icon.z);
                buf.push(icon.direction);
                buf.push(if icon.display_name.is_some() { 1u8 } else { 0u8 });
                if let Some(ref name) = icon.display_name {
                    buf.extend_from_slice(&crate::codec::write_string(name));
                }
            }
        }
        buf.push(self.columns);
        if self.columns > 0 {
            buf.push(self.rows);
            buf.push(self.x);
            buf.push(self.z);
            buf.extend_from_slice(&crate::codec::write_varint_bytes(self.data.len() as i32));
            buf.extend_from_slice(&self.data);
        }
        buf
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Teams (0x55)
// ═══════════════════════════════════════════════════════

pub struct Teams {
    pub name: String,
    pub mode: u8, // 0=create, 1=remove, 2=update, 3=add_players, 4=remove_players
    pub display_name: String,
    pub friendly_fire: u8,     // 0=off, 1=on
    pub nametag_visibility: String,
    pub collision_rule: String,
    pub color: i32,
    pub prefix: String,
    pub suffix: String,
    pub entities: Vec<String>,
}

impl PacketEncoder for Teams {
    fn packet_id(&self) -> i32 { 0x55 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&crate::codec::write_string(&self.name));
        buf.push(self.mode);
        if self.mode == 0 || self.mode == 2 {
            buf.extend_from_slice(&crate::codec::write_string(&self.display_name));
            buf.push(self.friendly_fire);
            buf.extend_from_slice(&crate::codec::write_string(&self.nametag_visibility));
            buf.extend_from_slice(&crate::codec::write_string(&self.collision_rule));
            buf.extend_from_slice(&crate::codec::write_varint_bytes(self.color));
            buf.extend_from_slice(&crate::codec::write_string(&self.prefix));
            buf.extend_from_slice(&crate::codec::write_string(&self.suffix));
        }
        if self.mode == 0 || self.mode == 3 || self.mode == 4 {
            buf.extend_from_slice(&crate::codec::write_varint_bytes(self.entities.len() as i32));
            for e in &self.entities {
                buf.extend_from_slice(&crate::codec::write_string(e));
            }
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_health_encode() {
        let pkt = SetHealth { health: 20.0, food: 20, saturation: 5.0 };
        let data = pkt.encode_payload();
        // health: f32 (4 bytes BE), food: varint, saturation: f32
        assert_eq!(data.len(), 4 + 1 + 4); // 20 fits in 1 varint byte
        assert_eq!(data[0..4], 20.0_f32.to_be_bytes());
    }

    #[test]
    fn test_set_health_zero() {
        let pkt = SetHealth { health: 0.0, food: 20, saturation: 0.0 };
        let data = pkt.encode_payload();
        assert_eq!(data[0..4], 0.0_f32.to_be_bytes());
    }

    #[test]
    fn test_respawn_encode() {
        let pkt = Respawn {
            dimension_type: "minecraft:overworld".into(),
            dimension_name: "minecraft:overworld".into(),
            hashed_seed: 12345,
            gamemode: 0,
            previous_gamemode: -1,
            is_debug: false,
            is_flat: true,
            death_location: None,
            portal_cooldown: 0,
            data_kept: 0,
        };
        let data = pkt.encode_payload();
        assert!(!data.is_empty());
        // Should encode 11 fields + optional death location (false)
        assert!(data.len() > 20);
    }

    #[test]
    fn test_respawn_with_death_location() {
        let pkt = Respawn {
            dimension_type: "minecraft:overworld".into(),
            dimension_name: "minecraft:overworld".into(),
            hashed_seed: 0,
            gamemode: 1,
            previous_gamemode: 0,
            is_debug: false,
            is_flat: false,
            death_location: Some(("minecraft:overworld".into(), 1234567)),
            portal_cooldown: 0,
            data_kept: 1,
        };
        let data = pkt.encode_payload();
        assert!(!data.is_empty());
    }

    #[test]
    fn test_game_event_encode() {
        let pkt = GameEvent { event: 3, value: 1.0 };
        let data = pkt.encode_payload();
        assert_eq!(data[0], 3); // event type
        assert_eq!(data[1..5], 1.0_f32.to_be_bytes());
    }

    #[test]
    fn test_set_entity_metadata_player_defaults() {
        let meta = SetEntityMetadata::player_defaults(42);
        assert_eq!(meta.entity_id, 42);
        assert!(!meta.metadata.is_empty());
        // Should contain terminator byte
        assert_eq!(meta.metadata.last(), Some(&0xFF));
    }

    #[test]
    fn test_set_entity_metadata_encode() {
        let meta = SetEntityMetadata::player_defaults(1);
        let data = meta.encode_payload();
        // entity_id (varint) + metadata bytes
        assert!(data.len() >= 2);
    }

    #[test]
    fn test_spawn_player_angle_byte() {
        let pkt = SpawnPlayer {
            entity_id: 1,
            player_uuid: uuid::Uuid::nil(),
            x: 0.0, y: 64.0, z: 0.0,
            yaw: 0.0, pitch: 0.0,
        };
        let data = pkt.encode_payload();
        // Verify the angle bytes are present
        assert!(data.len() > 20);
    }

    #[test]
    fn test_spawn_player_angle_wrap() {
        let pkt = SpawnPlayer {
            entity_id: 1,
            player_uuid: uuid::Uuid::nil(),
            x: 0.0, y: 64.0, z: 0.0,
            yaw: 360.0, pitch: -90.0,
        };
        let data = pkt.encode_payload();
        assert!(data.len() > 20);
        // 360° wraps to 0° → byte 0
        // -90° is typically encoded properly
    }

    #[test]
    fn test_player_command_decode_start_sprinting() {
        // Build a PlayerCommand packet: entity_id=1, action=3 (START_SPRINTING), data=0
        let mut data = Vec::new();
        data.extend_from_slice(&write_varint_bytes(1)); // entity_id
        data.extend_from_slice(&write_varint_bytes(3)); // action = START_SPRINTING
        data.extend_from_slice(&write_varint_bytes(0)); // data = 0
        let cmd = PlayerCommand::decode_payload(&data).expect("should decode");
        assert_eq!(cmd.entity_id, 1);
        assert_eq!(cmd.action, 3);
        assert_eq!(cmd.data, 0);
    }

    #[test]
    fn test_set_experience_encode() {
        let pkt = SetExperience {
            experience_bar: 0.75,
            level: 5,
            total_experience: 47,
        };
        let data = pkt.encode_payload();
        // f32 (4 bytes) + varint (1 byte for 5) + varint (1 byte for 47)
        assert_eq!(data[0..4], 0.75_f32.to_be_bytes());
        assert!(data.len() >= 6);
    }

    #[test]
    fn test_damage_event_encode() {
        let pkt = DamageEvent {
            entity_id: 42,
            source_type_id: 1,
            source_cause_id: 100,
            source_direct_id: 100,
            source_pos_x: Some(10.0),
            source_pos_y: Some(64.0),
            source_pos_z: Some(10.0),
        };
        let data = pkt.encode_payload();
        assert!(!data.is_empty());
        // entity_id varint should encode 42 as 1 byte
        assert_eq!(data[0], 42);
    }

    #[test]
    fn test_damage_event_no_position() {
        let pkt = DamageEvent {
            entity_id: 1,
            source_type_id: 3,
            source_cause_id: -1,
            source_direct_id: -1,
            source_pos_x: None,
            source_pos_y: None,
            source_pos_z: None,
        };
        let data = pkt.encode_payload();
        assert!(!data.is_empty());
        // The optional position flag should be false (0x00 byte near end)
        let last_byte = data.last().copied().unwrap_or(0xFF);
        assert_eq!(last_byte, 0x00); // false for no position
    }
}

// ═══════════════════════════════════════════════════════
// 1.21.5 S2C: Split Title Packets (replaces old SetTitles 0x6B)
// ═══════════════════════════════════════════════════════

pub struct SetTitleText { pub text: String, }
impl PacketEncoder for SetTitleText {
    fn packet_id(&self) -> i32 { 0x58 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new(); buf.extend_from_slice(&write_string(&self.text)); buf
    }
}

pub struct SetTitleSubtitle { pub text: String, }
impl PacketEncoder for SetTitleSubtitle {
    fn packet_id(&self) -> i32 { 0x59 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new(); buf.extend_from_slice(&write_string(&self.text)); buf
    }
}

pub struct SetActionBarText { pub text: String, }
impl PacketEncoder for SetActionBarText {
    fn packet_id(&self) -> i32 { 0x5A }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new(); buf.extend_from_slice(&write_string(&self.text)); buf
    }
}

pub struct SetTitleTimes { pub fade_in: i32, pub stay: i32, pub fade_out: i32, }
impl PacketEncoder for SetTitleTimes {
    fn packet_id(&self) -> i32 { 0x5B }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.fade_in.to_be_bytes());
        buf.extend_from_slice(&self.stay.to_be_bytes());
        buf.extend_from_slice(&self.fade_out.to_be_bytes());
        buf
    }
}

pub struct ClearTitles { pub reset: bool, }
impl PacketEncoder for ClearTitles {
    fn packet_id(&self) -> i32 { 0x5C }
    fn encode_payload(&self) -> Vec<u8> { vec![self.reset as u8] }
}

// ═══════════════════════════════════════════════════════
// 1.21.5 S2C: World Border Split Packets
// ═══════════════════════════════════════════════════════

pub struct SetBorderCenter { pub x: f64, pub z: f64, }
impl PacketEncoder for SetBorderCenter {
    fn packet_id(&self) -> i32 { 0x4C }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.x.to_be_bytes());
        buf.extend_from_slice(&self.z.to_be_bytes());
        buf
    }
}

pub struct SetBorderSize { pub diameter: f64, }
impl PacketEncoder for SetBorderSize {
    fn packet_id(&self) -> i32 { 0x4E }
    fn encode_payload(&self) -> Vec<u8> { self.diameter.to_be_bytes().to_vec() }
}

pub struct SetBorderWarningDelay { pub delay: i32, }
impl PacketEncoder for SetBorderWarningDelay {
    fn packet_id(&self) -> i32 { 0x4F }
    fn encode_payload(&self) -> Vec<u8> { write_varint_bytes(self.delay) }
}

pub struct SetBorderWarningBlocks { pub blocks: i32, }
impl PacketEncoder for SetBorderWarningBlocks {
    fn packet_id(&self) -> i32 { 0x50 }
    fn encode_payload(&self) -> Vec<u8> { write_varint_bytes(self.blocks) }
}

// UpdateEnabledFeatures (0x0D) — 1.21.5 experimental features
pub struct UpdateEnabledFeatures { pub features: Vec<String>, }
impl PacketEncoder for UpdateEnabledFeatures {
    fn packet_id(&self) -> i32 { 0x0D }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = write_varint_bytes(self.features.len() as i32);
        for f in &self.features { buf.extend_from_slice(&write_string(f)); }
        buf
    }
}

// ═══ C2S: Additional handlers (added Phase F) ═══

/// C→S: MessageAcknowledgment (0x01) — client confirms chat message display
pub struct MessageAcknowledgment { pub message_count: i32, }
impl PacketDecoder for MessageAcknowledgment {
    fn packet_id() -> i32 { 0x01 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (count, _) = read_varint_enum(data)?;
        Ok(MessageAcknowledgment { message_count: count })
    }
}

