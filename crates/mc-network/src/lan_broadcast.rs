//! LAN 服务发现广播
//!
//! 通过 UDP 多播到 224.0.2.60:4445，使第三方启动器的"扫描局域网"
//! 功能能够自动发现服务器。
//!
//! 协议格式（纯文本）：
//! ```text
//! [MOTD]A Minecraft Server[/MOTD][AD]25565[/AD]
//! ```

use tokio::net::UdpSocket;
use tracing::{debug, error, info};

/// LAN 广播器
pub struct LanBroadcaster {
    socket: UdpSocket,
    message: String,
    interval_ms: u64,
    /// 多播目标地址 (e.g. 224.0.2.60:4445)
    target_addr: String,
}

/// 构建 LAN 广播消息 (公开以供测试)
pub fn build_lan_message(motd: &str, port: u16) -> String {
    format!("[MOTD]{}[/MOTD][AD]{}[/AD]", motd, port)
}

impl LanBroadcaster {
    /// 创建并绑定 LAN 广播 socket
    pub async fn new(motd: &str, port: u16, multicast_group: &str, interval_ms: u64) -> std::io::Result<Self> {
        let message = build_lan_message(motd, port);
        let target_addr = format!("{}:{}", multicast_group, 4445);

        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.set_broadcast(true)?;
        // Set TTL to 1 for LAN-local multicast (don't route beyond local network)
        socket.set_ttl(1).ok();

        info!("LAN broadcast enabled: '{}' → {}", motd, target_addr);

        Ok(Self {
            socket,
            message,
            interval_ms,
            target_addr,
        })
    }

    /// 发送一次广播
    pub async fn broadcast_once(&self) -> std::io::Result<()> {
        self.socket.send_to(self.message.as_bytes(), &self.target_addr).await?;
        Ok(())
    }

    /// 启动广播循环 — 按配置的间隔重复广播
    pub async fn run(self) {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_millis(self.interval_ms)
        );

        loop {
            interval.tick().await;
            if let Err(e) = self.broadcast_once().await {
                error!("LAN broadcast error: {}", e);
            } else {
                debug!("LAN broadcast sent: {}", self.message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lan_message_format() {
        let msg = build_lan_message("My Server", 25565);
        assert!(msg.starts_with("[MOTD]"));
        assert!(msg.contains("[/MOTD]"));
        assert!(msg.contains("[AD]25565[/AD]"));
        assert_eq!(msg, "[MOTD]My Server[/MOTD][AD]25565[/AD]");
    }

    #[test]
    fn test_lan_message_empty_motd() {
        let msg = build_lan_message("", 12345);
        assert_eq!(msg, "[MOTD][/MOTD][AD]12345[/AD]");
    }

    #[test]
    fn test_lan_message_special_port() {
        let msg = build_lan_message("Test", 65535);
        assert!(msg.ends_with("[AD]65535[/AD]"));
    }
}
