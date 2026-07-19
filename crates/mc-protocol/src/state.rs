/// 连接状态 — 定义客户端与服务端交互的阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// 初始握手阶段
    Handshake,
    /// 服务器列表 ping 响应
    Status,
    /// 登录认证阶段
    Login,
    /// 配置阶段 (1.20.2+ / protocol 764+ 必需)
    Config,
    /// 正常游戏阶段
    Play,
}

impl ConnectionState {
    pub fn from_next_state(next: i32) -> Option<Self> {
        match next {
            1 => Some(Self::Status),
            2 => Some(Self::Login),
            _ => None,
        }
    }
}
