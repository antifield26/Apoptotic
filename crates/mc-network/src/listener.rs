//! TCP 监听器 — 接受客户端连接

use tokio::net::{TcpListener, TcpStream};

/// Minecraft 服务器 TCP 监听器
pub struct ServerListener {
    listener: TcpListener,
}

impl ServerListener {
    /// 绑定到指定地址并开始监听
    pub async fn bind(host: &str, port: u16) -> std::io::Result<Self> {
        let addr = format!("{}:{}", host, port);
        let listener = TcpListener::bind(&addr).await?;
        tracing::info!("Listening on {}", addr);
        Ok(Self { listener })
    }

    /// 接受一个连接
    pub async fn accept(&self) -> std::io::Result<(TcpStream, std::net::SocketAddr)> {
        self.listener.accept().await
    }
}
