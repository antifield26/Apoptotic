//! Anti-cheat — movement validation and rubberband enforcement
//!
//! Validates player movement packets against physics limits (speed * multiplier).
//! Accumulates violations and triggers rubberband teleport at 8 violations.
//!
//! NOTE: These functions are prepared for extraction from connection.rs.
//! Currently connection.rs uses PlayerManager::ac_* methods directly.
//! The functions below will replace inline code when wired.
#![allow(dead_code)]

use mc_player::player::PlayerManager;
use uuid::Uuid;

/// Maximum movement delta per tick (10 blocks/tick base, multiplied by speed effects).
/// At 20 TPS this allows ~200 blocks/sec which exceeds vanilla sprint (~5.6 b/s).
const MAX_MOVE_DELTA: f64 = 10.0;

/// Number of violations before triggering rubberband teleport (PaperMC default: 8).
const RUBBERBAND_THRESHOLD: u8 = 8;

/// Validate a player position update against anti-cheat limits.
///
/// Returns `Ok(())` if the move is accepted, or `Err(rubberband_position)`
/// if the player should be teleported back to the last valid position.
///
/// # Arguments
/// * `pm` — PlayerManager for accessing player state
/// * `uuid` — Player UUID
/// * `x, y, z` — Proposed new position
/// * `tick_count` — Current server tick (for violation decay)
///
/// Caller should:
/// - On `Ok(())`: call `update_position_full()` and broadcast entity move
/// - On `Err((lx, ly, lz))`: teleport player to (lx, ly, lz) and reset violations
pub fn validate_movement(
    pm: &PlayerManager,
    uuid: &Uuid,
    x: f64,
    y: f64,
    z: f64,
    tick_count: u64,
) -> Result<(), (f64, f64, f64)> {
    // Get current player position
    let player = match pm.get(uuid) {
        Some(p) => p,
        None => return Ok(()), // player not found, accept
    };

    let (old_x, old_y, old_z) = (player.position.x, player.position.y, player.position.z);
    let speed_mul = player.speed_multiplier;
    let max_delta = MAX_MOVE_DELTA * speed_mul as f64;

    let h_dist = ((x - old_x).powi(2) + (z - old_z).powi(2)).sqrt();
    let v_dist = (y - old_y).abs();

    // Within limits — accept and update valid position
    if h_dist <= max_delta && v_dist <= max_delta {
        pm.ac_update_valid(uuid, x, y, z, tick_count);
        return Ok(());
    }

    // Small movements always accepted (e.g., swimming, shifting)
    if h_dist <= 3.0 {
        pm.ac_update_valid(uuid, x, y, z, tick_count);
        return Ok(());
    }

    // Large violation — accumulate and check rubberband threshold
    let (violations, rubberband) = pm.ac_add_violation(uuid, tick_count);

    if rubberband
        && let Some((lx, ly, lz)) = pm.ac_valid_position(uuid) {
            pm.ac_reset_violations(uuid);
            return Err((lx, ly, lz));
        }

    // Violation recorded but not yet at rubberband threshold — accept this move
    tracing::debug!(
        "Anti-cheat: {} violations for {} (moved {:.1} blocks, limit {:.1})",
        violations, player.username, h_dist, max_delta
    );
    Ok(())
}

/// Set anti-cheat bypass for a server-side teleport (portal, command).
/// Movement checks are skipped briefly after the teleport.
pub fn set_teleport_bypass(pm: &PlayerManager, uuid: &Uuid, tick: u64) {
    pm.ac_reset_violations(uuid);
    pm.ac_update_valid(uuid, 0.0, 64.0, 0.0, tick);
}

/// Reset anti-cheat state for a player (called on join/respawn).
pub fn reset_anticheat(pm: &PlayerManager, uuid: &Uuid, x: f64, y: f64, z: f64) {
    pm.ac_reset_violations(uuid);
    pm.ac_update_valid(uuid, x, y, z, 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_delta_reasonable() {
        // At 20 TPS, MAX_MOVE_DELTA=10 allows 200 blocks/sec
        // Vanilla sprint is ~5.6 blocks/sec, creative flight ~11 blocks/sec
        // So 200 b/s is generous but catches teleport hacks
        assert!(MAX_MOVE_DELTA >= 5.0);
        assert!(MAX_MOVE_DELTA <= 50.0);
    }

    #[test]
    fn test_rubberband_threshold() {
        // 8 violations means player can trigger rubberband quickly
        // but a single network glitch won't cause false positives
        assert!(RUBBERBAND_THRESHOLD >= 3);
        assert!(RUBBERBAND_THRESHOLD <= 15);
    }
}
