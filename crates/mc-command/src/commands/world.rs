//! 世界命令: time, weather, difficulty

use crate::dispatcher::{Command, CommandContext, CommandResult};
use mc_core::world_state::{Difficulty, Weather};

pub struct TimeCommand;
impl Command for TimeCommand {
    fn name(&self) -> &str { "time" }
    fn description(&self) -> &str { "Query or change world time" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let action = ctx.args.first().map(|s| s.as_str()).unwrap_or("query");
        match action {
            "set" => {
                let time_str = ctx.args.get(1).ok_or("Usage: /time set <value|day|night|noon|midnight>")?;
                let time: u64 = match time_str.as_str() {
                    "day" => 1000,
                    "night" => 13000,
                    "noon" => 6000,
                    "midnight" => 18000,
                    _ => time_str.parse::<u64>().map_err(|e| format!("Invalid number: {}", e))?,
                };
                let mut ws = ctx.world_state.write();
                ws.set_time(time);
                Ok(format!("Time set to {}", time))
            }
            "add" => {
                let amount: u64 = ctx.args.get(1)
                    .ok_or("Usage: /time add <ticks>")?
                    .parse()
                    .map_err(|e| format!("Invalid number: {}", e))?;
                let mut ws = ctx.world_state.write();
                ws.add_time(amount);
                Ok(format!("Added {} ticks (now {})", amount, ws.time))
            }
            "query" => {
                let ws = ctx.world_state.read();
                Ok(format!("Time: {} (daytime: {})", ws.time, ws.daytime))
            }
            _ => Err("Usage: /time <set|add|query> [value]".into()),
        }
    }
}

pub struct WeatherCommand;
impl Command for WeatherCommand {
    fn name(&self) -> &str { "weather" }
    fn description(&self) -> &str { "Set the weather" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let weather = ctx.args.first().map(|s| s.as_str()).unwrap_or("clear");
        let duration = ctx.args.get(1)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(6000);
        match weather {
            "clear" => {
                ctx.world_state.write().set_weather(Weather::Clear, duration);
                Ok(format!("Weather set to clear for {} ticks", duration))
            }
            "rain" => {
                ctx.world_state.write().set_weather(Weather::Rain, duration);
                Ok(format!("Weather set to rain for {} ticks", duration))
            }
            "thunder" => {
                ctx.world_state.write().set_weather(Weather::Thunder, duration);
                Ok(format!("Weather set to thunder for {} ticks", duration))
            }
            _ => Err("Usage: /weather <clear|rain|thunder> [duration]".into()),
        }
    }
}

pub struct DifficultyCommand;
impl Command for DifficultyCommand {
    fn name(&self) -> &str { "difficulty" }
    fn description(&self) -> &str { "Set the difficulty level" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let diff = ctx.args.first().map(|s| s.as_str()).unwrap_or("normal");
        let difficulty = match diff {
            "peaceful" | "p" => Difficulty::Peaceful,
            "easy" | "e" => Difficulty::Easy,
            "normal" | "n" => Difficulty::Normal,
            "hard" | "h" => Difficulty::Hard,
            _ => return Err("Usage: /difficulty <peaceful|easy|normal|hard>".into()),
        };
        ctx.world_state.write().set_difficulty(difficulty);
        Ok(format!("Difficulty set to {:?}", difficulty))
    }
}

/// /tick — control server tick rate
pub struct TickCommand;
impl Command for TickCommand {
    fn name(&self) -> &str { "tick" }
    fn description(&self) -> &str { "Control server tick rate" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let action = ctx.args.first().map(|s| s.as_str()).unwrap_or("query");
        match action {
            "freeze" => {
                ctx.world_state.write().tick_frozen = true;
                Ok("Tick frozen. Use /tick unfreeze to resume.".into())
            }
            "unfreeze" | "resume" => {
                ctx.world_state.write().tick_frozen = false;
                Ok("Tick unfrozen. Resuming normal tick rate.".into())
            }
            "sprint" => {
                let rate: u32 = ctx.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(40);
                ctx.world_state.write().tick_sprint_rate = rate.clamp(1, 1000);
                Ok(format!("Tick sprint set to {} tps", rate))
            }
            "query" => {
                let ws = ctx.world_state.read();
                let status = if ws.tick_frozen { "frozen" } else { "running" };
                let rate = if ws.tick_sprint_rate > 0 { ws.tick_sprint_rate } else { 20 };
                Ok(format!("Tick: {} at {} tps", status, rate))
            }
            _ => Err("Usage: /tick <freeze|unfreeze|sprint <rate>|query>".into()),
        }
    }
}

/// /gamerule <rule> [value]
pub struct GameruleCommand;
impl Command for GameruleCommand {
    fn name(&self) -> &str { "gamerule" }
    fn description(&self) -> &str { "View or set game rules" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let rule = ctx.args.first().map(|s| s.as_str());
        match rule {
            Some(r) => {
                let mut ws = ctx.world_state.write();
                if let Some(value) = ctx.args.get(1) {
                    ws.game_rules.insert(r.to_string(), value.to_string());
                    Ok(format!("Game rule {} updated to {}", r, value))
                } else if let Some(v) = ws.game_rules.get(r) {
                    Ok(format!("{} = {}", r, v))
                } else {
                    Err(format!("Unknown game rule: {}", r))
                }
            }
            None => {
                let ws = ctx.world_state.read();
                let mut rules: Vec<_> = ws.game_rules.iter().collect();
                rules.sort_by_key(|(k, _)| *k);
                let list: Vec<_> = rules.iter().map(|(k, v)| format!("  {} = {}", k, v)).collect();
                Ok(format!("Game rules:\n{}", list.join("\n")))
            }
        }
    }
}
