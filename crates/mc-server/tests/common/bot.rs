//! 无头 Minecraft 客户端 — E2E 集成测试

use mc_protocol::codec::{MinecraftCodec, PacketDecoder, PacketEncoder};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// 无头 Minecraft 机器人客户端 (基础版)
pub struct BotClient {
    stream: TcpStream,
    codec: MinecraftCodec,
}

impl BotClient {
    /// Connect to a Minecraft server
    pub fn connect(host: &str, port: u16) -> std::io::Result<Self> {
        let stream = TcpStream::connect_timeout(
            &format!("{}:{}", host, port).parse().unwrap(),
            Duration::from_secs(10),
        )?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        Ok(Self { stream, codec: MinecraftCodec::new(0) })
    }

    /// Send a handshake packet
    pub fn handshake(&mut self, protocol: i32, next_state: mc_protocol::state::ConnectionState) -> Result<(), String> {
        let hs = mc_protocol::packets::handshake::HandshakePacket {
            protocol_version: protocol,
            server_address: "localhost".into(),
            server_port: 25565,
            next_state,
        };
        self.send(&hs)
    }

    /// Request server status
    pub fn request_status(&mut self) -> Result<mc_protocol::packets::status::StatusResponse, String> {
        self.send(&mc_protocol::packets::status::StatusRequest {})?;
        self.read()
    }

    /// Ping-pong
    pub fn ping(&mut self, payload: i64) -> Result<mc_protocol::packets::status::PongResponse, String> {
        self.send(&mc_protocol::packets::status::PingRequest { payload })?;
        self.read()
    }

    /// Login start (offline mode)
    pub fn login(&mut self, username: &str) -> Result<(), String> {
        self.send(&mc_protocol::packets::login::LoginStart {
            username: username.to_string(),
            player_uuid: None,
        })?;
        Ok(())
    }

    /// Wait for JoinGame after login
    pub fn wait_for_join(&mut self) -> Result<mc_protocol::packets::play::JoinGame, String> {
        self.read()
    }

    /// Read and decode a packet
    pub fn read<T: PacketDecoder>(&mut self) -> Result<T, String> {
        let len = self.read_varint()? as usize;
        let mut data = vec![0u8; len];
        self.stream.read_exact(&mut data).map_err(|e| format!("Read: {}", e))?;
        T::decode_payload(&data).map_err(|e| format!("Decode: {:?}", e))
    }

    /// Encode and send a packet
    pub fn send(&mut self, packet: &(dyn PacketEncoder + Sync)) -> Result<(), String> {
        let frame = self.codec.encode(packet).map_err(|e| format!("Encode: {:?}", e))?;
        self.stream.write_all(&frame).map_err(|e| format!("Write: {}", e))?;
        Ok(())
    }

    fn read_varint(&mut self) -> Result<i32, String> {
        let mut value = 0i32;
        for i in 0..5u32 {
            let mut buf = [0u8; 1];
            self.stream.read_exact(&mut buf).map_err(|e| format!("VarInt: {}", e))?;
            let byte = buf[0];
            value |= ((byte & 0x7F) as i32) << (i * 7);
            if byte & 0x80 == 0 { return Ok(value); }
        }
        Err("VarInt too big".into())
    }

    /// Send a chat command (via ChatCommand packet 0x05)
    pub fn send_command(&mut self, command: &str) -> Result<(), String> {
        let cmd = mc_protocol::packets::play::ChatCommand {
            command: command.to_string(),
        };
        self.send(&cmd)
    }

    /// Read any play packet as raw bytes (for tolerance testing)
    pub fn read_raw(&mut self) -> Result<Vec<u8>, String> {
        let len = self.read_varint()? as usize;
        let mut data = vec![0u8; len];
        self.stream.read_exact(&mut data).map_err(|e| format!("Read: {}", e))?;
        Ok(data)
    }
}
