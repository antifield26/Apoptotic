//! 统一保存管理器

use crate::player_data::PlayerDatabase;
use crate::world_save::WorldSaver;
use mc_world::world::World;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info};

/// 保存管理器 — 单线程设计，避免 rusqlite Send 问题
pub struct SaveManager {
    player_db: PlayerDatabase,
    world_saver: WorldSaver,
    base_path: PathBuf,
}

impl SaveManager {
    pub fn new(base_path: &Path, player_db_url: &str) -> Result<Self, String> {
        let db = PlayerDatabase::open(player_db_url)
            .map_err(|e| format!("Failed to open player database: {}", e))?;
        std::fs::create_dir_all(base_path).ok();

        info!("SaveManager: path={}", base_path.display());

        Ok(Self {
            player_db: db,
            world_saver: WorldSaver::new(),
            base_path: base_path.to_path_buf(),
        })
    }

    /// 保存世界数据
    pub fn save_world(&self, world: &World) {
        let world_path = self.base_path.join(&world.level_name);
        match self.world_saver.save_world(world, world_path.to_str().unwrap_or("world")) {
            Ok(count) => {
                if count > 0 {
                    info!("Saved {} chunks to {}", count, world_path.display());
                }
            }
            Err(e) => error!("Failed to save world: {}", e),
        }
    }

    /// 保存玩家完整状态
    pub fn save_player(&self, uuid: &uuid::Uuid, username: &str) {
        let uuid_str = uuid.to_string();
        if let Err(e) = self.player_db.conn.execute(
            "INSERT OR REPLACE INTO players (uuid, username, last_login)
             VALUES (?1, ?2, CURRENT_TIMESTAMP)",
            rusqlite::params![uuid_str, username],
        ) {
            error!("Failed to save player {}: {}", uuid, e);
        } else {
            debug!("Saved player data for {}", username);
        }
    }

    /// 保存玩家完整状态 (含位置、生命值、饥饿值、游戏模式、朝向)
    #[allow(clippy::too_many_arguments)]
    pub fn save_player_full(
        &self,
        uuid: &uuid::Uuid,
        username: &str,
        health: f32,
        food: i32,
        saturation: f32,
        gamemode: u8,
        pos_x: f64,
        pos_y: f64,
        pos_z: f64,
        yaw: f32,
        pitch: f32,
        is_op: bool,
        inventory_blob: Option<Vec<u8>>,
    ) {
        let uuid_str = uuid.to_string();
        if let Err(e) = self.player_db.conn.execute(
            "INSERT OR REPLACE INTO players
             (uuid, username, health, food, saturation, gamemode, pos_x, pos_y, pos_z, yaw, pitch, is_op, inventory, last_login)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, CURRENT_TIMESTAMP)",
            rusqlite::params![
                uuid_str,
                username,
                health,
                food,
                saturation,
                gamemode as i32,
                pos_x,
                pos_y,
                pos_z,
                yaw,
                pitch,
                is_op as i32,
                inventory_blob.unwrap_or_default(),
            ],
        ) {
            error!("Failed to save full player data for {}: {}", username, e);
        } else {
            debug!("Saved full state for {}", username);
        }
    }

    /// 尝试加载玩家数据
    pub fn load_player(&self, uuid: &uuid::Uuid) -> Option<crate::player_data::PlayerRow> {
        self.player_db.load_player(*uuid)
    }

    /// 加载所有被封禁的 UUID
    pub fn load_banned_uuids(&self) -> Vec<uuid::Uuid> {
        let mut stmt = match self.player_db.conn.prepare("SELECT uuid FROM bans") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([], |row| {
            let uuid_str: String = row.get(0)?;
            Ok(uuid::Uuid::parse_str(&uuid_str).unwrap_or(uuid::Uuid::nil()))
        })
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// 加载所有白名单 UUID
    pub fn load_whitelist_uuids(&self) -> Vec<uuid::Uuid> {
        let mut stmt = match self.player_db.conn.prepare("SELECT uuid FROM whitelist") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([], |row| {
            let uuid_str: String = row.get(0)?;
            Ok(uuid::Uuid::parse_str(&uuid_str).unwrap_or(uuid::Uuid::nil()))
        })
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// 加载所有 OP UUID（返回 UUID → level 映射）
    pub fn load_ops(&self) -> std::collections::HashMap<uuid::Uuid, i32> {
        let mut stmt = match self.player_db.conn.prepare("SELECT uuid, level FROM ops") {
            Ok(s) => s,
            Err(_) => return std::collections::HashMap::new(),
        };
        stmt.query_map([], |row| {
            let uuid_str: String = row.get(0)?;
            let level: i32 = row.get(1)?;
            Ok((uuid::Uuid::parse_str(&uuid_str).unwrap_or(uuid::Uuid::nil()), level))
        })
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// 持久化封禁列表（全量同步）
    pub fn persist_bans(&self, banned: &[uuid::Uuid]) {
        if let Err(e) = self.player_db.conn.execute("DELETE FROM bans", []) {
            error!("Failed to clear bans: {}", e);
            return;
        }
        for uuid in banned {
            if let Err(e) = self.player_db.conn.execute(
                "INSERT OR REPLACE INTO bans (uuid) VALUES (?1)",
                rusqlite::params![uuid.to_string()],
            ) {
                error!("Failed to persist ban {}: {}", uuid, e);
            }
        }
        debug!("Persisted {} bans", banned.len());
    }

    /// 持久化白名单（全量同步）
    pub fn persist_whitelist(&self, entries: &[uuid::Uuid]) {
        if let Err(e) = self.player_db.conn.execute("DELETE FROM whitelist", []) {
            error!("Failed to clear whitelist: {}", e);
            return;
        }
        for uuid in entries {
            if let Err(e) = self.player_db.conn.execute(
                "INSERT OR REPLACE INTO whitelist (uuid, username) VALUES (?1, '')",
                rusqlite::params![uuid.to_string()],
            ) {
                error!("Failed to persist whitelist {}: {}", uuid, e);
            }
        }
        debug!("Persisted {} whitelist entries", entries.len());
    }

    /// 持久化 OP 状态
    pub fn persist_op(&self, uuid: &uuid::Uuid, is_op: bool) {
        if is_op {
            if let Err(e) = self.player_db.conn.execute(
                "INSERT OR REPLACE INTO ops (uuid, level) VALUES (?1, 1)",
                rusqlite::params![uuid.to_string()],
            ) {
                error!("Failed to persist op {}: {}", uuid, e);
            }
        } else {
            if let Err(e) = self.player_db.conn.execute(
                "DELETE FROM ops WHERE uuid = ?1",
                rusqlite::params![uuid.to_string()],
            ) {
                error!("Failed to remove op {}: {}", uuid, e);
            }
        }
    }

    /// 加载所有玩家存档数据（返回 UUID → PlayerRow 映射，用于登录时恢复状态）
    pub fn load_all_player_data(&self) -> std::collections::HashMap<uuid::Uuid, crate::player_data::PlayerRow> {
        let mut stmt = match self.player_db.conn.prepare(
            "SELECT uuid, username, health, food, saturation, gamemode,
                    pos_x, pos_y, pos_z, yaw, pitch, is_op, inventory FROM players"
        ) {
            Ok(s) => s,
            Err(_) => return std::collections::HashMap::new(),
        };
        stmt.query_map([], |row| {
            let uuid_str: String = row.get(0)?;
            let uuid = uuid::Uuid::parse_str(&uuid_str).unwrap_or(uuid::Uuid::nil());
            Ok((uuid, crate::player_data::PlayerRow {
                username: row.get(1)?,
                health: row.get(2)?,
                food: row.get(3)?,
                saturation: row.get(4)?,
                gamemode: row.get::<_, i32>(5)? as u8,
                pos_x: row.get(6)?,
                pos_y: row.get(7)?,
                pos_z: row.get(8)?,
                yaw: row.get(9)?,
                pitch: row.get(10)?,
                is_op: row.get::<_, i32>(11)? != 0,
                inventory_blob: row.get(12).ok(),
            }))
        })
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }
}
