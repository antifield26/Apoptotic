//! /execute 命令 — 以其他实体身份/位置执行命令
//!
//! 支持子命令:
//!   execute as <target> run <command>     — 以目标身份运行命令
//!   execute at <target> run <command>     — 在目标位置运行命令
//!   execute positioned <x> <y> <z> run... — 在指定坐标运行命令
//!
//! 示例:
//!   /execute as @a run say Hello
//!   /execute at @p run setblock ~ ~-1 ~ stone
//!   /execute as @a at @s run tp @s ~ ~10 ~

use crate::dispatcher::{resolve_player_targets, CommandContext, CommandResult, CommandSource};

pub struct ExecuteCommand;

impl crate::dispatcher::Command for ExecuteCommand {
    fn name(&self) -> &str { "execute" }
    fn description(&self) -> &str { "Execute a command on behalf of/at other entities" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if ctx.args.len() < 4 {
            return Err("Usage: /execute as|at <target> run <command>".into());
        }

        let modifier = ctx.args[0].to_lowercase();

        match modifier.as_str() {
            "as" => execute_as(ctx),
            "at" => execute_at(ctx),
            "positioned" => execute_positioned(ctx),
            _ => Err(format!(
                "Unknown execute modifier: {}. Supported: as, at, positioned",
                modifier
            )),
        }
    }
}

/// Execute `as <target> run <command>` — run as the target entity
fn execute_as(ctx: &CommandContext) -> CommandResult {
    let selector = &ctx.args[1];
    let run_idx = ctx.args.iter().position(|a| a.to_lowercase() == "run")
        .ok_or("Expected 'run' after target. Usage: execute as <target> run <command>")?;
    if run_idx < 2 { return Err("Expected 'run' after target selector".into()); }

    let targets = resolve_player_targets(selector, ctx);
    if targets.is_empty() {
        return Err(format!("No targets matched selector: {}", selector));
    }

    let subcommand_args: Vec<String> = ctx.args[run_idx + 1..].iter().map(|s| s.to_string()).collect();
    let dispatcher = ctx.dispatcher.ok_or("Internal error: dispatcher not available")?;

    let mut results: Vec<String> = Vec::new();
    for (uuid, username) in &targets {
        let source = CommandSource::Player { uuid: *uuid, username: username.clone() };
        let sub_ctx = CommandContext {
            source,
            args: subcommand_args.clone(),
            ..*ctx
        };

        match dispatcher.dispatch(&sub_ctx) {
            Ok(msg) => { if !msg.is_empty() { results.push(format!("[{}] {}", username, msg)); } }
            Err(e) => { results.push(format!("[{}] Error: {}", username, e)); }
        }
    }

    if results.is_empty() {
        Ok(format!("Executed as {} target(s)", targets.len()))
    } else {
        Ok(results.join("\n"))
    }
}

/// Execute `at <target> run <command>` — run at the target's position
fn execute_at(ctx: &CommandContext) -> CommandResult {
    let selector = &ctx.args[1];
    let run_idx = ctx.args.iter().position(|a| a.to_lowercase() == "run")
        .ok_or("Expected 'run' after target")?;

    let targets = resolve_player_targets(selector, ctx);
    if targets.is_empty() {
        return Err(format!("No targets matched selector: {}", selector));
    }

    let subcommand_args: Vec<String> = ctx.args[run_idx + 1..].iter().map(|s| s.to_string()).collect();
    let dispatcher = ctx.dispatcher.ok_or("Internal error: dispatcher not available")?;

    let mut results: Vec<String> = Vec::new();
    for (uuid, username) in &targets {
        // Get target's position and inject as ~ reference via a sub-execution
        if let Some(player) = ctx.player_manager.get(uuid) {
            // Replace ~ coordinates with absolute values in subcommand args
            let resolved_args: Vec<String> = subcommand_args.iter().map(|arg| {
                if arg == "~" {
                    format!("{}", player.position.x as i32)
                } else if arg == "~ ~" {
                    format!("{} {}", player.position.x as i32, player.position.y as i32)
                } else {
                    arg.clone()
                }
            }).collect();

            let sub_ctx = CommandContext {
                source: ctx.source.clone(),
                args: resolved_args,
                ..*ctx
            };

            match dispatcher.dispatch(&sub_ctx) {
                Ok(msg) => { if !msg.is_empty() { results.push(format!("[{}] {}", username, msg)); } }
                Err(e) => { results.push(format!("[{}] Error: {}", username, e)); }
            }
        }
    }

    if results.is_empty() {
        Ok(format!("Executed at {} target(s)", targets.len()))
    } else {
        Ok(results.join("\n"))
    }
}

/// Execute `positioned <x> <y> <z> run <command>` — run at specific coordinates
fn execute_positioned(ctx: &CommandContext) -> CommandResult {
    if ctx.args.len() < 6 {
        return Err("Usage: /execute positioned <x> <y> <z> run <command>".into());
    }

    let x = &ctx.args[1];
    let y = &ctx.args[2];
    let z = &ctx.args[3];

    if ctx.args[4].to_lowercase() != "run" {
        return Err("Expected 'run' after coordinates".into());
    }

    let subcommand_args: Vec<String> = ctx.args[5..].iter().map(|s| {
        // Replace ~ with absolute coordinates
        if s == "~" { x.clone() }
        else if s == "~ ~" { format!("{} {}", x, y) }
        else if s == "~ ~ ~" { format!("{} {} {}", x, y, z) }
        else { s.clone() }
    }).collect();

    let dispatcher = ctx.dispatcher.ok_or("Internal error: dispatcher not available")?;
    let sub_ctx = CommandContext {
        source: ctx.source.clone(),
        args: subcommand_args,
        player_manager: ctx.player_manager,
        shutdown_tx: ctx.shutdown_tx,
        world_state: ctx.world_state,
        motd: ctx.motd,
        max_players: ctx.max_players,
        chunk_store: ctx.chunk_store,
        save_trigger: ctx.save_trigger,
        dispatcher: Some(dispatcher),
        mob_manager: ctx.mob_manager,
        team_manager: ctx.team_manager,
    };
    dispatcher.dispatch(&sub_ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::{Command, CommandDispatcher};
    use mc_core::world_state::WorldState;
    use mc_player::player::PlayerManager;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    fn setup() -> (CommandDispatcher, Arc<PlayerManager>, Arc<parking_lot::RwLock<WorldState>>, broadcast::Sender<()>) {
        let pm = Arc::new(PlayerManager::new());
        let ws = Arc::new(parking_lot::RwLock::new(WorldState::new(42)));
        let (tx, _) = broadcast::channel(1);
        (CommandDispatcher::new(), pm, ws, tx)
    }

    #[test]
    fn test_execute_missing_args() {
        let cmd = ExecuteCommand;
        let ctx = CommandContext {
            source: CommandSource::Console,
            args: vec!["as".into()],
            player_manager: &Arc::new(PlayerManager::new()),
            shutdown_tx: &broadcast::channel(1).0,
            world_state: &Arc::new(parking_lot::RwLock::new(WorldState::new(0))),
            motd: "",
            max_players: 20,
            chunk_store: None,
            save_trigger: None,
            dispatcher: None,
            mob_manager: None,
            team_manager: None,
        };
        assert!(cmd.execute(&ctx).is_err());
    }

    #[test]
    fn test_execute_bad_subcommand() {
        let (disp, pm, ws, tx) = setup();
        // Add a player so @a resolves
        let uuid = mc_core::auth::offline_uuid("TestPlayer");
        pm.add_player(uuid, "TestPlayer".into());

        let cmd = ExecuteCommand;
        let ctx = CommandContext {
            source: CommandSource::Console,
            args: vec!["as".into(), "@a".into(), "run".into(), "nonexistent".into()],
            player_manager: &pm,
            shutdown_tx: &tx,
            world_state: &ws,
            motd: "",
            max_players: 20,
            chunk_store: None,
            save_trigger: None,
            dispatcher: Some(&disp),
            mob_manager: None,
            team_manager: None,
        };
        let result = cmd.execute(&ctx);
        // Should not panic — returns error from subcommand dispatch
        assert!(result.is_ok()); // execute itself succeeded, subcommand error is in the output
    }

    #[test]
    fn test_execute_as_self() {
        let mut disp = CommandDispatcher::new();

        // Register a simple test command
        struct TestCmd;
        impl crate::dispatcher::Command for TestCmd {
            fn name(&self) -> &str { "testcmd" }
            fn execute(&self, ctx: &CommandContext) -> CommandResult {
                Ok(format!("source={}", ctx.source.name()))
            }
        }
        disp.register(TestCmd);

        let pm = Arc::new(PlayerManager::new());
        let uuid = mc_core::auth::offline_uuid("Alice");
        pm.add_player(uuid, "Alice".into());
        let ws = Arc::new(parking_lot::RwLock::new(WorldState::new(42)));
        let (tx, _) = broadcast::channel(1);

        let cmd = ExecuteCommand;
        let ctx = CommandContext {
            source: CommandSource::Console,
            args: vec!["as".into(), "@a".into(), "run".into(), "testcmd".into()],
            player_manager: &pm,
            shutdown_tx: &tx,
            world_state: &ws,
            motd: "",
            max_players: 20,
            chunk_store: None,
            save_trigger: None,
            dispatcher: Some(&disp),
            mob_manager: None,
            team_manager: None,
        };
        let result = cmd.execute(&ctx).unwrap();
        // Should contain "Alice" as the source of testcmd
        assert!(result.contains("Alice"), "Expected 'Alice' in result: {}", result);
    }
}
