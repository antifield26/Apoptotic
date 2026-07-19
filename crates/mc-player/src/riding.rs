//! 骑乘系统 — 管理玩家与坐骑实体之间的骑乘关系
//!
//! 支持: 马(28)/猪(12)/船(23)/矿车(24) 骑乘

use std::collections::HashMap;
use uuid::Uuid;

/// 骑乘管理器
pub struct RideManager {
    pub riders: HashMap<Uuid, i32>, // player_uuid → mount_entity_id
}

impl Default for RideManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RideManager {
    pub fn new() -> Self {
        Self { riders: HashMap::new() }
    }

    /// 玩家骑乘实体
    pub fn mount(&mut self, player_uuid: Uuid, entity_id: i32) -> bool {
        if self.riders.contains_key(&player_uuid) { return false; }
        self.riders.insert(player_uuid, entity_id);
        true
    }

    /// 玩家下坐骑
    pub fn dismount(&mut self, player_uuid: &Uuid) -> Option<i32> {
        self.riders.remove(player_uuid)
    }

    /// 获取玩家骑乘的实体
    pub fn get_mount(&self, player_uuid: &Uuid) -> Option<i32> {
        self.riders.get(player_uuid).copied()
    }

    /// 检查实体是否被骑乘
    pub fn is_mounted(&self, entity_id: i32) -> bool {
        self.riders.values().any(|&e| e == entity_id)
    }

    /// 每 tick 同步骑乘者位置到坐骑
    pub fn tick(&self, mob_manager: &crate::mob::MobManager, player_manager: &crate::player::PlayerManager) {
        for (player_uuid, mount_eid) in &self.riders {
            if let Some(mob) = mob_manager.get(*mount_eid) {
                let _ = player_manager.update_position(
                    player_uuid,
                    mob.position.x,
                    mob.position.y + 1.5,
                    mob.position.z,
                );
            }
        }
    }
}
