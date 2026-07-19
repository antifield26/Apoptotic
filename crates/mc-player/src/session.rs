//! 登录会话管理

use uuid::Uuid;

/// 玩家登录会话
#[derive(Debug, Clone)]
pub struct LoginSession {
    pub username: String,
    pub uuid: Uuid,
    /// 是否在线模式验证
    pub online_mode: bool,
}

impl LoginSession {
    pub fn offline(username: String) -> Self {
        let uuid = mc_core::auth::offline_uuid(&username);
        Self {
            username,
            uuid,
            online_mode: false,
        }
    }
}
