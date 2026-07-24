//! Configuration 阶段数据包 (1.20.2+ / protocol 764+)
//!
//! Login 完成后进入此阶段，用于同步注册表、功能标志等。

use crate::codec::*;
use crate::packets::play::read_u8;

// ═══════════════════════════════════════════════════════
// S→C: Plugin Message (custom payload) — generic plugin channel
// Protocol ID in config: varies, we use "minecraft:brand"
// ═══════════════════════════════════════════════════════

pub struct ConfigPluginMessage {
    pub channel: String,
    pub data: Vec<u8>,
}

impl PacketEncoder for ConfigPluginMessage {
    fn packet_id(&self) -> i32 { 0x01 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_string(&self.channel));
        buf.extend_from_slice(&self.data);
        buf
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Registry Data (0x07) — dimension/biome registry
// ═══════════════════════════════════════════════════════

pub struct RegistryData {
    pub registry_id: String,       // e.g. "minecraft:dimension_type"
    pub entries: Vec<RegistryEntry>,
}

pub struct RegistryEntry {
    pub key: String,      // e.g. "minecraft:overworld"
    pub data: Option<Vec<u8>>,  // NBT data (None = use defaults)
}

impl PacketEncoder for RegistryData {
    fn packet_id(&self) -> i32 { 0x07 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_string(&self.registry_id));
        buf.extend_from_slice(&write_varint_bytes(self.entries.len() as i32));
        for entry in &self.entries {
            buf.extend_from_slice(&write_string(&entry.key));
            buf.extend_from_slice(&write_bool(entry.data.is_some()));
            if let Some(ref data) = entry.data {
                buf.extend_from_slice(data);
            }
        }
        buf
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Feature Flags (0x0C) — enabled feature flags
// ═══════════════════════════════════════════════════════

pub struct FeatureFlags {
    pub flags: Vec<String>,
}

impl PacketEncoder for FeatureFlags {
    fn packet_id(&self) -> i32 { 0x0D }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.flags.len() as i32));
        for flag in &self.flags {
            buf.extend_from_slice(&write_string(flag));
        }
        buf
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Finish Configuration (0x03) — signal end of config
// ═══════════════════════════════════════════════════════

pub struct FinishConfiguration;

impl PacketEncoder for FinishConfiguration {
    fn packet_id(&self) -> i32 { 0x03 }
    fn encode_payload(&self) -> Vec<u8> {
        Vec::new() // empty payload
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Known Packs (0x07 alternative) — server data packs
// ═══════════════════════════════════════════════════════

pub struct KnownPacks {
    pub known_packs: Vec<KnownPack>,
}

pub struct KnownPack {
    pub namespace: String,
    pub id: String,
    pub version: String,
}

impl PacketEncoder for KnownPacks {
    fn packet_id(&self) -> i32 { 0x0F }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.known_packs.len() as i32));
        for pack in &self.known_packs {
            buf.extend_from_slice(&write_string(&pack.namespace));
            buf.extend_from_slice(&write_string(&pack.id));
            buf.extend_from_slice(&write_string(&pack.version));
        }
        buf
    }
}

// ═══════════════════════════════════════════════════════
// S→C: Keep Alive (Config) — same structure as Play keep-alive
// ═══════════════════════════════════════════════════════

pub struct ConfigKeepAlive {
    pub id: i64,
}

impl PacketEncoder for ConfigKeepAlive {
    fn packet_id(&self) -> i32 { 0x04 }
    fn encode_payload(&self) -> Vec<u8> {
        write_i64(self.id).to_vec()
    }
}

// ═══════════════════════════════════════════════════════
// C→S: Acknowledge Finish Configuration (0x03)
// ═══════════════════════════════════════════════════════

pub struct AckFinishConfig;

impl PacketDecoder for AckFinishConfig {
    fn packet_id() -> i32 { 0x03 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Ok(Self)
    }
}

// ═══════════════════════════════════════════════════════
// C→S: Client Information (Config) — same as play ClientInformation
// ═══════════════════════════════════════════════════════

pub struct ConfigClientInformation {
    pub locale: String,
    pub view_distance: u8,
    pub chat_mode: i32,
    pub chat_colors: bool,
    pub displayed_skin_parts: u8,
    pub main_hand: i32,
    pub enable_text_filtering: bool,
    pub allow_server_listings: bool,
}

impl PacketDecoder for ConfigClientInformation {
    fn packet_id() -> i32 { 0x00 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (locale, mut offset) = read_string(data)?;
        let (view_distance, n) = read_u8(&data[offset..])?; offset += n;
        let (chat_mode, n) = read_varint_enum(&data[offset..])?; offset += n;
        let (chat_colors, n) = read_bool(&data[offset..])?; offset += n;
        let (displayed_skin_parts, n) = read_u8(&data[offset..])?; offset += n;
        let (main_hand, n) = read_varint_enum(&data[offset..])?; offset += n;
        let (enable_text_filtering, n) = read_bool(&data[offset..])?; offset += n;
        let (allow_server_listings, _) = read_bool(&data[offset..])?;
        Ok(Self { locale: locale.to_string(), view_distance, chat_mode, chat_colors, displayed_skin_parts, main_hand, enable_text_filtering, allow_server_listings })
    }
}

// ═══════════════════════════════════════════════════════
// C→S: Pong (Config) — response to server keep-alive
// ═══════════════════════════════════════════════════════

pub struct ConfigPong {
    pub id: i32,
}

impl PacketDecoder for ConfigPong {
    fn packet_id() -> i32 { 0x04 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (id, _) = read_varint_enum(data)?;
        Ok(Self { id })
    }
}

// ═══════════════════════════════════════════════════════
// C→S: Known Packs (Serverbound)
// ═══════════════════════════════════════════════════════

pub struct ServerboundKnownPacks {
    pub known_packs: Vec<KnownPack>,
}

impl PacketDecoder for ServerboundKnownPacks {
    fn packet_id() -> i32 { 0x07 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let mut known_packs = Vec::new();
        let (count, mut offset) = read_varint_enum(data)?;
        for _ in 0..count {
            let (namespace, n) = read_string(&data[offset..])?; offset += n;
            let (id, n) = read_string(&data[offset..])?; offset += n;
            let (version, n) = read_string(&data[offset..])?; offset += n;
            known_packs.push(KnownPack { namespace: namespace.to_string(), id: id.to_string(), version: version.to_string() });
        }
        Ok(Self { known_packs })
    }
}
