use serde::{Deserialize, Serialize};

/// 方块坐标 (整数)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub const ZERO: Self = Self { x: 0, y: 0, z: 0 };

    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// 转换为区块坐标
    pub fn to_chunk_pos(self) -> ChunkPos {
        ChunkPos {
            x: self.x.div_euclid(16),
            z: self.z.div_euclid(16),
        }
    }

    /// 区块内相对坐标 (0..16)
    pub fn chunk_relative(self) -> (usize, usize) {
        (
            self.x.rem_euclid(16) as usize,
            self.z.rem_euclid(16) as usize,
        )
    }

    /// 所在 section 的 Y 索引
    pub fn section_y(self) -> i32 {
        self.y.div_euclid(16)
    }

    /// Section 内相对 Y
    pub fn section_relative_y(self) -> usize {
        self.y.rem_euclid(16) as usize
    }
}

/// 区块坐标
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    pub fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

/// Section 坐标 (区块 + Y 层级)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionPos {
    pub chunk: ChunkPos,
    pub y: i32, // section index (-4 to 19 for 1.18+)
}

impl SectionPos {
    pub fn new(chunk: ChunkPos, y: i32) -> Self {
        Self { chunk, y }
    }
}

/// 实体浮点坐标
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
}

impl Position {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self {
            x,
            y,
            z,
            yaw: 0.0,
            pitch: 0.0,
        }
    }

    pub fn block_pos(self) -> BlockPos {
        BlockPos {
            x: self.x.floor() as i32,
            y: self.y.floor() as i32,
            z: self.z.floor() as i32,
        }
    }
}

/// 区块列中包含的 section 数量 (1.18+: -4 到 19 = 24 sections)
pub const SECTIONS_PER_CHUNK: usize = 24;
/// 最低 section Y 索引
pub const MIN_SECTION_Y: i32 = -4;
/// 世界高度上限
pub const WORLD_MAX_Y: i32 = 320;
/// 世界高度下限
pub const WORLD_MIN_Y: i32 = -64;
/// Section 边长
pub const SECTION_SIZE: usize = 16;
/// 区块最大方块实体数
pub const MAX_BLOCK_ENTITIES_PER_CHUNK: usize = 256;
