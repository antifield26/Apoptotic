//! 调色板容器 — 可变位宽方块存储
//!
//! Minecraft 使用三种模式高效存储 16×16×16=4096 个方块：
//! - Single:  所有方块相同，占用 0 bits/block
//! - Indirect: 使用调色板映射，占用 ceil(log2(palette.len())) bits/block
//! - Direct:   直接存储全局 ID，占用 ceil(log2(global_states)) bits/block
//!
//! 参考: https://wiki.vg/Chunk_Format

use mc_core::block::BlockState;

/// Section 中的方块总数
const BLOCKS_PER_SECTION: usize = 4096; // 16×16×16

/// 全局直接模式位宽（Minecraft ~1.21 约 30000+ 方块状态 → 15 bits）
const GLOBAL_BITS: u8 = 15;

/// 从调色板大小计算需要的位数 (ceil(log2(count)))
const fn bits_for(count: usize) -> u8 {
    if count <= 1 {
        0
    } else {
        // Use count-1 to get ceil(log2(count)) rather than floor(log2(count))+1
        // e.g. count=2 → bits=1, count=4 → bits=2, count=8 → bits=3
        (usize::BITS - (count - 1).leading_zeros()) as u8
    }
}

/// 调色板容器
#[derive(Debug, Clone)]
pub struct PalettedContainer {
    mode: ContainerMode,
}

#[derive(Debug, Clone)]
enum ContainerMode {
    /// 所有方块相同
    Single(BlockState),
    /// 调色板映射模式
    Indirect {
        palette: Vec<BlockState>,
        data: Vec<u64>,
        bits_per_entry: u8,
    },
    /// 直接全局 ID
    Direct {
        data: Vec<u64>,
    },
}

impl PalettedContainer {
    /// 创建全空气容器
    pub fn new() -> Self {
        Self {
            mode: ContainerMode::Single(BlockState::AIR),
        }
    }

    /// 以指定方块填充创建
    pub fn filled(block: BlockState) -> Self {
        Self {
            mode: ContainerMode::Single(block),
        }
    }

    /// 读取指定坐标的方块
    pub fn get(&self, x: usize, y: usize, z: usize) -> BlockState {
        let idx = index(x, y, z);
        match &self.mode {
            ContainerMode::Single(b) => *b,
            ContainerMode::Indirect {
                palette,
                data,
                bits_per_entry,
            } => {
                if *bits_per_entry == 0 {
                    palette[0]
                } else {
                    let palette_idx = read_packed(data, idx, *bits_per_entry);
                    palette.get(palette_idx as usize).copied().unwrap_or(BlockState::AIR)
                }
            }
            ContainerMode::Direct { data } => {
                let id = read_packed(data, idx, GLOBAL_BITS);
                BlockState::new(id as u32)
            }
        }
    }

    /// 设置指定坐标的方块
    pub fn set(&mut self, x: usize, y: usize, z: usize, block: BlockState) {
        let idx = index(x, y, z);

        match &mut self.mode {
            ContainerMode::Single(current) => {
                if *current == block {
                    return; // no change
                }
                // Upgrade: Single → Indirect
                let bits = 1u8; // start with 1 bit (2-entry palette)
                let palette = vec![*current, block];
                let mut new_data = new_packed(BLOCKS_PER_SECTION, bits);
                // Write old value at idx (already 0 since data is zeroed)
                write_packed(&mut new_data, idx, bits, 1); // index 1 = new block
                self.mode = ContainerMode::Indirect {
                    palette,
                    data: new_data,
                    bits_per_entry: bits,
                };
            }
            ContainerMode::Indirect {
                palette,
                data,
                bits_per_entry,
            } => {
                // Try to insert into existing palette
                if let Some(pos) = palette.iter().position(|b| *b == block) {
                    write_packed(data, idx, *bits_per_entry, pos as u64);
                    return;
                }

                // Need to add to palette — may need to grow bits
                let new_count = palette.len() + 1;
                let new_bits = bits_for(new_count);

                if new_bits > *bits_per_entry {
                    // Upgrade: expand bits or go to Direct
                    if new_bits > GLOBAL_BITS {
                        // Too many entries → go to Direct
                        self.upgrade_to_direct();
                        // Recursively set
                        self.set(x, y, z, block);
                        return;
                    }
                    // Expand bits
                    let mut new_data = new_packed(BLOCKS_PER_SECTION, new_bits);
                    for i in 0..BLOCKS_PER_SECTION {
                        let old_val = read_packed(data, i, *bits_per_entry);
                        write_packed(&mut new_data, i, new_bits, old_val);
                    }
                    palette.push(block);
                    let new_idx = palette.len() - 1;
                    write_packed(&mut new_data, idx, new_bits, new_idx as u64);
                    *data = new_data;
                    *bits_per_entry = new_bits;
                } else {
                    // Same bit width, just add to palette
                    palette.push(block);
                    write_packed(data, idx, *bits_per_entry, (palette.len() - 1) as u64);
                }
            }
            ContainerMode::Direct { data } => {
                write_packed(data, idx, GLOBAL_BITS, block.id as u64);
            }
        }
    }

    /// 升级到 Direct 模式
    fn upgrade_to_direct(&mut self) {
        let mut new_data = new_packed(BLOCKS_PER_SECTION, GLOBAL_BITS);
        for i in 0..BLOCKS_PER_SECTION {
            let block = match &self.mode {
                ContainerMode::Single(b) => *b,
                ContainerMode::Indirect {
                    palette,
                    data,
                    bits_per_entry,
                } => {
                    let palette_idx = read_packed(data, i, *bits_per_entry);
                    palette.get(palette_idx as usize).copied().unwrap_or(BlockState::AIR)
                }
                ContainerMode::Direct { .. } => return, // already direct, no-op
            };
            write_packed(&mut new_data, i, GLOBAL_BITS, block.id as u64);
        }
        self.mode = ContainerMode::Direct { data: new_data };
    }

    /// 所有方块的迭代器
    pub fn iter_blocks(&self) -> Vec<BlockState> {
        (0..BLOCKS_PER_SECTION)
            .map(|i| {
                let x = i & 0xF;
                let y = (i >> 8) & 0xF;
                let z = (i >> 4) & 0xF;
                self.get(x, y, z)
            })
            .collect()
    }

    /// 容器中的方块种类数（用于序列化时选择编码方式）
    pub fn palette_size(&self) -> usize {
        match &self.mode {
            ContainerMode::Single(_) => 1,
            ContainerMode::Indirect { palette, .. } => palette.len(),
            ContainerMode::Direct { .. } => 0, // Direct 没有调色板
        }
    }

    /// 序列化为网络数据包的字节格式
    /// 格式: bits_per_entry (u8) → palette (if indirect) → data array (varint-prefixed u64s)
    pub fn encode_network(&self) -> Vec<u8> {
        // Pre-allocate buffer based on mode to avoid reallocations
        let estimated = match &self.mode {
            ContainerMode::Single(_) => 1 + 5 + 5 + 5, // ~16 bytes
            ContainerMode::Indirect { palette, data, .. } => {
                1 + 5 + palette.len() * 5 + 5 + data.len() * 8 // ~few KB
            }
            ContainerMode::Direct { data } => {
                1 + 5 + 5 + data.len() * 8
            }
        };
        let mut buf = Vec::with_capacity(estimated);

        match &self.mode {
            ContainerMode::Single(block) => {
                buf.push(0u8);
                buf.extend_from_slice(&mc_protocol::varint::write_varint(1));
                buf.extend_from_slice(&mc_protocol::varint::write_varint(block.id as i32));
                buf.extend_from_slice(&mc_protocol::varint::write_varint(0));
            }
            ContainerMode::Indirect {
                palette,
                data,
                bits_per_entry,
            } => {
                buf.push(*bits_per_entry);
                buf.extend_from_slice(&mc_protocol::varint::write_varint(palette.len() as i32));
                for block in palette {
                    buf.extend_from_slice(&mc_protocol::varint::write_varint(block.id as i32));
                }
                buf.extend_from_slice(&mc_protocol::varint::write_varint(data.len() as i32));
                for &word in data {
                    buf.extend_from_slice(&word.to_be_bytes());
                }
            }
            ContainerMode::Direct { data } => {
                buf.push(GLOBAL_BITS);
                buf.extend_from_slice(&mc_protocol::varint::write_varint(0));
                buf.extend_from_slice(&mc_protocol::varint::write_varint(data.len() as i32));
                for &word in data {
                    buf.extend_from_slice(&word.to_be_bytes());
                }
            }
        }

        buf
    }

    /// 从网络格式解码创建（encode_network 的逆操作）
    pub fn decode_network(&mut self, data: &[u8]) -> Result<(), String> {
        if data.is_empty() {
            return Err("empty data".into());
        }

        let bits_per_entry = data[0];
        let mut offset = 1;

        // Read palette
        let (palette_len, varint_bytes) = mc_protocol::varint::read_varint(&data[offset..])
            .map_err(|e| format!("read palette len: {}", e))?;
        offset += varint_bytes;

        if palette_len == 0 {
            // Direct mode
            let (data_len, vb) = mc_protocol::varint::read_varint(&data[offset..])
                .map_err(|e| format!("read data len: {}", e))?;
            offset += vb;

            let long_count = data_len as usize;
            let mut packed = Vec::with_capacity(long_count);
            for _i in 0..long_count {
                if offset + 8 > data.len() {
                    return Err("data too short".into());
                }
                let word = u64::from_be_bytes(
                    data[offset..offset+8].try_into()
                        .map_err(|_| "slice conversion failed".to_string())?
                );
                packed.push(word);
                offset += 8;
            }

            self.mode = ContainerMode::Direct { data: packed };
        } else {
            // Indirect mode
            let palette_len = palette_len as usize;
            let mut palette = Vec::with_capacity(palette_len);
            for _ in 0..palette_len {
                let (block_id, vb) = mc_protocol::varint::read_varint(&data[offset..])
                    .map_err(|e| format!("read palette entry: {}", e))?;
                offset += vb;
                palette.push(BlockState::new(block_id as u32));
            }

            let (data_len, vb) = mc_protocol::varint::read_varint(&data[offset..])
                .map_err(|e| format!("read data len: {}", e))?;
            offset += vb;

            let long_count = data_len as usize;
            let mut packed = Vec::with_capacity(long_count);
            for _ in 0..long_count {
                if offset + 8 > data.len() {
                    return Err("data too short".into());
                }
                let word = u64::from_be_bytes(
                    data[offset..offset+8].try_into()
                        .map_err(|_| "slice conversion failed".to_string())?
                );
                packed.push(word);
                offset += 8;
            }

            if palette_len == 1 {
                self.mode = ContainerMode::Single(palette[0]);
            } else {
                self.mode = ContainerMode::Indirect {
                    palette,
                    data: packed,
                    bits_per_entry,
                };
            }
        }

        Ok(())
    }

    /// 二进制编码 (用于 LZ4 存储, 非网络格式)
    pub fn encode_binary(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match &self.mode {
            ContainerMode::Single(block) => {
                buf.push(0u8);
                buf.extend_from_slice(&block.id.to_le_bytes());
            }
            ContainerMode::Indirect { palette, data, bits_per_entry } => {
                buf.push(1u8);
                buf.extend_from_slice(&(palette.len() as u16).to_le_bytes());
                for b in palette { buf.extend_from_slice(&b.id.to_le_bytes()); }
                buf.push(*bits_per_entry);
                buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
                for d in data { buf.extend_from_slice(&d.to_le_bytes()); }
            }
            ContainerMode::Direct { data } => {
                buf.push(2u8);
                buf.push(15u8);
                buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
                for d in data { buf.extend_from_slice(&d.to_le_bytes()); }
            }
        }
        buf
    }

    /// 二进制解码 (从 encode_binary 产生的数据)
    pub fn decode_binary(&mut self, data: &[u8]) -> Result<usize, String> {
        if data.is_empty() { return Err("empty data".into()); }
        let mode_byte = data[0];
        let mut pos = 1usize;
        match mode_byte {
            0 => {
                if pos + 4 > data.len() { return Err("truncated single".into()); }
                let id = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]);
                pos += 4;
                self.mode = crate::paletted::ContainerMode::Single(BlockState::new(id));
                Ok(pos)
            }
            1 => {
                if pos + 2 > data.len() { return Err("truncated palette len".into()); }
                let plen = u16::from_le_bytes([data[pos], data[pos+1]]) as usize;
                pos += 2;
                let mut palette = Vec::with_capacity(plen);
                for _ in 0..plen {
                    if pos + 4 > data.len() { return Err("truncated palette entry".into()); }
                    let id = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]);
                    pos += 4;
                    palette.push(BlockState::new(id));
                }
                if pos >= data.len() { return Err("truncated bits".into()); }
                let bits = data[pos]; pos += 1;
                if pos + 4 > data.len() { return Err("truncated data len".into()); }
                let dlen = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                let mut entries = Vec::with_capacity(dlen);
                for _ in 0..dlen {
                    if pos + 8 > data.len() { return Err("truncated data entry".into()); }
                    entries.push(u64::from_le_bytes([
                        data[pos], data[pos+1], data[pos+2], data[pos+3],
                        data[pos+4], data[pos+5], data[pos+6], data[pos+7],
                    ]));
                    pos += 8;
                }
                self.mode = ContainerMode::Indirect { palette, data: entries, bits_per_entry: bits };
                Ok(pos)
            }
            2 => {
                if pos >= data.len() { return Err("truncated direct bits".into()); }
                let _bits = data[pos]; pos += 1;
                if pos + 4 > data.len() { return Err("truncated direct data len".into()); }
                let dlen = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                let mut entries = Vec::with_capacity(dlen);
                for _ in 0..dlen {
                    if pos + 8 > data.len() { return Err("truncated direct entry".into()); }
                    entries.push(u64::from_le_bytes([
                        data[pos], data[pos+1], data[pos+2], data[pos+3],
                        data[pos+4], data[pos+5], data[pos+6], data[pos+7],
                    ]));
                    pos += 8;
                }
                self.mode = ContainerMode::Direct { data: entries };
                Ok(pos)
            }
            _ => Err(format!("unknown container mode: {}", mode_byte)),
        }
    }
}

impl Default for PalettedContainer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Packed array helpers ──

/// 计算存储 BLOCKS_PER_SECTION 个条目需要的 u64 数量
fn packed_words(count: usize, bits: u8) -> usize {
    if bits == 0 {
        return 0;
    }
    (count * bits as usize).div_ceil(64)
}

/// 创建新的 packed array
fn new_packed(count: usize, bits: u8) -> Vec<u64> {
    vec![0u64; packed_words(count, bits)]
}

/// 从 packed array 读取值
fn read_packed(data: &[u64], index: usize, bits: u8) -> u64 {
    if bits == 0 {
        return 0;
    }
    let bits_u = bits as usize;
    let bits_u64 = bits as u64;
    let bit_index = index * bits_u;
    let word_index = bit_index / 64;
    let bit_offset = bit_index % 64;

    let value = data[word_index] >> bit_offset;
    if bit_offset + bits_u > 64 {
        // Value straddles word boundary
        let next_bits = (bit_offset + bits_u) - 64;
        value | ((data[word_index + 1] & ((1u64 << next_bits) - 1)) << (bits_u64 - next_bits as u64))
    } else {
        value & ((1u64 << bits_u64) - 1)
    }
}

/// 向 packed array 写入值
fn write_packed(data: &mut [u64], index: usize, bits: u8, value: u64) {
    if bits == 0 {
        return;
    }
    let bits_u = bits as usize;
    let bits_u64 = bits as u64;
    let mask = (1u64 << bits_u64) - 1;
    let value = value & mask;
    let bit_index = index * bits_u;
    let word_index = bit_index / 64;
    let bit_offset = bit_index % 64;

    data[word_index] &= !(mask << bit_offset);
    data[word_index] |= value << bit_offset;

    if bit_offset + bits_u > 64 {
        let next_bits = (bit_offset + bits_u) - 64;
        let next_mask = (1u64 << next_bits) - 1;
        data[word_index + 1] &= !next_mask;
        data[word_index + 1] |= value >> (bits_u64 - next_bits as u64);
    }
}

/// YZX 索引 (vanilla Minecraft 顺序)
fn index(x: usize, y: usize, z: usize) -> usize {
    (y << 8) | (z << 4) | x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_is_all_air() {
        let c = PalettedContainer::new();
        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    assert_eq!(c.get(x, y, z), BlockState::AIR);
                }
            }
        }
    }

    #[test]
    fn test_filled() {
        let stone = BlockState::new(1);
        let c = PalettedContainer::filled(stone);
        assert_eq!(c.get(0, 0, 0), stone);
        assert_eq!(c.get(15, 15, 15), stone);
    }

    #[test]
    fn test_set_single_to_indirect() {
        let mut c = PalettedContainer::filled(BlockState::new(1));
        let dirt = BlockState::new(2);
        c.set(0, 0, 0, dirt);
        assert_eq!(c.get(0, 0, 0), dirt);
        // Rest should still be stone
        assert_eq!(c.get(1, 0, 0), BlockState::new(1));
    }

    #[test]
    fn test_set_many_blocks() {
        let mut c = PalettedContainer::new(); // all air
        for x in 0..16 {
            for y in 0..4 {
                for z in 0..16 {
                    c.set(x, y, z, BlockState::new(1)); // stone layer
                }
            }
        }
        assert_eq!(c.get(8, 0, 8), BlockState::new(1));
        assert_eq!(c.get(8, 10, 8), BlockState::AIR);
    }

    #[test]
    fn test_roundtrip_encode_decode() {
        let mut c = PalettedContainer::new();
        c.set(0, 0, 0, BlockState::new(1));
        c.set(8, 8, 8, BlockState::new(2));
        c.set(15, 15, 15, BlockState::new(3));

        let encoded = c.encode_network();
        assert!(!encoded.is_empty());

        // Verify we can still read after encoding
        assert_eq!(c.get(0, 0, 0), BlockState::new(1));
        assert_eq!(c.get(8, 8, 8), BlockState::new(2));
        assert_eq!(c.get(0, 1, 0), BlockState::AIR);
    }
}
