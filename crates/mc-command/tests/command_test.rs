//! 命令集成测试
//!
//! 测试命令注册、分发、选择器解析。

use mc_command::commands::admin::*;
use mc_command::commands::help::HelpCommand;
use mc_command::commands::info::*;
use mc_command::commands::msg::{MeCommand, MsgCommand};
use mc_command::commands::player::*;
use mc_command::commands::world::*;
use mc_command::dispatcher::*;
use mc_core::world_state::SharedWorldState;
use mc_player::player::SharedPlayerManager;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::broadcast;

fn setup() -> (CommandDispatcher, SharedPlayerManager, SharedWorldState, broadcast::Sender<()>) {
    let mut dispatcher = CommandDispatcher::new();
    dispatcher.register(HelpCommand);
    dispatcher.register(MsgCommand);
    dispatcher.register(MeCommand);
    dispatcher.register(OpCommand);
    dispatcher.register(DeopCommand);
    dispatcher.register(StopCommand);
    dispatcher.register(KickCommand);
    dispatcher.register(BanCommand);
    dispatcher.register(PardonCommand);
    dispatcher.register(BanlistCommand);
    dispatcher.register(WhitelistCommand);
    dispatcher.register(GamemodeCommand);
    dispatcher.register(DefaultGamemodeCommand);
    dispatcher.register(TpCommand);
    dispatcher.register(GiveCommand);
    dispatcher.register(KillCommand);
    dispatcher.register(TimeCommand);
    dispatcher.register(WeatherCommand);
    dispatcher.register(DifficultyCommand);
    dispatcher.register(ListCommand);
    dispatcher.register(SeedCommand);
    dispatcher.register(SayCommand);
    dispatcher.register(StatusCommand);

    let pm = Arc::new(mc_player::player::PlayerManager::new());
    let ws = Arc::new(RwLock::new(mc_core::world_state::WorldState::new(12345)));
    let (tx, _) = broadcast::channel(1);

    (dispatcher, pm, ws, tx)
}

#[test]
fn test_help_command() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("help", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    assert!(result.unwrap().contains("/help"));
}

#[test]
fn test_help_alias_question() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("?", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
}

#[test]
fn test_list_empty() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("list", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    assert!(result.unwrap().contains("(none)"));
}

#[test]
fn test_seed_command() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("seed", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    assert!(result.unwrap().contains("12345"));
}

#[test]
fn test_unknown_command() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("nonexistent", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_err());
}

#[test]
fn test_time_set() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("time set day", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    assert_eq!(ws.read().time, 1000);
}

#[test]
fn test_difficulty_set() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("difficulty peaceful", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
}

#[test]
fn test_list_with_player() {
    let (disp, pm, ws, tx) = setup();
    let uuid = mc_core::auth::offline_uuid("TestPlayer");
    pm.add_player(uuid, "TestPlayer".into());
    let result = disp.dispatch_input("list", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    assert!(result.unwrap().contains("TestPlayer"));
}

#[test]
fn test_gamemode_self() {
    let (disp, pm, ws, tx) = setup();
    let uuid = mc_core::auth::offline_uuid("TestPlayer");
    pm.add_player(uuid, "TestPlayer".into());
    let source = CommandSource::player("TestPlayer", uuid);
    let result = disp.dispatch_input("gamemode creative", source, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    let player = pm.get(&uuid).unwrap();
    assert_eq!(player.gamemode, mc_core::types::GameMode::Creative);
}

#[test]
fn test_gamemode_target_all() {
    let (disp, pm, ws, tx) = setup();
    let u1 = mc_core::auth::offline_uuid("P1");
    let u2 = mc_core::auth::offline_uuid("P2");
    pm.add_player(u1, "P1".into());
    pm.add_player(u2, "P2".into());
    let result = disp.dispatch_input("gamemode survival @a", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    assert_eq!(pm.get(&u1).unwrap().gamemode, mc_core::types::GameMode::Survival);
    assert_eq!(pm.get(&u2).unwrap().gamemode, mc_core::types::GameMode::Survival);
}

#[test]
fn test_msg_alias_tell() {
    let (disp, pm, ws, tx) = setup();
    let uuid = mc_core::auth::offline_uuid("Target");
    pm.add_player(uuid, "Target".into());
    let source = CommandSource::player("Sender", mc_core::auth::offline_uuid("Sender"));
    let result = disp.dispatch_input("tell Target hello world", source, &pm, &tx, &ws, "test", 20, None, None);
    // Should succeed even though target isn't subscribed to chat
    assert!(result.is_ok());
}

#[test]
fn test_msg_no_args() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("msg", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_err());
}

#[test]
fn test_ban_flow() {
    let (disp, pm, ws, tx) = setup();
    let uuid = mc_core::auth::offline_uuid("BadGuy");
    pm.add_player(uuid, "BadGuy".into());
    // Ban
    let result = disp.dispatch_input("ban BadGuy hacking", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    assert!(pm.is_banned(&uuid));
}

#[test]
fn test_whitelist_flow() {
    let (disp, pm, ws, tx) = setup();
    let uuid = mc_core::auth::offline_uuid("GoodGuy");
    // Enable whitelist
    disp.dispatch_input("whitelist on", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None).ok();
    assert!(pm.is_whitelist_enabled());
    // Not whitelisted
    assert!(!pm.is_whitelisted(&uuid));
    // Add
    let result = disp.dispatch_input("whitelist add GoodGuy", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    assert!(pm.is_whitelisted(&uuid));
    // Remove
    disp.dispatch_input("whitelist remove GoodGuy", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None).ok();
    assert!(!pm.is_whitelisted(&uuid));
    // Off
    disp.dispatch_input("whitelist off", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None).ok();
    assert!(!pm.is_whitelist_enabled());
}

#[test]
fn test_kill_self() {
    let (disp, pm, ws, tx) = setup();
    let uuid = mc_core::auth::offline_uuid("Victim");
    pm.add_player(uuid, "Victim".into());
    let source = CommandSource::player("Victim", uuid);
    let result = disp.dispatch_input("kill", source, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    assert_eq!(pm.get(&uuid).unwrap().health, 0.0);
}

#[test]
fn test_kill_target_a() {
    let (disp, pm, ws, tx) = setup();
    let u1 = mc_core::auth::offline_uuid("P1");
    let u2 = mc_core::auth::offline_uuid("P2");
    pm.add_player(u1, "P1".into());
    pm.add_player(u2, "P2".into());
    let result = disp.dispatch_input("kill @a", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    assert_eq!(pm.get(&u1).unwrap().health, 0.0);
    assert_eq!(pm.get(&u2).unwrap().health, 0.0);
}

#[test]
fn test_say_broadcast() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("say Hello World!", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
}

#[test]
fn test_status() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("status", CommandSource::Console, &pm, &tx, &ws, "MOTD here", 20, None, None);
    assert!(result.is_ok());
    assert!(result.unwrap().contains("MOTD here"));
}

#[test]
fn test_tp_coordinates() {
    let (disp, pm, ws, tx) = setup();
    let uuid = mc_core::auth::offline_uuid("Traveler");
    pm.add_player(uuid, "Traveler".into());
    let source = CommandSource::player("Traveler", uuid);
    let result = disp.dispatch_input("tp 100.5 64.0 -200.5", source, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    let p = pm.get(&uuid).unwrap();
    assert_eq!(p.position.x, 100.5);
    assert_eq!(p.position.z, -200.5);
}

// ── New command tests (C1-C5 additions) ──

#[test]
fn test_me_command() {
    let (disp, pm, ws, tx) = setup();
    let uuid = mc_core::auth::offline_uuid("RolePlayer");
    pm.add_player(uuid, "RolePlayer".into());
    let source = CommandSource::player("RolePlayer", uuid);
    let result = disp.dispatch_input("me waves at everyone", source, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    let msg = result.unwrap();
    assert!(msg.contains("* RolePlayer"));
    assert!(msg.contains("waves at everyone"));
}

#[test]
fn test_me_empty_args() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("me", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_err());
}

#[test]
fn test_pardon_flow() {
    let (disp, pm, ws, tx) = setup();
    let uuid = mc_core::auth::offline_uuid("BadGuy");
    // Ban first
    pm.ban(uuid);
    assert!(pm.is_banned(&uuid));
    // Pardon
    let result = disp.dispatch_input("pardon BadGuy", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    assert!(!pm.is_banned(&uuid));
}

#[test]
fn test_pardon_not_banned() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("pardon CleanPlayer", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not banned"));
}

#[test]
fn test_banlist_empty() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("banlist", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    assert!(result.unwrap().contains("No banned"));
}

#[test]
fn test_banlist_with_bans() {
    let (disp, pm, ws, tx) = setup();
    let u1 = mc_core::auth::offline_uuid("Hacker1");
    let u2 = mc_core::auth::offline_uuid("Hacker2");
    pm.ban(u1);
    pm.ban(u2);
    let result = disp.dispatch_input("banlist", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    let msg = result.unwrap();
    assert!(msg.contains("Banned (2)"));
}

#[test]
fn test_defaultgamemode_set() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("defaultgamemode creative", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    assert_eq!(ws.read().default_gamemode, mc_core::types::GameMode::Creative);
}

#[test]
fn test_defaultgamemode_invalid() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("defaultgamemode invalidmode", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_err());
}

#[test]
fn test_random_selector() {
    let (_disp, pm, ws, tx) = setup();
    let u1 = mc_core::auth::offline_uuid("P1");
    pm.add_player(u1, "P1".into());
    // @r on a single player should return that player
    let targets = resolve_player_targets("@r", &CommandContext {
        source: CommandSource::Console,
        args: vec!["@r".into()],
        player_manager: &pm,
        shutdown_tx: &tx,
        world_state: &ws,
        motd: "test",
        max_players: 20, chunk_store: None, save_trigger: None, dispatcher: None, mob_manager: None, team_manager: None,
    });
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].1, "P1");
}

#[test]
fn test_random_selector_empty() {
    let (_disp, pm, ws, tx) = setup();
    let targets = resolve_player_targets("@r", &CommandContext {
        source: CommandSource::Console,
        args: vec!["@r".into()],
        player_manager: &pm,
        shutdown_tx: &tx,
        world_state: &ws,
        motd: "test",
        max_players: 20, chunk_store: None, save_trigger: None, dispatcher: None, mob_manager: None, team_manager: None,
    });
    assert!(targets.is_empty());
}

#[test]
fn test_gamemode_with_at_r() {
    let (disp, pm, ws, tx) = setup();
    let u1 = mc_core::auth::offline_uuid("R1");
    pm.add_player(u1, "R1".into());
    let result = disp.dispatch_input("gamemode creative @r", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    assert_eq!(pm.get(&u1).unwrap().gamemode, mc_core::types::GameMode::Creative);
}

#[test]
fn test_pardon_arg_required() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("pardon", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_err());
}

#[test]
fn test_defaultgamemode_arg_required() {
    let (disp, pm, ws, tx) = setup();
    let result = disp.dispatch_input("defaultgamemode", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_err());
}

#[test]
fn test_e_selector_all_entities() {
    let (_disp, pm, ws, tx) = setup();
    let u1 = mc_core::auth::offline_uuid("E1");
    let u2 = mc_core::auth::offline_uuid("E2");
    pm.add_player(u1, "E1".into());
    pm.add_player(u2, "E2".into());
    let targets = resolve_player_targets("@e", &CommandContext {
        source: CommandSource::Console,
        args: vec!["@e".into()],
        player_manager: &pm,
        shutdown_tx: &tx,
        world_state: &ws,
        motd: "test",
        max_players: 20, chunk_store: None, save_trigger: None, dispatcher: None, mob_manager: None, team_manager: None,
    });
    assert_eq!(targets.len(), 2);
}

#[test]
fn test_kill_with_e_selector() {
    let (disp, pm, ws, tx) = setup();
    let u1 = mc_core::auth::offline_uuid("E1");
    let u2 = mc_core::auth::offline_uuid("E2");
    pm.add_player(u1, "E1".into());
    pm.add_player(u2, "E2".into());
    let result = disp.dispatch_input("kill @e", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    assert_eq!(pm.get(&u1).unwrap().health, 0.0);
    assert_eq!(pm.get(&u2).unwrap().health, 0.0);
}

#[test]
fn test_give_invalid_item() {
    let (disp, pm, ws, tx) = setup();
    let uuid = mc_core::auth::offline_uuid("Receiver");
    pm.add_player(uuid, "Receiver".into());
    let result = disp.dispatch_input("give Receiver nonexistent_item_xyz", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_err());
}

#[test]
fn test_give_valid_item() {
    let (disp, pm, ws, tx) = setup();
    let uuid = mc_core::auth::offline_uuid("Receiver");
    pm.add_player(uuid, "Receiver".into());
    let result = disp.dispatch_input("give Receiver stone 10", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
    // Verify items were added to inventory
    let player = pm.get(&uuid).unwrap();
    let stone = mc_core::item::resolve_item("stone").unwrap();
    assert_eq!(player.inventory.count_item(stone), 10);
}

#[test]
fn test_give_with_minecraft_prefix() {
    let (disp, pm, ws, tx) = setup();
    let uuid = mc_core::auth::offline_uuid("Receiver");
    pm.add_player(uuid, "Receiver".into());
    let result = disp.dispatch_input("give Receiver minecraft:diamond 5", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_ok());
}

#[test]
fn test_give_missing_item_arg() {
    let (disp, pm, ws, tx) = setup();
    let uuid = mc_core::auth::offline_uuid("Receiver");
    pm.add_player(uuid, "Receiver".into());
    let result = disp.dispatch_input("give Receiver", CommandSource::Console, &pm, &tx, &ws, "test", 20, None, None);
    assert!(result.is_err());
}
