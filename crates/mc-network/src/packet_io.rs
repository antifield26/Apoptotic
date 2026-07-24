//! 数据包 I/O — TCP 流的分帧读写 (支持 AES-CFB8 加密)

use mc_protocol::codec::MinecraftCodec;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

/// 从 TCP 流读取一帧完整的 Minecraft 数据包
pub struct PacketStream {
    read: OwnedReadHalf,
    write: OwnedWriteHalf,
    codec: MinecraftCodec,
    /// AES-CFB8 加密器 (在线模式启用后设置)
    encryption: Option<crate::encryption::AesCfb8>,
}

impl PacketStream {
    pub fn new(
        read: OwnedReadHalf,
        write: OwnedWriteHalf,
        compression_threshold: u32,
    ) -> Self {
        Self {
            read,
            write,
            codec: MinecraftCodec::new(compression_threshold),
            encryption: None,
        }
    }

    /// 启用 AES-CFB8 加密 (在线模式登录完成后调用)
    pub fn enable_encryption(&mut self, key: &[u8; 16]) {
        self.encryption = Some(crate::encryption::AesCfb8::new(key));
    }

    /// 读取一个完整的帧（阻塞直到帧完整）
    pub async fn read_frame(&mut self) -> std::io::Result<Vec<u8>> {
        // 读取 frame length (VarInt)
        let mut len_buf = vec![0u8; 1];
        let mut length: i32 = 0;
        let mut shift: u32 = 0;

        loop {
            let n = self.read.read_exact(&mut len_buf).await?;
            if n == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "connection closed",
                ));
            }
            let byte = len_buf[0];
            length |= ((byte & 0x7F) as i32) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift >= 32 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "VarInt too large",
                ));
            }
        }

        // 读取 frame body
        let frame_len = length as usize;
        if frame_len > 2_097_152 {
            // 2MB limit
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("frame too large: {}", frame_len),
            ));
        }

        let mut frame = vec![0u8; frame_len];
        self.read.read_exact(&mut frame).await?;

        // 解密 (如果加密已启用) — incoming data is ciphertext
        if let Some(ref mut cipher) = self.encryption {
            cipher.decrypt(&mut frame);
        }

        Ok(frame)
    }

    /// 发送一个编码后的帧
    /// `data` is the full frame: [VarInt length prefix] + [packet body].
    /// When encryption is enabled, ONLY the packet body is encrypted;
    /// the length prefix MUST remain plaintext per Minecraft protocol spec.
    pub async fn write_frame(&mut self, data: &[u8]) -> std::io::Result<()> {
        if let Some(ref mut cipher) = self.encryption {
            // Parse VarInt length prefix to find body boundary
            let mut prefix_len: usize = 0;
            let mut _length: i32 = 0;
            let mut shift: u32 = 0;
            for &byte in data.iter() {
                prefix_len += 1;
                _length |= ((byte & 0x7F) as i32) << shift;
                if byte & 0x80 == 0 {
                    break;
                }
                shift += 7;
                if shift >= 32 || prefix_len >= 5 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "VarInt too large in outgoing frame",
                    ));
                }
            }
            // Split: plaintext length prefix + encrypt body only
            let prefix = &data[..prefix_len];
            let body = &data[prefix_len..];
            let mut encrypted_body = body.to_vec();
            cipher.encrypt(&mut encrypted_body);
            // Write [plaintext length prefix] + [encrypted body]
            self.write.write_all(prefix).await?;
            self.write.write_all(&encrypted_body).await?;
        } else {
            self.write.write_all(data).await?;
        }
        Ok(())
    }

    /// 获取底层 codec 的引用
    pub fn codec(&self) -> &MinecraftCodec {
        &self.codec
    }

    /// 获取 codec 的可变引用（用于在压缩启用后更新）
    pub fn codec_mut(&mut self) -> &mut MinecraftCodec {
        &mut self.codec
    }
}
