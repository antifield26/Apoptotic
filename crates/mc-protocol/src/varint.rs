//! VarInt / VarLong 编码 — Minecraft 协议的可变长度整数
//!
//! 每字节低 7 位为数据，最高位 (MSB) 表示是否还有后续字节。

/// 读取 VarInt（返回值和读取字节数）
pub fn read_varint(buf: &[u8]) -> Result<(i32, usize), VarIntError> {
    let mut value: i32 = 0;
    let mut shift: u32 = 0;
    let mut bytes_read = 0;

    for &byte in buf.iter() {
        bytes_read += 1;
        value |= ((byte & 0x7F) as i32) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, bytes_read));
        }
        shift += 7;
        if shift >= 32 {
            return Err(VarIntError::TooLarge);
        }
    }
    Err(VarIntError::Incomplete)
}

/// 将 i32 编码为 VarInt 字节
pub fn write_varint(value: i32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(5);
    write_varint_to(value, &mut buf);
    buf
}

/// 将 VarInt 直接写入已有缓冲区 (零分配)
#[inline]
pub fn write_varint_to(value: i32, buf: &mut Vec<u8>) {
    let mut v = value as u32;
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if v == 0 {
            break;
        }
    }
}

/// 读取 VarLong（返回值和读取字节数）
pub fn read_varlong(buf: &[u8]) -> Result<(i64, usize), VarIntError> {
    let mut value: i64 = 0;
    let mut shift: u32 = 0;
    let mut bytes_read = 0;

    for &byte in buf.iter() {
        bytes_read += 1;
        value |= ((byte & 0x7F) as i64) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, bytes_read));
        }
        shift += 7;
        if shift >= 64 {
            return Err(VarIntError::TooLarge);
        }
    }
    Err(VarIntError::Incomplete)
}

/// 将 i64 编码为 VarLong 字节
pub fn write_varlong(value: i64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(10);
    let mut v = value as u64;
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if v == 0 {
            break;
        }
    }
    buf
}

/// 获取 VarInt 的编码长度（不分配内存）
pub fn varint_size(value: i32) -> usize {
    let v = value as u32;
    if v == 0 {
        return 1;
    }
    let bits = 32 - v.leading_zeros();
    bits.div_ceil(7) as usize
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarIntError {
    Incomplete,
    TooLarge,
}

impl std::fmt::Display for VarIntError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VarIntError::Incomplete => write!(f, "incomplete VarInt"),
            VarIntError::TooLarge => write!(f, "VarInt too large"),
        }
    }
}

impl std::error::Error for VarIntError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_zero() {
        let encoded = write_varint(0);
        assert_eq!(encoded, vec![0x00]);
        let (value, bytes) = read_varint(&encoded).unwrap();
        assert_eq!(value, 0);
        assert_eq!(bytes, 1);
    }

    #[test]
    fn test_varint_positive() {
        for val in [1, 2, 127, 128, 255, 25565, 2147483647] {
            let encoded = write_varint(val);
            let (decoded, _) = read_varint(&encoded).unwrap();
            assert_eq!(decoded, val, "failed for {}", val);
        }
    }

    #[test]
    fn test_varint_negative() {
        for val in [-1, -127, -128, -2147483648] {
            let encoded = write_varint(val);
            let (decoded, _) = read_varint(&encoded).unwrap();
            assert_eq!(decoded, val, "failed for {}", val);
        }
    }

    #[test]
    fn test_varlong() {
        for val in [0, 1, -1, i64::MAX, i64::MIN] {
            let encoded = write_varlong(val);
            let (decoded, _) = read_varlong(&encoded).unwrap();
            assert_eq!(decoded, val, "failed for {}", val);
        }
    }

    #[test]
    fn test_varint_size() {
        assert_eq!(varint_size(0), 1);
        assert_eq!(varint_size(127), 1);
        assert_eq!(varint_size(128), 2);
        assert_eq!(varint_size(25565), 3);
        assert_eq!(varint_size(2147483647), 5);
    }

    #[test]
    fn test_read_incomplete() {
        assert!(matches!(
            read_varint(&[0x80]),
            Err(VarIntError::Incomplete)
        ));
    }
}
