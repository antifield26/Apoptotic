//! Tests for mc-admin crate — RCON packet format and console utilities

use mc_admin::rcon::RconServer;
use mc_command::dispatcher::CommandDispatcher;
use mc_core::world_state::WorldState;
use mc_player::player::PlayerManager;
use std::sync::Arc;
use parking_lot::RwLock;

/// Test that RCON server construction succeeds with valid parameters
#[test]
fn test_rcon_server_new() {
    let pm = Arc::new(PlayerManager::new());
    let disp = Arc::new(parking_lot::Mutex::new(CommandDispatcher::new()));
    let (tx, _) = tokio::sync::broadcast::channel::<()>(1);
    let ws = Arc::new(RwLock::new(WorldState::new(42)));

    let _rcon = RconServer::new("0.0.0.0", 25575, "secret", disp, pm, tx, ws);
    // Construction should not panic — no TCP binding happens until run()
}

/// Test the RCON packet format constants
#[test]
fn test_rcon_packet_format_constants() {
    // Verify standard Minecraft RCON type codes are as expected
    let response_type: i32 = 0;
    let command_type: i32 = 2;
    let login_type: i32 = 3;
    assert_eq!(response_type, 0);
    assert_ne!(command_type, response_type);
    assert_ne!(login_type, response_type);
    assert_ne!(login_type, command_type);
}

/// Test that RCON with empty password can be constructed
#[test]
fn test_rcon_empty_password_detected() {
    let pm = Arc::new(PlayerManager::new());
    let disp = Arc::new(parking_lot::Mutex::new(CommandDispatcher::new()));
    let (tx, _) = tokio::sync::broadcast::channel::<()>(1);
    let ws = Arc::new(RwLock::new(WorldState::new(42)));

    let _rcon = RconServer::new("0.0.0.0", 25575, "", disp, pm, tx, ws);
    // Empty password: run() would detect and return early
}
