//! BossBar 注册表 — 管理活跃的 Boss 血条

use std::collections::HashMap;
use uuid::Uuid;

/// 单个 BossBar 数据
#[derive(Debug, Clone)]
pub struct BossBarData {
    pub uuid: Uuid,
    pub title: String,
    pub health: f32,      // 0.0 - 1.0
    pub color: i32,       // 0=pink, 1=blue, 2=red, 3=green, 4=yellow, 5=purple, 6=white
    pub division: i32,    // 0=none, 1=6, 2=10, 3=12, 4=20
    pub flags: u8,        // 0x01=darken_sky, 0x02=dragon_bar, 0x04=create_fog
}

/// BossBar 注册表
pub struct BossBarRegistry {
    bars: HashMap<String, BossBarData>,
}

impl Default for BossBarRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BossBarRegistry {
    pub fn new() -> Self {
        Self { bars: HashMap::new() }
    }

    /// 添加新 BossBar (action=0 add)
    pub fn add(&mut self, id: &str, title: &str) -> Option<BossBarData> {
        let data = BossBarData {
            uuid: Uuid::new_v4(),
            title: title.to_string(),
            health: 1.0,
            color: 2, // red
            division: 0, // no divisions
            flags: 0,
        };
        self.bars.insert(id.to_string(), data.clone());
        Some(data)
    }

    /// 移除 BossBar (action=1 remove)
    pub fn remove(&mut self, id: &str) -> Option<BossBarData> {
        self.bars.remove(id)
    }

    /// 获取 BossBar
    pub fn get(&self, id: &str) -> Option<&BossBarData> {
        self.bars.get(id)
    }

    /// 更新生命值 (action=2)
    pub fn update_health(&mut self, id: &str, health: f32) -> Option<BossBarData> {
        if let Some(bar) = self.bars.get_mut(id) {
            bar.health = health.clamp(0.0, 1.0);
            Some(bar.clone())
        } else { None }
    }

    /// 设置颜色 (action=4)
    pub fn set_color(&mut self, id: &str, color_name: &str) -> Option<BossBarData> {
        let color_id = match color_name {
            "pink" => 0, "blue" => 1, "red" => 2, "green" => 3,
            "yellow" => 4, "purple" => 5, "white" => 6,
            _ => 2, // default red
        };
        if let Some(bar) = self.bars.get_mut(id) {
            bar.color = color_id;
            Some(bar.clone())
        } else { None }
    }

    /// 更新标题 (action=3)
    pub fn update_title(&mut self, id: &str, title: &str) -> Option<BossBarData> {
        if let Some(bar) = self.bars.get_mut(id) {
            bar.title = title.to_string();
            Some(bar.clone())
        } else { None }
    }

    /// 列出所有活跃的 BossBar
    pub fn list(&self) -> Vec<(String, BossBarData)> {
        self.bars.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}
