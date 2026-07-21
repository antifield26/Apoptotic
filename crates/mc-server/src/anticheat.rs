//! Anti-cheat utilities (Phase E2)
//!
//! Server-side teleport bypass and anti-cheat state reset helpers.
//! Core movement validation uses PlayerManager::ac_* public methods.

use mc_player::player::PlayerManager;

/// Set anti-cheat bypass for a server-side teleport (portal, command).
/// Movement checks are skipped briefly after the teleport.
#[allow(dead_code)]
pub fn set_teleport_bypass(pm: &PlayerManager, uuid: &uuid::Uuid, tick: u64) {
    // Bypass is handled via ac_reset_violations + ac_update_valid for now
    pm.ac_reset_violations(uuid);
    pm.ac_update_valid(uuid, 0.0, 64.0, 0.0, tick);
}

/// Reset anti-cheat state for a player (called on join/respawn).
#[allow(dead_code)]
pub fn reset_anticheat(pm: &PlayerManager, uuid: &uuid::Uuid, x: f64, y: f64, z: f64) {
    pm.ac_reset_violations(uuid);
    pm.ac_update_valid(uuid, x, y, z, 0);
}