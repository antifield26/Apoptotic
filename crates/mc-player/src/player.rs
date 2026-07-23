//! 玩家管理器 — 在线玩家追踪
//!
//! 使用 Arc<RwLock<>> 在多个连接之间共享。

use crate::inventory::{Inventory, ItemStack};
use mc_core::position::Position;
use mc_core::types::GameMode;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

/// 全服广播消息
#[derive(Debug, Clone)]
pub struct ChatBroadcast {
    pub sender_name: String,
    pub message: String,
    pub msg_type: BroadcastType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BroadcastType {
    Chat,
    System,
    Join,
    Leave,
    Kick(Uuid, String),
    /// 私聊 (目标 UUID, 消息内容)
    Private(Uuid, String),
}

// ═══════════════════════════════════════════════════
// 实体可见性事件 (多人联机)
// ═══════════════════════════════════════════════════

/// 实体事件 (用于多玩家间同步)
#[derive(Debug, Clone)]
pub struct EntityEvent {
    pub entity_id: i32,
    pub uuid: Uuid,
    pub username: String,
    pub kind: EntityEventKind,
}

#[derive(Debug, Clone)]
pub enum EntityEventKind {
    /// 玩家实体生成 (x, y, z, yaw, pitch)
    Spawn(f64, f64, f64, f32, f32),
    /// 生物实体生成 (mob_type, x, y, z)
    MobSpawn(i32, f64, f64, f64),
    /// 实体位置更新
    Move(f64, f64, f64, f32, f32),
    /// 实体消失
    Despawn,
    /// 生物消失 (用于多玩家同步)
    MobDespawn,
}

/// 玩家状态事件 — 命令触发后通知连接发送对应数据包
#[derive(Debug, Clone)]
pub struct PlayerStateEvent {
    pub uuid: Uuid,
    pub kind: PlayerStateEventKind,
}

#[derive(Debug, Clone)]
pub enum PlayerStateEventKind {
    /// 生命值更新 (新值)
    HealthUpdate(f32),
    /// 游戏模式更新
    GamemodeUpdate(mc_core::types::GameMode),
    /// 传送完成 (x, y, z, yaw, pitch, teleport_id)
    Teleport(f64, f64, f64, f32, f32, i32),
    /// 经验值更新 (bar, level, total)
    XpUpdate(f32, i32, i32),
    /// 饥饿值更新 (food_level, saturation)
    FoodUpdate(i32, f32),
    /// 标题消息 (action: 0=title,1=subtitle,2=actionbar, text)
    Title(i32, String),
    /// 播放音效 (sound_name, category, volume, pitch)
    PlaySound(String, i32, f32, f32),
    /// 停止音效
    StopSound,
    /// 清空背包
    ClearInventory,
    /// 附魔手持物品 (enchantment_name, level)
    EnchantHeld(String, i32),
    /// 添加状态效果 (entity_id, effect_id, amplifier, duration, flags)
    EffectAdd(i32, i32, u8, i32, u8),
    /// 移除状态效果 (entity_id, effect_id)
    EffectRemove(i32, i32),
    /// 打开村民交易界面 (profession_id, entity_id)
    VillagerTrade(i32, i32),
    /// 切换维度 (dimension_name, x, y, z)
    SwitchDimension(String, f64, f64, f64),
    /// 全局: 计分板目标更新 (objective_name, action: 0=create,1=remove,2=update, display_name, criteria)
    ScoreboardObjective(String, u8, String, String),
    /// 全局: 计分板分数更新 (entity_name, objective_name, score, action: 0=set,1=remove)
    ScoreboardScore(String, String, i32, u8),
    /// 全局: 计分板显示 (position: 0=list,1=sidebar,2=belowName, objective_name)
    ScoreboardDisplay(u8, String),
    /// 全局: Team 更新 (team_name, action: 0=create,1=remove,2=update,3=add_player,4=remove_player, display_name, prefix, suffix, color, friendly_fire, player_names)
    TeamUpdate(String, u8, String, String, String, String, bool, Vec<String>),
    /// 全局: BossBar 更新 (bar_id, action: 0=add,1=remove,2=update_health,3=update_title,4=update_style, title, health, color, division, flags)
    BossBarUpdate(String, u8, String, f32, i32, i32, u8),
    /// 全局: GameEvent (event_type, value) — 天气变化/游戏模式等
    GameEventGlobal(u8, f32),
    /// Transfer player to another server (host, port)
    TransferPlayer(String, i32),
}

/// 玩家完整信息
#[derive(Debug, Clone)]
pub struct Player {
    pub entity_id: i32,
    pub uuid: Uuid,
    pub username: String,
    pub position: Position,
    pub gamemode: GameMode,
    pub health: f32,
    pub is_op: bool,
    /// 背包 (Clone 通过 arc 实现)
    pub inventory: Inventory,
    /// 坠落距离追踪 (用于坠落伤害)
    pub fall_distance: f32,
    /// 上次受伤时间 (用于无敌帧)
    pub last_damage_tick: u64,
    /// 活跃状态效果
    pub active_effects: Vec<mc_core::effect::ActiveEffect>,
    /// 疾跑状态
    pub is_sprinting: bool,
    /// 潜行状态
    pub is_sneaking: bool,
    /// 盾牌格挡状态
    pub is_blocking: bool,
    /// 速度倍率 (Speed/Slowness 效果, 默认 1.0)
    pub speed_multiplier: f32,
    /// 挖掘速度倍率 (MiningFatigue 效果, 默认 1.0)
    pub mining_multiplier: f32,
    /// 跳跃倍率 (JumpBoost 效果, 默认 1.0)
    pub jump_multiplier: f32,
    /// 坠落伤害倍率 (SlowFalling 效果, 默认 1.0, 激活时 0.0)
    pub fall_damage_multiplier: f32,
    /// 游泳速度倍率 (DolphinGrace 效果, 默认 1.0)
    pub swim_speed_multiplier: f32,
    /// 待确认的传送 ID (用于 TeleportConfirm 0x00 校验)
    pub pending_teleport_id: Option<i32>,
    /// 客户端视距设置 (ClientInformation 0x0C)
    pub client_view_distance: u8,
    /// 客户端语言设置
    pub client_locale: String,
    /// 上一次消息确认计数 (MessageAcknowledgment 0x01)
    pub last_acknowledged_count: i32,
    /// 已知配方 ID 集合 (RecipeBookData 0x13)
    pub known_recipes: std::collections::HashSet<u16>,
    /// 经验条进度 (0.0-1.0)
    pub xp_bar: f32,
    /// 经验等级
    pub xp_level: i32,
    /// 总经验点数
    pub xp_total: i32,
    /// 饥饿值 (0-20)
    pub food_level: i32,
    /// 饱和值 (0.0-20.0)
    pub food_saturation: f32,
    /// 饥饿消耗累积 (0.0-4.0, >=4.0 时减少 food)
    pub food_exhaustion: f32,
    /// 钓鱼状态 (None = 未钓鱼)
    pub fishing: Option<crate::fishing::FishingState>,
    /// 已驯服实体的 entity_id 列表
    pub tamed_entities: Vec<i32>,
    /// 实体标签 (用于 /tag 命令)
    pub tags: std::collections::HashSet<String>,
    /// 个人出生点 (用于 /spawnpoint, 重生时使用)
    pub spawn_position: Option<(f64, f64, f64, f32)>,
    /// 正在食用的物品 (None = 未在进食)
    pub eating_item: Option<u32>,
    /// 开始进食的 tick
    pub eating_start_tick: u64,
    /// 当前所在维度
    pub dimension: String,
    /// 光标物品 (玩家鼠标上拿着的物品, 用于容器交互)
    pub cursor_item: Option<ItemStack>,
    /// 鞘翅滑翔状态
    pub is_flying: bool,
    /// Absorption 金心 (额外血量, 伤害优先扣除)
    pub absorption_health: f32,
    /// E2: Anti-cheat — last known valid position for rubberband
    pub ac_last_valid_x: f64,
    pub ac_last_valid_y: f64,
    pub ac_last_valid_z: f64,
    /// E2: Violation counter (decays over time when player moves legally)
    pub ac_violations: u8,
    /// E2: Tick of last violation
    pub ac_last_violation_tick: u64,
    /// E2: Pending teleport flag (server-side teleports bypass anti-cheat)
    pub ac_bypass_until: u64,
    /// E6: Last PlayerInput forward strength (-1.0 to 1.0)
    pub last_input_forward: f32,
    /// E6: Last PlayerInput sideways strength (-1.0 to 1.0)
    pub last_input_sideways: f32,
    /// E6: Last PlayerInput jump flag
    pub last_input_jump: bool,
}

/// 共享的玩家管理器 — DashMap 无锁并发
pub struct PlayerManager {
    players: DashMap<Uuid, Player>,
    next_entity_id: std::sync::atomic::AtomicI32,
    chat_tx: broadcast::Sender<ChatBroadcast>,
    /// 实体事件广播 (玩家可见性)
    entity_tx: broadcast::Sender<EntityEvent>,
    /// 玩家状态事件 (生命值/模式/传送 — 连接据此发送数据包)
    player_state_tx: broadcast::Sender<PlayerStateEvent>,
    player_entities: RwLock<HashMap<u32, Uuid>>,
    banned: RwLock<HashSet<Uuid>>,
    whitelist: RwLock<(bool, HashSet<Uuid>)>,
    /// Spatial index: chunk → player UUIDs (for O(1) proximity queries)
    chunk_players: DashMap<(i32, i32), Vec<Uuid>>,
}

impl Default for PlayerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerManager {
    pub fn new() -> Self {
        let (chat_tx, _) = broadcast::channel::<ChatBroadcast>(64);
        let (entity_tx, _) = broadcast::channel::<EntityEvent>(128);
        let (player_state_tx, _) = broadcast::channel::<PlayerStateEvent>(64);
        Self {
            players: DashMap::new(),
            next_entity_id: std::sync::atomic::AtomicI32::new(0),
            chat_tx,
            entity_tx,
            player_state_tx,
            chunk_players: DashMap::new(),
            player_entities: RwLock::new(HashMap::new()),
            banned: RwLock::new(HashSet::new()),
            whitelist: RwLock::new((false, HashSet::new())),
        }
    }

    /// 分配一个新的实体 ID (lock-free)
    pub fn allocate_entity_id(&self) -> i32 {
        self.next_entity_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// 玩家加入
    pub fn add_player(&self, uuid: Uuid, username: String) -> Player {
        let entity_id = self.allocate_entity_id();
        let player = Player {
            entity_id,
            uuid,
            username,
            position: Position::new(0.0, 64.0, 0.0),
            gamemode: GameMode::Survival,
            health: 20.0,
            is_op: false,
            inventory: Inventory::new(),
            fall_distance: 0.0,
            last_damage_tick: 0,
            active_effects: Vec::new(),
            is_sprinting: false,
            is_sneaking: false,
            is_blocking: false,
            speed_multiplier: 1.0,
            mining_multiplier: 1.0,
            jump_multiplier: 1.0,
            fall_damage_multiplier: 1.0,
            swim_speed_multiplier: 1.0,
            pending_teleport_id: None,
            client_view_distance: 8,
            client_locale: String::new(),
            last_acknowledged_count: 0,
            known_recipes: std::collections::HashSet::new(),
            xp_bar: 0.0,
            xp_level: 0,
            xp_total: 0,
            food_level: 20,
            food_saturation: 5.0,
            food_exhaustion: 0.0,
            fishing: None,
            tamed_entities: Vec::new(),
            tags: std::collections::HashSet::new(),
            spawn_position: None,
            eating_item: None,
            eating_start_tick: 0,
            dimension: "minecraft:overworld".into(),
            cursor_item: None,
            is_flying: false,
            absorption_health: 0.0,
            ac_last_valid_x: 0.0, ac_last_valid_y: 64.0, ac_last_valid_z: 0.0,
            ac_violations: 0, ac_last_violation_tick: 0, ac_bypass_until: 0,
            last_input_forward: 0.0, last_input_sideways: 0.0, last_input_jump: false,
        };

        let name = player.username.clone();
        self.players.insert(uuid, player.clone());
        // Init spatial index
        let px = player.position.x; let pz = player.position.z;
        self.init_player_chunk(&uuid, (px.floor() as i32).div_euclid(16), (pz.floor() as i32).div_euclid(16));

        tracing::info!(
            "Player '{}' joined (entity_id={}, uuid={}) — {} online",
            name,
            entity_id,
            uuid,
            self.players.len()
        );

        player
    }

    /// 玩家离开
    pub fn remove_player(&self, uuid: &Uuid) -> Option<Player> {
        // Clean up spatial index before removing
        if let Some(ref p) = self.players.get(uuid) {
            let cx = (p.position.x.floor() as i32).div_euclid(16);
            let cz = (p.position.z.floor() as i32).div_euclid(16);
            self.remove_player_chunk(uuid, cx, cz);
        }
        let removed = self.players.remove(uuid).map(|(_, v)| v);
        if let Some(ref p) = removed {
            tracing::info!(
                "Player '{}' left — {} online",
                p.username,
                self.players.len()
            );
        }
        removed
    }

    /// 获取玩家
    pub fn get(&self, uuid: &Uuid) -> Option<Player> {
        self.players.get(uuid).map(|r| r.clone())
    }

    /// 根据用户名查找玩家
    pub fn get_by_name(&self, username: &str) -> Option<Player> {
        let lower = username.to_lowercase();
        self.players.iter().find(|r| r.value().username.to_lowercase() == lower).map(|r| r.value().clone())
    }

    /// 在线玩家数
    pub fn online_count(&self) -> usize {
        self.players.len()
    }

    /// 获取所有在线玩家的快照
    pub fn all_players(&self) -> Vec<Player> {
        self.players.iter().map(|r| r.value().clone()).collect()
    }

    /// 获取除指定玩家外的所有在线玩家
    pub fn others(&self, exclude: &Uuid) -> Vec<Player> {
        self.players.iter()
            .filter(|r| *r.key() != *exclude)
            .map(|r| r.value().clone())
            .collect()
    }

    // ── Mutation API ──

    /// 停止玩家的所有音效 (通过 PlayerStateEvent)
    pub fn stop_sound(&self, uuid: &Uuid) -> Result<(), String> {
        let _ = self.player_state_tx.send(PlayerStateEvent {
            uuid: *uuid,
            kind: PlayerStateEventKind::StopSound,
        });
        Ok(())
    }

    /// 为玩家添加标签
    pub fn add_tag(&self, uuid: &Uuid, tag: &str) -> bool {
        if let Some(mut p) = self.players.get_mut(uuid) {
            p.tags.insert(tag.to_string())
        } else { false }
    }

    /// 移除玩家标签
    pub fn remove_tag(&self, uuid: &Uuid, tag: &str) -> bool {
        if let Some(mut p) = self.players.get_mut(uuid) {
            p.tags.remove(tag)
        } else { false }
    }

    /// 列出玩家标签
    pub fn list_tags(&self, uuid: &Uuid) -> Vec<String> {
        self.players.get(uuid)
            .map(|p| p.tags.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// 设置玩家个人出生点
    pub fn set_spawn_position(&self, uuid: &Uuid, x: f64, y: f64, z: f64, yaw: f32) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => { p.spawn_position = Some((x, y, z, yaw)); Ok(()) }
            None => Err("Player not found".into()),
        }
    }

    /// 恢复玩家的饥饿值 (食用食物)
    pub fn add_food(&self, uuid: &Uuid, nutrition: i32, saturation: f32) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                p.food_level = (p.food_level + nutrition).min(20);
                p.food_saturation = (p.food_saturation + saturation).min(p.food_level as f32);
                p.eating_item = None;
                p.eating_start_tick = 0;
                let _ = self.player_state_tx.send(PlayerStateEvent {
                    uuid: *uuid,
                    kind: PlayerStateEventKind::FoodUpdate(p.food_level, p.food_saturation),
                });
                Ok(())
            }
            None => Err("Player not found".into()),
        }
    }

    /// 开始进食
    pub fn start_eating(&self, uuid: &Uuid, item_id: u32, tick: u64) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                p.eating_item = Some(item_id);
                p.eating_start_tick = tick;
                Ok(())
            }
            None => Err("Player not found".into()),
        }
    }

    /// 取消进食
    pub fn cancel_eating(&self, uuid: &Uuid) {
        if let Some(mut p) = self.players.get_mut(uuid) {
            p.eating_item = None;
            p.eating_start_tick = 0;
        }
    }

    /// 检查玩家是否正在进食，如果是则完成进食
    /// 返回 Some(item_id) 表示进食完成
    pub fn check_eating_done(&self, uuid: &Uuid, current_tick: u64) -> Option<u32> {
        if let Some(p) = self.players.get(uuid).map(|r| r.clone())
            && let Some(item_id) = p.eating_item {
                let duration = crate::food::eating_duration_ticks(item_id) as u64;
                if current_tick.saturating_sub(p.eating_start_tick) >= duration {
                    return Some(item_id);
                }
            }
        None
    }

    /// 设置玩家 OP 状态
    pub fn set_op(&self, uuid: &Uuid, is_op: bool) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                p.is_op = is_op;
                Ok(())
            }
            None => Err("Player not found".into()),
        }
    }

    /// 设置玩家游戏模式
    pub fn set_gamemode(&self, uuid: &Uuid, gm: GameMode) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                p.gamemode = gm;
                let _ = self.player_state_tx.send(PlayerStateEvent {
                    uuid: *uuid,
                    kind: PlayerStateEventKind::GamemodeUpdate(gm),
                });
                Ok(())
            }
            None => Err("Player not found".into()),
        }
    }

    /// 设置玩家生命值
    // ═══ E2: Anti-cheat state accessors ═══
    /// Update anti-cheat valid position and decay violations
    pub fn ac_update_valid(&self, uuid: &Uuid, x: f64, y: f64, z: f64, tick: u64) {
        if let Some(mut p) = self.players.get_mut(uuid) {
            if tick - p.ac_last_violation_tick >= 20 {
                p.ac_violations = p.ac_violations.saturating_sub(1);
            }
            p.ac_last_valid_x = x;
            p.ac_last_valid_y = y;
            p.ac_last_valid_z = z;
        }
    }
    /// Accumulate violations and check rubberband threshold. Returns (violations, should_rubberband).
    pub fn ac_add_violation(&self, uuid: &Uuid, tick: u64) -> (u8, bool) {
        if let Some(mut p) = self.players.get_mut(uuid) {
            p.ac_violations = p.ac_violations.saturating_add(2);
            p.ac_last_violation_tick = tick;
            let should_rubberband = p.ac_violations >= 8;
            (p.ac_violations, should_rubberband)
        } else {
            (0, false)
        }
    }
    /// Get last valid anti-cheat position
    pub fn ac_valid_position(&self, uuid: &Uuid) -> Option<(f64, f64, f64)> {
        self.players.get(uuid).map(|p| (p.ac_last_valid_x, p.ac_last_valid_y, p.ac_last_valid_z))
    }
    /// Reset anti-cheat violations after rubberband
    pub fn ac_reset_violations(&self, uuid: &Uuid) {
        if let Some(mut p) = self.players.get_mut(uuid) {
            p.ac_violations = 0;
        }
    }

    pub fn set_health(&self, uuid: &Uuid, health: f32) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                p.health = health.max(0.0);
                let _ = self.player_state_tx.send(PlayerStateEvent {
                    uuid: *uuid,
                    kind: PlayerStateEventKind::HealthUpdate(p.health),
                });
                Ok(())
            }
            None => Err("Player not found".into()),
        }
    }

    /// 更新玩家位置 + 朝向 (从移动包)
    pub fn update_position(&self, uuid: &Uuid, x: f64, y: f64, z: f64) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                p.position.x = x;
                p.position.y = y;
                p.position.z = z;
                Ok(())
            }
            None => Err("Player not found".into()),
        }
    }

    /// 更新玩家位置 + 朝向 (完整)
    pub fn update_position_full(&self, uuid: &Uuid, x: f64, y: f64, z: f64, yaw: f32, pitch: f32) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                p.position.x = x;
                p.position.y = y;
                p.position.z = z;
                p.position.yaw = yaw;
                p.position.pitch = pitch;
                Ok(())
            }
            None => Err("Player not found".into()),
        }
    }

    /// 找到距离给定坐标最近的玩家
    pub fn nearest_player(&self, x: f64, y: f64, z: f64, exclude: Option<&Uuid>) -> Option<Player> {
        self.players
            .iter()
            .filter(|p| exclude.is_none_or(|e| p.uuid != *e))
            .min_by(|a, b| {
                let da = (a.position.x - x).powi(2) + (a.position.y - y).powi(2) + (a.position.z - z).powi(2);
                let db = (b.position.x - x).powi(2) + (b.position.y - y).powi(2) + (b.position.z - z).powi(2);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|r| r.clone())
    }

    /// 获取指定位置范围内的所有玩家 (用于距离裁剪广播)
    /// Spatial-indexed proximity query: only checks players in nearby chunks.
    /// Reduces O(P×E) to O(E×C) where C = players in adjacent chunks.
    pub fn players_in_range(&self, x: f64, y: f64, z: f64, radius: f64) -> Vec<(Uuid, f64)> {
        let r2 = radius * radius;
        let cx = (x.floor() as i32).div_euclid(16);
        let cz = (z.floor() as i32).div_euclid(16);
        let chunk_radius = ((radius / 16.0).ceil() as i32).max(1);
        let mut results = Vec::new();
        for dcx in -chunk_radius..=chunk_radius {
            for dcz in -chunk_radius..=chunk_radius {
                let key = (cx + dcx, cz + dcz);
                if let Some(uuids) = self.chunk_players.get(&key) {
                    for uuid in uuids.iter() {
                        if let Some(ref player) = self.players.get(uuid) {
                            let dx = player.position.x - x;
                            let dy = player.position.y - y;
                            let dz = player.position.z - z;
                            let d2 = dx*dx + dy*dy + dz*dz;
                            if d2 <= r2 {
                                results.push((*uuid, d2));
                            }
                        }
                    }
                }
            }
        }
        results
    }

    /// Update spatial index when a player moves to a new chunk
    pub fn update_player_chunk(&self, uuid: &Uuid, old_cx: i32, old_cz: i32, new_cx: i32, new_cz: i32) {
        // Remove from old chunk
        if let Some(mut list) = self.chunk_players.get_mut(&(old_cx, old_cz)) {
            list.retain(|u| u != uuid);
        }
        // Add to new chunk
        self.chunk_players.entry((new_cx, new_cz)).or_default().push(*uuid);
    }

    /// Initialize player in spatial index (call on join)
    pub fn init_player_chunk(&self, uuid: &Uuid, cx: i32, cz: i32) {
        self.chunk_players.entry((cx, cz)).or_default().push(*uuid);
    }

    /// Remove player from spatial index (call on leave)
    pub fn remove_player_chunk(&self, uuid: &Uuid, cx: i32, cz: i32) {
        if let Some(mut list) = self.chunk_players.get_mut(&(cx, cz)) {
            list.retain(|u| u != uuid);
        }
    }

    /// 获取实体 ID (用于发送定向包)
    pub fn get_entity_id(&self, uuid: &Uuid) -> Option<i32> {
        self.players.get(uuid).map(|p| p.entity_id)
    }

    /// 广播聊天消息
    pub fn broadcast_chat(&self, sender_name: &str, message: &str, is_cmd: bool) {
        let _ = self.chat_tx.send(ChatBroadcast {
            sender_name: sender_name.to_string(),
            message: message.to_string(),
            msg_type: if is_cmd { BroadcastType::System } else { BroadcastType::Chat },
        });
    }

    /// 发送私聊消息 (只有目标玩家能收到)
    pub fn send_private_msg(&self, sender_name: &str, target_uuid: Uuid, message: &str) {
        let _ = self.chat_tx.send(ChatBroadcast {
            sender_name: sender_name.to_string(),
            message: message.to_string(),
            msg_type: BroadcastType::Private(target_uuid, message.to_string()),
        });
    }

    /// 广播玩家加入
    pub fn broadcast_join(&self, username: &str) {
        let _ = self.chat_tx.send(ChatBroadcast {
            sender_name: "Server".into(),
            message: format!("{} joined the game", username),
            msg_type: BroadcastType::Join,
        });
    }

    /// 广播玩家离开
    pub fn broadcast_leave(&self, username: &str) {
        let _ = self.chat_tx.send(ChatBroadcast {
            sender_name: "Server".into(),
            message: format!("{} left the game", username),
            msg_type: BroadcastType::Leave,
        });
    }

    /// 通知特定玩家被踢 (连接检测到后发送 Disconnect 包)
    pub fn kick_player(&self, uuid: Uuid, reason: &str) {
        let _ = self.chat_tx.send(ChatBroadcast {
            sender_name: "Server".into(),
            message: reason.to_string(),
            msg_type: BroadcastType::Kick(uuid, reason.to_string()),
        });
    }

    // ── Entity visibility (multiplayer) ──

    /// 订阅实体事件
    pub fn subscribe_entities(&self) -> broadcast::Receiver<EntityEvent> {
        self.entity_tx.subscribe()
    }

    /// 订阅玩家状态事件 (生命值/模式/传送)
    pub fn subscribe_player_state(&self) -> broadcast::Receiver<PlayerStateEvent> {
        self.player_state_tx.subscribe()
    }

    /// 广播实体生成
    pub fn broadcast_entity_spawn(&self, entity_id: i32, uuid: Uuid, username: &str, x: f64, y: f64, z: f64, yaw: f32, pitch: f32) {
        let _ = self.entity_tx.send(EntityEvent {
            entity_id, uuid, username: username.to_string(),
            kind: EntityEventKind::Spawn(x, y, z, yaw, pitch),
        });
    }

    /// 广播实体移动
    pub fn broadcast_entity_move(&self, entity_id: i32, uuid: Uuid, x: f64, y: f64, z: f64, yaw: f32, pitch: f32) {
        let _ = self.entity_tx.send(EntityEvent {
            entity_id, uuid, username: String::new(),
            kind: EntityEventKind::Move(x, y, z, yaw, pitch),
        });
    }

    /// 广播实体消失
    pub fn broadcast_entity_despawn(&self, entity_id: i32, uuid: Uuid) {
        let _ = self.entity_tx.send(EntityEvent {
            entity_id, uuid, username: String::new(),
            kind: EntityEventKind::Despawn,
        });
    }

    /// 广播生物生成 (多玩家同步)
    pub fn broadcast_mob_spawn(&self, entity_id: i32, mob_uuid: Uuid, mob_type: i32, x: f64, y: f64, z: f64) {
        let _ = self.entity_tx.send(EntityEvent {
            entity_id, uuid: mob_uuid, username: String::new(),
            kind: EntityEventKind::MobSpawn(mob_type, x, y, z),
        });
    }

    /// 广播生物消失 (多玩家同步)
    pub fn broadcast_mob_despawn(&self, entity_id: i32, mob_uuid: Uuid) {
        let _ = self.entity_tx.send(EntityEvent {
            entity_id, uuid: mob_uuid, username: String::new(),
            kind: EntityEventKind::MobDespawn,
        });
    }

    /// 订阅聊天广播
    pub fn subscribe_chat(&self) -> broadcast::Receiver<ChatBroadcast> {
        self.chat_tx.subscribe()
    }

    // ── Inventory access ──

    /// 累积坠落距离（在下落过程中调用）
    pub fn add_fall_distance(&self, uuid: &Uuid, delta: f32) {
        if let Some(mut p) = self.players.get_mut(uuid) {
            // SlowFalling (28): reduce fall distance accumulation by half
            let has_slow_fall = p.active_effects.iter().any(|e| e.effect.id() == 28);
            let multiplier = if has_slow_fall { 0.5 } else { 1.0 };
            p.fall_distance += delta * multiplier;
        }
    }

    /// 获取并重置坠落距离（着陆时调用），返回着陆前的距离
    pub fn take_fall_distance(&self, uuid: &Uuid) -> f32 {
        if let Some(mut p) = self.players.get_mut(uuid) {
            let d = p.fall_distance;
            p.fall_distance = 0.0;
            d
        } else {
            0.0
        }
    }

    /// 检查玩家是否在伤害无敌帧中 (0.5 秒冷却)
    pub fn can_take_damage(&self, uuid: &Uuid, current_tick: u64) -> bool {
        if let Some(p) = self.players.get(uuid).map(|r| r.clone()) {
            current_tick.saturating_sub(p.last_damage_tick) >= 10 // 0.5s at 20 TPS
        } else {
            false
        }
    }

    /// 记录伤害时间戳
    pub fn mark_damage_taken(&self, uuid: &Uuid, tick: u64) {
        if let Some(mut p) = self.players.get_mut(uuid) {
            p.last_damage_tick = tick;
        }
    }

    /// 获取玩家的总护甲点数和韧性
    pub fn get_armor_values(&self, uuid: &Uuid) -> (f32, f32) {
        if let Some(p) = self.players.get(uuid).map(|r| r.clone()) {
            (p.inventory.total_armor_points(), p.inventory.total_armor_toughness())
        } else {
            (0.0, 0.0)
        }
    }

    /// 应用伤害到玩家，自动计算护甲减免
    /// 返回实际造成的伤害值 (减免后)
    pub fn apply_damage(&self, uuid: &Uuid, raw_damage: f32, tick: u64) -> Result<f32, String> {
        self.apply_damage_with_enchants(uuid, raw_damage, tick, None, None, None)
    }

    /// Apply damage with optional enchantment modifiers from attacker.
    /// `attacker_item_nbt` — the NBT blob of the attacker's held item (parsed for Sharpness/Smite/Bane)
    /// `defender_armor_nbt` — the NBT blobs of the defender's armor pieces (parsed for Protection variants + Thorns)
    pub fn apply_damage_with_enchants(
        &self, uuid: &Uuid, raw_damage: f32, tick: u64,
        attacker_uuid: Option<&Uuid>,
        attacker_item_nbt: Option<&std::collections::HashMap<String, u8>>,
        defender_armor_nbt: Option<&[std::collections::HashMap<String, u8>]>,
    ) -> Result<f32, String> {
        // Apply Strength/Weakness from attacker
        let mut effective = raw_damage;
        if let Some(attacker) = attacker_uuid {
            let strength = self.get_effect_level(attacker, 4) as f32; // strength=4
            let weakness = self.get_effect_level(attacker, 17) as f32; // weakness=17
            effective += strength * 3.0;
            effective = (effective - weakness * 4.0).max(0.0);
        }
        // Attacker weapon enchantments: Sharpness (+1.25/level), Smite (+2.5×level vs undead), Bane (+2.5×level vs arthropods)
        if let Some(enchants) = attacker_item_nbt
            && let Some(&sharp) = enchants.get("sharpness") {
                effective += sharp as f32 * 1.25;
            }
            // Smite and Bane handled by caller based on target type
        // Resistance effect (each level -20% damage)
        let resistance_level = self.get_effect_level(uuid, 10); // resistance=10
        if resistance_level > 0 {
            effective *= (1.0 - resistance_level as f32 * 0.2).max(0.0);
        }
        // Armor enchantments: Protection variants reduce damage (capped at 80%)
        // General Protection: 4% per level. Specific: 8% per level.
        if let Some(armor_ench_list) = defender_armor_nbt {
            let general_prot: u8 = armor_ench_list.iter().filter_map(|e| e.get("protection")).sum();
            let fire_prot: u8 = armor_ench_list.iter().filter_map(|e| e.get("fire_protection")).sum();
            let blast_prot: u8 = armor_ench_list.iter().filter_map(|e| e.get("blast_protection")).sum();
            let proj_prot: u8 = armor_ench_list.iter().filter_map(|e| e.get("projectile_protection")).sum();
            let epf = ((general_prot as f32 * 4.0) + (fire_prot as f32 * 8.0)
                + (blast_prot as f32 * 8.0) + (proj_prot as f32 * 8.0)).min(80.0);
            effective *= 1.0 - epf / 100.0;
        }
        let (armor, toughness) = self.get_armor_values(uuid);
        let reduced = calculate_armor_reduction(armor, toughness, effective);
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                p.health = (p.health - reduced).max(0.0);
                p.last_damage_tick = tick;
                let _ = self.player_state_tx.send(PlayerStateEvent {
                    uuid: *uuid,
                    kind: PlayerStateEventKind::HealthUpdate(p.health),
                });
                // Thorns: reflect damage back to attacker
                if let (Some(attacker), Some(armor_ench_list)) = (attacker_uuid, defender_armor_nbt) {
                    let max_thorns: u8 = armor_ench_list.iter()
                        .filter_map(|e| e.get("thorns").copied())
                        .max()
                        .unwrap_or(0);
                    if max_thorns > 0 {
                        let thorns_chance = max_thorns as f32 * 0.15;
                        if fastrand::f32() < thorns_chance {
                            let thorns_dmg = 1.0 + fastrand::f32() * 3.0;
                            if let Some(mut ap) = self.players.get_mut(attacker) {
                                ap.health = (ap.health - thorns_dmg).max(0.0);
                                let _ = self.player_state_tx.send(PlayerStateEvent {
                                    uuid: *attacker,
                                    kind: PlayerStateEventKind::HealthUpdate(ap.health),
                                });
                            }
                        }
                    }
                }
                Ok(reduced)
            }
            None => Err("Player not found".into()),
        }
    }

    /// Broadcast a dimension change (Respawn) to a specific player
    pub fn broadcast_player_respawn(&self, uuid: &Uuid, dimension: &str, x: f64, y: f64, z: f64) -> Result<(), String> {
        let _ = self.player_state_tx.send(PlayerStateEvent {
            uuid: *uuid,
            kind: PlayerStateEventKind::SwitchDimension(dimension.to_string(), x, y, z),
        });
        Ok(())
    }

    /// Apply fall damage based on fall distance (with Feather Falling enchant)
    pub fn apply_fall_damage(&self, uuid: &Uuid, fall_distance: f32, feather_falling: u8) {
        // SlowFalling(28): complete fall damage immunity
        let slow_fall = self.get_effect_level(uuid, 27); // SlowFalling=27
        if slow_fall > 0 { return; }
        if fall_distance > 3.0 {
            let mut damage = (fall_distance - 3.0) * 1.0;
            // Feather Falling: -12% per level
            if feather_falling > 0 {
                damage *= (1.0 - feather_falling as f32 * 0.12).max(0.0);
            }
            let tick = self.players.get(uuid).map(|p| p.last_damage_tick).unwrap_or(0);
            let _ = self.apply_damage(uuid, damage, tick + 1);
        }
    }

    /// Apply environmental damage (void, fire, drowning)
    pub fn apply_environmental_damage(&self, uuid: &Uuid, dmg_type: &str) {
        // Fire Resistance cancels fire damage
        if dmg_type == "fire" && self.get_effect_level(uuid, 11) > 0 { return; } // FireResistance=11
        // Water Breathing prevents drowning
        if dmg_type == "drowning" && (self.get_effect_level(uuid, 12) > 0 || self.get_effect_level(uuid, 28) > 0) { return; } // WaterBreathing=12, ConduitPower=28
        let damage = match dmg_type {
            "void" => 4.0,
            "fire" => 1.0,
            "drowning" => 2.0,
            _ => 1.0,
        };
        let tick = self.players.get(uuid).map(|p| p.last_damage_tick).unwrap_or(0);
        let _ = self.apply_damage(uuid, damage, tick + 1);
    }

    /// Track player weapon cooldown (1.9+ combat)
    pub fn set_last_attack(&self, uuid: &Uuid, tick: u64) {
        if let Some(mut p) = self.players.get_mut(uuid) {
            p.last_damage_tick = tick; // reuse as attack timestamp
        }
    }

    pub fn get_attack_cooldown(&self, uuid: &Uuid, item_id: u32, current_tick: u64) -> f32 {
        let base_ticks = match item_id {
            780 | 785 | 792 => 32,  // swords (iron/stone/diamond) — 1.6s
            769 | 787 | 790 => 24,  // pickaxes — 1.2s
            770 | 788 | 791 => 20,  // axes — 1.0s (high damage)
            768 | 786 | 789 => 20,  // shovels — 1.0s
            _ => 20,                 // other — 1.0s
        };
        if let Some(p) = self.players.get(uuid) {
            let elapsed = (current_tick - p.last_damage_tick) as f32;
            (elapsed / base_ticks as f32).min(1.0)
        } else { 1.0 }
    }

    /// Get the effective max health, accounting for HealthBoost effect
    pub fn get_max_health(&self, uuid: &Uuid) -> f32 {
        let boost_lvl = self.get_effect_level(uuid, 20); // HealthBoost=20
        20.0 + 4.0 * boost_lvl as f32
    }

    /// Get effect amplifier level for a player (returns 1+amplifier, or 0 if not active)
    pub fn get_effect_level(&self, uuid: &Uuid, effect_id: i32) -> u8 {
        self.players.get(uuid)
            .map(|p| {
                p.active_effects.iter()
                    .find(|e| e.effect.id() == effect_id as u8)
                    .map(|e| e.amplifier + 1)
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }

    /// Apply periodic damage effects (poison, wither) — call every 25 ticks
    pub fn tick_effect_damage(&self, uuid: &Uuid) {
        let poison_level = self.get_effect_level(uuid, 18); // Poison=18
        let wither_level = self.get_effect_level(uuid, 19); // Wither=19
        if (poison_level > 0 || wither_level > 0)
            && let Some(mut p) = self.players.get_mut(uuid) {
                if poison_level > 0 && p.health > 1.0 {
                    p.health = (p.health - (poison_level as f32 * 0.5)).max(1.0);
                }
                if wither_level > 0 {
                    p.health = (p.health - (wither_level as f32 * 1.0)).max(0.0);
                }
                let _ = self.player_state_tx.send(PlayerStateEvent {
                    uuid: *uuid,
                    kind: PlayerStateEventKind::HealthUpdate(p.health),
                });
            }
    }

    /// 设置疾跑状态
    pub fn set_sprinting(&self, uuid: &Uuid, sprinting: bool) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => { p.is_sprinting = sprinting; Ok(()) }
            None => Err("Player not found".into()),
        }
    }

    /// 设置潜行状态
    pub fn set_sneaking(&self, uuid: &Uuid, sneaking: bool) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => { p.is_sneaking = sneaking; Ok(()) }
            None => Err("Player not found".into()),
        }
    }

    /// 记录最新的 PlayerInput 移动输入 (用于反作弊验证)
    pub fn set_movement_input(&self, uuid: &Uuid, forward: f32, sideways: f32, jump: bool) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                p.last_input_forward = forward;
                p.last_input_sideways = sideways;
                p.last_input_jump = jump;
                Ok(())
            }
            None => Err("Player not found".into()),
        }
    }

    /// 设置盾牌格挡状态
    pub fn set_blocking(&self, uuid: &Uuid, blocking: bool) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => { p.is_blocking = blocking; Ok(()) }
            None => Err("Player not found".into()),
        }
    }

    /// 设置玩家所在维度
    pub fn set_dimension(&self, uuid: &Uuid, dim: &str) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => { p.dimension = dim.to_string(); Ok(()) }
            None => Err("Player not found".into()),
        }
    }

    /// 获取玩家所在维度
    pub fn get_dimension(&self, uuid: &Uuid) -> Result<String, String> {
        self.players.get(uuid)
            .map(|p| p.dimension.clone())
            .ok_or_else(|| "Player not found".into())
    }

    /// 广播全局事件 (scoreboard/team/bossbar) 到所有连接的客户端
    pub fn broadcast_global(&self, kind: PlayerStateEventKind) {
        let _ = self.player_state_tx.send(PlayerStateEvent {
            uuid: uuid::Uuid::nil(), // nil UUID = 全局事件
            kind,
        });
    }

    /// Send Transfer packet to a specific player (hub-and-spoke server switching)
    pub fn send_transfer(&self, uuid: &Uuid, host: &str, port: i32) -> Result<(), String> {
        if !self.players.contains_key(uuid) {
            return Err("Player not online".into());
        }
        let _ = self.player_state_tx.send(PlayerStateEvent {
            uuid: *uuid,
            kind: PlayerStateEventKind::TransferPlayer(host.to_string(), port),
        });
        Ok(())
    }

    /// 设置玩家经验值
    pub fn set_xp(&self, uuid: &Uuid, bar: f32, level: i32, total: i32) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                p.xp_bar = bar.clamp(0.0, 1.0);
                p.xp_level = level.max(0);
                p.xp_total = total.max(0);
                let _ = self.player_state_tx.send(PlayerStateEvent {
                    uuid: *uuid,
                    kind: PlayerStateEventKind::XpUpdate(p.xp_bar, p.xp_level, p.xp_total),
                });
                Ok(())
            }
            None => Err("Player not found".into()),
        }
    }

    // ── Status effects ──

    /// 给玩家添加状态效果
    pub fn add_effect(&self, uuid: &Uuid, effect: mc_core::effect::ActiveEffect) -> Result<String, String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                // Remove existing effect of same type
                p.active_effects.retain(|e| e.effect != effect.effect);
                // Apply instant effects immediately
                if effect.effect.is_instant() {
                    let amp = effect.amplifier;
                    match effect.effect {
                        mc_core::effect::EffectType::InstantHealth => {
                            let heal = (4i32 << amp) as f32; // 4 at level I, 8 at II, 16 at III...
                            let new_health = (self.get(uuid).map(|p| p.health).unwrap_or(20.0) + heal).min(20.0);
                            self.set_health(uuid, new_health)?;
                        }
                        mc_core::effect::EffectType::InstantDamage => {
                            let damage = (3i32 << amp) as f32; // 3 at level I, 6 at II...
                            let new_health = (self.get(uuid).map(|p| p.health).unwrap_or(20.0) - damage).max(0.0);
                            self.set_health(uuid, new_health)?;
                        }
                        _ => {}
                    }
                    return Ok(format!("Applied instant effect {:?}", effect.effect));
                }
                p.active_effects.push(effect.clone());
                let eid = p.entity_id;
                // Send EffectAdd event for client visuals
                let _ = self.player_state_tx.send(PlayerStateEvent {
                    uuid: *uuid,
                    kind: PlayerStateEventKind::EffectAdd(eid, effect.effect.id() as i32, effect.amplifier, effect.duration_ticks as i32, 0x06),
                });
                Ok(format!("Applied {:?} x{} for {} ticks", effect.effect, effect.amplifier + 1, effect.duration_ticks))
            }
            None => Err("Player not found".into()),
        }
    }

    /// 清除玩家的所有状态效果
    pub fn clear_effects(&self, uuid: &Uuid) -> Result<String, String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                let count = p.active_effects.len();
                p.active_effects.clear();
                Ok(format!("Cleared {} effect(s)", count))
            }
            None => Err("Player not found".into()),
        }
    }

    /// 向玩家发送标题消息
    pub fn send_title(&self, uuid: &Uuid, action: i32, text: String) -> Result<(), String> {
        if self.players.contains_key(uuid) {
            let _ = self.player_state_tx.send(PlayerStateEvent {
                uuid: *uuid,
                kind: PlayerStateEventKind::Title(action, text),
            });
            Ok(())
        } else {
            Err("Player not found".into())
        }
    }

    /// 向玩家播放音效
    pub fn play_sound(&self, uuid: &Uuid, sound_name: String, category: i32, volume: f32, pitch: f32) -> Result<(), String> {
        if self.players.contains_key(uuid) {
            let _ = self.player_state_tx.send(PlayerStateEvent {
                uuid: *uuid,
                kind: PlayerStateEventKind::PlaySound(sound_name, category, volume, pitch),
            });
            Ok(())
        } else {
            Err("Player not found".into())
        }
    }

    /// 清空玩家背包
    pub fn clear_inventory(&self, uuid: &Uuid) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                p.inventory = Inventory::new();
                let _ = self.player_state_tx.send(PlayerStateEvent {
                    uuid: *uuid,
                    kind: PlayerStateEventKind::ClearInventory,
                });
                Ok(())
            }
            None => Err("Player not found".into()),
        }
    }

    /// Tick hunger for all online players (called from main tick loop)
    /// Applies exhaustion, food drain, starvation damage, and natural regeneration
    pub fn tick_hunger(&self) {
        let mut events: Vec<(Uuid, PlayerStateEventKind)> = Vec::new();
        let uuids: Vec<Uuid> = self.players.iter().map(|r| *r.key()).collect();
        for uuid in &uuids {
            if let Some(mut player) = self.players.get_mut(uuid) {
                // Skip creative/spectator
                if player.gamemode == GameMode::Creative || player.gamemode == GameMode::Spectator {
                    continue;
                }

                // Calculate exhaustion based on activity
                let exhaustion_delta = if player.is_sprinting {
                    0.1
                } else if player.is_sneaking {
                    0.005
                } else {
                    0.01
                };

                player.food_exhaustion += exhaustion_delta;
                let mut food_changed = false;

                while player.food_exhaustion >= 4.0 {
                    player.food_exhaustion -= 4.0;
                    if player.food_saturation > 0.0 {
                        player.food_saturation = (player.food_saturation - 1.0).max(0.0);
                    } else if player.food_level > 0 {
                        player.food_level -= 1;
                    }
                    food_changed = true;
                }

                // Starvation: if food == 0, apply damage on food change
                if player.food_level == 0 && food_changed {
                    player.food_exhaustion = 0.0;
                    let new_health = (player.health - 1.0).max(0.0);
                    player.health = new_health;
                    events.push((player.uuid, PlayerStateEventKind::HealthUpdate(new_health)));
                    events.push((player.uuid, PlayerStateEventKind::FoodUpdate(player.food_level, player.food_saturation)));
                } else if player.food_level >= 18 && player.food_saturation > 0.0 && food_changed {
                    // Natural regeneration (clamped to max HP including HealthBoost)
                    let max_hp = self.get_max_health(&player.uuid);
                    if player.health >= max_hp { break; }
                    player.food_exhaustion += 3.0;
                    let new_health = (player.health + 1.0).min(max_hp);
                    player.health = new_health;
                    events.push((player.uuid, PlayerStateEventKind::HealthUpdate(new_health)));
                    events.push((player.uuid, PlayerStateEventKind::FoodUpdate(player.food_level, player.food_saturation)));
                } else if food_changed {
                    events.push((player.uuid, PlayerStateEventKind::FoodUpdate(player.food_level, player.food_saturation)));
                }
            }
        } // write lock released here

        // Emit all events
        for (uuid, kind) in events {
            let _ = self.player_state_tx.send(PlayerStateEvent { uuid, kind });
        }
    }

    /// Tick effects for all online players (called from main tick loop)
    pub fn tick_effects(&self, tick_count: u64) {
        let uuids: Vec<Uuid> = self.players.iter().map(|r| *r.key()).collect();
        for uuid in &uuids {
            if let Some(mut player) = self.players.get_mut(uuid) {
            // ═══ Periodic effect application ═══
            // Regeneration: +1 HP every 50/(amp+1) ticks
            let regen_level = player.active_effects.iter()
                .find(|e| e.effect.id() == 10) // regeneration=10
                .map(|e| e.amplifier + 1).unwrap_or(0);
            if regen_level > 0 && tick_count.is_multiple_of((50 / regen_level as u64).max(1)) {
                let max_hp = self.get_max_health(uuid);
                player.health = (player.health + 1.0).min(max_hp);
                let _ = self.player_state_tx.send(PlayerStateEvent {
                    uuid: *uuid,
                    kind: PlayerStateEventKind::HealthUpdate(player.health),
                });
            }

            // Hunger effect: increase food exhaustion
            let hunger_level = player.active_effects.iter()
                .find(|e| e.effect.id() == 17) // hunger=17
                .map(|e| e.amplifier + 1).unwrap_or(0);
            if hunger_level > 0 && tick_count.is_multiple_of(20) {
                player.food_exhaustion += 0.005 * hunger_level as f32;
            }

            // Saturation effect: restore food
            let saturation_level = player.active_effects.iter()
                .find(|e| e.effect.id() == 23) // saturation=23
                .map(|e| e.amplifier + 1).unwrap_or(0);
            if saturation_level > 0 && tick_count.is_multiple_of(20) {
                player.food_saturation = (player.food_saturation + saturation_level as f32).min(20.0);
                player.food_level = (player.food_level + saturation_level as i32).min(20);
            }

            // Speed (1): +20% movement speed per amplifier level
            let speed_lvl = player.active_effects.iter()
                .find(|e| e.effect.id() == 1)
                .map(|e| e.amplifier + 1).unwrap_or(0);
            let slowness_lvl = player.active_effects.iter()
                .find(|e| e.effect.id() == 2)
                .map(|e| e.amplifier + 1).unwrap_or(0);
            player.speed_multiplier = 1.0 + 0.2 * speed_lvl as f32 - 0.15 * slowness_lvl as f32;
            if player.speed_multiplier < 0.1 { player.speed_multiplier = 0.1; }

            // MiningFatigue (4): -30% mining speed per amplifier level
            let fatigue_lvl = player.active_effects.iter()
                .find(|e| e.effect.id() == 4)
                .map(|e| e.amplifier + 1).unwrap_or(0);
            player.mining_multiplier = (1.0 - 0.3 * fatigue_lvl as f32).max(0.05);

            // Haste (3): +20% mining speed per amplifier level
            let haste_lvl = player.active_effects.iter()
                .find(|e| e.effect.id() == 3)
                .map(|e| e.amplifier + 1).unwrap_or(0);
            player.mining_multiplier = (player.mining_multiplier + 0.2 * haste_lvl as f32).min(5.0);

            // JumpBoost (8): +0.5 blocks jump height per level
            let jump_lvl = player.active_effects.iter()
                .find(|e| e.effect.id() == 8)
                .map(|e| e.amplifier + 1).unwrap_or(0);
            player.jump_multiplier = 1.0 + 0.5 * jump_lvl as f32;

            // SlowFalling (28): flag for fall damage immunity (checked in environmental damage tick)
            let slow_fall_lvl = player.active_effects.iter()
                .find(|e| e.effect.id() == 28)
                .map(|e| e.amplifier + 1).unwrap_or(0);
            player.fall_damage_multiplier = if slow_fall_lvl > 0 { 0.0 } else { 1.0 };

            // DolphinGrace (30): swim speed boost when in water
            let dolphin_lvl = player.active_effects.iter()
                .find(|e| e.effect.id() == 30)
                .map(|e| e.amplifier + 1).unwrap_or(0);
            player.swim_speed_multiplier = 1.0 + 0.5 * dolphin_lvl as f32;

            // Blindness (15): prevent sprinting (same pattern as Invisibility)
            let blind_lvl = player.active_effects.iter()
                .find(|e| e.effect.id() == 15)
                .map(|e| e.amplifier + 1).unwrap_or(0);
            if blind_lvl > 0 {
                player.is_sprinting = false;
            }

            // Invisibility (14): reduce mob detection range — tracked in mob AI
            let invis_lvl = player.active_effects.iter()
                .find(|e| e.effect.id() == 14)
                .map(|e| e.amplifier + 1).unwrap_or(0);
            if invis_lvl > 0 {
                // Mobs ignore invisible players at > 8 blocks
                player.is_sprinting = false; // subtle: can't sprint while invisible
            }

            // HealthBoost (21): client renders extra hearts; server tracks level for damage calc
            // (damage absorption from extra HP is handled by health clamping in add_effect)

            // ConduitPower (29): grants water breathing while active — checked in env_damage tick
            // DolphinGrace (30): swim speed boost — handled client-side
            // JumpBoost (8): jump height — handled client-side
            // MiningFatigue (4): mining speed reduction — handled client-side
            // Darkness (33): applied when near Warden — mob AI handles this
            // Luck (26) / Unluck (27): fishing treasure% — handled in FishingManager.roll_loot()
            // Absorption: apply extra golden hearts when effect is active
            let absorption_lvl = player.active_effects.iter()
                .find(|e| e.effect.id() == 22)
                .map(|e| e.amplifier + 1).unwrap_or(0);
            if absorption_lvl > 0 && player.absorption_health <= 0.0 {
                player.absorption_health = 4.0 * absorption_lvl as f32;
            }

            // Levitation: float upward each tick
            let levitation_lvl = player.active_effects.iter()
                .find(|e| e.effect.id() == 25)
                .map(|e| e.amplifier + 1).unwrap_or(0);
            if levitation_lvl > 0 {
                player.position.y = (player.position.y + 0.05 * levitation_lvl as f64).min(256.0);
                let eid = player.entity_id;
                let px = player.position.x;
                let py = player.position.y;
                let pz = player.position.z;
                drop(player);
                let _ = self.player_state_tx.send(PlayerStateEvent {
                    uuid: *uuid,
                    kind: PlayerStateEventKind::Teleport(px, py, pz, 0.0, 0.0, eid),
                });
                // Re-acquire to process remaining effects
                if let Some(mut p) = self.players.get_mut(uuid) {
                    let mut expired = Vec::new();
                    for effect in &mut p.active_effects {
                        if effect.duration_ticks > 0 { effect.duration_ticks -= 1; }
                        if effect.duration_ticks == 0 { expired.push(effect.effect); }
                    }
                    if !expired.is_empty() {
                        p.active_effects.retain(|e| e.duration_ticks > 0);
                        for effect_type in expired {
                            if effect_type.id() == 22 { p.absorption_health = 0.0; }
                            tracing::debug!("Effect {:?} expired for player '{}'", effect_type, p.username);
                        }
                    }
                }
                continue;
            }

            // Decrement durations and collect expired
            let mut expired = Vec::new();
            for effect in &mut player.active_effects {
                if effect.duration_ticks > 0 {
                    effect.duration_ticks -= 1;
                }
                if effect.duration_ticks == 0 {
                    expired.push(effect.effect);
                }
            }
            if !expired.is_empty() {
                player.active_effects.retain(|e| e.duration_ticks > 0);
                for effect_type in expired {
                    tracing::debug!("Effect {:?} expired for player '{}'", effect_type, player.username);
                }
            }
            }
        }
    }

    /// Add XP to a player and sync to client
    pub fn add_xp(&self, uuid: &Uuid, amount: i32) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                p.xp_total += amount;
                // Calculate level from total XP using vanilla formula
                let mut level = 0i32;
                let mut xp_for_next;
                let mut remaining = p.xp_total;
                loop {
                    xp_for_next = if level < 16 {
                        2 * level + 7
                    } else if level < 31 {
                        5 * level - 38
                    } else {
                        9 * level - 158
                    };
                    if remaining >= xp_for_next {
                        remaining -= xp_for_next;
                        level += 1;
                    } else {
                        break;
                    }
                }
                p.xp_level = level;
                p.xp_bar = if xp_for_next > 0 { remaining as f32 / xp_for_next as f32 } else { 0.0 };
                let bar = p.xp_bar;
                let lvl = p.xp_level;
                let total = p.xp_total;
                let _ = self.player_state_tx.send(PlayerStateEvent {
                    uuid: *uuid,
                    kind: PlayerStateEventKind::XpUpdate(bar, lvl, total),
                });
                Ok(())
            }
            None => Err("Player not found".into()),
        }
    }

    /// 获取玩家活跃效果列表
    pub fn get_effects(&self, uuid: &Uuid) -> Result<Vec<mc_core::effect::ActiveEffect>, String> {
        match self.players.get(uuid).map(|r| r.clone()) {
            Some(p) => Ok(p.active_effects.clone()),
            None => Err("Player not found".into()),
        }
    }

    /// Get the enchantment level on the player's boots for a given enchant name.
    /// Returns 0 if no boots or enchant not present.
    pub fn get_boots_enchant(&self, uuid: &Uuid, enchant_name: &str) -> u8 {
        self.players.get(uuid).and_then(|p| {
            p.inventory.armor.first().and_then(|opt| opt.as_ref())
                .and_then(|stack| stack.nbt.as_ref())
                .map(|nbt| crate::enchant::enchant_level(&Some(nbt.clone()), enchant_name))
        }).unwrap_or(0)
    }

    /// Compute movement modifier from boot enchantments.
    /// Returns (frost_walker_level, soul_speed_level, swift_sneak_level).
    pub fn get_boot_enchant_levels(&self, uuid: &Uuid) -> (u8, u8, u8) {
        self.players.get(uuid).and_then(|p| {
            p.inventory.armor.first().and_then(|opt| opt.as_ref())
                .and_then(|stack| stack.nbt.as_ref())
                .map(|nbt| {
                    let nbt_opt = Some(nbt.clone());
                    (
                        crate::enchant::enchant_level(&nbt_opt, "frost_walker"),
                        crate::enchant::enchant_level(&nbt_opt, "soul_speed"),
                        crate::enchant::enchant_level(&nbt_opt, "swift_sneak"),
                    )
                })
        }).unwrap_or((0, 0, 0))
    }

    /// 获取玩家当前手持物品
    pub fn get_held_item(&self, uuid: &Uuid) -> Option<ItemStack> {
        self.players.get(uuid).and_then(|p| {
            p.inventory.items.get(p.inventory.selected_slot as usize)
                .and_then(|opt| opt.clone())
        })
    }

    /// 减少手持物品耐久, 耐久归零时物品破碎
    pub fn damage_held_item(&self, uuid: &Uuid, amount: u16) -> bool {
        if let Some(mut p) = self.players.get_mut(uuid) {
            let slot = p.inventory.selected_slot as usize;
            if let Some(item_slot) = p.inventory.items.get_mut(slot) {
                let mut should_break = false;
                if let Some(stack) = item_slot
                    && let Some(dur) = &mut stack.durability {
                        if *dur <= amount {
                            should_break = true;
                        } else {
                            *dur -= amount;
                        }
                    }
                if should_break {
                    *item_slot = None;
                    return true;
                }
            }
        }
        false
    }

    /// 修复手持物品的耐久度
    pub fn repair_held_item(&self, uuid: &Uuid, amount: u16) -> bool {
        if let Some(mut p) = self.players.get_mut(uuid) {
            let slot = p.inventory.selected_slot as usize;
            if let Some(item_slot) = p.inventory.items.get_mut(slot)
                && let Some(stack) = item_slot
                    && let Some(ref mut dur) = stack.durability {
                        *dur = dur.saturating_sub(amount);
                        return true;
                    }
        }
        false
    }

    /// 更新手持物品的 NBT 数据
    pub fn update_held_item_nbt(&self, uuid: &Uuid, nbt: Option<Vec<u8>>) {
        if let Some(mut p) = self.players.get_mut(uuid) {
            let slot = p.inventory.selected_slot as usize;
            if let Some(item_slot) = p.inventory.items.get_mut(slot)
                && let Some(item) = item_slot {
                    item.nbt = nbt;
                }
        }
    }

    /// 获取玩家当前手持槽位索引
    pub fn get_held_slot(&self, uuid: &Uuid) -> Option<u8> {
        self.players.get(uuid).map(|p| p.inventory.selected_slot)
    }

    /// 获取玩家光标物品 (容器交互时鼠标上拿着的物品)
    pub fn get_cursor_item(&self, uuid: &Uuid) -> Option<ItemStack> {
        self.players.get(uuid).and_then(|p| p.cursor_item.clone())
    }

    /// 设置玩家光标物品
    pub fn set_cursor_item(&self, uuid: &Uuid, item: Option<ItemStack>) {
        if let Some(mut p) = self.players.get_mut(uuid) {
            p.cursor_item = item;
        }
    }

    /// 设置玩家背包指定槽位物品
    pub fn set_inventory_slot(&self, uuid: &Uuid, slot: u8, item: Option<ItemStack>) {
        if let Some(mut p) = self.players.get_mut(uuid)
            && (slot as usize) < p.inventory.items.len() {
                p.inventory.items[slot as usize] = item;
            }
    }

    /// Get enchantment level from a specific armor slot (36=boots, 37=leggings, 38=chestplate, 39=helmet)
    pub fn get_armor_enchant_level(&self, uuid: &Uuid, slot: usize, enchant_name: &str) -> u32 {
        let nbt_data = self.players.get(uuid)
            .and_then(|p| p.inventory.items.get(slot).and_then(|o| o.as_ref().and_then(|s| s.nbt.clone())));
        match nbt_data {
            Some(nbt) => {
                let enchants = crate::enchant::parse_item_enchants(&Some(nbt));
                enchants.get(enchant_name).copied().unwrap_or(0) as u32
            }
            None => 0,
        }
    }

    /// Check if player is elytra gliding
    pub fn is_flying(&self, uuid: &Uuid) -> bool {
        self.players.get(uuid).map(|p| p.is_flying).unwrap_or(false)
    }

    /// Set elytra gliding state
    pub fn set_flying(&self, uuid: &Uuid, flying: bool) {
        if let Some(mut p) = self.players.get_mut(uuid) {
            p.is_flying = flying;
            if !flying {
                p.fall_distance = 0.0; // reset fall distance on landing/stop
            }
        }
    }

    /// Set player speed multiplier (enchantment-based, resets each tick)
    pub fn set_speed_multiplier(&self, uuid: &Uuid, mult: f32) {
        if let Some(mut p) = self.players.get_mut(uuid) {
            p.speed_multiplier = mult;
        }
    }

    /// Apply durability damage to boots (for Soul Speed, Frost Walker)
    pub fn damage_boots(&self, uuid: &Uuid, amount: u16) -> bool {
        if let Some(mut p) = self.players.get_mut(uuid)
            && let Some(ref mut stack) = p.inventory.armor.get_mut(0).and_then(|opt| opt.as_mut())
        {
            if stack.durability.is_some() {
                let current = stack.durability.unwrap_or(0);
                if current <= amount {
                    // Boots break
                    p.inventory.armor[0] = None;
                    return true; // broken
                }
                stack.durability = Some(current - amount);
            }
            return false;
        }
        false
    }

    /// Set pending teleport ID for TeleportConfirm validation
    pub fn set_pending_teleport(&self, uuid: &Uuid, teleport_id: i32) {
        if let Some(mut p) = self.players.get_mut(uuid) {
            p.pending_teleport_id = Some(teleport_id);
        }
    }

    /// Clear pending teleport after client confirmation
    pub fn clear_pending_teleport(&self, uuid: &Uuid) {
        if let Some(mut p) = self.players.get_mut(uuid) {
            p.pending_teleport_id = None;
        }
    }

    /// Store client view distance from ClientInformation packet
    pub fn set_client_view_distance(&self, uuid: &Uuid, view_distance: u8, locale: &str) {
        if let Some(mut p) = self.players.get_mut(uuid) {
            p.client_view_distance = view_distance;
            p.client_locale = locale.to_string();
        }
    }

    /// Store last acknowledged message count for chat signing
    pub fn set_acknowledged_count(&self, uuid: &Uuid, count: i32) {
        if let Some(mut p) = self.players.get_mut(uuid) {
            p.last_acknowledged_count = count;
        }
    }

    /// Store book content in item NBT at specific slot (EditBook 0x0E)
    pub fn update_item_nbt_at_slot(&self, uuid: &Uuid, slot: i32, text: &str) -> Result<(), String> {
        if let Some(mut p) = self.players.get_mut(uuid) {
            if slot >= 0 && (slot as usize) < p.inventory.items.len() {
                let nbt_bytes = format!("pages:{}", text).into_bytes();
                let idx = slot as usize;
                if let Some(ref mut stack) = p.inventory.items[idx] {
                    stack.nbt = Some(nbt_bytes);
                }
            }
            Ok(())
        } else { Err("Player not found".into()) }
    }

    /// Store custom display name on held item (RenameItem 0x12)
    pub fn set_held_item_name(&self, uuid: &Uuid, name: &str) -> Result<(), String> {
        if let Some(mut p) = self.players.get_mut(uuid) {
            let idx = p.inventory.selected_slot as usize;
            let nbt_bytes = format!("display:{{\"Name\":\"{}\"}}", name).into_bytes();
            if let Some(ref mut stack) = p.inventory.items[idx] {
                stack.nbt = Some(nbt_bytes);
            }
            Ok(())
        } else { Err("Player not found".into()) }
    }

    /// Track player's unlocked recipe IDs (RecipeBookData 0x13)
    pub fn add_known_recipe(&self, uuid: &Uuid, recipe_id: u16) {
        if let Some(mut p) = self.players.get_mut(uuid)
            && p.known_recipes.len() < 500 {
                p.known_recipes.insert(recipe_id);
            }
    }

    /// Apply boat paddle input (PaddleBoat 0x19)
    pub fn apply_boat_paddle(&self, uuid: &Uuid, eid: i32, left: bool, right: bool) {
        if let Some(p) = self.players.get(uuid) {
            let mul: f64 = if left && right { 0.15 } else if left || right { 0.08 } else { 0.0 };
            if mul > 0.0 {
                let angle = p.position.yaw.to_radians() as f64;
                let vx = -angle.sin() * mul;
                let vz = angle.cos() * mul;
                let new_x = p.position.x + vx;
                let new_z = p.position.z + vz;
                let _ = self.update_position_full(uuid, new_x, p.position.y, new_z, p.position.yaw, p.position.pitch);
                self.broadcast_entity_move(eid, *uuid, new_x, p.position.y, new_z, p.position.yaw, p.position.pitch);
            }
        }
    }

    /// Apply elytra air drag velocity adjustment
    pub fn set_flying_velocity(&self, _uuid: &Uuid, _vx: f64, _vy: f64, _vz: f64) {
        // Elytra velocity is applied by modifying position directly in the tick loop
        // This is a placeholder for future physics integration
    }

    /// 从指定槽位移除 1 个物品 (食用/使用后)
    pub fn remove_one_from_slot(&self, uuid: &Uuid, slot: u8) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                if let Some(ref mut item) = p.inventory.items.get_mut(slot as usize).and_then(|o| o.as_mut()) {
                    if item.count > 1 {
                        item.count -= 1;
                    } else {
                        p.inventory.items[slot as usize] = None;
                    }
                }
                Ok(())
            }
            None => Err("Player not found".into()),
        }
    }

    /// 从玩家背包指定槽位移除物品
    pub fn remove_item_from_slot(&self, uuid: &Uuid, _slot: u8, item: mc_core::block::BlockState, count: u32) -> Result<u32, String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                // Check the specific slot first, then fall back to scanning
                let slot_idx = _slot as usize;
                if slot_idx < p.inventory.items.len()
                    && let Some(ref stack) = p.inventory.items[slot_idx]
                        && stack.item == item {
                            let take = count.min(stack.count as u32);
                            let mut new_stack = stack.clone();
                            new_stack.count -= take as u8;
                            p.inventory.items[slot_idx] = if new_stack.count > 0 { Some(new_stack) } else { None };
                            return Ok(take);
                        }
                Ok(0) // nothing removed from this slot
            }
            None => Err("Player not found".into()),
        }
    }

    /// 获取玩家背包指定槽位的物品
    pub fn get_inventory_slot(&self, uuid: &Uuid, slot: u8) -> Option<crate::inventory::ItemStack> {
        self.players
            .get(uuid)
            .and_then(|p| p.inventory.items.get(slot as usize).and_then(|o| o.clone()))
    }

    /// 添加物品到玩家背包
    pub fn add_item_to_player(&self, uuid: &Uuid, item: mc_core::block::BlockState, count: u32) -> Result<u32, String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                let leftover = p.inventory.add_item(item, count);
                Ok(count - leftover) // number actually added
            }
            None => Err("Player not found".into()),
        }
    }

    /// 从背包任意位置移除指定物品 (扫描全部槽位)
    pub fn remove_item(&self, uuid: &Uuid, item: mc_core::block::BlockState, count: u32) -> Result<u32, String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => Ok(p.inventory.remove_item(item, count)),
            None => Err("Player not found".into()),
        }
    }

    /// 设置玩家背包选中槽位
    pub fn set_selected_slot(&self, uuid: &Uuid, slot: u8) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) if (slot as usize) < 36 => {
                p.inventory.selected_slot = slot;
                Ok(())
            }
            Some(_) => Err("Invalid slot".into()),
            None => Err("Player not found".into()),
        }
    }

    /// 添加物品到背包 (接受 ItemStack)
    pub fn add_item(&self, uuid: &Uuid, stack: crate::inventory::ItemStack) -> Result<u32, String> {
        self.add_item_to_player(uuid, stack.item, stack.count as u32)
    }

    /// 设置玩家钓鱼状态
    pub fn set_fishing(&self, uuid: &Uuid, state: crate::fishing::FishingState) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => { p.fishing = Some(state); Ok(()) }
            None => Err("Player not found".into()),
        }
    }

    /// 清除玩家钓鱼状态
    pub fn clear_fishing(&self, uuid: &Uuid) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => { p.fishing = None; Ok(()) }
            None => Err("Player not found".into()),
        }
    }

    // ── Ban management ──

    /// 封禁玩家
    pub fn ban(&self, uuid: Uuid) {
        self.banned.write().insert(uuid);
    }

    /// 解封玩家
    pub fn unban(&self, uuid: &Uuid) {
        self.banned.write().remove(uuid);
    }

    /// 检查是否被封禁
    pub fn is_banned(&self, uuid: &Uuid) -> bool {
        self.banned.read().contains(uuid)
    }

    /// 获取所有被封禁的 UUID
    pub fn get_banned(&self) -> Vec<Uuid> {
        self.banned.read().iter().copied().collect()
    }

    /// 获取白名单条目
    pub fn get_whitelist_entries(&self) -> Vec<Uuid> {
        self.whitelist.read().1.iter().copied().collect()
    }

    // ── Whitelist management ──

    /// 启用/禁用白名单
    pub fn set_whitelist_enabled(&self, enabled: bool) {
        self.whitelist.write().0 = enabled;
    }

    /// 白名单是否启用
    pub fn is_whitelist_enabled(&self) -> bool {
        self.whitelist.read().0
    }

    /// 添加玩家到白名单
    pub fn add_whitelist(&self, uuid: Uuid) {
        self.whitelist.write().1.insert(uuid);
    }

    /// 从白名单移除玩家
    pub fn remove_whitelist(&self, uuid: &Uuid) {
        self.whitelist.write().1.remove(uuid);
    }

    /// 检查玩家是否在白名单中
    pub fn is_whitelisted(&self, uuid: &Uuid) -> bool {
        let wl = self.whitelist.read();
        !wl.0 || wl.1.contains(uuid) // if disabled, everyone allowed
    }

    /// 注册 ECS 实体 (在 ECS World 中 spawn 后调用)
    pub fn register_entity(&self, bevy_entity_id: u32, uuid: Uuid) {
        self.player_entities.write().insert(bevy_entity_id, uuid);
    }

    /// 取消注册 ECS 实体
    pub fn unregister_entity(&self, bevy_entity_id: u32) {
        self.player_entities.write().remove(&bevy_entity_id);
    }

    /// 根据 ECS 实体 ID 获取 UUID
    pub fn uuid_by_entity(&self, bevy_entity_id: u32) -> Option<Uuid> {
        self.player_entities.read().get(&bevy_entity_id).copied()
    }

    /// 获取所有 ECS 实体映射的快照
    pub fn all_entity_mappings(&self) -> Vec<(u32, Uuid)> {
        self.player_entities.read().iter().map(|(k, v)| (*k, *v)).collect()
    }

    /// 设置玩家完整背包
    pub fn set_inventory(&self, uuid: &Uuid, inv: crate::inventory::Inventory) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => { p.inventory = inv; Ok(()) }
            None => Err("Player not found".into()),
        }
    }

    /// 设置玩家食物值
    pub fn set_food(&self, uuid: &Uuid, level: i32, saturation: f32) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                p.food_level = level.clamp(0, 20);
                p.food_saturation = saturation.clamp(0.0, 20.0);
                Ok(())
            }
            None => Err("Player not found".into()),
        }
    }

    /// 消耗经验等级 (附魔/铁砧 等), 不足时返回 Err
    pub fn remove_xp_levels(&self, uuid: &Uuid, levels: i32) -> Result<(), String> {
        match self.players.get_mut(uuid) {
            Some(mut p) => {
                if p.xp_level < levels {
                    return Err(format!("Need {} levels, have {}", levels, p.xp_level));
                }
                // Deduct XP: calculate total XP needed for 'levels' levels from current
                let mut xp_to_remove = 0i32;
                let mut lvl = p.xp_level;
                for _ in 0..levels {
                    let xp_for_lvl = if lvl <= 16 { 2 * (lvl - 1) + 7 }
                        else if lvl <= 31 { 5 * lvl - 38 }
                        else { 9 * lvl - 158 };
                    xp_to_remove += xp_for_lvl;
                    lvl -= 1;
                }
                p.xp_total = (p.xp_total - xp_to_remove).max(0);
                p.xp_level -= levels;
                // Recalculate bar
                let xp_for_next = if p.xp_level < 16 { 2 * p.xp_level + 7 }
                    else if p.xp_level < 31 { 5 * p.xp_level - 38 }
                    else { 9 * p.xp_level - 158 };
                if xp_for_next > 0 {
                    // Calculate remaining XP toward next level
                    let mut remaining = p.xp_total;
                    for l in 0..p.xp_level {
                        let cost = if l < 16 { 2 * l + 7 } else if l < 31 { 5 * l - 38 } else { 9 * l - 158 };
                        remaining -= cost;
                    }
                    p.xp_bar = (remaining as f32 / xp_for_next as f32).clamp(0.0, 1.0);
                } else {
                    p.xp_bar = 0.0;
                }
                let _ = self.player_state_tx.send(PlayerStateEvent {
                    uuid: *uuid,
                    kind: PlayerStateEventKind::XpUpdate(p.xp_bar, p.xp_level, p.xp_total),
                });
                Ok(())
            }
            None => Err("Player not found".into()),
        }
    }

    /// 消耗青金石 (附魔台), 返回是否成功
    pub fn remove_lapis(&self, uuid: &Uuid, count: u32) -> bool {
        let lapis_id = mc_core::block::BlockState::new(571); // lapis_lazuli item
        if let Some(mut p) = self.players.get_mut(uuid) {
            let removed = p.inventory.remove_item(lapis_id, count);
            removed >= count
        } else { false }
    }
}

/// 可共享的玩家管理器引用
pub type SharedPlayerManager = Arc<PlayerManager>;

/// 计算护甲减免后的伤害 (原版公式)
/// 有效护甲 = min(20, max(armor_points - 4*damage/(toughness+8), armor_points * 0.2))
/// 减免倍数 = 1 - 有效护甲 / 25
pub fn calculate_armor_reduction(armor_points: f32, armor_toughness: f32, raw_damage: f32) -> f32 {
    if raw_damage <= 0.0 { return 0.0; }
    let effective = (armor_points - 4.0 * raw_damage / (armor_toughness + 8.0))
        .max(armor_points * 0.2)
        .min(20.0);
    let reduction = 1.0 - effective / 25.0;
    (raw_damage * reduction).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_uuid(name: &str) -> Uuid {
        mc_core::auth::offline_uuid(name)
    }

    #[test]
    fn test_ban_unban() {
        let pm = PlayerManager::new();
        let uuid = test_uuid("BadPlayer");
        assert!(!pm.is_banned(&uuid));
        pm.ban(uuid);
        assert!(pm.is_banned(&uuid));
        pm.unban(&uuid);
        assert!(!pm.is_banned(&uuid));
    }

    #[test]
    fn test_whitelist() {
        let pm = PlayerManager::new();
        let uuid = test_uuid("GoodPlayer");
        // Whitelist disabled by default → everyone allowed
        assert!(pm.is_whitelisted(&uuid));

        pm.set_whitelist_enabled(true);
        assert!(!pm.is_whitelisted(&uuid));

        pm.add_whitelist(uuid);
        assert!(pm.is_whitelisted(&uuid));

        pm.remove_whitelist(&uuid);
        assert!(!pm.is_whitelisted(&uuid));
    }

    #[test]
    fn test_player_mutation() {
        let pm = PlayerManager::new();
        let uuid = test_uuid("TestPlayer");
        let _player = pm.add_player(uuid, "TestPlayer".into());

        // OP
        assert!(pm.set_op(&uuid, true).is_ok());
        assert!(pm.get(&uuid).unwrap().is_op);

        // Gamemode
        assert!(pm.set_gamemode(&uuid, mc_core::types::GameMode::Creative).is_ok());
        assert_eq!(pm.get(&uuid).unwrap().gamemode, mc_core::types::GameMode::Creative);

        // Health
        assert!(pm.set_health(&uuid, 5.0).is_ok());
        assert_eq!(pm.get(&uuid).unwrap().health, 5.0);

        // Position
        assert!(pm.update_position(&uuid, 100.0, 64.0, -200.0).is_ok());
        let p = pm.get(&uuid).unwrap();
        assert_eq!(p.position.x, 100.0);
        assert_eq!(p.position.z, -200.0);
    }

    #[test]
    fn test_held_item() {
        let pm = PlayerManager::new();
        let uuid = test_uuid("Builder");
        pm.add_player(uuid, "Builder".into());

        // Default: no item in slot 0
        assert!(pm.get_held_item(&uuid).is_none());
    }

    #[test]
    fn test_online_count() {
        let pm = PlayerManager::new();
        assert_eq!(pm.online_count(), 0);

        let u1 = test_uuid("P1");
        let u2 = test_uuid("P2");
        pm.add_player(u1, "P1".into());
        pm.add_player(u2, "P2".into());
        assert_eq!(pm.online_count(), 2);

        pm.remove_player(&u1);
        assert_eq!(pm.online_count(), 1);
    }

    #[test]
    fn test_get_banned() {
        let pm = PlayerManager::new();
        assert!(pm.get_banned().is_empty());

        let u1 = test_uuid("Bad1");
        let u2 = test_uuid("Bad2");
        pm.ban(u1);
        pm.ban(u2);
        let banned = pm.get_banned();
        assert_eq!(banned.len(), 2);
        assert!(banned.contains(&u1));
        assert!(banned.contains(&u2));

        pm.unban(&u1);
        let banned = pm.get_banned();
        assert_eq!(banned.len(), 1);
        assert!(!banned.contains(&u1));
    }

    #[test]
    fn test_get_whitelist_entries() {
        let pm = PlayerManager::new();
        assert!(pm.get_whitelist_entries().is_empty());

        let u = test_uuid("GoodPlayer");
        pm.add_whitelist(u);
        let entries = pm.get_whitelist_entries();
        assert_eq!(entries.len(), 1);
        assert!(entries.contains(&u));

        pm.remove_whitelist(&u);
        assert!(pm.get_whitelist_entries().is_empty());
    }

    #[test]
    fn test_player_state_event_health() {
        let pm = PlayerManager::new();
        let uuid = test_uuid("StateTest");
        pm.add_player(uuid, "StateTest".into());

        let mut rx = pm.subscribe_player_state();
        // set_health should trigger a PlayerStateEvent
        pm.set_health(&uuid, 10.0).ok();
        let ev = rx.try_recv().expect("Should receive HealthUpdate event");
        assert_eq!(ev.uuid, uuid);
        if let PlayerStateEventKind::HealthUpdate(h) = ev.kind {
            assert_eq!(h, 10.0);
        } else {
            panic!("Expected HealthUpdate, got {:?}", ev.kind);
        }
    }

    #[test]
    fn test_player_state_event_gamemode() {
        let pm = PlayerManager::new();
        let uuid = test_uuid("GMTest");
        pm.add_player(uuid, "GMTest".into());

        let mut rx = pm.subscribe_player_state();
        pm.set_gamemode(&uuid, mc_core::types::GameMode::Creative).ok();
        let ev = rx.try_recv().expect("Should receive GamemodeUpdate event");
        assert_eq!(ev.uuid, uuid);
        if let PlayerStateEventKind::GamemodeUpdate(gm) = ev.kind {
            assert_eq!(gm, mc_core::types::GameMode::Creative);
        } else {
            panic!("Expected GamemodeUpdate, got {:?}", ev.kind);
        }
    }
}
