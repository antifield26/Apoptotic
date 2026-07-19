//! Login 阶段数据包

use crate::codec::*;
use uuid::Uuid;

/// C→S: Login Start (0x00)
#[derive(Debug, Clone)]
pub struct LoginStart {
    pub username: String,
    pub player_uuid: Option<Uuid>,
}

impl PacketEncoder for LoginStart {
    fn packet_id(&self) -> i32 { 0x00 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = write_string(&self.username);
        if let Some(ref uuid) = self.player_uuid {
            buf.extend_from_slice(&write_bool(true));
            buf.extend_from_slice(&write_uuid(uuid));
        } else {
            buf.extend_from_slice(&write_bool(false));
        }
        buf
    }
}

impl PacketDecoder for LoginStart {
    fn packet_id() -> i32 { 0x00 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (username, mut offset) = read_string(data)?;
        // In newer protocol versions, there may be a UUID field
        let player_uuid = if offset < data.len() {
            let (has_uuid, n) = read_bool(&data[offset..])?;
            offset += n;
            if has_uuid {
                let (uuid, _) = read_uuid(&data[offset..])?;
                Some(uuid)
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            username: username.to_string(),
            player_uuid,
        })
    }
}

/// S→C: Login Success (0x02)
#[derive(Debug, Clone)]
pub struct LoginSuccess {
    pub uuid: Uuid,
    pub username: String,
    pub properties: Vec<Property>,
}

impl PacketEncoder for LoginSuccess {
    fn packet_id(&self) -> i32 { 0x02 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_uuid(&self.uuid));
        buf.extend_from_slice(&write_string(&self.username));

        // properties (varint-prefixed list)
        buf.extend_from_slice(&write_varint_bytes(self.properties.len() as i32));
        for prop in &self.properties {
            buf.extend_from_slice(&write_string(&prop.name));
            buf.extend_from_slice(&write_string(&prop.value));
            if let Some(ref sig) = prop.signature {
                buf.extend_from_slice(&write_bool(true));
                buf.extend_from_slice(&write_string(sig));
            } else {
                buf.extend_from_slice(&write_bool(false));
            }
        }
        buf
    }
}

impl PacketDecoder for LoginSuccess {
    fn packet_id() -> i32 { 0x02 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("LoginSuccess decode not implemented".into()))
    }
}

#[derive(Debug, Clone)]
pub struct Property {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}

/// S→C: Login Disconnect (0x00)
#[derive(Debug, Clone)]
pub struct LoginDisconnect {
    pub reason: String,
}

impl PacketEncoder for LoginDisconnect {
    fn packet_id(&self) -> i32 { 0x00 }
    fn encode_payload(&self) -> Vec<u8> {
        write_string(&self.reason)
    }
}

impl PacketDecoder for LoginDisconnect {
    fn packet_id() -> i32 { 0x00 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Err(CodecError::Malformed("LoginDisconnect decode not implemented".into()))
    }
}

/// C→S: Login Acknowledged (0x03)
/// 客户端在收到 LoginSuccess 后发送此包确认进入 PLAY 状态
#[derive(Debug, Clone)]
pub struct LoginAcknowledged;

impl PacketEncoder for LoginAcknowledged {
    fn packet_id(&self) -> i32 { 0x03 }
    fn encode_payload(&self) -> Vec<u8> { Vec::new() }
}

impl PacketDecoder for LoginAcknowledged {
    fn packet_id() -> i32 { 0x03 }
    fn decode_payload(_data: &[u8]) -> Result<Self, CodecError> {
        Ok(LoginAcknowledged)
    }
}

/// S→C: Encryption Request (0x01) — 在线模式: 发送公钥和验证令牌
pub struct EncryptionRequest {
    pub server_id: String,     // always "" in modern Minecraft
    pub public_key: Vec<u8>,   // DER-encoded RSA public key
    pub verify_token: Vec<u8>, // 4 random bytes
}

impl PacketEncoder for EncryptionRequest {
    fn packet_id(&self) -> i32 { 0x01 }
    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_string(&self.server_id));
        buf.extend_from_slice(&write_varint_bytes(self.public_key.len() as i32));
        buf.extend_from_slice(&self.public_key);
        buf.extend_from_slice(&write_varint_bytes(self.verify_token.len() as i32));
        buf.extend_from_slice(&self.verify_token);
        buf
    }
}

/// C→S: Encryption Response (0x01) — 在线模式: 客户端返回加密的共享密钥
pub struct EncryptionResponse {
    pub shared_secret: Vec<u8>, // RSA-encrypted 16-byte shared secret
    pub verify_token: Vec<u8>,  // RSA-encrypted 4-byte verify token
}

impl PacketDecoder for EncryptionResponse {
    fn packet_id() -> i32 { 0x01 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (secret_len, mut offset) = read_varint_enum(data)?;
        let secret_end = offset + secret_len as usize;
        if secret_end > data.len() { return Err(CodecError::Malformed("shared secret too short".into())); }
        let shared_secret = data[offset..secret_end].to_vec();
        offset = secret_end;

        let (token_len, _) = read_varint_enum(&data[offset..])?; offset += {
            let (_, n) = read_varint_enum(&data[offset..])?; n
        };
        let token_end = offset + token_len as usize;
        if token_end > data.len() { return Err(CodecError::Malformed("verify token too short".into())); }
        let verify_token = data[offset..token_end].to_vec();

        Ok(Self { shared_secret, verify_token })
    }
}

/// S→C: Set Compression (0x03)
pub struct SetCompression {
    pub threshold: i32,
}

impl PacketEncoder for SetCompression {
    fn packet_id(&self) -> i32 { 0x03 }
    fn encode_payload(&self) -> Vec<u8> {
        write_varint_bytes(self.threshold)
    }
}

impl PacketDecoder for SetCompression {
    fn packet_id() -> i32 { 0x03 }
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (threshold, _) = read_varint_enum(data)?;
        Ok(Self { threshold })
    }
}
