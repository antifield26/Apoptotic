//! 玩家数据持久化 (SQLite)

use rusqlite::{params, Connection};
use tracing::info;
use uuid::Uuid;

/// 玩家数据库
pub struct PlayerDatabase {
    pub(crate) conn: Connection,
}

impl PlayerDatabase {
    /// 打开或创建数据库
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        let db_path = path.strip_prefix("sqlite://").unwrap_or(path);
        if let Some(parent) = std::path::Path::new(db_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(db_path)?;
        let db = Self { conn };
        db.init_tables()?;

        info!("Player database opened: {}", db_path);
        Ok(db)
    }

    fn init_tables(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS players (
                uuid TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                health REAL DEFAULT 20.0,
                food INTEGER DEFAULT 20,
                saturation REAL DEFAULT 5.0,
                gamemode INTEGER DEFAULT 0,
                pos_x REAL DEFAULT 0.0,
                pos_y REAL DEFAULT 64.0,
                pos_z REAL DEFAULT 0.0,
                yaw REAL DEFAULT 0.0,
                pitch REAL DEFAULT 0.0,
                is_op INTEGER DEFAULT 0,
                inventory BLOB,
                last_login TEXT DEFAULT CURRENT_TIMESTAMP,
                first_login TEXT DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS ops (
                uuid TEXT PRIMARY KEY,
                level INTEGER DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS bans (
                uuid TEXT PRIMARY KEY,
                reason TEXT,
                banned_at TEXT DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS whitelist (
                uuid TEXT PRIMARY KEY,
                username TEXT NOT NULL
            );",
        )
    }

    /// 加载玩家数据
    pub fn load_player(&self, uuid: Uuid) -> Option<PlayerRow> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT username, health, food, saturation, gamemode,
                        pos_x, pos_y, pos_z, yaw, pitch, is_op
                 FROM players WHERE uuid = ?1",
            )
            .ok()?;

        stmt.query_row(params![uuid.to_string()], |row| {
            Ok(PlayerRow {
                username: row.get(0)?,
                health: row.get(1)?,
                food: row.get(2)?,
                saturation: row.get(3)?,
                gamemode: row.get::<_, i32>(4)? as u8,
                pos_x: row.get(5)?,
                pos_y: row.get(6)?,
                pos_z: row.get(7)?,
                yaw: row.get(8)?,
                pitch: row.get(9)?,
                is_op: row.get::<_, i32>(10)? != 0,
                inventory_blob: row.get(11).ok(),
            })
        })
        .ok()
    }

    /// 通过 PlayerManager 保存玩家数据
    /// PlayerManager 需要扩展以提供数据访问
    pub fn save_player_from_manager(&self, uuid: &Uuid) -> Result<(), rusqlite::Error> {
        // 简化实现：存储占位数据
        // 实际项目中需要从 PlayerManager 读取完整玩家状态
        self.conn.execute(
            "INSERT OR REPLACE INTO players
             (uuid, username, health, food, saturation, gamemode,
              pos_x, pos_y, pos_z, yaw, pitch, is_op, last_login)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, CURRENT_TIMESTAMP)",
            params![
                uuid.to_string(),
                "(offline)",    // placeholder — will be updated by real impl
                20.0_f32, 20_i32, 5.0_f32, 0_i32,
                0.0_f64, 64.0_f64, 0.0_f64, 0.0_f32, 0.0_f32,
                0_i32,
            ],
        )?;
        Ok(())
    }

    /// 保存玩家完整数据 (含背包 BLOB)
    pub fn save_player_full(&self, uuid: &Uuid, row: &PlayerRow) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT OR REPLACE INTO players
             (uuid, username, health, food, saturation, gamemode,
              pos_x, pos_y, pos_z, yaw, pitch, is_op, inventory, last_login)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, CURRENT_TIMESTAMP)",
            params![
                uuid.to_string(),
                row.username.clone(),
                row.health,
                row.food,
                row.saturation,
                row.gamemode as i32,
                row.pos_x,
                row.pos_y,
                row.pos_z,
                row.yaw,
                row.pitch,
                row.is_op as i32,
                row.inventory_blob.clone().unwrap_or_default(),
            ],
        )?;
        Ok(())
    }

    /// 加载玩家背包 BLOB
    pub fn load_inventory(&self, uuid: Uuid) -> Option<Vec<u8>> {
        self.conn.query_row(
            "SELECT inventory FROM players WHERE uuid = ?1",
            params![uuid.to_string()],
            |row| row.get(0),
        ).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_db() -> PlayerDatabase {
        PlayerDatabase::open("sqlite://:memory:").expect("open in-memory DB")
    }

    #[test]
    fn test_open_in_memory() {
        let db = in_memory_db();
        // Verify tables exist by inserting and querying
        db.conn.execute(
            "INSERT INTO bans (uuid, reason) VALUES (?1, ?2)",
            rusqlite::params!["test-uuid", "test reason"],
        ).expect("insert ban");
        let count: i32 = db.conn
            .query_row("SELECT COUNT(*) FROM bans", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_save_and_load_player() {
        let db = in_memory_db();
        let uuid = uuid::Uuid::new_v4();
        let row = PlayerRow {
            username: "Notch".into(),
            health: 20.0,
            food: 20,
            saturation: 5.0,
            gamemode: 0,
            pos_x: 100.0,
            pos_y: 64.0,
            pos_z: -50.0,
            yaw: 45.0,
            pitch: 0.0,
            is_op: true,
            inventory_blob: None,
        };
        db.save_player_full(&uuid, &row).expect("save");
        let loaded = db.load_player(uuid).expect("load");
        assert_eq!(loaded.username, "Notch");
        assert_eq!(loaded.health, 20.0);
        assert_eq!(loaded.gamemode, 0);
        assert_eq!(loaded.pos_x, 100.0);
        assert_eq!(loaded.pos_y, 64.0);
        assert_eq!(loaded.pos_z, -50.0);
        assert!(loaded.is_op);
    }

    #[test]
    fn test_load_nonexistent_player() {
        let db = in_memory_db();
        assert!(db.load_player(uuid::Uuid::nil()).is_none());
    }

    #[test]
    fn test_update_existing_player() {
        let db = in_memory_db();
        let uuid = uuid::Uuid::new_v4();
        let row1 = PlayerRow {
            username: "Steve".into(),
            health: 20.0,
            food: 20,
            saturation: 5.0,
            gamemode: 0,
            pos_x: 0.0,
            pos_y: 64.0,
            pos_z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            is_op: false,
            inventory_blob: None,
        };
        db.save_player_full(&uuid, &row1).expect("save1");

        let row2 = PlayerRow {
            username: "Steve".into(),
            health: 10.0,
            food: 15,
            saturation: 3.0,
            gamemode: 1,
            pos_x: 50.0,
            pos_y: 70.0,
            pos_z: 100.0,
            yaw: 90.0,
            pitch: -45.0,
            is_op: true,
            inventory_blob: None,
        };
        db.save_player_full(&uuid, &row2).expect("save2");

        let loaded = db.load_player(uuid).expect("load");
        assert_eq!(loaded.health, 10.0);
        assert_eq!(loaded.gamemode, 1);
        assert_eq!(loaded.pos_x, 50.0);
        assert_eq!(loaded.pos_z, 100.0);
        assert!(loaded.is_op);
    }
}

#[derive(Debug, Clone)]
pub struct PlayerRow {
    pub username: String,
    pub health: f32,
    pub food: i32,
    pub saturation: f32,
    pub gamemode: u8,
    pub pos_x: f64,
    pub pos_y: f64,
    pub pos_z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub is_op: bool,
    pub inventory_blob: Option<Vec<u8>>,
}
