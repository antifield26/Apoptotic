//! /msg /tell /w — 私聊, /me — 动作

use crate::dispatcher::{resolve_player_target, Command, CommandContext, CommandResult};

pub struct MsgCommand;

impl Command for MsgCommand {
    fn name(&self) -> &str { "msg" }
    fn description(&self) -> &str { "Send a private message to a player" }
    fn aliases(&self) -> &[&str] { &["tell", "w"] }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let target_name = ctx.args.first().ok_or("Usage: /msg <player> <message>")?;
        let message = ctx.args.get(1..).map(|parts| parts.join(" ")).ok_or("Usage: /msg <player> <message>")?;

        if message.is_empty() {
            return Err("Usage: /msg <player> <message>".into());
        }

        match resolve_player_target(target_name, ctx) {
            Some((uuid, target_name)) => {
                let sender = ctx.source.name();
                ctx.player_manager.send_private_msg(&sender, uuid, &message);
                Ok(format!("[You → {}] {}", target_name, message))
            }
            None => Err(format!("Player not found: {}", target_name)),
        }
    }
}

/// /me — 角色扮演动作广播
pub struct MeCommand;

impl Command for MeCommand {
    fn name(&self) -> &str { "me" }
    fn description(&self) -> &str { "Broadcast an action to all players" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let action = ctx.args.join(" ");
        if action.is_empty() {
            return Err("Usage: /me <action>".into());
        }
        let username = ctx.source.name();
        let message = format!("* {} {}", username, action);
        tracing::info!("{}", message);
        ctx.player_manager.broadcast_chat("Server", &message, true);
        Ok(message)
    }
}
