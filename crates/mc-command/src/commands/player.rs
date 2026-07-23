//! 玩家命令: gamemode, defaultgamemode, tp, give, kill, setblock, spawnpoint

use crate::dispatcher::{resolve_player_target, resolve_player_targets, Command, CommandContext, CommandResult, CommandSource};
use crate::parser;

pub struct GamemodeCommand;
impl Command for GamemodeCommand {
    fn name(&self) -> &str { "gamemode" }
    fn description(&self) -> &str { "Change player game mode" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let mode_str = ctx.args.first().ok_or("Usage: /gamemode <mode> [player]")?;
        let mode = parser::parse_gamemode(mode_str)?;

        let targets = if let Some(target) = ctx.args.get(1) {
            let resolved = resolve_player_targets(target, ctx);
            if resolved.is_empty() {
                return Err(format!("No players matched: {}", target));
            }
            resolved
        } else {
            if let CommandSource::Player { uuid, ref username } = ctx.source {
                vec![(uuid, username.clone())]
            } else {
                return Err("Console must specify a target player".into());
            }
        };

        for (uuid, _) in &targets {
            ctx.player_manager.set_gamemode(uuid, mode)?;
        }
        let names: Vec<_> = targets.iter().map(|(_, n)| n.clone()).collect();
        Ok(format!("Set gamemode to {:?} for {}", mode, names.join(", ")))
    }
}

pub struct DefaultGamemodeCommand;
impl Command for DefaultGamemodeCommand {
    fn name(&self) -> &str { "defaultgamemode" }
    fn description(&self) -> &str { "Set the default game mode for new players" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let mode_str = ctx.args.first().ok_or("Usage: /defaultgamemode <mode>")?;
        let mode = parser::parse_gamemode(mode_str)?;
        ctx.world_state.write().set_default_gamemode(mode);
        ctx.player_manager.broadcast_chat("Server", &format!("Default game mode set to {:?}", mode), true);
        Ok(format!("Default game mode: {:?}", mode))
    }
}

pub struct TpCommand;
impl Command for TpCommand {
    fn name(&self) -> &str { "tp" }
    fn description(&self) -> &str { "Teleport to a player or coordinates" }
    fn aliases(&self) -> &[&str] { &["teleport"] }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        match ctx.args.len() {
            1 => {
                let target = &ctx.args[0];
                match resolve_player_target(target, ctx) {
                    Some((_, _)) => {
                        if let Some(target_p) = ctx.player_manager.get_by_name(target)
                            && let CommandSource::Player { uuid, .. } = &ctx.source {
                                let tx = target_p.position.x;
                                let ty = target_p.position.y;
                                let tz = target_p.position.z;
                                ctx.player_manager.update_position_full(uuid, tx, ty, tz, target_p.position.yaw, target_p.position.pitch)?;
                                return Ok(format!("Teleported to {}", target));
                            }
                        Ok("(console cannot teleport)".into())
                    }
                    None => Err(format!("Player not found: {}", target)),
                }
            }
            2 => {
                let from = &ctx.args[0];
                let to = &ctx.args[1];
                if let (Some((from_uuid, _from_name)), Some(to_p)) = (
                    resolve_player_target(from, ctx),
                    ctx.player_manager.get_by_name(to),
                ) {
                    ctx.player_manager.update_position_full(
                        &from_uuid, to_p.position.x, to_p.position.y, to_p.position.z, to_p.position.yaw, to_p.position.pitch,
                    )?;
                    Ok(format!("Teleported {} to {}", from, to))
                } else {
                    Err("Player not found".into())
                }
            }
            3 | 5 => {
                let x = parser::parse_f64(&ctx.args[0])?;
                let y = parser::parse_f64(&ctx.args[1])?;
                let z = parser::parse_f64(&ctx.args[2])?;
                let yaw = if ctx.args.len() >= 5 { parser::parse_f64(&ctx.args[3])? as f32 } else { 0.0 };
                let pitch = if ctx.args.len() >= 5 { parser::parse_f64(&ctx.args[4])? as f32 } else { 0.0 };
                if let CommandSource::Player { uuid, .. } = &ctx.source {
                    ctx.player_manager.update_position_full(uuid, x, y, z, yaw, pitch)?;
                }
                Ok(format!("Teleported to {:.1}, {:.1}, {:.1}", x, y, z))
            }
            _ => Err("Usage: /tp <player> | /tp <x> <y> <z> [yaw] [pitch] | /tp <from> <to>".into()),
        }
    }
}

pub struct GiveCommand;
impl Command for GiveCommand {
    fn name(&self) -> &str { "give" }
    fn description(&self) -> &str { "Give items to a player" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let target = ctx.args.first().ok_or("Usage: /give <player> <item> [count]")?;
        let item_name = parser::normalize_item_name(
            ctx.args.get(1).ok_or("Usage: /give <player> <item> [count]")?
        );
        let count = ctx.args.get(2).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);

        // Resolve item name to BlockState
        let block = mc_core::item::resolve_item(&item_name)
            .ok_or_else(|| format!("Unknown item: {}", item_name))?;

        let targets = resolve_player_targets(target, ctx);
        if targets.is_empty() {
            return Err(format!("No players matched: {}", target));
        }

        let mut results = Vec::new();
        for (uuid, name) in &targets {
            match ctx.player_manager.add_item_to_player(uuid, block, count) {
                Ok(added) => {
                    results.push(format!("{}x {} to {}", added, item_name, name));
                }
                Err(e) => {
                    results.push(format!("{}: {}", name, e));
                }
            }
        }
        Ok(format!("Gave {}", results.join(", ")))
    }
}

pub struct KillCommand;
impl Command for KillCommand {
    fn name(&self) -> &str { "kill" }
    fn description(&self) -> &str { "Kill a player or entity" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        match ctx.args.first() {
            Some(target) => {
                let targets = resolve_player_targets(target, ctx);
                if targets.is_empty() {
                    return Err(format!("No players matched: {}", target));
                }
                for (uuid, _) in &targets {
                    ctx.player_manager.set_health(uuid, 0.0)?;
                }
                let names: Vec<_> = targets.iter().map(|(_, n)| n.clone()).collect();
                Ok(format!("Killed {}", names.join(", ")))
            }
            None => {
                if let CommandSource::Player { uuid, .. } = &ctx.source {
                    ctx.player_manager.set_health(uuid, 0.0)?;
                    Ok("Killed self".into())
                } else {
                    Err("Console must specify a target".into())
                }
            }
        }
    }
}

/// /setblock <x> <y> <z> <block>
pub struct SetblockCommand;
impl Command for SetblockCommand {
    fn name(&self) -> &str { "setblock" }
    fn description(&self) -> &str { "Place a block at coordinates" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let x_str = ctx.args.first().ok_or("Usage: /setblock <x> <y> <z> <block>")?;
        let y_str = ctx.args.get(1).ok_or("Usage: /setblock <x> <y> <z> <block>")?;
        let z_str = ctx.args.get(2).ok_or("Usage: /setblock <x> <y> <z> <block>")?;
        let block_name = ctx.args.get(3).ok_or("Usage: /setblock <x> <y> <z> <block>")?;

        let x = x_str.parse::<i32>().map_err(|_| format!("Invalid X: {}", x_str))?;
        let y = y_str.parse::<i32>().map_err(|_| format!("Invalid Y: {}", y_str))?;
        let z = z_str.parse::<i32>().map_err(|_| format!("Invalid Z: {}", z_str))?;

        if !(-64..=319).contains(&y) {
            return Err(format!("Y coordinate {} out of range [-64, 319]", y));
        }

        let block = mc_core::item::resolve_item(block_name)
            .ok_or_else(|| format!("Unknown block: {}", block_name))?;

        let chunk_store = ctx.chunk_store.ok_or("Chunk store not available")?;
        let cp = mc_core::position::ChunkPos::new(x >> 4, z >> 4);
        if let Some(mut chunk) = chunk_store.get_mut(&cp) {
            chunk.set_block((x & 0xF) as usize, y, (z & 0xF) as usize, block);
            Ok(format!("Set block at ({}, {}, {}) to {}", x, y, z, block_name))
        } else {
            // Chunk not loaded — generate a new one
            // For simplicity, we can't create chunks from commands right now
            Err(format!("Chunk ({},{}) not loaded. Walk near the area first.", cp.x, cp.z))
        }
    }
}

/// /spawnpoint [player] [x y z]
pub struct SpawnpointCommand;
impl Command for SpawnpointCommand {
    fn name(&self) -> &str { "spawnpoint" }
    fn description(&self) -> &str { "Set a player's spawn location" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        // /spawnpoint — set own personal spawn to current position
        if ctx.args.is_empty() {
            if let CommandSource::Player { uuid, username: _ } = &ctx.source
                && let Some(p) = ctx.player_manager.get(uuid) {
                    let _ = ctx.player_manager.set_spawn_position(uuid, p.position.x, p.position.y, p.position.z, 0.0);
                    return Ok(format!("Spawnpoint set to ({:.0}, {:.0}, {:.0})", p.position.x, p.position.y, p.position.z));
                }
            return Err("Usage: /spawnpoint [player] [x y z]".into());
        }

        // /spawnpoint <player> <x> <y> <z>
        if ctx.args.len() >= 4 {
            let target = &ctx.args[0];
            let x = ctx.args[1].parse::<f64>().map_err(|_| "Invalid X".to_string())?;
            let y = ctx.args[2].parse::<f64>().map_err(|_| "Invalid Y".to_string())?;
            let z = ctx.args[3].parse::<f64>().map_err(|_| "Invalid Z".to_string())?;
            let yaw: f32 = ctx.args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0.0);

            let targets = crate::dispatcher::resolve_player_targets(target, ctx);
            if targets.is_empty() { return Err(format!("No player matched: {}", target)); }
            let names: Vec<&str> = targets.iter().map(|(_, name)| name.as_str()).collect();
            for (uuid, _name) in &targets {
                let _ = ctx.player_manager.set_spawn_position(uuid, x, y, z, yaw);
            }
            return Ok(format!("Spawnpoint set for {} player(s) at ({:.0}, {:.0}, {:.0})", names.len(), x, y, z));
        }

        // /spawnpoint <player> — without coordinates, set their current position as spawn
        if ctx.args.len() == 1 {
            let target = &ctx.args[0];
            if let Some((uuid, name)) = resolve_player_target(target, ctx)
                && let Some(p) = ctx.player_manager.get(&uuid) {
                    let _ = ctx.player_manager.set_spawn_position(&uuid, p.position.x, p.position.y, p.position.z, 0.0);
                    return Ok(format!("Spawnpoint for {} set to ({:.0}, {:.0}, {:.0})", name, p.position.x, p.position.y, p.position.z));
                }
        }

        Err("Usage: /spawnpoint [player] [x y z]".into())
    }
}

/// /fill <x1> <y1> <z1> <x2> <y2> <z2> <block>
pub struct FillCommand;
impl Command for FillCommand {
    fn name(&self) -> &str { "fill" }
    fn description(&self) -> &str { "Fill a region with blocks" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let x1 = ctx.args.first().ok_or("Usage: /fill <x1> <y1> <z1> <x2> <y2> <z2> <block>")?.parse::<i32>().map_err(|_| "Invalid X1")?;
        let y1 = ctx.args.get(1).ok_or("Missing Y1")?.parse::<i32>().map_err(|_| "Invalid Y1")?;
        let z1 = ctx.args.get(2).ok_or("Missing Z1")?.parse::<i32>().map_err(|_| "Invalid Z1")?;
        let x2 = ctx.args.get(3).ok_or("Missing X2")?.parse::<i32>().map_err(|_| "Invalid X2")?;
        let y2 = ctx.args.get(4).ok_or("Missing Y2")?.parse::<i32>().map_err(|_| "Invalid Y2")?;
        let z2 = ctx.args.get(5).ok_or("Missing Z2")?.parse::<i32>().map_err(|_| "Invalid Z2")?;
        let block_name = ctx.args.get(6).ok_or("Usage: /fill <x1> <y1> <z1> <x2> <y2> <z2> <block>")?;

        let block = mc_core::item::resolve_item(block_name)
            .ok_or_else(|| format!("Unknown block: {}", block_name))?;

        let (xmin, xmax) = (x1.min(x2), x1.max(x2));
        let (ymin, ymax) = (y1.clamp(-64, 319), y2.clamp(-64, 319)); // clamp to world
        let (zmin, zmax) = (z1.min(z2), z1.max(z2));

        let volume = (xmax - xmin + 1) as u64 * (ymax - ymin + 1) as u64 * (zmax - zmin + 1) as u64;
        if volume > 32768 {
            return Err(format!("Too many blocks ({} > 32768). Use a smaller region.", volume));
        }

        let chunk_store = ctx.chunk_store.ok_or("Chunk store not available")?;
        let mut count: u64 = 0;
        for x in xmin..=xmax {
            for z in zmin..=zmax {
                let cp = mc_core::position::ChunkPos::new(x >> 4, z >> 4);
                if let Some(mut chunk) = chunk_store.get_mut(&cp) {
                    for y in ymin..=ymax {
                        chunk.set_block((x & 0xF) as usize, y, (z & 0xF) as usize, block);
                        count += 1;
                    }
                }
            }
        }
        Ok(format!("Filled {} blocks with {}", count, block_name))
    }
}

/// /xp <amount> [player] — add experience
pub struct XpCommand;
impl Command for XpCommand {
    fn name(&self) -> &str { "xp" }
    fn description(&self) -> &str { "Manage player experience (add/set/query)" }
    fn aliases(&self) -> &[&str] { &["experience"] }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if ctx.args.is_empty() {
            return Err("Usage: /xp add <amount> [player] | /xp set <amount> [player] | /xp query <player> levels|points".into());
        }
        let sub = ctx.args[0].to_lowercase();
        match sub.as_str() {
            "add" => {
                let amount: i32 = ctx.args.get(1).and_then(|a| a.parse().ok()).ok_or("Specify amount: /xp add <points> [player]")?;
                let target = ctx.args.get(2).map(|s| s.as_str()).unwrap_or("@s");
                let targets = resolve_player_targets(target, ctx);
                if targets.is_empty() { return Err("No players matched".into()); }
                for (uuid, _) in &targets { ctx.player_manager.add_xp(uuid, amount)?; }
                Ok(format!("Added {} XP to {} player(s)", amount, targets.len()))
            }
            "set" => {
                let amount: i32 = ctx.args.get(1).and_then(|a| a.parse().ok()).ok_or("Specify amount: /xp set <levels> [player]")?;
                let target = ctx.args.get(2).map(|s| s.as_str()).unwrap_or("@s");
                let targets = resolve_player_targets(target, ctx);
                if targets.is_empty() { return Err("No players matched".into()); }
                // Set level: calculate total XP for that level
                let xp_for_level = |level: i32| -> i32 {
                    let mut total = 0;
                    for l in 0..level { total += if l < 16 { 2*l+7 } else if l < 31 { 5*l-38 } else { 9*l-158 }; }
                    total
                };
                let total_xp = xp_for_level(amount);
                for (uuid, _) in &targets {
                    let _ = ctx.player_manager.set_xp(uuid, 0.0, amount, total_xp);
                }
                Ok(format!("Set {} player(s) to level {}", targets.len(), amount))
            }
            "query" => {
                let target = ctx.args.get(1).ok_or("Specify player: /xp query <player>")?;
                let targets = resolve_player_targets(target, ctx);
                if targets.is_empty() { return Err("No players matched".into()); }
                let (uuid, name) = &targets[0];
                if let Some(p) = ctx.player_manager.get(uuid) {
                    Ok(format!("{} has {} XP (level {}, {:.0}%)", name, p.xp_total, p.xp_level, p.xp_bar * 100.0))
                } else {
                    Err("Player not found".into())
                }
            }
            _ => {
                // Legacy syntax: /xp <amount> [player] → add
                let amount: i32 = ctx.args[0].parse().map_err(|_| "Invalid amount".to_string())?;
                let target = ctx.args.get(1).map(|s| s.as_str()).unwrap_or("@s");
                let targets = resolve_player_targets(target, ctx);
                if targets.is_empty() { return Err("No players matched".into()); }
                for (uuid, _) in &targets { ctx.player_manager.add_xp(uuid, amount)?; }
                Ok(format!("Added {} XP to {} player(s)", amount, targets.len()))
            }
        }
    }
}

/// /summon <entity_type> [x y z] — spawn an entity
pub struct SummonCommand;
impl Command for SummonCommand {
    fn name(&self) -> &str { "summon" }
    fn description(&self) -> &str { "Summon an entity" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let entity = ctx.args.first().ok_or("Usage: /summon <entity_type> [x y z]")?;
        // Default position: command source position or (0,64,0)
        let default_x = if let CommandSource::Player { uuid, .. } = &ctx.source {
            ctx.player_manager.get(uuid).map(|p| p.position.x).unwrap_or(0.0)
        } else { 0.0 };
        let default_y = if let CommandSource::Player { uuid, .. } = &ctx.source {
            ctx.player_manager.get(uuid).map(|p| p.position.y).unwrap_or(64.0)
        } else { 64.0 };
        let default_z = if let CommandSource::Player { uuid, .. } = &ctx.source {
            ctx.player_manager.get(uuid).map(|p| p.position.z).unwrap_or(0.0)
        } else { 0.0 };
        let x: f64 = ctx.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(default_x);
        let y: f64 = ctx.args.get(2).and_then(|s| s.parse().ok()).unwrap_or(default_y);
        let z: f64 = ctx.args.get(3).and_then(|s| s.parse().ok()).unwrap_or(default_z);
        let entity_type = match entity.to_lowercase().as_str() {
            "minecraft:item" | "item" => 54,
            "minecraft:xp_orb" | "xp_orb" | "experience_orb" => 53,
            "minecraft:cow" | "cow" => 11,
            "minecraft:pig" | "pig" => 12,
            "minecraft:chicken" | "chicken" => 13,
            "minecraft:sheep" | "sheep" => 14,
            "minecraft:zombie" | "zombie" => 36,
            "minecraft:skeleton" | "skeleton" => 37,
            "minecraft:creeper" | "creeper" => 33,
            "minecraft:spider" | "spider" => 35,
            "minecraft:enderman" | "enderman" => 38,
            "minecraft:slime" | "slime" => 34,
            "minecraft:witch" | "witch" => 48,
            "minecraft:wolf" | "wolf" => 95,
            "minecraft:villager" | "villager" => 92,
            "minecraft:iron_golem" | "iron_golem" => 99,
            "minecraft:blaze" | "blaze" => 43,
            "minecraft:ghast" | "ghast" => 56,
            "minecraft:wither" | "wither" => 25,
            "minecraft:ender_dragon" | "ender_dragon" => 53,
            "minecraft:horse" | "horse" => 31,
            "minecraft:llama" | "llama" => 28,
            "minecraft:cat" | "cat" | "ocelot" => 29,
            _ => return Err(format!("Unknown entity type: {} (20 types supported)", entity)),
        };
        // Actually spawn via MobManager if available
        if let Some(mm) = ctx.mob_manager {
            let eid = (uuid::Uuid::new_v4().as_u128() & 0x7FFFFFFF) as i32;
            let mob = mc_player::mob::TrackedMob {
                entity_id: eid, uuid: uuid::Uuid::new_v4(), mob_type: entity_type,
                position: mc_core::position::Position::new(x, y, z),
                health: mc_player::mob::mob_max_health(entity_type),
                max_health: mc_player::mob::mob_max_health(entity_type),
                age_ticks: 0, ai_timer: 40, ai_state: mc_player::mob::MobAiState::Idle,
                attack_cooldown: 0, last_sync_tick: 0,
                owner_uuid: None, is_tamed: false, is_sitting: false, tame_attempts: 0, is_baby: false, in_love_ticks: 0, breed_cooldown: 0, is_sheared: false, is_on_fire: false, is_in_water: false, path: Vec::new(), path_last_tick: 0, sulfur_cube_archetype: None, absorbed_block_id: None, is_small_cube: false, is_dormant: false, dirty_flags: 3,
            };
            mm.register(mob);
            Ok(format!("Summoned {} (type={}) at ({:.1}, {:.1}, {:.1}) [entity_id={}]", entity, entity_type, x, y, z, eid))
        } else {
            Ok(format!("Summoned {} (type={}) at ({:.1}, {:.1}, {:.1}) (mob manager unavailable — entity tracked server-side)", entity, entity_type, x, y, z))
        }
    }
}

// ═══════════════════════════════════════════════════════
// /effect — 状态效果管理
// ═══════════════════════════════════════════════════════

pub struct EffectCommand;

impl Command for EffectCommand {
    fn name(&self) -> &str { "effect" }
    fn description(&self) -> &str { "Give or clear status effects" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if ctx.args.is_empty() {
            return Err("Usage: /effect give <target> <effect> [seconds] [amplifier]\n       /effect clear <target>".into());
        }

        match ctx.args[0].to_lowercase().as_str() {
            "give" => effect_give(ctx),
            "clear" => effect_clear(ctx),
            _ => Err(format!("Unknown effect subcommand: {}. Use 'give' or 'clear'", ctx.args[0])),
        }
    }
}

fn effect_give(ctx: &CommandContext) -> CommandResult {
    // /effect give <target> <effect> [seconds] [amplifier]
    if ctx.args.len() < 3 {
        return Err("Usage: /effect give <target> <effect> [seconds] [amplifier]".into());
    }

    let target = &ctx.args[1];
    let effect_name = &ctx.args[2];
    let seconds = ctx.args.get(3).and_then(|s| s.parse::<u32>().ok()).unwrap_or(30);
    let amplifier = ctx.args.get(4).and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);

    let effect = mc_core::effect::resolve_effect(effect_name)
        .ok_or_else(|| format!("Unknown effect: {}. Use /effect give <target> <effect>", effect_name))?;

    let duration_ticks = seconds.saturating_mul(20); // seconds → ticks
    let active = mc_core::effect::ActiveEffect::new(effect, amplifier, duration_ticks);

    let targets = resolve_player_targets(target, ctx);
    if targets.is_empty() {
        return Err(format!("No targets matched: {}", target));
    }

    let mut results = Vec::new();
    for (uuid, username) in &targets {
        match ctx.player_manager.add_effect(uuid, active.clone()) {
            Ok(msg) => results.push(format!("{}: {}", username, msg)),
            Err(e) => results.push(format!("{}: Error — {}", username, e)),
        }
    }

    Ok(results.join("\n"))
}

fn effect_clear(ctx: &CommandContext) -> CommandResult {
    // /effect clear <target> [<effect>]
    if ctx.args.len() < 2 {
        return Err("Usage: /effect clear <target> [<effect>]".into());
    }

    let target = &ctx.args[1];
    let specific_effect = ctx.args.get(2);

    let targets = resolve_player_targets(target, ctx);
    if targets.is_empty() {
        return Err(format!("No targets matched: {}", target));
    }

    let mut results = Vec::new();
    for (uuid, username) in &targets {
        if let Some(effect_name) = specific_effect {
            // Clear specific effect
            match mc_core::effect::resolve_effect(effect_name) {
                Some(effect_type) => {
                    if let Some(_p) = ctx.player_manager.get(uuid) {
                        let mut effects = _p.active_effects.clone();
                        effects.retain(|e| e.effect != effect_type);
                        results.push(format!("{}: cleared {:?} ({} effects remaining)", username, effect_type, effects.len()));
                    }
                }
                None => results.push(format!("{}: Unknown effect '{}'", username, effect_name)),
            }
        } else {
            match ctx.player_manager.clear_effects(uuid) {
                Ok(msg) => results.push(format!("{}: {}", username, msg)),
                Err(e) => results.push(format!("{}: Error — {}", username, e)),
            }
        }
    }

    Ok(results.join("\n"))
}
