//! 交互式控制台 — stdin 命令输入
//!
//! 在 tokio 任务中读取 stdin 行并分发到命令系统。

use mc_command::dispatcher::{CommandDispatcher, CommandSource};
use mc_core::world_state::SharedWorldState;
use mc_player::player::SharedPlayerManager;
use std::sync::Arc;
use tokio::sync::broadcast;

/// 控制台输入处理器
pub struct ConsoleInput {
    dispatcher: Arc<parking_lot::Mutex<CommandDispatcher>>,
    player_manager: SharedPlayerManager,
    shutdown_tx: broadcast::Sender<()>,
    world_state: SharedWorldState,
}

impl ConsoleInput {
    pub fn new(
        dispatcher: Arc<parking_lot::Mutex<CommandDispatcher>>,
        player_manager: SharedPlayerManager,
        shutdown_tx: broadcast::Sender<()>,
        world_state: SharedWorldState,
    ) -> Self {
        Self {
            dispatcher,
            player_manager,
            shutdown_tx,
            world_state,
        }
    }

    /// 启动控制台读取循环
    pub async fn run(self) {
        use tokio::io::AsyncBufReadExt;
        let stdin = tokio::io::BufReader::new(tokio::io::stdin());
        let mut lines = stdin.lines();

        tracing::info!("Console input ready — type 'help' for available commands");

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    // Special: "help" lists commands
                    if trimmed == "help" || trimmed == "?" {
                        let disp = self.dispatcher.lock();
                        let cmds = disp.list_commands();
                        let mut sorted = cmds.clone();
                        sorted.sort();
                        println!("Available commands: {}", sorted.join(", "));
                        println!("Use /<command> or just type the command name");
                        continue;
                    }

                    // Dispatch via command system
                    let result = {
                        let disp = self.dispatcher.lock();
                        disp.dispatch_input(
                            trimmed,
                            CommandSource::Console,
                            &self.player_manager,
                            &self.shutdown_tx,
                            &self.world_state,
                            "Console",
                            20,
                            None, None,
                        )
                    };

                    match result {
                        Ok(msg) => {
                            if !msg.is_empty() {
                                println!("{}", msg);
                            }
                        }
                        Err(e) => {
                            println!("Error: {}", e);
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::error!("stdin error: {}", e);
                    break;
                }
            }
        }
    }
}
