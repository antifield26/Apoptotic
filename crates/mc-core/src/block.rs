/// 方块状态 — 全局 palette ID + 属性编码
///
/// `id` = base block type (lower ~20 bits, matching protocol block registry)
/// `props` = property bits (facing, waterlogged, half, etc.) — 0 = default state
///
/// Backward-compatible: all existing `block.id == N` patterns work unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BlockState {
    /// Base block type ID (global palette, protocol-level)
    pub id: u32,
    /// Property bits: bit0=waterlogged, bit1-3=facing(0-5), bit4=upper_half, ...
    /// 0 = default state (no special properties)
    pub props: u16,
}

impl BlockState {
    pub const AIR: Self = Self { id: 0, props: 0 };

    pub const fn new(id: u32) -> Self {
        Self { id, props: 0 }
    }

    /// Create with property bits set
    pub const fn with_props(id: u32, props: u16) -> Self {
        Self { id, props }
    }

    pub fn is_air(self) -> bool {
        self.id == 0
    }

    pub fn is_solid(self) -> bool {
        if self.id == 0 { return false; }
        if self.waterlogged() { return false; }
        // Liquids are non-solid
        if self.id == 267 || self.id == 268 { return false; }
        // Common non-solid passable blocks (plants, torches, redstone, etc.)
        !matches!(self.id,
            31 | 32 | 37 | 38 | 39 | 40 | 51 | 55 | 59 | 63 |
            64 | 65 | 66 | 67 | 68 | 69 | 70 | 71 | 75 | 76 |
            77 | 78 | 83 | 93 | 94 | 96 | 97 | 98 | 99 | 100 |
            101 | 102 | 103 | 104 | 131 | 132 | 143 | 144 | 175 | 176
        )
    }

    // ── Property accessors ──

    /// Whether this block is waterlogged (e.g., slab/stairs in water)
    pub fn waterlogged(self) -> bool {
        self.props & 0x0001 != 0
    }

    /// Set the waterlogged flag
    pub fn set_waterlogged(&mut self, wl: bool) {
        if wl { self.props |= 0x0001u16; } else { self.props &= !0x0001u16; }
    }

    /// Facing direction (0-5): 0=down, 1=up, 2=north, 3=south, 4=west, 5=east
    pub fn facing(self) -> u8 {
        ((self.props >> 1) & 0x07) as u8
    }

    /// Set facing direction
    pub fn set_facing(&mut self, dir: u8) {
        self.props = (self.props & !0x000Eu16) | (((dir & 0x07) as u16) << 1);
    }

    /// Whether this is the upper half (stairs, slabs, doors)
    pub fn upper_half(self) -> bool {
        self.props & 0x0010 != 0
    }

    /// Set upper half flag
    pub fn set_upper_half(&mut self, upper: bool) {
        if upper { self.props |= 0x0010u16; } else { self.props &= !0x0010u16; }
    }

    /// Axis orientation: 0=x, 1=y, 2=z (for logs, pillars, etc.)
    pub fn axis(self) -> u8 {
        ((self.props >> 5) & 0x03) as u8
    }

    /// Set axis orientation
    pub fn set_axis(&mut self, ax: u8) {
        self.props = (self.props & !0x0060u16) | (((ax & 0x03) as u16) << 5);
    }

    /// Powered state (for redstone components)
    pub fn powered(self) -> bool {
        self.props & 0x0080 != 0
    }

    /// Set powered state
    pub fn set_powered(&mut self, p: bool) {
        if p { self.props |= 0x0080u16; } else { self.props &= !0x0080u16; }
    }

    /// Crop age (0-7, bits 8-10)
    pub fn age(self) -> u8 {
        ((self.props >> 8) & 0x07) as u8
    }

    /// Set crop age
    pub fn set_age(&mut self, age: u8) {
        self.props = (self.props & !0x0700u16) | (((age & 0x07) as u16) << 8);
    }
}

/// 方块状态注册表 — 管理全局 palette
#[derive(Debug, Default)]
pub struct BlockRegistry {
    states: Vec<BlockState>,
    default_state: BlockState,
}

impl BlockRegistry {
    pub fn new() -> Self {
        let air = BlockState::AIR;
        Self {
            states: vec![air],
            default_state: air,
        }
    }

    pub fn get(&self, id: u32) -> Option<BlockState> {
        self.states.get(id as usize).copied()
    }

    pub fn default_state(&self) -> BlockState {
        self.default_state
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    pub fn register(&mut self) -> BlockState {
        let id = self.states.len() as u32;
        let state = BlockState::new(id);
        self.states.push(state);
        state
    }
}

/// 方块实体 — 存储容器方块（箱子、熔炉等）的附加数据
#[derive(Debug, Clone)]
pub struct BlockEntity {
    pub pos: crate::position::BlockPos,
    pub nbt_data: Vec<u8>,
    pub entity_type: BlockEntityType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockEntityType {
    Chest,
    Furnace,
    Sign,
    Beacon,
    Lectern,
}

impl BlockEntityType {
    pub fn id(self) -> &'static str {
        match self {
            BlockEntityType::Chest => "minecraft:chest",
            BlockEntityType::Furnace => "minecraft:furnace",
            BlockEntityType::Sign => "minecraft:sign",
            BlockEntityType::Beacon => "minecraft:beacon",
            BlockEntityType::Lectern => "minecraft:lectern",
        }
    }
}
