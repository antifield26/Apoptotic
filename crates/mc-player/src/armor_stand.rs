//! 盔甲架系统 — 实体追踪, 装备管理, 姿态控制
//!
//! 盔甲架 entity_type=1 (协议中), 使用元数据控制可见部件

use mc_core::block::BlockState;

/// 盔甲架数据
#[derive(Debug, Clone)]
pub struct ArmorStandData {
    pub entity_id: i32,
    pub pos: (f64, f64, f64),
    pub yaw: f32,
    pub show_arms: bool,
    pub base_plate: bool,
    pub small: bool,
    pub equipment: [Option<BlockState>; 4], // boots, leggings, chestplate, helmet
    pub hand_main: Option<BlockState>,
    pub hand_off: Option<BlockState>,
}

impl ArmorStandData {
    pub fn new(entity_id: i32, x: f64, y: f64, z: f64, yaw: f32) -> Self {
        Self {
            entity_id, pos: (x, y, z), yaw,
            show_arms: false, base_plate: true, small: false,
            equipment: [None, None, None, None],
            hand_main: None, hand_off: None,
        }
    }

    /// 构建 SetEntityMetadata 元数据字节
    pub fn build_metadata(&self) -> Vec<u8> {
        let mut meta = Vec::new();
        // Index 0: status flags (invisible=false, no_gravity=false, etc.)
        meta.push(0); meta.push(0); meta.push(0x00);
        // Index 15: show_arms, small, base_plate bits
        let mut flags: u8 = 0;
        if self.show_arms { flags |= 0x04; }
        if self.small { flags |= 0x01; }
        if self.base_plate { flags |= 0x08; }
        meta.push(15); meta.push(0); meta.push(flags);
        meta.push(0xFF); // terminator
        meta
    }
}
