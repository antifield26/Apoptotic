//! /scoreboard 命令 — 计分板目标管理
//!
//! 子命令:
//!   scoreboard objectives add <name> <criteria> [displayName]   — 创建目标
//!   scoreboard objectives remove <name>                         — 删除目标
//!   scoreboard objectives list                                  — 列出所有目标
//!   scoreboard players set <player> <objective> <value>         — 设置分数
//!   scoreboard players get <player> <objective>                 — 获取分数
//!   scoreboard players add <player> <objective> <value>         — 增加分数

use crate::dispatcher::{Command, CommandContext, CommandResult};
use std::collections::HashMap;

/// 计分板目标
#[derive(Debug, Clone)]
pub struct ScoreboardObjective {
    pub name: String,
    pub criteria: String,   // "dummy", "deathCount", etc.
    pub display_name: String,
}

/// 计分板存储 (所有目标的所有玩家分数)
#[derive(Debug, Clone, Default)]
pub struct ScoreboardData {
    pub objectives: HashMap<String, ScoreboardObjective>,
    pub scores: HashMap<String, HashMap<String, i32>>, // objective → (player_name → value)
}

impl ScoreboardData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_score(&mut self, player: &str, objective: &str, value: i32) -> Result<(), String> {
        if !self.objectives.contains_key(objective) {
            return Err(format!("Unknown objective: {}", objective));
        }
        self.scores
            .entry(objective.to_string())
            .or_default()
            .insert(player.to_string(), value);
        Ok(())
    }

    pub fn get_score(&self, player: &str, objective: &str) -> Option<i32> {
        self.scores.get(objective)?.get(player).copied()
    }

    pub fn add_score(&mut self, player: &str, objective: &str, delta: i32) -> Result<i32, String> {
        let current = self.get_score(player, objective).unwrap_or(0);
        let new_val = current + delta;
        self.set_score(player, objective, new_val)?;
        Ok(new_val)
    }

    pub fn list_objectives(&self) -> Vec<&ScoreboardObjective> {
        self.objectives.values().collect()
    }
}

use parking_lot::Mutex;

/// 全局计分板实例
static SCOREBOARD: std::sync::LazyLock<Mutex<ScoreboardData>> =
    std::sync::LazyLock::new(|| Mutex::new(ScoreboardData::new()));

/// 获取全局计分板 (用于 /trigger 等命令)
pub fn global_scoreboard() -> parking_lot::MutexGuard<'static, ScoreboardData> {
    SCOREBOARD.lock()
}

fn scoreboard() -> parking_lot::MutexGuard<'static, ScoreboardData> {
    SCOREBOARD.lock()
}

pub struct ScoreboardCommand;

impl Command for ScoreboardCommand {
    fn name(&self) -> &str { "scoreboard" }
    fn description(&self) -> &str { "Manage scoreboard objectives and scores" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if ctx.args.is_empty() {
            return Err("Usage: /scoreboard objectives add|remove|list ...\n       /scoreboard players set|get|add <player> <objective> [value]".into());
        }

        match ctx.args[0].to_lowercase().as_str() {
            "objectives" => scoreboard_objectives(ctx),
            "players" => scoreboard_players(ctx),
            _ => Err(format!("Unknown subcommand: {}. Use 'objectives' or 'players'", ctx.args[0])),
        }
    }
}

fn scoreboard_objectives(ctx: &CommandContext) -> CommandResult {
    // /scoreboard objectives add|remove|list [args...]
    if ctx.args.len() < 2 {
        return Err("Usage: /scoreboard objectives add <name> <criteria> [display]\n       /scoreboard objectives remove <name>\n       /scoreboard objectives list".into());
    }

    match ctx.args[1].to_lowercase().as_str() {
        "add" => {
            if ctx.args.len() < 4 {
                return Err("Usage: /scoreboard objectives add <name> <criteria> [display]".into());
            }
            let name = &ctx.args[2];
            let criteria = &ctx.args[3];
            let display = ctx.args.get(4).cloned().unwrap_or_else(|| name.clone());

            let mut sb = scoreboard();
            if sb.objectives.contains_key(name) {
                return Err(format!("Objective '{}' already exists", name));
            }
            sb.objectives.insert(name.clone(), ScoreboardObjective {
                name: name.clone(),
                criteria: criteria.clone(),
                display_name: display.clone(),
            });
            // Sync to all clients
            ctx.player_manager.broadcast_global(
                mc_player::player::PlayerStateEventKind::ScoreboardObjective(
                    name.clone(), 0, display, criteria.clone(),
                ),
            );
            Ok(format!("Created objective '{}' (criteria: {})", name, criteria))
        }
        "remove" => {
            if ctx.args.len() < 3 {
                return Err("Usage: /scoreboard objectives remove <name>".into());
            }
            let name = &ctx.args[2];
            let mut sb = scoreboard();
            if sb.objectives.remove(name).is_some() {
                sb.scores.remove(name);
                // Sync to all clients
                ctx.player_manager.broadcast_global(
                    mc_player::player::PlayerStateEventKind::ScoreboardObjective(
                        name.clone(), 1, String::new(), String::new(),
                    ),
                );
                Ok(format!("Removed objective '{}'", name))
            } else {
                Err(format!("Objective '{}' not found", name))
            }
        }
        "list" => {
            let sb = scoreboard();
            let objs = sb.list_objectives();
            if objs.is_empty() {
                Ok("No objectives defined".into())
            } else {
                let lines: Vec<String> = objs.iter()
                    .map(|o| format!("  {} (criteria: {}, display: {})", o.name, o.criteria, o.display_name))
                    .collect();
                Ok(format!("Objectives:\n{}", lines.join("\n")))
            }
        }
        _ => Err(format!("Unknown operation: {}. Use add, remove, or list", ctx.args[1])),
    }
}

fn scoreboard_players(ctx: &CommandContext) -> CommandResult {
    // /scoreboard players set|get|add <player> <objective> [value]
    if ctx.args.len() < 4 {
        return Err("Usage: /scoreboard players set <player> <objective> <value>\n       /scoreboard players get <player> <objective>\n       /scoreboard players add <player> <objective> <value>".into());
    }

    let operation = &ctx.args[1];
    let player_name = &ctx.args[2];
    let objective_name = &ctx.args[3];

    match operation.to_lowercase().as_str() {
        "set" => {
            if ctx.args.len() < 5 {
                return Err("Usage: /scoreboard players set <player> <objective> <value>".into());
            }
            let value: i32 = ctx.args[4].parse().map_err(|_| format!("Invalid value: {}", ctx.args[4]))?;
            scoreboard().set_score(player_name, objective_name, value)?;
            // Sync to all clients
            ctx.player_manager.broadcast_global(
                mc_player::player::PlayerStateEventKind::ScoreboardScore(
                    player_name.clone(), objective_name.clone(), value, 0,
                ),
            );
            Ok(format!("Set {}'s score in '{}' to {}", player_name, objective_name, value))
        }
        "get" => {
            match scoreboard().get_score(player_name, objective_name) {
                Some(val) => Ok(format!("{}'s score in '{}': {}", player_name, objective_name, val)),
                None => Ok(format!("{} has no score in '{}'", player_name, objective_name)),
            }
        }
        "add" => {
            if ctx.args.len() < 5 {
                return Err("Usage: /scoreboard players add <player> <objective> <value>".into());
            }
            let delta: i32 = ctx.args[4].parse().map_err(|_| format!("Invalid value: {}", ctx.args[4]))?;
            let new_val = scoreboard().add_score(player_name, objective_name, delta)?;
            // Sync to all clients
            ctx.player_manager.broadcast_global(
                mc_player::player::PlayerStateEventKind::ScoreboardScore(
                    player_name.clone(), objective_name.clone(), new_val, 0,
                ),
            );
            Ok(format!("{}'s score in '{}' is now {}", player_name, objective_name, new_val))
        }
        _ => Err(format!("Unknown operation: {}. Use set, get, or add", operation)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::{CommandDispatcher, CommandSource};
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

    fn make_ctx<'a>(pm: &'a Arc<PlayerManager>, ws: &'a Arc<parking_lot::RwLock<WorldState>>, tx: &'a broadcast::Sender<()>, args: Vec<&str>) -> CommandContext<'a> {
        CommandContext {
            source: CommandSource::Console,
            args: args.iter().map(|s| s.to_string()).collect(),
            player_manager: pm,
            shutdown_tx: tx,
            world_state: ws,
            motd: "",
            max_players: 20,
            chunk_store: None,
            save_trigger: None,
            dispatcher: None,
            mob_manager: None,
            team_manager: None,
        }
    }

    /// 生成唯一目标名以避免测试间竞争
    fn unique_obj(name: &str) -> String {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        format!("{}_{}", name, COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    #[test]
    fn test_scoreboard_objectives_add_list() {
        let (_disp, pm, ws, tx) = setup();
        let cmd = ScoreboardCommand;
        let obj = unique_obj("deaths");

        let ctx = make_ctx(&pm, &ws, &tx, vec!["objectives", "add", &obj, "dummy", "Deaths"]);
        let result = cmd.execute(&ctx).unwrap();
        assert!(result.contains("Created"), "{}", result);

        let ctx = make_ctx(&pm, &ws, &tx, vec!["objectives", "list"]);
        let result = cmd.execute(&ctx).unwrap();
        assert!(result.contains(&obj), "{}", result);
    }

    #[test]
    fn test_scoreboard_players_set_get() {
        let (_disp, pm, ws, tx) = setup();
        let cmd = ScoreboardCommand;
        let obj = unique_obj("kills");

        let ctx = make_ctx(&pm, &ws, &tx, vec!["objectives", "add", &obj, "dummy"]);
        cmd.execute(&ctx).unwrap();

        let ctx = make_ctx(&pm, &ws, &tx, vec!["players", "set", "Steve", &obj, "42"]);
        let result = cmd.execute(&ctx).unwrap();
        assert!(result.contains("42"), "{}", result);

        let ctx = make_ctx(&pm, &ws, &tx, vec!["players", "get", "Steve", &obj]);
        let result = cmd.execute(&ctx).unwrap();
        assert!(result.contains("42"), "{}", result);
    }

    #[test]
    fn test_scoreboard_players_add() {
        let (_disp, pm, ws, tx) = setup();
        let cmd = ScoreboardCommand;
        let obj = unique_obj("score");

        let ctx = make_ctx(&pm, &ws, &tx, vec!["objectives", "add", &obj, "dummy"]);
        cmd.execute(&ctx).unwrap();

        let ctx = make_ctx(&pm, &ws, &tx, vec!["players", "add", "Alice", &obj, "10"]);
        let result = cmd.execute(&ctx).unwrap();
        assert!(result.contains("10"), "{}", result);

        let ctx = make_ctx(&pm, &ws, &tx, vec!["players", "add", "Alice", &obj, "5"]);
        let result = cmd.execute(&ctx).unwrap();
        assert!(result.contains("15"), "{}", result);
    }

    #[test]
    fn test_scoreboard_objectives_remove() {
        let (_disp, pm, ws, tx) = setup();
        let cmd = ScoreboardCommand;
        let obj = unique_obj("tmp");

        let ctx = make_ctx(&pm, &ws, &tx, vec!["objectives", "add", &obj, "dummy"]);
        cmd.execute(&ctx).unwrap();

        let ctx = make_ctx(&pm, &ws, &tx, vec!["objectives", "remove", &obj]);
        let result = cmd.execute(&ctx).unwrap();
        assert!(result.contains("Removed"), "{}", result);
    }
}
