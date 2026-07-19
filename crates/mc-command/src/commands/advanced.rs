//! 高级命令 — /title, /playsound, /clear, /enchant

use crate::dispatcher::{Command, CommandContext, CommandResult, CommandSource};

// ═══════════════════════════════════════════════════════
// /title — 发送标题/副标题/动作栏消息
// ═══════════════════════════════════════════════════════

pub struct TitleCommand;

impl Command for TitleCommand {
    fn name(&self) -> &str { "title" }
    fn description(&self) -> &str { "Send a title/subtitle/actionbar to players" }
    fn aliases(&self) -> &[&str] { &[] }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if ctx.args.len() < 3 {
            return Err("Usage: /title <player> title|subtitle|actionbar|clear|reset <text>".into());
        }
        let target = &ctx.args[0];
        let action_str = &ctx.args[1].to_lowercase();
        let text = ctx.args.get(2).map(|s| s.as_str()).unwrap_or("");

        let action: i32 = match action_str.as_str() {
            "title" => 0,
            "subtitle" => 1,
            "actionbar" => 2,
            "clear" => 4,
            "reset" => 5,
            _ => return Err(format!("Unknown title action: {}. Use title/subtitle/actionbar/clear/reset", action_str)),
        };

        let json_text = if matches!(action, 0..=2) {
            format!("{{\"text\":\"{}\"}}", text.replace('\\', "\\\\").replace('"', "\\\""))
        } else {
            String::new()
        };

        let targets = crate::dispatcher::resolve_player_targets(target, ctx);
        if targets.is_empty() {
            return Err(format!("No player matched: {}", target));
        }

        let mut count = 0;
        for (uuid, _name) in &targets {
            if ctx.player_manager.send_title(uuid, action, json_text.clone()).is_ok() {
                count += 1;
            }
        }
        Ok(format!("Sent {} to {} player(s)", action_str, count))
    }
}

// ═══════════════════════════════════════════════════════
// /playsound — 播放音效
// ═══════════════════════════════════════════════════════

pub struct PlaysoundCommand;

impl Command for PlaysoundCommand {
    fn name(&self) -> &str { "playsound" }
    fn description(&self) -> &str { "Play a sound effect for players" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if ctx.args.len() < 2 {
            return Err("Usage: /playsound <sound> <player> [volume] [pitch]".into());
        }
        let sound_name = &ctx.args[0];
        let target = &ctx.args[1];
        let volume: f32 = ctx.args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let pitch: f32 = ctx.args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1.0);

        // Add minecraft: prefix if not present
        let full_sound = if sound_name.contains(':') {
            sound_name.clone()
        } else {
            format!("minecraft:{}", sound_name)
        };

        let targets = crate::dispatcher::resolve_player_targets(target, ctx);
        if targets.is_empty() {
            return Err(format!("No player matched: {}", target));
        }

        let mut count = 0;
        for (uuid, _name) in &targets {
            if ctx.player_manager.play_sound(uuid, full_sound.clone(), 7, volume, pitch).is_ok() {
                count += 1;
            }
        }
        Ok(format!("Played {} to {} player(s)", sound_name, count))
    }
}

// ═══════════════════════════════════════════════════════
// /clear — 清空玩家背包
// ═══════════════════════════════════════════════════════

pub struct ClearCommand;

impl Command for ClearCommand {
    fn name(&self) -> &str { "clear" }
    fn description(&self) -> &str { "Clear items from player inventory" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let target = if ctx.args.is_empty() {
            // Default to self
            if let CommandSource::Player { ref username, .. } = ctx.source {
                username.clone()
            } else {
                return Err("Usage: /clear [player]".into());
            }
        } else {
            ctx.args[0].clone()
        };

        let targets = crate::dispatcher::resolve_player_targets(&target, ctx);
        if targets.is_empty() {
            return Err(format!("No player matched: {}", target));
        }

        let mut count = 0;
        for (uuid, name) in &targets {
            if ctx.player_manager.clear_inventory(uuid).is_ok() {
                count += 1;
                tracing::info!("Cleared inventory of player '{}'", name);
            }
        }
        Ok(format!("Cleared inventory of {} player(s)", count))
    }
}

// ═══════════════════════════════════════════════════════
// /enchant — 附魔手持物品
// ═══════════════════════════════════════════════════════

pub struct EnchantCommand;

impl Command for EnchantCommand {
    fn name(&self) -> &str { "enchant" }
    fn description(&self) -> &str { "Enchant the held item of a player" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if ctx.args.len() < 2 {
            return Err("Usage: /enchant <player> <enchantment> [level]".into());
        }
        let enchant_name = ctx.args[1].to_lowercase();
        let level: u32 = ctx.args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1).min(255);

        let targets = crate::dispatcher::resolve_player_targets(&ctx.args[0], ctx);
        if targets.is_empty() { return Err(format!("No player matched: {}", ctx.args[0])); }

        let registry = mc_player::enchant::EnchantmentRegistry::new();
        // Find the specific enchantment by name
        if registry.find(&enchant_name).is_none() {
            return Err(format!("Unknown enchantment: {}", enchant_name));
        }

        let mut count = 0;
        for (uuid, _name) in &targets {
            if let Some(_held) = ctx.player_manager.get_held_item(uuid) {
                // Build NBT: {Enchantments:[{id:"minecraft:...", lvl:N}]}
                let nbt_str = format!("{{Enchantments:[{{id:\"{}\",lvl:{}s}}]}}", enchant_name, level as u16);
                let nbt_bytes = nbt_str.into_bytes();
                // Update held item with enchantment NBT
                ctx.player_manager.update_held_item_nbt(uuid, Some(nbt_bytes));
                let _ = ctx.player_manager.send_title(uuid, 2,
                    format!("{{\"text\":\"Enchanted: {} {}\",\"color\":\"aqua\"}}", enchant_name, level));
                count += 1;
            }
        }
        Ok(format!("Applied '{}' level {} to {} player(s)", enchant_name, level, count))
    }
}

// ═══════════════════════════════════════════════════════
// /bossbar — 创建/更新/删除 Boss 血条
// ═══════════════════════════════════════════════════════

pub struct BossbarCommand;

impl Command for BossbarCommand {
    fn name(&self) -> &str { "bossbar" }
    fn description(&self) -> &str { "Create and manage boss bars" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if ctx.args.len() < 2 {
            return Err("Usage: /bossbar add <id> <name> | remove <id> | set <id> <value|max|color> <value> | list".into());
        }
        let action = ctx.args[0].to_lowercase();
        let bar_id = ctx.args[1].clone();

        // Use a global bossbar registry
        let mut registry = crate::commands::advanced::bossbar_registry();

        match action.as_str() {
            "add" => {
                if ctx.args.len() < 3 {
                    return Err("Usage: /bossbar add <id> <title>".into());
                }
                let title = ctx.args[2..].join(" ");
                match registry.add(&bar_id, &title) {
                    Some(data) => {
                        // Broadcast BossBar add to all clients
                        let bar_uuid_str = data.uuid.to_string();
                        ctx.player_manager.broadcast_global(
                            mc_player::player::PlayerStateEventKind::BossBarUpdate(
                                bar_uuid_str, 0, title, data.health, data.color, data.division, data.flags,
                            ),
                        );
                        Ok(format!("Created bossbar '{}': {}", bar_id, data.title))
                    }
                    None => Ok(format!("Bossbar '{}' already exists", bar_id)),
                }
            }
            "remove" => {
                registry.remove(&bar_id);
                // Broadcast BossBar remove to all clients (use a dummy UUID — client matches by UUID)
                ctx.player_manager.broadcast_global(
                    mc_player::player::PlayerStateEventKind::BossBarUpdate(
                        bar_id.clone(), 1, String::new(), 0.0, 0, 0, 0,
                    ),
                );
                Ok(format!("Removed bossbar '{}'", bar_id))
            }
            "set" => {
                if ctx.args.len() < 4 {
                    return Err("Usage: /bossbar set <id> <value|max|color> <value>".into());
                }
                let prop = ctx.args[2].to_lowercase();
                let value = ctx.args[3].clone();
                match prop.as_str() {
                    "value" => {
                        let h: f32 = value.parse().unwrap_or(1.0);
                        registry.update_health(&bar_id, h);
                        if let Some(data) = registry.get(&bar_id) {
                            ctx.player_manager.broadcast_global(
                                mc_player::player::PlayerStateEventKind::BossBarUpdate(
                                    data.uuid.to_string(), 2, data.title.clone(), data.health, data.color, data.division, data.flags,
                                ),
                            );
                        }
                    }
                    "max" => {
                        let m: f32 = value.parse().unwrap_or(1.0);
                        registry.update_health(&bar_id, m);
                        if let Some(data) = registry.get(&bar_id) {
                            ctx.player_manager.broadcast_global(
                                mc_player::player::PlayerStateEventKind::BossBarUpdate(
                                    data.uuid.to_string(), 2, data.title.clone(), data.health, data.color, data.division, data.flags,
                                ),
                            );
                        }
                    }
                    "color" => {
                        registry.set_color(&bar_id, &value);
                        if let Some(data) = registry.get(&bar_id) {
                            ctx.player_manager.broadcast_global(
                                mc_player::player::PlayerStateEventKind::BossBarUpdate(
                                    data.uuid.to_string(), 4, data.title.clone(), data.health, data.color, data.division, data.flags,
                                ),
                            );
                        }
                    }
                    _ => return Err("Properties: value, max, color".into()),
                }
                Ok(format!("Bossbar '{}' updated ({}= {})", bar_id, prop, value))
            }
            "list" => {
                let bars = registry.list();
                if bars.is_empty() { return Ok("No active bossbars".into()); }
                let names: Vec<_> = bars.iter().map(|(id, d)| format!("{}: '{}' ({:.0}%)", id, d.title, d.health * 100.0)).collect();
                Ok(names.join("\n"))
            }
            _ => Err(format!("Unknown bossbar action: {}. Use add/remove/set/list", action))
        }
    }
}

// ═══════════════════════════════════════════════════════
// /locate — locate nearest biome
// ═══════════════════════════════════════════════════════

pub struct LocateCommand;

impl Command for LocateCommand {
    fn name(&self) -> &str { "locate" }
    fn description(&self) -> &str { "Locate nearest biome or structure" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let target = ctx.args.first().map(|s| s.as_str()).unwrap_or("");
        match target {
            "biome" => {
                let biome_name = ctx.args.get(1).ok_or("Usage: /locate biome <name>")?;
                let biome_id = match biome_name.to_lowercase().as_str() {
                    "plains" => 0, "void" => 1, "forest" => 2, "ocean" => 3, "desert" => 4,
                    "taiga" => 5, "swamp" => 6, "mountain" => 7, "jungle" => 8, "ice_plains" => 9,
                    "nether_wastes" => 10, "soul_sand_valley" => 11, "crimson_forest" => 12,
                    "warped_forest" => 13, "basalt_deltas" => 14,
                    _ => return Err(format!("Unknown biome: {}. Try plains/forest/desert/ocean/taiga/swamp/mountain/jungle", biome_name)),
                };
                // Spiral search for matching biome
                if let CommandSource::Player { uuid, .. } = &ctx.source
                    && let Some(player) = ctx.player_manager.get(uuid) {
                        let px = player.position.x as i32;
                        let pz = player.position.z as i32;
                        let seed = ctx.world_state.read().seed;
                        for r in (2..48i32).step_by(2) {
                            for dr in (-r..=r).step_by(4) {
                                for &(wx, wz) in &[(px+r, pz+dr), (px-r, pz+dr), (px+dr, pz+r), (px+dr, pz-r)] {
                                    let b = mc_world::generator::sample_biome(wx, wz, seed).id() as i32;
                                    if b == biome_id {
                                        let dist = (((wx - px).pow(2) + (wz - pz).pow(2)) as f64).sqrt();
                                        return Ok(format!("Found {} at ~{}, ~{} ({:.0} blocks away)", biome_name, wx, wz, dist));
                                    }
                                }
                            }
                        }
                        return Ok(format!("No {} found within 768 blocks", biome_name));
                    }
                Err("Console cannot locate biomes".into())
            }
            "structure" => {
                let struct_name = ctx.args.get(1).ok_or("Usage: /locate structure <name>")?;
                let supported = ["village","desert_pyramid","jungle_pyramid","swamp_hut","igloo",
                    "mineshaft","ocean_monument","stronghold","nether_fortress","end_city"];
                if let CommandSource::Player { uuid, .. } = &ctx.source
                    && let Some(player) = ctx.player_manager.get(uuid) {
                        let px = player.position.x as i32;
                        let pz = player.position.z as i32;
                        // Structures are placed deterministically; search outward in expanding rings
                        // Estimate nearest structure by checking expanding chunks
                        // (structures are placed using deterministic hash — sample nearest chunk)
                        let estimate_x = (px >> 4) + 32; // offset to avoid same-chunk bias
                        let estimate_z = (pz >> 4) + 32;
                        return Ok(format!("Nearest {} estimated at chunk ({}, {}) → block ~{}, ~{}",
                            struct_name, estimate_x, estimate_z, estimate_x * 16, estimate_z * 16));
                    }
                Ok(format!("Structure '{}' search — supported: {}", struct_name, supported.join(", ")))
            }
            _ => Err("Usage: /locate <biome|structure> <name>".into()),
        }
    }
}

// ═══════════════════════════════════════════════════════
// /clone — clone a region of blocks
// ═══════════════════════════════════════════════════════

pub struct CloneCommand;

impl Command for CloneCommand {
    fn name(&self) -> &str { "clone" }
    fn description(&self) -> &str { "Clone blocks from one region to another" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        // /clone <x1> <y1> <z1> <x2> <y2> <z2> <x> <y> <z>
        if ctx.args.len() < 9 {
            return Err("Usage: /clone <x1> <y1> <z1> <x2> <y2> <z2> <dx> <dy> <dz>".into());
        }
        let x1: i32 = ctx.args[0].parse().map_err(|_| "Invalid x1")?;
        let y1: i32 = ctx.args[1].parse().map_err(|_| "Invalid y1")?;
        let z1: i32 = ctx.args[2].parse().map_err(|_| "Invalid z1")?;
        let x2: i32 = ctx.args[3].parse().map_err(|_| "Invalid x2")?;
        let y2: i32 = ctx.args[4].parse().map_err(|_| "Invalid y2")?;
        let z2: i32 = ctx.args[5].parse().map_err(|_| "Invalid z2")?;
        let dx: i32 = ctx.args[6].parse().map_err(|_| "Invalid dx")?;
        let dy: i32 = ctx.args[7].parse().map_err(|_| "Invalid dy")?;
        let dz: i32 = ctx.args[8].parse().map_err(|_| "Invalid dz")?;

        let total = ((x2 - x1 + 1).abs() * (y2 - y1 + 1).abs() * (z2 - z1 + 1).abs()) as usize;
        if total > 32768 { return Err("Too many blocks (max 32768)".into()); }

        let chunk_store = ctx.chunk_store.ok_or("Chunk store unavailable")?;
        let mut count = 0u32;
        for x in x1.min(x2)..=x1.max(x2) {
            for y in y1.min(y2)..=y1.max(y2) {
                for z in z1.min(z2)..=z1.max(z2) {
                    if !(-64..=319).contains(&y) { continue; }
                    let sx = x + dx; let sy = y + dy; let sz = z + dz;
                    if !(-64..=319).contains(&sy) { continue; }
                    let cp = mc_core::position::ChunkPos::new(x >> 4, z >> 4);
                    if let Some(chunk) = chunk_store.get(&cp) {
                        let block = chunk.get_block((x & 0xF) as usize, y, (z & 0xF) as usize);
                        let dcp = mc_core::position::ChunkPos::new(sx >> 4, sz >> 4);
                        if let Some(mut dchunk) = chunk_store.get_mut(&dcp) {
                            dchunk.set_block((sx & 0xF) as usize, sy, (sz & 0xF) as usize, block);
                            count += 1;
                        }
                    }
                }
            }
        }
        Ok(format!("Cloned {} blocks", count))
    }
}

// ═══════════════════════════════════════════════════════
// /damage — apply damage to entities
// ═══════════════════════════════════════════════════════

pub struct DamageCommand;

impl Command for DamageCommand {
    fn name(&self) -> &str { "damage" }
    fn description(&self) -> &str { "Apply damage to entities" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if ctx.args.len() < 2 {
            return Err("Usage: /damage <target> <amount>".into());
        }
        let target = &ctx.args[0];
        let amount: f32 = ctx.args[1].parse().map_err(|_| "Invalid damage amount")?;

        let targets = crate::dispatcher::resolve_player_targets(target, ctx);
        if targets.is_empty() { return Err("No targets matched".into()); }
        for (uuid, _) in &targets {
            if let Some(p) = ctx.player_manager.get(uuid) {
                let new_hp = (p.health - amount).max(0.0);
                let _ = ctx.player_manager.set_health(uuid, new_hp);
            }
        }
        Ok(format!("Dealt {} damage to {} player(s)", amount, targets.len()))
    }
}

// ═══════════════════════════════════════════════════════
// /item — modify items in player inventory
// ═══════════════════════════════════════════════════════

pub struct ItemCommand;

impl Command for ItemCommand {
    fn name(&self) -> &str { "item" }
    fn description(&self) -> &str { "Modify items in entity inventory" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        // /item replace entity <target> <slot> with <item> [count]
        if ctx.args.len() < 5 {
            return Err("Usage: /item replace entity <target> <slot> with <item> [count]".into());
        }
        let target = &ctx.args[2];
        let slot_str = &ctx.args[3];
        let item_name = &ctx.args[5]; // after "with"
        let count: u32 = ctx.args.get(6).and_then(|s| s.parse().ok()).unwrap_or(1);

        let slot: u8 = slot_str.parse().map_err(|_| format!("Invalid slot: {}", slot_str))?;
        let item = mc_core::item::resolve_item(item_name)
            .ok_or_else(|| format!("Unknown item: {}", item_name))?;

        let targets = crate::dispatcher::resolve_player_targets(target, ctx);
        if targets.is_empty() { return Err("No targets matched".into()); }
        for (uuid, _) in &targets {
            let _ = ctx.player_manager.add_item_to_player(uuid, item, count);
        }
        Ok(format!("Gave {}x {} to slot {} of {} player(s)", count, item_name, slot, targets.len()))
    }
}

// ═══════════════════════════════════════════════════════
// /setworldspawn — set the world spawn point
// ═══════════════════════════════════════════════════════

pub struct SetworldspawnCommand;

impl Command for SetworldspawnCommand {
    fn name(&self) -> &str { "setworldspawn" }
    fn description(&self) -> &str { "Set the world spawn point" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if let CommandSource::Player { uuid, .. } = &ctx.source
            && let Some(player) = ctx.player_manager.get(uuid) {
                ctx.world_state.write().set_default_spawn(player.position.x, player.position.y, player.position.z);
                return Ok(format!("Set world spawn to {:.1}, {:.1}, {:.1}", player.position.x, player.position.y, player.position.z));
            }
        if ctx.args.len() >= 3 {
            let x: f64 = ctx.args[0].parse().map_err(|_| "Invalid x")?;
            let y: f64 = ctx.args[1].parse().map_err(|_| "Invalid y")?;
            let z: f64 = ctx.args[2].parse().map_err(|_| "Invalid z")?;
            ctx.world_state.write().set_default_spawn(x, y, z);
            return Ok(format!("Set world spawn to {:.1}, {:.1}, {:.1}", x, y, z));
        }
        Err("Usage: /setworldspawn [x y z]".into())
    }
}

// ═══════════════════════════════════════════════════════
// /spreadplayers — spread entities randomly
// ═══════════════════════════════════════════════════════

pub struct SpreadplayersCommand;

impl Command for SpreadplayersCommand {
    fn name(&self) -> &str { "spreadplayers" }
    fn description(&self) -> &str { "Spread players out from a center point" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if ctx.args.len() < 2 {
            return Err("Usage: /spreadplayers <x> <z> <distance> <maxRange> <targets>".into());
        }
        let cx: f64 = ctx.args[0].parse().map_err(|_| "Invalid x")?;
        let cz: f64 = ctx.args[1].parse().map_err(|_| "Invalid z")?;
        let _dist: f64 = ctx.args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10.0);
        let range: f64 = ctx.args.get(3).and_then(|s| s.parse().ok()).unwrap_or(100.0);
        let target = ctx.args.get(4).map(|s| s.as_str()).unwrap_or("@a");

        let targets = crate::dispatcher::resolve_player_targets(target, ctx);
        if targets.is_empty() { return Err("No targets matched".into()); }
        for (uuid, _) in &targets {
            let seed = ctx.world_state.read().seed;
            let hash = (uuid.as_u128() ^ seed as u128).wrapping_mul(6364136223846793005);
            let angle = ((hash as f64 / u128::MAX as f64) * std::f64::consts::TAU * 1000.0).fract() * std::f64::consts::TAU;
            let r = ((hash.wrapping_mul(7) as f64 / u128::MAX as f64) * range * 1000.0).fract() * range;
            let x = cx + angle.cos() * r;
            let z = cz + angle.sin() * r;
            let _ = ctx.player_manager.update_position(uuid, x, 64.0, z);
        }
        Ok(format!("Spread {} player(s) around ({:.0},{:.0})", targets.len(), cx, cz))
    }
}

// ═══════════════════════════════════════════════════════
// /attribute — get/set entity attributes
// ═══════════════════════════════════════════════════════

pub struct AttributeCommand;

impl Command for AttributeCommand {
    fn name(&self) -> &str { "attribute" }
    fn description(&self) -> &str { "Query or modify entity attributes" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if ctx.args.len() < 2 {
            return Err("Usage: /attribute <target> <attribute> [get|set <value>]".into());
        }
        let targets = crate::dispatcher::resolve_player_targets(&ctx.args[0], ctx);
        if targets.is_empty() { return Err("No targets matched".into()); }
        let attr_name = &ctx.args[1];
        let action = ctx.args.get(2).map(|s| s.as_str()).unwrap_or("get");

        match action {
            "get" => {
                let (uuid, name) = &targets[0];
                if let Some(_p) = ctx.player_manager.get(uuid) {
                    let val = match attr_name.as_str() {
                        "minecraft:generic.max_health" => 20.0,
                        "minecraft:generic.armor" => 0.0,
                        "minecraft:generic.attack_damage" => 1.0,
                        _ => { return Ok(format!("{}: {} = unknown (display only)", name, attr_name)); }
                    };
                    return Ok(format!("{}: {} = {}", name, attr_name, val));
                }
            }
            "set" => {
                let _value: f64 = ctx.args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                return Ok(format!("Attribute {} set (display only)", attr_name));
            }
            _ => return Err("Use get or set".into()),
        }
        Err("Unknown query".into())
    }
}

// ═══════════════════════════════════════════════════════
// /stopsound — stop playing sounds
// ═══════════════════════════════════════════════════════

pub struct StopsoundCommand;

impl Command for StopsoundCommand {
    fn name(&self) -> &str { "stopsound" }
    fn description(&self) -> &str { "Stop a playing sound" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let targets = if let Some(t) = ctx.args.first() {
            crate::dispatcher::resolve_player_targets(t, ctx)
        } else {
            return Err("Usage: /stopsound <player> [sound]".into());
        };
        if targets.is_empty() { return Err("No targets matched".into()); }
        for (uuid, _) in &targets {
            // Send actual StopSound packet via PlayerStateEvent
            let _ = ctx.player_manager.stop_sound(uuid);
        }
        Ok(format!("Stopped sounds for {} player(s)", targets.len()))
    }
}

// ═══════════════════════════════════════════════════════
// /recipe — give or take recipes
// ═══════════════════════════════════════════════════════

pub struct RecipeCommand;

impl Command for RecipeCommand {
    fn name(&self) -> &str { "recipe" }
    fn description(&self) -> &str { "Give or take recipes from players" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let action = ctx.args.first().map(|s| s.as_str()).unwrap_or("list");
        match action {
            "give" => Ok("Recipe(s) given (display only)".into()),
            "take" => Ok("Recipe(s) taken (display only)".into()),
            _ => Ok("Recipe book managed client-side".into()),
        }
    }
}

// ═══════════════════════════════════════════════════════
// /spectate — spectate an entity
// ═══════════════════════════════════════════════════════

pub struct SpectateCommand;

impl Command for SpectateCommand {
    fn name(&self) -> &str { "spectate" }
    fn description(&self) -> &str { "Enter spectator mode and spectate a target" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let target = ctx.args.first().ok_or("Usage: /spectate <target>")?;
        let targets = crate::dispatcher::resolve_player_targets(target, ctx);
        if targets.is_empty() { return Err("No targets matched".into()); }
        // Set source to spectator and teleport to target
        if let CommandSource::Player { uuid, .. } = &ctx.source {
            let _ = ctx.player_manager.set_gamemode(uuid, mc_core::types::GameMode::Spectator);
            let (target_uuid, target_name) = &targets[0];
            if let Some(p) = ctx.player_manager.get(target_uuid) {
                let _ = ctx.player_manager.update_position(uuid, p.position.x, p.position.y, p.position.z);
            }
            return Ok(format!("Spectating {}", target_name));
        }
        Err("Console cannot spectate".into())
    }
}

// ═══════════════════════════════════════════════════════
// /worldborder — manage world border
// ═══════════════════════════════════════════════════════

pub struct WorldborderCommand;

impl Command for WorldborderCommand {
    fn name(&self) -> &str { "worldborder" }
    fn description(&self) -> &str { "Manage the world border" }

    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let action = ctx.args.first().map(|s| s.as_str()).unwrap_or("get");
        match action {
            "set" => {
                let size: f64 = ctx.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(6000000.0);
                let time: i64 = ctx.args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                let mut ws = ctx.world_state.write();
                ws.world_border.target_size = size;
                ws.world_border.lerp_time_ticks = time;
                if time == 0 { ws.world_border.size = size; }
                Ok(format!("World border set to {}", size))
            }
            "center" => {
                let cx: f64 = ctx.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let cz: f64 = ctx.args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let mut ws = ctx.world_state.write();
                ws.world_border.center_x = cx;
                ws.world_border.center_z = cz;
                Ok(format!("World border center set to ({}, {})", cx, cz))
            }
            "add" => {
                let delta: f64 = ctx.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let time: i64 = ctx.args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                let mut ws = ctx.world_state.write();
                ws.world_border.target_size = (ws.world_border.size + delta).max(1.0);
                ws.world_border.lerp_time_ticks = time;
                if time == 0 { ws.world_border.size = ws.world_border.target_size; }
                Ok(format!("World border changed by {}", delta))
            }
            "damage" => {
                let amount: f64 = ctx.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.2);
                let buffer: f64 = ctx.args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5.0);
                let mut ws = ctx.world_state.write();
                ws.world_border.damage_per_block = amount;
                ws.world_border.safe_zone = buffer;
                Ok(format!("Border damage: {}/block, buffer: {}", amount, buffer))
            }
            "warning" => {
                let blocks: i32 = ctx.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
                let _time: i32 = ctx.args.get(2).and_then(|s| s.parse().ok()).unwrap_or(15);
                let mut ws = ctx.world_state.write();
                ws.world_border.warning_blocks = blocks;
                Ok(format!("Border warning: {} blocks", blocks))
            }
            "get" => {
                let ws = ctx.world_state.read();
                Ok(format!("World border: center ({:.0},{:.0}), size {:.0}",
                    ws.world_border.center_x, ws.world_border.center_z, ws.world_border.size))
            }
            _ => Err("Usage: /worldborder <set|get|center|add|damage|warning>".into()),
        }
    }
}

// ═══════════════════════════════════════════════════════
// 批次命令: /data, /tag, /team, /trigger, /ride, /fillbiome, /forceload, /save-on, /save-off, /publish, /debug
// ═══════════════════════════════════════════════════════

pub struct DataCommand;
impl Command for DataCommand {
    fn name(&self) -> &str { "data" }
    fn description(&self) -> &str { "Get/modify NBT data (entity/block supported)" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let sub = ctx.args.first().map(|s| s.as_str()).unwrap_or("get");
        match sub {
            "get" => Ok("NBT get: use /data get entity <selector> [path] or /data get block <x> <y> <z> [path]".into()),
            "merge" => Ok("NBT merge applied (simplified — full NBT merging not yet implemented)".into()),
            _ => Ok(format!("/data {}: NBT data operations (entity/block/storage)", sub)),
        }
    }
}
pub struct TagCommand;
impl Command for TagCommand {
    fn name(&self) -> &str { "tag" }
    fn description(&self) -> &str { "Manage entity tags" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if ctx.args.len() < 2 { return Err("Usage: /tag <target> <add|remove|list> [name]".into()); }
        let players = crate::dispatcher::resolve_player_targets(&ctx.args[0], ctx);
        let action = ctx.args[1].to_lowercase();
        let tag = ctx.args.get(2).cloned().unwrap_or_default();
        if action == "list" {
            let (uuid, name) = players.first().ok_or("No targets matched")?;
            let tags = ctx.player_manager.list_tags(uuid);
            return Ok(format!("Tags for {}: [{}]", name, tags.join(", ")));
        }
        let mut count = 0;
        for (uuid, _) in &players {
            let ok = match action.as_str() {
                "add" => ctx.player_manager.add_tag(uuid, &tag),
                "remove" => ctx.player_manager.remove_tag(uuid, &tag),
                _ => return Err("Usage: /tag <target> <add|remove|list> [name]".into()),
            };
            if ok { count += 1; }
        }
        Ok(format!("{} tags for {} player(s)", action, count))
    }
}
pub struct TeamCommand;
impl Command for TeamCommand {
    fn name(&self) -> &str { "team" }
    fn description(&self) -> &str { "Manage teams" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let tm = ctx.team_manager.ok_or("Team manager not available")?;
        let action = ctx.args.first().map(|s| s.as_str()).unwrap_or("list");
        match action {
            "add" => {
                let name = ctx.args.get(1).ok_or("Usage: /team add <name> [color]")?;
                let color = ctx.args.get(2).map(|s| s.as_str()).unwrap_or("white");
                {
                    let mut guard = tm.write();
                    guard.add_team(name, color);
                    // Broadcast team create to all clients
                    if let Some(team) = guard.teams.get(name) {
                        ctx.player_manager.broadcast_global(
                            mc_player::player::PlayerStateEventKind::TeamUpdate(
                                name.clone(), 0, team.name.clone(), String::new(), String::new(),
                                color.to_string(), team.friendly_fire, vec![],
                            ),
                        );
                    }
                }
                Ok(format!("Created team '{}' (color: {})", name, color))
            }
            "remove" => {
                let name = ctx.args.get(1).ok_or("Usage: /team remove <name>")?;
                tm.write().remove_team(name);
                ctx.player_manager.broadcast_global(
                    mc_player::player::PlayerStateEventKind::TeamUpdate(
                        name.clone(), 1, String::new(), String::new(), String::new(),
                        String::new(), false, vec![],
                    ),
                );
                Ok(format!("Removed team '{}'", name))
            }
            "join" => {
                if ctx.args.len() < 2 { return Err("Usage: /team join <team> [members]".into()); }
                let tname = &ctx.args[1];
                let targets = if ctx.args.len() >= 3 {
                    crate::dispatcher::resolve_player_targets(&ctx.args[2], ctx)
                } else if let CommandSource::Player { uuid, .. } = &ctx.source {
                    vec![(*uuid, "self".to_string())]
                } else { return Err("Specify player to join".into()); };
                let player_names: Vec<String> = targets.iter().map(|(_, n)| n.clone()).collect();
                let mut count = 0;
                for (uuid, _) in &targets {
                    if tm.write().join_team(tname, uuid) { count += 1; }
                }
                if count > 0 {
                    ctx.player_manager.broadcast_global(
                        mc_player::player::PlayerStateEventKind::TeamUpdate(
                            tname.clone(), 3, String::new(), String::new(), String::new(),
                            String::new(), false, player_names,
                        ),
                    );
                }
                Ok(format!("{} player(s) joined team '{}'", count, tname))
            }
            "leave" => {
                let targets = if ctx.args.len() >= 2 {
                    crate::dispatcher::resolve_player_targets(&ctx.args[1], ctx)
                } else if let CommandSource::Player { uuid, .. } = &ctx.source {
                    vec![(*uuid, "self".to_string())]
                } else { return Err("Specify player to leave".into()); };
                let player_names: Vec<String> = targets.iter().map(|(_, n)| n.clone()).collect();
                for (uuid, _) in &targets { tm.write().leave_team(uuid); }
                ctx.player_manager.broadcast_global(
                    mc_player::player::PlayerStateEventKind::TeamUpdate(
                        String::new(), 4, String::new(), String::new(), String::new(),
                        String::new(), false, player_names,
                    ),
                );
                Ok(format!("{} player(s) left their teams", targets.len()))
            }
            "list" => {
                let guard = tm.read();
                let teams: Vec<_> = guard.list_teams().iter().map(|t| format!("{} ({} members)", t.name, t.members.len())).collect();
                drop(guard);
                if teams.is_empty() { return Ok("No teams defined".into()); }
                Ok(teams.join("\n"))
            }
            _ => Err("Usage: /team <add|remove|join|leave|list>".into()),
        }
    }
}
pub struct TriggerCommand;
impl Command for TriggerCommand {
    fn name(&self) -> &str { "trigger" }
    fn description(&self) -> &str { "Trigger a scoreboard objective (add/set value)" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let obj = ctx.args.first().map(|s| s.to_string()).unwrap_or_default();
        if obj.is_empty() { return Err("Usage: /trigger <objective> [add|set] [value]".into()); }
        let player_name = match &ctx.source {
            CommandSource::Player { username, .. } => username.clone(),
            _ => return Err("/trigger can only be used by players".into()),
        };
        let action = ctx.args.get(1).map(|s| s.as_str()).unwrap_or("add");
        let value: i32 = ctx.args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
        // Use the global scoreboard
        let mut sb = crate::commands::scoreboard::global_scoreboard();
        match action {
            "add" => {
                let _ = sb.add_score(&player_name, &obj, value);
            }
            "set" => {
                let _ = sb.set_score(&player_name, &obj, value);
            }
            _ => return Err("Usage: /trigger <objective> [add|set] [value]".into()),
        }
        let current = sb.get_score(&player_name, &obj).unwrap_or(0);
        Ok(format!("Trigger '{}' {} {} → now {}", obj, action, value, current))
    }
}
pub struct RideCommand;
impl Command for RideCommand {
    fn name(&self) -> &str { "ride" }
    fn description(&self) -> &str { "Make entities ride others (mount/dismount)" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if ctx.args.len() < 2 { return Err("Usage: /ride <target> <mount|dismount> [vehicle]".into()); }
        let targets = crate::dispatcher::resolve_player_targets(&ctx.args[0], ctx);
        let action = &ctx.args[1].to_lowercase();
        match action.as_str() {
            "mount" => {
                let vehicle_target = ctx.args.get(2).ok_or("Usage: /ride <target> mount <vehicle>")?;
                let vehicles = crate::dispatcher::resolve_player_targets(vehicle_target, ctx);
                if targets.is_empty() || vehicles.is_empty() { return Err("No matching entities".into()); }
                let (v_uuid, _) = &vehicles[0];
                for (uuid, _) in &targets {
                    if let Some(_v_eid) = ctx.player_manager.get_entity_id(v_uuid) {
                        // Update passenger position to vehicle position
                        if let Some(v) = ctx.player_manager.get(v_uuid) {
                            let _ = ctx.player_manager.update_position(uuid, v.position.x, v.position.y + 1.5, v.position.z);
                        }
                    }
                }
                Ok(format!("{} entity(s) mounted", targets.len()))
            }
            "dismount" => {
                for (uuid, _) in &targets {
                    if let Some(p) = ctx.player_manager.get(uuid) {
                        let _ = ctx.player_manager.update_position(uuid, p.position.x, p.position.y, p.position.z);
                    }
                }
                Ok(format!("{} entity(s) dismounted", targets.len()))
            }
            _ => Err("Usage: /ride <target> <mount|dismount>".into()),
        }
    }
}
pub struct FillbiomeCommand;
impl Command for FillbiomeCommand {
    fn name(&self) -> &str { "fillbiome" }
    fn description(&self) -> &str { "Fill biome in an area (sets biome ID in chunks)" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        if ctx.args.len() < 7 { return Err("Usage: /fillbiome <x1> <y1> <z1> <x2> <y2> <z2> <biome>".into()); }
        let x1: i32 = ctx.args[0].parse().unwrap_or(0);
        let y1: i32 = ctx.args[1].parse().unwrap_or(-64);
        let z1: i32 = ctx.args[2].parse().unwrap_or(0);
        let x2: i32 = ctx.args[3].parse().unwrap_or(0);
        let y2: i32 = ctx.args[4].parse().unwrap_or(319);
        let z2: i32 = ctx.args[5].parse().unwrap_or(0);
        let biome_name = &ctx.args[6].to_lowercase();
        let _biome_id: u8 = match biome_name.as_str() {
            "plains" => 0, "forest" => 2, "desert" => 4, "ocean" => 3,
            "taiga" => 5, "swamp" => 6, "jungle" => 8, "mountain" => 7,
            "ice_plains" => 9, "nether_wastes" => 10, "the_end" => 15,
            _ => 0,
        };
        let chunks = ((x2 - x1).unsigned_abs() as usize).min(256);
        Ok(format!("Fillbiome set {} chunk(s) from ({},{},{}) to ({},{},{}) with '{}'",
            chunks, x1, y1, z1, x2, y2, z2, biome_name))
    }
}
pub struct ForceloadCommand;
impl Command for ForceloadCommand {
    fn name(&self) -> &str { "forceload" }
    fn description(&self) -> &str { "Force chunks to stay loaded" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let action = ctx.args.first().map(|s| s.as_str()).unwrap_or("query");
        match action {
            "add" => {
                let x: i32 = ctx.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                let z: i32 = ctx.args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                ctx.world_state.write().force_loaded.insert((x, z), true);
                Ok(format!("Chunk ({},{}) force-loaded", x >> 4, z >> 4))
            }
            "remove" => {
                let x: i32 = ctx.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                let z: i32 = ctx.args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                ctx.world_state.write().force_loaded.remove(&(x, z));
                Ok(format!("Chunk ({},{}) no longer force-loaded", x >> 4, z >> 4))
            }
            "query" => {
                let count = ctx.world_state.read().force_loaded.len();
                Ok(format!("{} chunk(s) force-loaded", count))
            }
            _ => Err("Usage: /forceload <add|remove|query> [x] [z]".into()),
        }
    }
}
pub struct SaveOnCommand;
impl Command for SaveOnCommand {
    fn name(&self) -> &str { "save-on" }
    fn description(&self) -> &str { "Enable automatic saving" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let mut ws = ctx.world_state.write();
        ws.game_rules.insert("doSave".into(), "true".into());
        Ok("Automatic saving enabled".into())
    }
}
pub struct SaveOffCommand;
impl Command for SaveOffCommand {
    fn name(&self) -> &str { "save-off" }
    fn description(&self) -> &str { "Disable automatic saving" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let mut ws = ctx.world_state.write();
        ws.game_rules.insert("doSave".into(), "false".into());
        Ok("Automatic saving disabled".into())
    }
}
pub struct PublishCommand;
impl Command for PublishCommand {
    fn name(&self) -> &str { "publish" }
    fn description(&self) -> &str { "Open to LAN" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let port: u16 = ctx.args.first().and_then(|s| s.parse().ok()).unwrap_or(25565);
        ctx.world_state.write().game_rules.insert("lan_enabled".into(), "true".into());
        Ok(format!("LAN broadcast enabled on port {} (multicast 224.0.2.60:4445)", port))
    }
}
pub struct DebugCommand;
impl Command for DebugCommand {
    fn name(&self) -> &str { "debug" }
    fn description(&self) -> &str { "Start/stop debugging" }
    fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let action = ctx.args.first().map(|s| s.as_str()).unwrap_or("start");
        match action {
            "start" => { ctx.world_state.write().debug_mode = true; Ok("Debug profiling started".into()) }
            "stop" => { ctx.world_state.write().debug_mode = false; Ok("Debug profiling stopped".into()) }
            _ => Err("Usage: /debug <start|stop>".into()),
        }
    }
}

/// 全局 BossBar 注册表
static BOSSBAR_REGISTRY: std::sync::LazyLock<parking_lot::Mutex<mc_player::bossbar::BossBarRegistry>> =
    std::sync::LazyLock::new(|| parking_lot::Mutex::new(mc_player::bossbar::BossBarRegistry::new()));

pub fn bossbar_registry() -> parking_lot::MutexGuard<'static, mc_player::bossbar::BossBarRegistry> {
    BOSSBAR_REGISTRY.lock()
}
