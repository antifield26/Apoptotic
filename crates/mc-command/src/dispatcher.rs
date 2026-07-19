//! 命令分发器 — Brigadier 风格
//!
//! 支持命令注册、参数解析、权限检查。

use mc_core::world_state::SharedWorldState;
use mc_player::player::SharedPlayerManager;
use std::collections::HashMap;
use tokio::sync::broadcast;
use tracing;

/// 命令执行上下文
pub struct CommandContext<'a> {
    pub source: CommandSource,
    pub args: Vec<String>,
    pub player_manager: &'a SharedPlayerManager,
    pub shutdown_tx: &'a broadcast::Sender<()>,
    pub world_state: &'a SharedWorldState,
    /// MOTD for status/say commands
    pub motd: &'a str,
    pub max_players: u32,
    /// Chunk store for block manipulation commands
    pub chunk_store: Option<&'a mc_world::chunk_store::ChunkStore>,
    /// Manual save trigger
    pub save_trigger: Option<&'a broadcast::Sender<()>>,
    /// Command dispatcher (for /execute to re-dispatch subcommands)
    pub dispatcher: Option<&'a CommandDispatcher>,
    /// Mob manager (for /summon)
    pub mob_manager: Option<&'a mc_player::mob::MobManager>,
    /// Team manager (for /team)
    pub team_manager: Option<&'a std::sync::Arc<parking_lot::RwLock<mc_core::team::TeamManager>>>,
}

/// 命令来源
#[derive(Debug, Clone)]
pub enum CommandSource {
    Player { uuid: uuid::Uuid, username: String },
    Console,
    Rcon,
}

impl CommandSource {
    pub fn has_permission(&self, player_manager: &SharedPlayerManager) -> bool {
        match self {
            CommandSource::Console | CommandSource::Rcon => true,
            CommandSource::Player { uuid, .. } => {
                player_manager.get(uuid).is_some_and(|p| p.is_op)
            }
        }
    }

    pub fn name(&self) -> String {
        match self {
            CommandSource::Player { username, .. } => username.clone(),
            CommandSource::Console => "Console".into(),
            CommandSource::Rcon => "RCON".into(),
        }
    }

    pub fn player(name: &str, uuid: uuid::Uuid) -> Self {
        CommandSource::Player {
            uuid,
            username: name.to_string(),
        }
    }
}

/// 命令 trait
pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str { "No description" }
    fn aliases(&self) -> &[&str] { &[] }
    fn execute(&self, ctx: &CommandContext) -> CommandResult;
}

pub type CommandResult = Result<String, String>;

/// 选择器参数 (解析自 @a[arg=val,...] 语法)
#[derive(Debug, Clone, Default)]
struct SelectorArgs {
    distance_min: Option<f64>,
    distance_max: Option<f64>,
    limit: Option<usize>,
    sort: SortOrder,
    name_filter: Option<String>,
    gamemode_filter: Option<mc_core::types::GameMode>,
    level_min: Option<i32>,
    level_max: Option<i32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum SortOrder {
    #[default]
    Arbitrary,
    Nearest,
    Furthest,
    Random,
}

/// 解析选择器参数字符串: `[key=value,key=value,...]`
fn parse_selector_args(args_str: &str) -> Result<SelectorArgs, String> {
    let mut args = SelectorArgs::default();
    // args_str is like "distance=..10,limit=3,sort=nearest"
    for part in args_str.split(',') {
        let part = part.trim();
        if part.is_empty() { continue; }
        let (key, value) = part.split_once('=')
            .ok_or_else(|| format!("Invalid selector argument: {}", part))?;
        let key = key.trim();
        let value = value.trim();

        match key {
            "distance" => {
                if let Some((min_str, max_str)) = value.split_once("..") {
                    if !min_str.is_empty() {
                        args.distance_min = Some(min_str.parse::<f64>()
                            .map_err(|_| format!("Invalid distance min: {}", min_str))?);
                    }
                    if !max_str.is_empty() {
                        args.distance_max = Some(max_str.parse::<f64>()
                            .map_err(|_| format!("Invalid distance max: {}", max_str))?);
                    }
                } else {
                    return Err("distance requires range like ..10 or 5..20".into());
                }
            }
            "limit" | "c" => {
                args.limit = Some(value.parse::<usize>()
                    .map_err(|_| format!("Invalid limit: {}", value))?);
            }
            "sort" => {
                args.sort = match value {
                    "nearest" => SortOrder::Nearest,
                    "furthest" => SortOrder::Furthest,
                    "random" => SortOrder::Random,
                    "arbitrary" => SortOrder::Arbitrary,
                    _ => return Err(format!("Unknown sort order: {}. Use nearest, furthest, random, or arbitrary", value)),
                };
            }
            "name" => {
                args.name_filter = Some(value.to_string());
            }
            "gamemode" | "m" => {
                args.gamemode_filter = Some(crate::parser::parse_gamemode(value)?);
            }
            "level" | "l" => {
                if let Some((min_str, max_str)) = value.split_once("..") {
                    if !min_str.is_empty() {
                        args.level_min = Some(min_str.parse::<i32>()
                            .map_err(|_| format!("Invalid level min: {}", min_str))?);
                    }
                    if !max_str.is_empty() {
                        args.level_max = Some(max_str.parse::<i32>()
                            .map_err(|_| format!("Invalid level max: {}", max_str))?);
                    }
                } else {
                    let lvl = value.parse::<i32>()
                        .map_err(|_| format!("Invalid level: {}", value))?;
                    args.level_min = Some(lvl);
                    args.level_max = Some(lvl);
                }
            }
            _ => {
                // Unknown args are silently ignored (for forward compatibility)
                tracing::debug!("Unknown selector argument: {}", key);
            }
        }
    }
    Ok(args)
}

/// 解析目标选择器 (含 @selector 和 [args] 语法)
fn parse_target_with_args(target: &str) -> (&str, Option<&str>) {
    if let Some(bracket_start) = target.find('[')
        && target.ends_with(']') {
            let selector = &target[..bracket_start];
            let args = &target[bracket_start + 1..target.len() - 1];
            return (selector, Some(args));
        }
    (target, None)
}

/// 目标选择器: 解析 @a/@p/@r/@s/@e 及参数筛选
pub fn resolve_player_targets(target: &str, ctx: &CommandContext) -> Vec<(uuid::Uuid, String)> {
    let (selector, args_str) = parse_target_with_args(target);
    let args = args_str.map(|s| parse_selector_args(s).unwrap_or_default()).unwrap_or_default();

    // Get reference position for distance calculations
    let (ref_x, ref_y, ref_z) = if let CommandSource::Player { uuid, .. } = &ctx.source {
        ctx.player_manager.get(uuid)
            .map(|p| (p.position.x, p.position.y, p.position.z))
            .unwrap_or((0.0, 64.0, 0.0))
    } else {
        (0.0, 64.0, 0.0)
    };

    // Collect candidates based on selector type
    let candidates: Vec<(uuid::Uuid, String)> = match selector {
        "@a" | "@e" => {
            ctx.player_manager.all_players()
                .into_iter()
                .map(|p| (p.uuid, p.username))
                .collect()
        }
        "@p" => {
            ctx.player_manager.nearest_player(ref_x, ref_y, ref_z, None)
                .map(|p| vec![(p.uuid, p.username)])
                .unwrap_or_default()
        }
        "@r" => {
            let players = ctx.player_manager.all_players();
            if players.is_empty() {
                vec![]
            } else {
                let idx = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos() as usize % players.len())
                    .unwrap_or(0);
                vec![(players[idx].uuid, players[idx].username.clone())]
            }
        }
        "@s" => {
            if let CommandSource::Player { uuid, ref username } = ctx.source {
                vec![(uuid, username.clone())]
            } else {
                vec![]
            }
        }
        name => {
            ctx.player_manager.get_by_name(name)
                .map(|p| vec![(p.uuid, p.username)])
                .unwrap_or_default()
        }
    };

    // Apply filters and sorting
    apply_selector_filters(candidates, &args, ctx, ref_x, ref_y, ref_z)
}

/// 对候选目标应用筛选和排序
fn apply_selector_filters(
    mut candidates: Vec<(uuid::Uuid, String)>,
    args: &SelectorArgs,
    ctx: &CommandContext,
    ref_x: f64, ref_y: f64, ref_z: f64,
) -> Vec<(uuid::Uuid, String)> {
    // Filter by distance
    if args.distance_min.is_some() || args.distance_max.is_some() {
        candidates.retain(|(uuid, _)| {
            if let Some(p) = ctx.player_manager.get(uuid) {
                let dx = p.position.x - ref_x;
                let dy = p.position.y - ref_y;
                let dz = p.position.z - ref_z;
                let dist_sq = dx * dx + dy * dy + dz * dz;
                let dist = dist_sq.sqrt();
                if let Some(min) = args.distance_min && dist < min { return false; }
                if let Some(max) = args.distance_max && dist > max { return false; }
                true
            } else {
                false
            }
        });
    }

    // Filter by name
    if let Some(ref name) = args.name_filter {
        candidates.retain(|(_, n)| n.eq_ignore_ascii_case(name));
    }

    // Filter by gamemode
    if let Some(gm) = args.gamemode_filter {
        candidates.retain(|(uuid, _)| {
            ctx.player_manager.get(uuid).is_some_and(|p| p.gamemode == gm)
        });
    }

    // Sort
    match args.sort {
        SortOrder::Nearest => {
            candidates.sort_by(|(a_id, _), (b_id, _)| {
                let da = ctx.player_manager.get(a_id).map(|p| {
                    let dx = p.position.x - ref_x;
                    let dy = p.position.y - ref_y;
                    let dz = p.position.z - ref_z;
                    dx * dx + dy * dy + dz * dz
                }).unwrap_or(f64::MAX);
                let db = ctx.player_manager.get(b_id).map(|p| {
                    let dx = p.position.x - ref_x;
                    let dy = p.position.y - ref_y;
                    let dz = p.position.z - ref_z;
                    dx * dx + dy * dy + dz * dz
                }).unwrap_or(f64::MAX);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        SortOrder::Furthest => {
            candidates.sort_by(|(a_id, _), (b_id, _)| {
                let da = ctx.player_manager.get(a_id).map(|p| {
                    let dx = p.position.x - ref_x;
                    let dy = p.position.y - ref_y;
                    let dz = p.position.z - ref_z;
                    dx * dx + dy * dy + dz * dz
                }).unwrap_or(f64::MIN);
                let db = ctx.player_manager.get(b_id).map(|p| {
                    let dx = p.position.x - ref_x;
                    let dy = p.position.y - ref_y;
                    let dz = p.position.z - ref_z;
                    dx * dx + dy * dy + dz * dz
                }).unwrap_or(f64::MIN);
                db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        SortOrder::Random => {
            // Fisher-Yates shuffle using time as seed (good enough for commands)
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as u64)
                .unwrap_or(42);
            for i in (1..candidates.len()).rev() {
                let j = (seed.wrapping_mul(i as u64 + 1) % (i as u64 + 1)) as usize;
                candidates.swap(i, j);
            }
        }
        SortOrder::Arbitrary => {}
    }

    // Apply limit
    if let Some(limit) = args.limit {
        candidates.truncate(limit);
    }

    candidates
}

/// 解析单个玩家目标 (多个时只取第一个)
pub fn resolve_player_target(target: &str, ctx: &CommandContext) -> Option<(uuid::Uuid, String)> {
    resolve_player_targets(target, ctx).into_iter().next()
}

/// 命令分发器
pub struct CommandDispatcher {
    commands: HashMap<String, Box<dyn Command>>,
    aliases: HashMap<String, String>, // alias → canonical name
}

impl CommandDispatcher {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    /// 注册命令
    pub fn register<C: Command + 'static>(&mut self, command: C) {
        let name = command.name().to_string();
        for alias in command.aliases() {
            self.aliases.insert(alias.to_lowercase(), name.to_lowercase());
        }
        tracing::debug!("Registered command: /{}", name);
        self.commands.insert(name.to_lowercase(), Box::new(command));
    }

    /// 解析命令名 (含别名)
    fn resolve_name(&self, name: &str) -> Option<String> {
        let lower = name.to_lowercase();
        if self.commands.contains_key(&lower) {
            Some(lower)
        } else {
            self.aliases.get(&lower).cloned()
        }
    }

    /// 分发并执行命令
    pub fn dispatch(&self, ctx: &CommandContext) -> CommandResult {
        if ctx.args.is_empty() {
            return Err("No command specified".into());
        }

        let cmd_name = ctx.args[0].to_lowercase();
        let args: Vec<String> = ctx.args[1..].iter().map(|s| s.to_string()).collect();

        let exec_ctx = CommandContext {
            source: ctx.source.clone(),
            args,
            player_manager: ctx.player_manager,
            shutdown_tx: ctx.shutdown_tx,
            world_state: ctx.world_state,
            motd: ctx.motd,
            max_players: ctx.max_players,
            chunk_store: ctx.chunk_store,
            save_trigger: ctx.save_trigger,
            dispatcher: ctx.dispatcher,
            mob_manager: ctx.mob_manager,
            team_manager: ctx.team_manager,
        };

        let resolved = self.resolve_name(&cmd_name);
        match resolved.and_then(|n| self.commands.get(&n)) {
            Some(cmd) => cmd.execute(&exec_ctx),
            None => Err(format!("Unknown command: /{}", cmd_name)),
        }
    }

    /// 从原始输入解析并分发
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_input(
        &self,
        input: &str,
        source: CommandSource,
        player_manager: &SharedPlayerManager,
        shutdown_tx: &broadcast::Sender<()>,
        world_state: &SharedWorldState,
        motd: &str,
        max_players: u32,
        chunk_store: Option<&mc_world::chunk_store::ChunkStore>,
        save_trigger: Option<&broadcast::Sender<()>>,
    ) -> CommandResult {
        let trimmed = input.trim().trim_start_matches('/');
        if trimmed.is_empty() {
            return Err("No command specified".into());
        }

        let parts: Vec<String> = trimmed
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        let ctx = CommandContext {
            source,
            args: parts,
            player_manager,
            shutdown_tx,
            world_state,
            motd,
            max_players,
            chunk_store,
            save_trigger,
            dispatcher: Some(self),
            mob_manager: None,
            team_manager: None,
        };

        self.dispatch(&ctx)
    }

    /// 列出所有已注册命令
    pub fn list_commands(&self) -> Vec<&str> {
        self.commands.keys().map(|s| s.as_str()).collect()
    }

    /// 获取命令详情
    pub fn get_command_info(&self) -> Vec<(&str, &str, &[&str])> {
        self.commands.values()
            .map(|c| (c.name(), c.description(), c.aliases()))
            .collect()
    }
}

impl Default for CommandDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
