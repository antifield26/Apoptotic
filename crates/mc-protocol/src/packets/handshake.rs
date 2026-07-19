//! C→S: Handshake (packet ID 0x00)
//!
//! 客户端连接时发送的第一个数据包。

use crate::codec::*;
use crate::state::ConnectionState;

#[derive(Debug, Clone)]
pub struct HandshakePacket {
    pub protocol_version: i32,
    pub server_address: String,
    pub server_port: u16,
    pub next_state: ConnectionState,
}

impl HandshakePacket {
    pub const PACKET_ID: i32 = 0x00;
}

impl PacketEncoder for HandshakePacket {
    fn packet_id(&self) -> i32 {
        Self::PACKET_ID
    }

    fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&write_varint_bytes(self.protocol_version));
        buf.extend_from_slice(&write_string(&self.server_address));
        buf.extend_from_slice(&write_u16(self.server_port));
        buf.extend_from_slice(&write_varint_bytes(match self.next_state {
            ConnectionState::Status => 1,
            ConnectionState::Login => 2,
            _ => 0,
        }));
        buf
    }
}

impl PacketDecoder for HandshakePacket {
    fn packet_id() -> i32 {
        Self::PACKET_ID
    }

    fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
        let (protocol_version, mut offset) = read_varint_enum(data)?;
        let (server_address, n) = read_string(&data[offset..])?;
        offset += n;
        let (server_port, n) = read_u16(&data[offset..])?;
        offset += n;
        let (next_state_id, _) = read_varint_enum(&data[offset..])?;

        let next_state = ConnectionState::from_next_state(next_state_id)
            .ok_or_else(|| CodecError::Malformed(format!("invalid next state: {}", next_state_id)))?;

        Ok(Self {
            protocol_version,
            server_address: server_address.to_string(),
            server_port,
            next_state,
        })
    }
}
