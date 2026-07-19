//! 管理命令: op, deop, stop, kick, ban, pardon, banlist, whitelist

use crate::dispatcher::{Command, CommandContext, CommandResult};

pub struct OpCommand;
impl Command for OpCommand {
    fn name(&self) -> &str { "op" }
    fn description(&self) -> &str { "Grant operator status" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let target = ctx.args.first().ok_or("Usage: /op <player>")?;
        if let Some(p) = ctx.player_manager.get_by_name(target) {
            ctx.player_manager.set_op(&p.uuid, true)?;
            // Broadcast feedback
            ctx.player_manager.broadcast_chat("Server", &format!("{} is now an operator", p.username), true);
            Ok(format!("Opped {}", target))
        } else {
            Err(format!("Player not found: {}", target))
        }
    }
}

pub struct DeopCommand;
impl Command for DeopCommand {
    fn name(&self) -> &str { "deop" }
    fn description(&self) -> &str { "Revoke operator status" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let target = ctx.args.first().ok_or("Usage: /deop <player>")?;
        if let Some(p) = ctx.player_manager.get_by_name(target) {
            ctx.player_manager.set_op(&p.uuid, false)?;
            ctx.player_manager.broadcast_chat("Server", &format!("{} is no longer an operator", p.username), true);
            Ok(format!("De-opped {}", target))
        } else {
            Err(format!("Player not found: {}", target))
        }
    }
}

pub struct StopCommand;
impl Command for StopCommand {
    fn name(&self) -> &str { "stop" }
    fn description(&self) -> &str { "Stop the server" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        tracing::info!("Server stop requested by {}", ctx.source.name());
        ctx.player_manager.broadcast_chat("Server", "Server is shutting down...", true);
        let _ = ctx.shutdown_tx.send(());
        Ok("Stopping server...".into())
    }
}

pub struct KickCommand;
impl Command for KickCommand {
    fn name(&self) -> &str { "kick" }
    fn description(&self) -> &str { "Kick a player from the server" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let target = ctx.args.first().ok_or("Usage: /kick <player> [reason]")?;
        let reason = ctx.args.get(1).map(|s| s.as_str()).unwrap_or("Kicked");
        if let Some(p) = ctx.player_manager.get_by_name(target) {
            // Send kick notification — target's connection will send Disconnect and exit
            ctx.player_manager.kick_player(p.uuid, reason);
            ctx.player_manager.broadcast_chat("Server", &format!("{} was kicked: {}", p.username, reason), true);
            Ok(format!("Kicked {}: {}", target, reason))
        } else {
            Err(format!("Player not found: {}", target))
        }
    }
}

pub struct BanCommand;
impl Command for BanCommand {
    fn name(&self) -> &str { "ban" }
    fn description(&self) -> &str { "Ban a player from the server" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let target = ctx.args.first().ok_or("Usage: /ban <player> [reason]")?;
        let reason = ctx.args.get(1).map(|s| s.as_str()).unwrap_or("Banned");
        // Find player (online or by name → offline_uuid)
        let uuid = if let Some(p) = ctx.player_manager.get_by_name(target) {
            let uuid = p.uuid;
            // Kick first (sends Disconnect packet), then ban
            ctx.player_manager.kick_player(uuid, &format!("Banned: {}", reason));
            ctx.player_manager.ban(uuid);
            uuid
        } else {
            // Player not online — ban by offline UUID
            let uuid = mc_core::auth::offline_uuid(target);
            ctx.player_manager.ban(uuid);
            uuid
        };
        ctx.player_manager.broadcast_chat("Server", &format!("{} was banned: {}", target, reason), true);
        Ok(format!("Banned {} ({}): {}", target, uuid, reason))
    }
}

pub struct PardonCommand;
impl Command for PardonCommand {
    fn name(&self) -> &str { "pardon" }
    fn description(&self) -> &str { "Unban a player" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let target = ctx.args.first().ok_or("Usage: /pardon <player>")?;
        let uuid = mc_core::auth::offline_uuid(target);
        if ctx.player_manager.is_banned(&uuid) {
            ctx.player_manager.unban(&uuid);
            ctx.player_manager.broadcast_chat("Server", &format!("{} was pardoned", target), true);
            Ok(format!("Pardoned {} ({}), they may rejoin now", target, uuid))
        } else {
            Err(format!("{} is not banned", target))
        }
    }
}

pub struct BanlistCommand;
impl Command for BanlistCommand {
    fn name(&self) -> &str { "banlist" }
    fn description(&self) -> &str { "List all banned players" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let banned = ctx.player_manager.get_banned();
        if banned.is_empty() {
            Ok("No banned players".into())
        } else {
            let entries: Vec<String> = banned.iter()
                .map(|u| format!("  - {}", u))
                .collect();
            Ok(format!("Banned ({}):\n{}", banned.len(), entries.join("\n")))
        }
    }
}

pub struct WhitelistCommand;
impl Command for WhitelistCommand {
    fn name(&self) -> &str { "whitelist" }
    fn description(&self) -> &str { "Manage the server whitelist" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let action = ctx.args.first().map(|s| s.as_str()).unwrap_or("list");
        match action {
            "add" => {
                let player = ctx.args.get(1).ok_or("Usage: /whitelist add <player>")?;
                let uuid = mc_core::auth::offline_uuid(player);
                ctx.player_manager.add_whitelist(uuid);
                ctx.player_manager.broadcast_chat("Server", &format!("{} added to whitelist", player), true);
                Ok(format!("Added {} to whitelist", player))
            }
            "remove" => {
                let player = ctx.args.get(1).ok_or("Usage: /whitelist remove <player>")?;
                let uuid = mc_core::auth::offline_uuid(player);
                ctx.player_manager.remove_whitelist(&uuid);
                Ok(format!("Removed {} from whitelist", player))
            }
            "list" => Ok("Whitelist: active (in-memory, not yet persisted)".into()),
            "on" => {
                ctx.player_manager.set_whitelist_enabled(true);
                ctx.player_manager.broadcast_chat("Server", "Whitelist enabled", true);
                Ok("Whitelist enabled".into())
            }
            "off" => {
                ctx.player_manager.set_whitelist_enabled(false);
                Ok("Whitelist disabled".into())
            }
            _ => Err("Usage: /whitelist <add|remove|list|on|off>".into()),
        }
    }
}

/// /transfer — transfer a player to another server (hub-and-spoke)
pub struct TransferCommand;
impl Command for TransferCommand {
    fn name(&self) -> &str { "transfer" }
    fn description(&self) -> &str { "Transfer a player to another server" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let target = ctx.args.first().ok_or("Usage: /transfer <player> <host> [port]")?;
        let host = ctx.args.get(1).ok_or("Usage: /transfer <player> <host> [port]")?;
        let port: i32 = ctx.args.get(2).and_then(|s| s.parse().ok()).unwrap_or(25565);

        let targets = crate::dispatcher::resolve_player_targets(target, ctx);
        if targets.is_empty() { return Err(format!("No player matched: {}", target)); }

        // Send Transfer packet to each matched player via PlayerStateEvent
        let mut count = 0;
        for (uuid, _name) in &targets {
            if ctx.player_manager.send_transfer(uuid, host, port).is_ok() {
                count += 1;
            }
        }
        Ok(format!("Transferring {} player(s) to {}:{}", count, host, port))
    }
}

/// /reload — reload server configuration at runtime
pub struct ReloadCommand;
impl Command for ReloadCommand {
    fn name(&self) -> &str { "reload" }
    fn description(&self) -> &str { "Reload server configuration (difficulty, motd) from disk" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let mut changed = Vec::new();

        // Reload difficulty
        if let Some(diff_arg) = ctx.args.first() {
            let new_diff = match diff_arg.to_lowercase().as_str() {
                "peaceful" | "0" => mc_core::world_state::Difficulty::Peaceful,
                "easy" | "1" => mc_core::world_state::Difficulty::Easy,
                "normal" | "2" => mc_core::world_state::Difficulty::Normal,
                "hard" | "3" => mc_core::world_state::Difficulty::Hard,
                _ => return Err("Usage: /reload [difficulty] — difficulty: peaceful|easy|normal|hard".into()),
            };
            let mut ws = ctx.world_state.write();
            ws.set_difficulty(new_diff);
            changed.push(format!("difficulty → {:?}", new_diff));
        } else {
            return Err("Usage: /reload <difficulty> — e.g. /reload normal".into());
        }

        Ok(format!("Config reloaded: {}", changed.join(", ")))
    }
}

/// /save-all — trigger immediate world save
pub struct SaveAllCommand;
impl Command for SaveAllCommand {
    fn name(&self) -> &str { "save-all" }
    fn description(&self) -> &str { "Save all world and player data immediately" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if let Some(tx) = ctx.save_trigger {
            let _ = tx.send(());
            Ok("Save triggered — all chunks and player data will be saved".into())
        } else {
            Err("Save trigger not available (console-only command)".into())
        }
    }
}
