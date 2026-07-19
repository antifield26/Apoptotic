//! 信息命令: list, seed, say, status

use crate::dispatcher::{Command, CommandContext, CommandResult};

pub struct ListCommand;
impl Command for ListCommand {
    fn name(&self) -> &str { "list" }
    fn description(&self) -> &str { "List online players" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let players = ctx.player_manager.all_players();
        let names: Vec<String> = players.iter().map(|p| p.username.clone()).collect();
        Ok(format!(
            "Players ({}/{}): {}",
            players.len(),
            ctx.max_players,
            if names.is_empty() { "(none)".into() } else { names.join(", ") }
        ))
    }
}

pub struct SeedCommand;
impl Command for SeedCommand {
    fn name(&self) -> &str { "seed" }
    fn description(&self) -> &str { "Show the world seed" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let ws = ctx.world_state.read();
        Ok(format!("Seed: {}", ws.seed))
    }
}

pub struct SayCommand;
impl Command for SayCommand {
    fn name(&self) -> &str { "say" }
    fn description(&self) -> &str { "Broadcast a message to all players" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let message = ctx.args.join(" ");
        if message.is_empty() {
            return Err("Usage: /say <message>".into());
        }
        let output = format!("[{}] {}", ctx.source.name(), message);
        tracing::info!("{}", output);
        ctx.player_manager.broadcast_chat("Server", &message, true);
        Ok(output)
    }
}

pub struct StatusCommand;
impl Command for StatusCommand {
    fn name(&self) -> &str { "status" }
    fn description(&self) -> &str { "Show server status" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let online = ctx.player_manager.online_count();
        let ws = ctx.world_state.read();
        Ok(format!(
            "Server: Minecraft LAN | Players: {}/{} | Time: {} | MOTD: {}",
            online, ctx.max_players, ws.time, ctx.motd
        ))
    }
}
