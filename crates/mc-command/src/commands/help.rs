//! /help — 显示可用命令列表

use crate::dispatcher::{Command, CommandContext, CommandResult};

pub struct HelpCommand;

impl Command for HelpCommand {
    fn name(&self) -> &str { "help" }
    fn description(&self) -> &str { "Show available commands" }
    fn aliases(&self) -> &[&str] { &["?"] }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if let Some(page_str) = ctx.args.first() {
            let _ = page_str;
        }

        let mut lines = vec!["§6--- Help: Commands ---§r".to_string()];
        // Dynamic listing from dispatcher (fallback to hardcoded if no dispatcher available)
        if let Some(dispatcher) = ctx.dispatcher {
            let cmds = dispatcher.get_command_info();
            for (name, desc, _aliases) in &cmds {
                lines.push(format!("§7/{}§r — {}", name, desc));
            }
            lines.push(format!("§7Total: {} commands§r", cmds.len()));
        } else {
            // Fallback for contexts without dispatcher
            let commands: &[(&str, &str)] = &[
                ("help", "Show available commands"),
                ("msg", "Send private message"),
                ("op", "Grant operator status"),
                ("stop", "Stop the server"),
                ("kick", "Kick a player"),
                ("gamemode", "Change game mode"),
                ("tp", "Teleport to player or coordinates"),
                ("give", "Give items to a player"),
                ("time", "Query or change world time"),
                ("list", "List online players"),
            ];
            for (name, desc) in commands {
                lines.push(format!("§7/{}§r — {}", name, desc));
            }
        }
        lines.push("§7Use @a (all), @p (nearest), @s (self) as target selectors§r".to_string());

        Ok(lines.join("\n"))
    }
}
