use uuid::Uuid;

/// 离线模式 UUID 命名空间 — 遵循 Minecraft 约定
///
/// 参见: https://wiki.vg/User_Authentication#Offline_mode
const OFFLINE_PLAYER_PREFIX: &str = "OfflinePlayer:";

/// 为离线模式玩家生成确定性 UUID
///
/// 使用 UUID v3 (MD5 hash of namespace + name)，确保同一用户名
/// 始终生成同一 UUID，与 vanilla Minecraft 行为一致。
///
/// # Example
/// ```
/// let uuid = mc_core::auth::offline_uuid("Player123");
/// assert_eq!(uuid.get_version_num(), 3); // v3 UUID
/// assert_eq!(uuid.to_string().len(), 36); // standard UUID format
/// ```
pub fn offline_uuid(username: &str) -> Uuid {
    let name = format!("{}{}", OFFLINE_PLAYER_PREFIX, username);
    Uuid::new_v3(&Uuid::NAMESPACE_OID, name.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offline_uuid_deterministic() {
        let a = offline_uuid("Player123");
        let b = offline_uuid("Player123");
        assert_eq!(a, b, "same username must produce same UUID");
    }

    #[test]
    fn test_offline_uuid_different() {
        let a = offline_uuid("Alice");
        let b = offline_uuid("Bob");
        assert_ne!(a, b, "different usernames must produce different UUIDs");
    }

    #[test]
    fn test_offline_uuid_format() {
        let uuid = offline_uuid("TestPlayer");
        assert_eq!(uuid.get_version_num(), 3, "must be v3 UUID");
        let s = uuid.to_string();
        assert_eq!(s.len(), 36, "UUID string must be 36 chars");
        assert_eq!(&s[14..15], "3", "version character must be '3'");
    }
}
