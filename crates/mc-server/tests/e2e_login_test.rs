//! E2E 测试 — 需要运行中的服务器 (cargo run)
//! 运行: cargo test -- --ignored --test-threads=1

mod common;
use common::bot::BotClient;
use mc_protocol::state::ConnectionState;

#[test]
#[ignore]
fn test_status_ping() {
    let mut bot = BotClient::connect("127.0.0.1", 25565).unwrap();
    bot.handshake(776, ConnectionState::Status).unwrap();
    let status = bot.request_status().unwrap();
    assert!(!status.description.text.is_empty(), "Status should have MOTD text");
    let pong = bot.ping(42).unwrap();
    assert_eq!(pong.payload, 42);
}

#[test]
#[ignore]
fn test_login_and_join() {
    let mut bot = BotClient::connect("127.0.0.1", 25565).unwrap();
    bot.handshake(776, ConnectionState::Login).unwrap();
    bot.login("TestBot").unwrap();
    let join = bot.wait_for_join().unwrap();
    assert!(join.dimension_name.contains("overworld"), "Should spawn in overworld");
    assert!(join.max_players > 0, "max_players should be positive");
}

#[test]
#[ignore]
fn test_two_bots() {
    let mut bot1 = BotClient::connect("127.0.0.1", 25565).unwrap();
    let mut bot2 = BotClient::connect("127.0.0.1", 25565).unwrap();
    bot1.handshake(776, ConnectionState::Login).unwrap();
    bot2.handshake(776, ConnectionState::Login).unwrap();
    bot1.login("BotOne").unwrap();
    bot2.login("BotTwo").unwrap();
    let j1 = bot1.wait_for_join().unwrap();
    let j2 = bot2.wait_for_join().unwrap();
    assert!(j1.entity_id != j2.entity_id, "Bots should have unique entity IDs");
}
