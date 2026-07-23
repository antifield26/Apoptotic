//! Protocol encode smoke tests — verify S2C packets can be encoded without panic.

use mc_protocol::codec::PacketEncoder;
use mc_protocol::packets::play::*;

fn encode_ok(packet: &impl PacketEncoder) {
    let _ = packet.encode_payload(); // must not panic
}

#[test] fn s2c_keep_alive()         { encode_ok(&KeepAlive { id: 42 }); }
#[test] fn s2c_system_chat()        { encode_ok(&SystemChatMessage { content: "test".into(), overlay: false }); }
#[test] fn s2c_disconnect()         { encode_ok(&PlayDisconnect { reason: "bye".into() }); }
#[test] fn s2c_set_health()         { encode_ok(&SetHealth { health: 20.0, food: 20, saturation: 5.0 }); }
#[test] fn s2c_player_abilities()   { encode_ok(&PlayerAbilities { flags: 0, flying_speed: 0.05, walking_speed: 0.1 }); }
#[test] fn s2c_remove_entities()    { encode_ok(&RemoveEntities { entity_ids: vec![1,2,3] }); }
#[test] fn s2c_set_experience()     { encode_ok(&SetExperience { experience_bar: 0.5, level: 10, total_experience: 100 }); }
#[test] fn s2c_container_content()  { encode_ok(&ContainerSetContent { window_id: 0, state_id: 0, items: vec![], carried_item: None }); }
#[test] fn s2c_update_attributes()  { encode_ok(&UpdateAttributes { entity_id: 1, attributes: vec![Attribute { key: "max_health".into(), value: 20.0, modifiers: vec![] }] }); }
#[test] fn s2c_set_held_item()      { encode_ok(&SetHeldItemS2C { slot: 0 }); }
#[test] fn s2c_block_update()       { encode_ok(&BlockUpdate { x: 0, y: 64, z: 0, block_id: 1 }); }
#[test] fn s2c_sound_effect()       { encode_ok(&SoundEffect { sound_id: 0, category: 0, x: 0, y: 0, z: 0, volume: 1.0, pitch: 1.0, seed: 0 }); }
#[test] fn s2c_open_screen()        { encode_ok(&OpenScreen { window_id: 0, window_type: 0, title: "Chest".into() }); }
#[test] fn s2c_player_info_remove() { encode_ok(&PlayerInfoRemove { uuids: vec![] }); }
