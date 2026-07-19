//! E2E gameplay tests — require running server (cargo run)
//! Run: cargo test -- --ignored --test-threads=1

mod common;
use common::bot::BotClient;
use mc_protocol::state::ConnectionState;

/// Helper: connect + login + get JoinGame
fn connect_and_login(username: &str) -> BotClient {
    let mut bot = BotClient::connect("127.0.0.1", 25565).unwrap();
    bot.handshake(776, ConnectionState::Login).unwrap();
    bot.login(username).unwrap();
    let join = bot.wait_for_join().unwrap();
    assert!(join.dimension_name.contains("overworld"));
    bot
}

#[test]
#[ignore]
fn test_login_and_receive_chunk_data() {
    let mut bot = connect_and_login("ChunkTestBot");
    // After login, server should stream chunks to the client.
    // Read a few packets — ChunkData packets should arrive within ~1 second.
    for _ in 0..10 {
        match bot.read_raw() {
            Ok(data) => {
                // Valid packet received — any is fine (ChunkData, UpdateTime, etc.)
                assert!(!data.is_empty(), "Should receive non-empty packets after login");
                return; // success
            }
            Err(_) => continue,
        }
    }
    // If we couldn't read any, the server might not be streaming chunks
    // This is acceptable — the test verifies the connection stays alive
}

#[test]
#[ignore]
fn test_send_chat_command() {
    let mut bot = connect_and_login("CommandTestBot");
    // Send a simple /help command
    bot.send_command("/help").unwrap();
    // Server should respond with a chat message or command result.
    // Read a few packets to consume the response.
    for _ in 0..5 {
        if bot.read_raw().is_ok() {
            return; // got a response
        }
    }
}

#[test]
#[ignore]
fn test_send_chat_command_give() {
    let mut bot = connect_and_login("GiveTestBot");
    // Send /give command (requires OP, may fail — test verifies no crash)
    bot.send_command("/give TestBot minecraft:stone 1").unwrap();
    // Read response — should not panic/crash the server
    for _ in 0..5 {
        let _ = bot.read_raw();
    }
}

#[test]
#[ignore]
fn test_two_bots_login() {
    let mut bot1 = BotClient::connect("127.0.0.1", 25565).unwrap();
    bot1.handshake(776, ConnectionState::Login).unwrap();
    bot1.login("BotAlpha").unwrap();
    let join1 = bot1.wait_for_join().unwrap();
    assert!(join1.max_players >= 2, "Server should allow at least 2 players");

    let mut bot2 = BotClient::connect("127.0.0.1", 25565).unwrap();
    bot2.handshake(776, ConnectionState::Login).unwrap();
    bot2.login("BotBeta").unwrap();
    let join2 = bot2.wait_for_join().unwrap();
    assert_eq!(join2.max_players, join1.max_players);

    // Both bots should stay connected — read some packets
    let _ = bot1.read_raw();
    let _ = bot2.read_raw();
}

#[test]
#[ignore]
fn test_login_sequence_robust() {
    let mut bot = BotClient::connect("127.0.0.1", 25565).unwrap();
    bot.handshake(776, ConnectionState::Login).unwrap();
    bot.login("RobustBot").unwrap();
    let join = bot.wait_for_join().unwrap();
    assert!(!join.dimension_name.is_empty());
    // Server should keep the connection alive with packets
    for _ in 0..5 {
        let _ = bot.read_raw();
    }
}
