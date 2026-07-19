//! RCON 协议实现 (Minecraft Remote Console)
//!
//! 协议: TCP, 数据包格式: [length: i32 LE] [request_id: i32 LE]
//! [type: i32 LE] [payload: null-terminated ASCII] [padding: 0x00]
//!
//! Types: 3=Login, 2=Command, 0=Response

use mc_command::dispatcher::{CommandDispatcher, CommandSource};
use mc_core::world_state::SharedWorldState;
use mc_player::player::SharedPlayerManager;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

const RCON_TYPE_RESPONSE: i32 = 0;
const RCON_TYPE_COMMAND: i32 = 2;
const RCON_TYPE_LOGIN: i32 = 3;
const RCON_AUTH_FAILED: i32 = -1;

pub struct RconServer {
    host: String,
    port: u16,
    password: String,
    dispatcher: Arc<parking_lot::Mutex<CommandDispatcher>>,
    player_manager: SharedPlayerManager,
    shutdown_tx: broadcast::Sender<()>,
    world_state: SharedWorldState,
}

impl RconServer {
    pub fn new(
        host: &str,
        port: u16,
        password: &str,
        dispatcher: Arc<parking_lot::Mutex<CommandDispatcher>>,
        player_manager: SharedPlayerManager,
        shutdown_tx: broadcast::Sender<()>,
        world_state: SharedWorldState,
    ) -> Self {
        Self {
            host: host.to_string(),
            port,
            password: password.to_string(),
            dispatcher,
            player_manager,
            shutdown_tx,
            world_state,
        }
    }

    /// 启动 RCON TCP 监听器
    pub async fn run(self) {
        if self.password.is_empty() {
            warn!("RCON disabled — no password set");
            return;
        }

        let addr = format!("{}:{}", self.host, self.port);
        let listener = match TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                error!("Failed to bind RCON on {}: {}", addr, e);
                return;
            }
        };
        info!("RCON listening on {}", addr);

        let mut shutdown_rx = self.shutdown_tx.subscribe();
        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            debug!("RCON connection from {}", addr);
                            let srv = RconConnection {
                                stream,
                                password: self.password.clone(),
                                dispatcher: self.dispatcher.clone(),
                                player_manager: self.player_manager.clone(),
                                shutdown_tx: self.shutdown_tx.clone(),
                                world_state: self.world_state.clone(),
                            };
                            tokio::spawn(async move { srv.handle().await });
                        }
                        Err(e) => error!("RCON accept error: {}", e),
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("RCON shutting down");
                    return;
                }
            }
        }
    }
}

struct RconConnection {
    stream: TcpStream,
    password: String,
    dispatcher: Arc<parking_lot::Mutex<CommandDispatcher>>,
    player_manager: SharedPlayerManager,
    shutdown_tx: broadcast::Sender<()>,
    world_state: SharedWorldState,
}

impl RconConnection {
    async fn handle(mut self) {
        // Phase 1: Authentication
        if !self.authenticate().await {
            return;
        }

        // Phase 2: Command loop
        loop {
            match self.read_packet().await {
                Ok((request_id, ptype, payload)) => {
                    if ptype == RCON_TYPE_COMMAND {
                        let cmd = String::from_utf8_lossy(&payload).trim().to_string();
                        if cmd.is_empty() {
                            let _ = self.send_packet(request_id, RCON_TYPE_RESPONSE, "").await;
                            continue;
                        }
                        debug!("RCON command: {}", cmd);
                        let result = {
                            let disp = self.dispatcher.lock();
                            disp.dispatch_input(
                                &cmd,
                                CommandSource::Rcon,
                                &self.player_manager,
                                &self.shutdown_tx,
                                &self.world_state,
                                "RCON",
                                20,
                                None, None,
                            )
                        };
                        let response = match result {
                            Ok(msg) => msg,
                            Err(e) => e,
                        };
                        let _ = self.send_packet(request_id, RCON_TYPE_RESPONSE, &response).await;
                    }
                }
                Err(e) => {
                    debug!("RCON read error: {}", e);
                    return;
                }
            }
        }
    }

    async fn authenticate(&mut self) -> bool {
        match self.read_packet().await {
            Ok((request_id, RCON_TYPE_LOGIN, payload)) => {
                let pass = String::from_utf8_lossy(&payload).trim().to_string();
                let valid = if let Some(hash) = self.password.strip_prefix("$sha1$") {
                    // SHA-1 hashed password — compare hex digests
                    use sha1::Digest;
                    let digest = sha1::Sha1::digest(pass.as_bytes());
                    hex::encode(&digest[..]) == hash
                } else {
                    pass == self.password
                };
                if valid {
                    debug!("RCON auth successful");
                    let _ = self.send_packet(request_id, RCON_TYPE_COMMAND, "").await;
                    true
                } else {
                    warn!("RCON auth failed — wrong password");
                    let _ = self.send_packet(RCON_AUTH_FAILED, RCON_TYPE_COMMAND, "").await;
                    false
                }
            }
            Ok((_, ptype, _)) => {
                debug!("RCON expected login (3), got type {}", ptype);
                let _ = self.send_packet(RCON_AUTH_FAILED, RCON_TYPE_COMMAND, "").await;
                false
            }
            Err(e) => {
                debug!("RCON auth read error: {}", e);
                false
            }
        }
    }

    async fn read_packet(&mut self) -> Result<(i32, i32, Vec<u8>), String> {
        // Read length (i32 LE)
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).await
            .map_err(|e| format!("read length: {}", e))?;
        let length = i32::from_le_bytes(len_buf);
        if !(10..=4110).contains(&length) {
            return Err(format!("invalid RCON length: {}", length));
        }

        let payload_len = (length - 10) as usize; // minus request_id(4) + type(4) + padding(2)
        let mut data = vec![0u8; length as usize];
        data[..4].copy_from_slice(&len_buf);
        self.stream.read_exact(&mut data[4..]).await
            .map_err(|e| format!("read data: {}", e))?;

        let request_id = i32::from_le_bytes(data[4..8].try_into().expect("guaranteed 4 bytes after read_exact"));
        let ptype = i32::from_le_bytes(data[8..12].try_into().expect("guaranteed 4 bytes after read_exact"));
        let payload = data[12..12 + payload_len].to_vec();

        Ok((request_id, ptype, payload))
    }

    async fn send_packet(&mut self, request_id: i32, ptype: i32, payload: &str) -> Result<(), String> {
        let payload_bytes = payload.as_bytes();
        let total_len = 10 + payload_bytes.len() as i32; // request_id + type + payload + padding

        let mut packet = Vec::with_capacity(4 + total_len as usize);
        packet.extend_from_slice(&total_len.to_le_bytes()); // length
        packet.extend_from_slice(&request_id.to_le_bytes()); // request_id
        packet.extend_from_slice(&ptype.to_le_bytes());      // type
        packet.extend_from_slice(payload_bytes);             // payload
        packet.push(0x00);                                   // null terminator
        packet.push(0x00);                                   // padding

        self.stream.write_all(&packet).await
            .map_err(|e| format!("write response: {}", e))
    }
}
