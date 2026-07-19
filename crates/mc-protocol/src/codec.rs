//! Minecraft 数据包编解码器
//!
//! 协议分帧格式：
//! - 未压缩: [length:VarInt] [packet_id:VarInt] [payload]
//! - 压缩:   [length:VarInt] [data_length:VarInt] [packet_id:VarInt | compressed_payload]
//!   - data_length == 0 表示未压缩

use crate::varint;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Read, Write};

/// 数据包编码 trait — 将数据包编码为原始字节
pub trait PacketEncoder: Send + Sync {
    fn packet_id(&self) -> i32;
    fn encode_payload(&self) -> Vec<u8>;
}

/// 数据包解码 trait — 从原始字节重建数据包
pub trait PacketDecoder: Sized {
    fn packet_id() -> i32;
    fn decode_payload(data: &[u8]) -> Result<Self, CodecError>;
}

/// 编解码错误
#[derive(Debug)]
pub enum CodecError {
    Io(std::io::Error),
    VarInt(crate::varint::VarIntError),
    InvalidPacketId { expected: i32, got: i32 },
    Malformed(String),
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecError::Io(e) => write!(f, "IO error: {}", e),
            CodecError::VarInt(e) => write!(f, "VarInt error: {}", e),
            CodecError::InvalidPacketId { expected, got } => {
                write!(f, "Invalid packet ID: expected {}, got {}", expected, got)
            }
            CodecError::Malformed(s) => write!(f, "Malformed packet: {}", s),
        }
    }
}

impl std::error::Error for CodecError {}

impl From<std::io::Error> for CodecError {
    fn from(e: std::io::Error) -> Self {
        CodecError::Io(e)
    }
}

impl From<crate::varint::VarIntError> for CodecError {
    fn from(e: crate::varint::VarIntError) -> Self {
        CodecError::VarInt(e)
    }
}

/// Minecraft 协议编解码器 — 管理压缩状态
pub struct MinecraftCodec {
    compression_threshold: u32,
}

impl MinecraftCodec {
    pub fn new(compression_threshold: u32) -> Self {
        Self {
            compression_threshold,
        }
    }

    /// 编码一个数据包为帧格式字节
    pub fn encode(&self, encoder: &(dyn PacketEncoder + Sync)) -> Result<Vec<u8>, CodecError> {
        let packet_id = encoder.packet_id();
        let payload = encoder.encode_payload();
        // 预分配: 5字节 varint + payload
        let mut packet_data = Vec::with_capacity(5 + payload.len());
        varint::write_varint_to(packet_id, &mut packet_data);
        packet_data.extend_from_slice(&payload);

        if self.compression_threshold > 0 {
            let data_len_varint;
            let body;
            if packet_data.len() >= self.compression_threshold as usize {
                // Pre-allocate compression buffer — avoids reallocation for large packets (ChunkData ~5-50KB)
                let comp_buf = Vec::with_capacity(32768);
                let mut enc = ZlibEncoder::new(comp_buf, Compression::default());
                enc.write_all(&packet_data)?;
                let compressed = enc.finish()?;
                data_len_varint = varint::write_varint(packet_data.len() as i32);
                body = compressed;
            } else {
                data_len_varint = varint::write_varint(0);
                body = packet_data;
            }
            let mut frame = Vec::with_capacity(5 + data_len_varint.len() + body.len());
            varint::write_varint_to((data_len_varint.len() + body.len()) as i32, &mut frame);
            frame.extend_from_slice(&data_len_varint);
            frame.extend_from_slice(&body);
            Ok(frame)
        } else {
            let mut frame = Vec::with_capacity(5 + packet_data.len());
            varint::write_varint_to(packet_data.len() as i32, &mut frame);
            frame.extend_from_slice(&packet_data);
            Ok(frame)
        }
    }

    /// 解码一帧为 (packet_id, payload_bytes)
    pub fn decode_frame(
        &self,
        frame: &[u8],
    ) -> Result<(i32, Vec<u8>), CodecError> {
        let (total_len, header_bytes) = varint::read_varint(frame)?;
        let remaining = &frame[header_bytes..];

        if remaining.len() < total_len as usize {
            return Err(CodecError::Malformed("frame too short".into()));
        }

        let packet_data = &remaining[..total_len as usize];

        if self.compression_threshold > 0 {
            // 压缩模式：读取 data_length
            let (data_len, dl_bytes) = varint::read_varint(packet_data)?;
            let payload = &packet_data[dl_bytes..];

            if data_len == 0 {
                // 未压缩
                self.parse_packet_id_and_payload(payload)
            } else {
                // 压缩
                let mut decoder = ZlibDecoder::new(payload);
                let mut decompressed = Vec::with_capacity(data_len as usize);
                decoder.read_to_end(&mut decompressed)?;
                self.parse_packet_id_and_payload(&decompressed)
            }
        } else {
            // 未压缩
            self.parse_packet_id_and_payload(packet_data)
        }
    }

    /// 解码一个已去帧的包体为特定包类型
    /// `frame` 应该是已经去除外层 VarInt 长度前缀的包体数据
    pub fn decode<T: PacketDecoder>(&self, frame: &[u8]) -> Result<T, CodecError> {
        let (packet_id, payload) = self.parse_packet_id_and_payload(frame)?;
        if packet_id != T::packet_id() {
            return Err(CodecError::InvalidPacketId {
                expected: T::packet_id(),
                got: packet_id,
            });
        }
        T::decode_payload(&payload)
    }

    /// 解析数据体为 (packet_id, payload_bytes)
    pub fn parse_packet_id_and_payload(&self, data: &[u8]) -> Result<(i32, Vec<u8>), CodecError> {
        let (packet_id, id_bytes) = varint::read_varint(data)?;
        let payload = data[id_bytes..].to_vec();
        Ok((packet_id, payload))
    }
}

// ── 便捷读写方法 ──

/// 从字节切片读取 VarInt 前缀字符串
pub fn read_string(data: &[u8]) -> Result<(&str, usize), CodecError> {
    let (len, len_bytes) = varint::read_varint(data)?;
    let end = len_bytes + len as usize;
    if end > data.len() {
        return Err(CodecError::Malformed("string data too short".into()));
    }
    let s = std::str::from_utf8(&data[len_bytes..end])
        .map_err(|e| CodecError::Malformed(format!("invalid UTF-8: {}", e)))?;
    Ok((s, end))
}

/// 写入 VarInt 前缀字符串
pub fn write_string(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut buf = varint::write_varint(bytes.len() as i32);
    buf.extend_from_slice(bytes);
    buf
}

/// 读取 i16 (big-endian)
pub fn read_i16(data: &[u8]) -> Result<(i16, usize), CodecError> {
    if data.len() < 2 {
        return Err(CodecError::Malformed("i16 too short".into()));
    }
    Ok((i16::from_be_bytes([data[0], data[1]]), 2))
}

/// 写入 i16 (big-endian)
pub fn write_i16(v: i16) -> [u8; 2] {
    v.to_be_bytes()
}

/// 读取 u16 (big-endian)
pub fn read_u16(data: &[u8]) -> Result<(u16, usize), CodecError> {
    if data.len() < 2 {
        return Err(CodecError::Malformed("u16 too short".into()));
    }
    Ok((u16::from_be_bytes([data[0], data[1]]), 2))
}

/// 写入 u16 (big-endian)
pub fn write_u16(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}

/// 读取 i64 (big-endian)
pub fn read_i64(data: &[u8]) -> Result<(i64, usize), CodecError> {
    if data.len() < 8 {
        return Err(CodecError::Malformed("i64 too short".into()));
    }
    Ok((i64::from_be_bytes(data[..8].try_into().expect("guaranteed 8 bytes after length check")), 8))
}

/// 写入 i64 (big-endian)
pub fn write_i64(v: i64) -> [u8; 8] {
    v.to_be_bytes()
}

/// 读取 f64 (big-endian)
pub fn read_f64(data: &[u8]) -> Result<(f64, usize), CodecError> {
    let (bits, n) = read_i64(data)?;
    Ok((f64::from_be_bytes(bits.to_be_bytes()), n))
}

/// 写入 f64 (big-endian)
pub fn write_f64(v: f64) -> [u8; 8] {
    v.to_be_bytes()
}

/// 读取 double (alias for f64)
pub fn read_double(data: &[u8]) -> Result<(f64, usize), CodecError> {
    read_f64(data)
}

/// 写入 double (alias for f64)
pub fn write_double(v: f64) -> [u8; 8] {
    v.to_be_bytes()
}

/// 读取 bool
pub fn read_bool(data: &[u8]) -> Result<(bool, usize), CodecError> {
    if data.is_empty() {
        return Err(CodecError::Malformed("bool too short".into()));
    }
    Ok((data[0] != 0, 1))
}

/// 写入 bool
pub fn write_bool(v: bool) -> [u8; 1] {
    [v as u8]
}

/// 读取 UUID (128 bits, big-endian)
pub fn read_uuid(data: &[u8]) -> Result<(uuid::Uuid, usize), CodecError> {
    if data.len() < 16 {
        return Err(CodecError::Malformed("UUID too short".into()));
    }
    Ok((uuid::Uuid::from_bytes(data[..16].try_into().expect("guaranteed 16 bytes after length check")), 16))
}

/// 写入 UUID (128 bits, big-endian)
pub fn write_uuid(u: &uuid::Uuid) -> [u8; 16] {
    *u.as_bytes()
}

/// VarInt 编码的枚举值
pub fn read_varint_enum(data: &[u8]) -> Result<(i32, usize), CodecError> {
    varint::read_varint(data).map_err(CodecError::VarInt)
}

/// 写入 VarInt
pub fn write_varint_bytes(v: i32) -> Vec<u8> {
    varint::write_varint(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codec_uncompressed_roundtrip() {
        let codec = MinecraftCodec::new(0); // no compression

        struct TestPacket { msg: String }
        impl PacketEncoder for TestPacket {
            fn packet_id(&self) -> i32 { 0x00 }
            fn encode_payload(&self) -> Vec<u8> {
                write_string(&self.msg)
            }
        }
        impl PacketDecoder for TestPacket {
            fn packet_id() -> i32 { 0x00 }
            fn decode_payload(data: &[u8]) -> Result<Self, CodecError> {
                let (msg, _) = read_string(data)?;
                Ok(Self { msg: msg.to_string() })
            }
        }

        let pkt = TestPacket { msg: "hello".into() };
        let framed = codec.encode(&pkt).unwrap();

        // Simulate PacketStream::read_frame: strip outer length prefix
        let (body_len, header_bytes) = crate::varint::read_varint(&framed).unwrap();
        let body = &framed[header_bytes..header_bytes + body_len as usize];

        // Now decode the body
        let decoded: TestPacket = codec.decode(body).unwrap();
        assert_eq!(decoded.msg, "hello");
    }

    #[test]
    fn test_read_write_string() {
        let s = "Minecraft Server";
        let encoded = write_string(s);
        let (decoded, bytes_read) = read_string(&encoded).unwrap();
        assert_eq!(decoded, s);
        assert_eq!(bytes_read, encoded.len());
    }

    #[test]
    fn test_read_write_i16() {
        for v in [0, 1, -1, 25565, -32768, 32767i16] {
            let encoded = write_i16(v);
            let (decoded, n) = read_i16(&encoded).unwrap();
            assert_eq!(decoded, v);
            assert_eq!(n, 2);
        }
    }

    #[test]
    fn test_read_write_i64() {
        let v: i64 = 1234567890123;
        let encoded = write_i64(v);
        let (decoded, n) = read_i64(&encoded).unwrap();
        assert_eq!(decoded, v);
        assert_eq!(n, 8);
    }

    #[test]
    fn test_read_write_uuid() {
        let u = uuid::Uuid::new_v4();
        let encoded = write_uuid(&u);
        let (decoded, n) = read_uuid(&encoded).unwrap();
        assert_eq!(decoded, u);
        assert_eq!(n, 16);
    }
}
