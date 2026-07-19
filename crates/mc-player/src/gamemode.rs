//! 游戏模式管理

use mc_core::types::GameMode;

/// 游戏模式管理器
pub struct GameModeManager;

impl GameModeManager {
    pub fn can_fly(mode: GameMode) -> bool {
        matches!(mode, GameMode::Creative | GameMode::Spectator)
    }

    pub fn is_invulnerable(mode: GameMode) -> bool {
        matches!(mode, GameMode::Creative | GameMode::Spectator)
    }

    pub fn can_build(mode: GameMode) -> bool {
        matches!(mode, GameMode::Survival | GameMode::Creative)
    }
}
