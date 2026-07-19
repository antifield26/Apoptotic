//! Status 阶段数据包 (server list ping)

use crate::codec::*;
use serde::{Deserialize, Serialize};

/// C→S: Status Request (0x00)
#[derive(Debug, Clone)]
pub struct StatusRequest;

impl PacketEncoder for StatusRequest {
    fn packet_id(&self) -> i32 { 0x00 }
    fn encode_payload(&self) -> Vec<u8> { Vec::new() }
}

impl PacketDecoder for StatusRequest {
    fn packet_id() -> i32 { 0x00 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Ok(Self)
    }
}

/// S→C: Status Response (0x00)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub version: VersionInfo,
    pub players: PlayersInfo,
    pub description: Description,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
    #[serde(rename = "enforcesSecureChat")]
    pub enforces_secure_chat: bool,
    #[serde(rename = "previewsChat")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previews_chat: Option<bool>,
}

impl PacketEncoder for StatusResponse {
    fn packet_id(&self) -> i32 { 0x00 }
    fn encode_payload(&self) -> Vec<u8> {
        let json = serde_json::to_string(self).unwrap_or_else(|_| "{}".into());
        write_string(&json)
    }
}

impl PacketDecoder for StatusResponse {
    fn packet_id() -> i32 { 0x00 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (json, _) = read_string(data)?;
        serde_json::from_str(json)
            .map_err(|e| CodecError::Malformed(format!("invalid JSON: {}", e)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub name: String,
    pub protocol: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayersInfo {
    pub max: u32,
    pub online: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sample: Vec<PlayerSampleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSampleEntry {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Description {
    pub text: String,
}

/// C→S: Ping Request (0x01)
#[derive(Debug, Clone)]
pub struct PingRequest {
    pub payload: i64,
}

impl PacketEncoder for PingRequest {
    fn packet_id(&self) -> i32 { 0x01 }
    fn encode_payload(&self) -> Vec<u8> {
        write_i64(self.payload).to_vec()
    }
}

impl PacketDecoder for PingRequest {
    fn packet_id() -> i32 { 0x01 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (payload, _) = read_i64(data)?;
        Ok(Self { payload })
    }
}

/// S→C: Pong Response (0x01)
#[derive(Debug, Clone)]
pub struct PongResponse {
    pub payload: i64,
}

impl PacketEncoder for PongResponse {
    fn packet_id(&self) -> i32 { 0x01 }
    fn encode_payload(&self) -> Vec<u8> {
        write_i64(self.payload).to_vec()
    }
}

impl PacketDecoder for PongResponse {
    fn packet_id() -> i32 { 0x01 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (payload, _) = read_i64(data)?;
        Ok(Self { payload })
    }
}
