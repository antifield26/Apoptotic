//! 装饰管理器 — 物品展示框和画的服务器端追踪
//!
//! 追踪放置位置、朝向、内容

use mc_core::block::BlockState;
use std::collections::HashMap;

/// 物品展示框数据
#[derive(Debug, Clone)]
pub struct FrameData {
    pub entity_id: i32,
    pub pos: (i32, i32, i32),
    pub facing: u8, // 0=down, 1=up, 2=north, 3=south, 4=west, 5=east
    pub item: Option<BlockState>,
    pub item_rotation: u8, // 0-7
}

/// 画数据
#[derive(Debug, Clone)]
pub struct PaintingData {
    pub entity_id: i32,
    pub pos: (i32, i32, i32),
    pub facing: u8,
    pub motive: String, // "alban", "aztec", "bomb", "kebab", "plant", "wasteland" etc.
}

/// 装饰管理器
pub struct DecorationManager {
    pub frames: HashMap<(i32, i32, i32), FrameData>,
    pub paintings: HashMap<(i32, i32, i32), PaintingData>,
}

impl Default for DecorationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DecorationManager {
    pub fn new() -> Self {
        Self { frames: HashMap::new(), paintings: HashMap::new() }
    }

    /// 注册物品展示框
    pub fn add_frame(&mut self, frame: FrameData) {
        self.frames.insert(frame.pos, frame);
    }

    /// 移除物品展示框 (返回掉落物品)
    pub fn remove_frame(&mut self, pos: (i32, i32, i32)) -> Option<FrameData> {
        self.frames.remove(&pos)
    }

    /// 获取展示框
    pub fn get_frame(&self, pos: (i32, i32, i32)) -> Option<&FrameData> {
        self.frames.get(&pos)
    }

    /// 获取所有展示框
    pub fn all_frames(&self) -> Vec<&FrameData> {
        self.frames.values().collect()
    }

    /// 注册画
    pub fn add_painting(&mut self, painting: PaintingData) {
        self.paintings.insert(painting.pos, painting);
    }

    /// 移除画
    pub fn remove_painting(&mut self, pos: (i32, i32, i32)) -> Option<PaintingData> {
        self.paintings.remove(&pos)
    }

    /// 随机选取画作主题
    pub fn random_motive() -> &'static str {
        const MOTIVES: &[&str] = &[
            "alban", "aztec", "aztec2", "bomb", "kebab", "plant",
            "wasteland", "wanderer", "pool", "courbet", "sea", "sunset",
            "creebet", "graham", "match", "bust", "stage", "void",
            "skull_and_roses", "wither", "fighters", "donkey_kong",
        ];
        MOTIVES[fastrand::usize(0..MOTIVES.len())]
    }
}
