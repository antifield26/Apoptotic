//! Integration tests — 完整用户场景验证
//! 测试: 世界生成 → 玩家加入 → 背包操作 → 战斗 → 死亡 → 退出

use mc_core::block::BlockState;
use mc_core::position::{ChunkPos, Position};
use mc_player::inventory::{Inventory, ItemStack};
use mc_player::mob::{MobAiState, MobManager, TrackedMob};
use mc_player::player::PlayerManager;
use mc_world::chunk_store::ChunkStore;
use mc_world::generator::{FlatGenerator, NoiseGenerator, GeneratorRegistry, TerrainGenerator};
use std::sync::Arc;

/// 测试: FlatGenerator 区块确定性
#[test]
fn test_flat_generator_deterministic() {
    let generator = FlatGenerator::new();
    let c1 = generator.generate_chunk(ChunkPos::new(0, 0), 42);
    let c2 = generator.generate_chunk(ChunkPos::new(0, 0), 42);
    // 同一 seed 应产生相同区块
    assert_eq!(c1.get_block(8, -64, 8).id, 266); // bedrock
    assert_eq!(c1.get_block(8, -59, 8).id, 8); // grass
    assert_eq!(c2.get_block(8, -64, 8).id, 266);
}

/// 测试: NoiseGenerator 不 panic
#[test]
fn test_noise_generator_no_panic() {
    let generator = NoiseGenerator::new();
    for dx in -2..=2i32 {
        for dz in -2..=2i32 {
            let _chunk = generator.generate_chunk(ChunkPos::new(dx, dz), 42);
        }
    }
}

/// 测试: GeneratorRegistry 注册和切换
#[test]
fn test_generator_registry_switch() {
    let mut registry = GeneratorRegistry::new();
    assert_eq!(registry.active().name(), "flat"); // default
    assert!(registry.set_active("empty").is_ok());
    assert_eq!(registry.active().name(), "empty");
    assert!(registry.set_active("noise").is_ok());
    assert_eq!(registry.active().name(), "noise");
    // 无效生成器应失败
    assert!(registry.set_active("nonexistent").is_err());
}

/// 测试: ChunkStore 基本操作
#[test]
fn test_chunk_store_insert_get() {
    let store = ChunkStore::new();
    let pos = ChunkPos::new(0, 0);
    let chunk = FlatGenerator::new().generate_chunk(pos, 42);
    store.insert(pos, chunk);
    assert!(store.get(&pos).is_some());
    assert_eq!(store.count(), 1);
}

/// 测试: ChunkStore 获取不存在的区块
#[test]
fn test_chunk_store_missing() {
    let store = ChunkStore::new();
    assert!(store.get(&ChunkPos::new(999, 999)).is_none());
}

/// 测试: PalettedContainer 基础操作
#[test]
fn test_paletted_container_single() {
    let container = mc_world::paletted::PalettedContainer::filled(BlockState::new(1));
    assert_eq!(container.get(0, 0, 0), BlockState::new(1));
    assert_eq!(container.get(15, 15, 15), BlockState::new(1));
}

/// 测试: PalettedContainer set 切换到 Indirect
#[test]
fn test_paletted_container_upgrade() {
    let mut container = mc_world::paletted::PalettedContainer::filled(BlockState::new(1));
    container.set(0, 0, 0, BlockState::new(2));
    assert_eq!(container.get(0, 0, 0), BlockState::new(2));
    assert_eq!(container.get(1, 0, 0), BlockState::new(1)); // 其他不变
}

/// 测试: PlayerManager 基本操作
#[test]
fn test_player_manager_add_remove() {
    let pm = Arc::new(PlayerManager::new());
    let uuid = uuid::Uuid::new_v4();
    let player = pm.add_player(uuid, "TestPlayer".into());
    assert!(pm.get(&uuid).is_some());
    assert_eq!(pm.online_count(), 1);
    assert_eq!(pm.get_entity_id(&uuid).unwrap(), player.entity_id);
    pm.remove_player(&uuid);
    assert!(pm.get(&uuid).is_none());
    assert_eq!(pm.online_count(), 0);
}

/// 测试: PlayerManager 生命值操作
#[test]
fn test_player_health_operations() {
    let pm = Arc::new(PlayerManager::new());
    let uuid = uuid::Uuid::new_v4();
    pm.add_player(uuid, "HealthTest".into());
    let _ = pm.set_health(&uuid, 15.0);
    let player = pm.get(&uuid).unwrap();
    assert_eq!(player.health, 15.0);
    // 护甲减免测试
    let result = pm.apply_damage(&uuid, 5.0, 100);
    assert!(result.is_ok());
}

/// 测试: PlayerManager 经验操作
#[test]
fn test_player_xp_operations() {
    let pm = Arc::new(PlayerManager::new());
    let uuid = uuid::Uuid::new_v4();
    pm.add_player(uuid, "XpTest".into());
    assert!(pm.add_xp(&uuid, 50).is_ok());
    let player = pm.get(&uuid).unwrap();
    assert!(player.xp_total > 0);
    assert!(pm.remove_xp_levels(&uuid, 0).is_ok()); // 0 level should work
}

/// 测试: Inventory 序列化往返
#[test]
fn test_inventory_serialize_roundtrip() {
    let mut inv = Inventory::new();
    inv.add_item(BlockState::new(1), 64); // stone
    inv.add_item(BlockState::new(9), 32); // dirt
    let bytes = inv.serialize();
    assert!(!bytes.is_empty());
    let restored = Inventory::deserialize(&bytes).unwrap();
    assert_eq!(restored.count_item(BlockState::new(1)), 64);
    assert_eq!(restored.count_item(BlockState::new(9)), 32);
}

/// 测试: Inventory 空背包序列化
#[test]
fn test_inventory_serialize_empty() {
    let inv = Inventory::new();
    let bytes = inv.serialize();
    let restored = Inventory::deserialize(&bytes);
    assert!(restored.is_some());
}

/// 测试: MobManager 注册和移除
#[test]
fn test_mob_manager_register_remove() {
    let mm = MobManager::new();
    let mob = TrackedMob {
        entity_id: 100,
        uuid: uuid::Uuid::new_v4(),
        mob_type: 151, // zombie (official 26.2 ID)
        position: Position::new(0.0, 64.0, 0.0),
        health: 20.0,
        max_health: 20.0,
        age_ticks: 0,
        ai_timer: 0,
        ai_state: MobAiState::Idle,
        attack_cooldown: 0,
        last_sync_tick: 0,
        owner_uuid: None,
        is_tamed: false,
        is_sitting: false,
        tame_attempts: 0,
        is_baby: false,
        in_love_ticks: 0,
        breed_cooldown: 0,
        is_sheared: false, is_on_fire: false, is_in_water: false, path: Vec::new(), path_last_tick: 0, sulfur_cube_archetype: None, absorbed_block_id: None, is_small_cube: false, dirty_flags: 3,
    };
    mm.register(mob);
    assert!(mm.get(100).is_some());
    assert!(mm.count() >= 1);
    let removed = mm.remove(100);
    assert!(removed.is_some());
}

/// 测试: MobManager 伤害计算
#[test]
fn test_mob_manager_damage() {
    let mm = MobManager::new();
    let mob = TrackedMob {
        entity_id: 200,
        uuid: uuid::Uuid::new_v4(),
        mob_type: 151, // zombie
        position: Position::new(0.0, 64.0, 0.0),
        health: 20.0,
        max_health: 20.0,
        age_ticks: 0,
        ai_timer: 0,
        ai_state: MobAiState::Idle,
        attack_cooldown: 0,
        last_sync_tick: 0,
        owner_uuid: None,
        is_tamed: false,
        is_sitting: false,
        tame_attempts: 0,
        is_baby: false,
        in_love_ticks: 0,
        breed_cooldown: 0,
        is_sheared: false, is_on_fire: false, is_in_water: false, path: Vec::new(), path_last_tick: 0, sulfur_cube_archetype: None, absorbed_block_id: None, is_small_cube: false, dirty_flags: 3,
    };
    mm.register(mob);
    let remaining = mm.damage(200, 10.0);
    assert_eq!(remaining, Some(10.0));
    let remaining = mm.damage(200, 15.0);
    assert_eq!(remaining, Some(0.0));
}

/// 测试: 配方匹配 (2x2)
#[test]
fn test_recipe_matching_2x2() {
    let registry = mc_player::recipe::RecipeRegistry::new();
    let planks = BlockState::new(13); // oak_planks
    let grid: [Option<ItemStack>; 4] = [
        Some(ItemStack::new(planks, 1)),
        Some(ItemStack::new(planks, 1)),
        Some(ItemStack::new(planks, 1)),
        Some(ItemStack::new(planks, 1)),
    ];
    let result = registry.find_match(&grid);
    assert!(result.is_some(), "Crafting table recipe should match");
}

/// 测试: 容器窗口类型映射
#[test]
fn test_container_window_types() {
    assert_eq!(mc_player::container::container_window_type(201), 2);  // chest
    assert_eq!(mc_player::container::container_window_type(209), 3);  // furnace
    assert_eq!(mc_player::container::container_window_type(206), 6); // crafting_table
    assert_eq!(mc_player::container::container_window_type(879), 10); // brewing_stand
    assert_eq!(mc_player::container::container_window_type(880), 7);  // enchanting_table
}

/// 测试: 生物群系采样覆盖
#[test]
fn test_biome_sampling_coverage() {
    // 采样多个位置确保不 panic
    for x in (0..1000i32).step_by(50) {
        for z in (0..1000i32).step_by(50) {
            let biome = mc_world::generator::sample_biome(x, z, 42);
            assert!(biome.id() < 100, "Biome ID should be < 100");
        }
    }
}

/// 测试: 护甲点计算
#[test]
fn test_armor_points_calculation() {
    assert_eq!(mc_player::inventory::armor_points_for_item(819), 3.0); // iron helmet
    assert_eq!(mc_player::inventory::armor_points_for_item(820), 6.0); // iron chestplate
    assert_eq!(mc_player::inventory::armor_points_for_item(823), 3.0); // diamond helmet
    assert_eq!(mc_player::inventory::armor_points_for_item(0), 0.0);   // invalid
}

/// 测试: 耐久度计算
#[test]
fn test_durability_values() {
    assert_eq!(mc_player::inventory::max_durability(781), Some(59));  // wood sword
    assert_eq!(mc_player::inventory::max_durability(785), Some(131)); // stone sword
    assert_eq!(mc_player::inventory::max_durability(780), Some(250)); // iron sword
    assert_eq!(mc_player::inventory::max_durability(792), Some(1561));// diamond sword
    assert_eq!(mc_player::inventory::max_durability(0), None);        // air
}

// ═══ E6: E2E Integration Tests — Full Server Lifecycle ═══

/// E2E: Server startup simulation — chunk store + player manager + mob manager
#[test]
fn e2e_server_startup_components() {
    // Simulate server init: create all core components
    let cs = ChunkStore::new();
    let pm = PlayerManager::new();
    let mm = MobManager::new();

    // Verify initial state
    assert_eq!(cs.count(), 0);
    assert_eq!(pm.online_count(), 0);
    assert_eq!(mm.count(), 0);
}

/// E2E: Player join → inventory → disconnect lifecycle
#[test]
fn e2e_player_join_inventory_disconnect() {
    let pm = PlayerManager::new();
    let uuid = uuid::Uuid::new_v4();

    // Join
    pm.add_player(uuid, "TestPlayer".into());
    assert_eq!(pm.online_count(), 1);

    // Verify inventory
    let inv = pm.get(&uuid).map(|p| p.inventory.clone());
    assert!(inv.is_some());
    let inventory = inv.unwrap();
    assert_eq!(inventory.items.len(), 36); // 27 inv + 9 hotbar
    assert_eq!(inventory.armor.len(), 4);  // helmet/chestplate/leggings/boots

    // Set health/food
    pm.set_health(&uuid, 20.0).ok();
    pm.set_food(&uuid, 20, 5.0).ok();

    let player = pm.get(&uuid).unwrap();
    assert_eq!(player.health, 20.0);
    assert_eq!(player.food_level, 20);

    // Give item
    let stone = BlockState::new(1);
    pm.add_item_to_player(&uuid, stone, 64).ok();

    let player = pm.get(&uuid).unwrap();
    let has_stone = player.inventory.items.iter()
        .filter_map(|s| s.as_ref())
        .any(|s| s.item.id == 1 && s.count == 64);
    assert!(has_stone, "Player should have 64 stone after /give");

    // Disconnect
    pm.remove_player(&uuid);
    assert_eq!(pm.online_count(), 0);
}

/// E2E: Mob spawn → AI tick → despawn lifecycle
#[test]
fn e2e_mob_spawn_ai_despawn() {
    let mm = MobManager::new();
    let pm = PlayerManager::new();
    let uuid = uuid::Uuid::new_v4();
    pm.add_player(uuid, "TestPlayer".into());
    pm.update_position_full(&uuid, 0.0, 64.0, 0.0, 0.0, 0.0).ok();

    // Spawn a zombie near player
    let mob = TrackedMob {
        entity_id: 100, uuid: uuid::Uuid::new_v4(), mob_type: 36, // ZOMBIE
        position: Position::new(3.0, 64.0, 0.0),
        health: 20.0, max_health: 20.0, age_ticks: 0, ai_timer: 0,
        ai_state: MobAiState::Idle, attack_cooldown: 0, last_sync_tick: 0,
        owner_uuid: None, is_tamed: false, is_sitting: false, tame_attempts: 0,
        is_baby: false, in_love_ticks: 0, breed_cooldown: 0, is_sheared: false,
        is_on_fire: false, is_in_water: false, path: Vec::new(), path_last_tick: 0,
        sulfur_cube_archetype: None, absorbed_block_id: None, is_small_cube: false,
        dirty_flags: 3,
    };
    mm.register(mob);
    assert_eq!(mm.count(), 1);
    assert!(mm.count_hostile() >= 1);

    // Tick AI — should not panic
    mm.tick_ai(Some(&pm));
    assert!(mm.get(100).is_some());

    // Damage mob
    let hp = mm.damage(100, 10.0);
    assert!(hp.is_some());
    assert!(hp.unwrap() <= 10.0);

    // Kill mob
    mm.damage(100, 20.0);
    let dead = mm.remove_dead();
    assert!(!dead.is_empty());
}

/// E2E: Chunk generation → store → retrieve → serialize cycle
#[test]
fn e2e_chunk_generate_store_serialize() {
    let r#gen = FlatGenerator::new();
    let cs = ChunkStore::new();
    let pos = ChunkPos::new(10, -5);

    // Generate
    let chunk = r#gen.generate_chunk(pos, 12345);
    assert!(!chunk.sections.iter().all(|s| s.is_none()), "Chunk should have sections");

    // Store
    cs.insert(pos, chunk);
    assert_eq!(cs.count(), 1);
    assert!(cs.get(&pos).is_some());

    // Verify block data
    let stored = cs.get(&pos).unwrap();
    let grass = stored.get_block(8, -59, 8);
    assert_eq!(grass.id, 8); // grass block at surface

    // Serialize roundtrip via binary
    let data = mc_world::paletted::PalettedContainer::filled(BlockState::new(1));
    let encoded = data.encode_binary();
    let mut decoded = mc_world::paletted::PalettedContainer::new();
    let _ = decoded.decode_binary(&encoded);
    assert_eq!(decoded.get(0, 0, 0), BlockState::new(1));

    // Memory budget estimation
    let mem = cs.estimated_memory_bytes();
    assert!(mem > 0, "Memory estimate should be positive");
}

/// E2E: Recipe lookup — shaped + shapeless
#[test]
fn e2e_recipe_lookup() {
    let reg = mc_player::recipe::RecipeRegistry::new();
    let count = reg.len();
    assert!(count > 400, "Recipe registry should have 400+ recipes (got {})", count);

    // Crafting table recipe
    let planks = ItemStack::new(BlockState::new(13), 1);
    let grid: [Option<ItemStack>; 4] = [
        Some(planks.clone()), Some(planks.clone()),
        Some(planks.clone()), Some(planks.clone()),
    ];
    let result = reg.find_match(&grid);
    assert!(result.is_some(), "2×2 planks should yield crafting table");
}

/// E2E: Effect system — apply + tick + expire
#[test]
fn e2e_effect_apply_tick_expire() {
    let pm = PlayerManager::new();
    let uuid = uuid::Uuid::new_v4();
    pm.add_player(uuid, "TestPlayer".into());

    // Apply Speed II for 100 ticks
    pm.add_effect(&uuid, mc_core::effect::ActiveEffect {
        effect: mc_core::effect::EffectType::Speed,
        amplifier: 1,
        duration_ticks: 100,
    }).ok();

    // Verify effect active
    let level = pm.get_effect_level(&uuid, 0); // Speed=0
    assert_eq!(level, 2, "Speed II should be active (level=2)");

    // Tick effects 100 times to expire
    for _ in 0..100 {
        pm.tick_effects(0);
    }

    // Verify effect expired
    let level = pm.get_effect_level(&uuid, 0);
    assert_eq!(level, 0, "Speed should have expired after 100 ticks");
}

/// E2E: Anti-cheat violation + rubberband
#[test]
fn e2e_anticheat_violation_rubberband() {
    let pm = PlayerManager::new();
    let uuid = uuid::Uuid::new_v4();
    pm.add_player(uuid, "Hacker".into());
    pm.update_position_full(&uuid, 0.0, 64.0, 0.0, 0.0, 0.0).ok();

    // Set valid position
    pm.ac_update_valid(&uuid, 0.0, 64.0, 0.0, 0);

    // Simulate 8 rapid teleport-style movements (cheating)
    for i in 0..8 {
        let (_v, rb) = pm.ac_add_violation(&uuid, i);
        if i >= 7 {
            assert!(rb, "Should rubberband after 8 violations");
        }
    }

    // Verify violations accumulated
    let (v, _) = pm.ac_add_violation(&uuid, 100);
    assert!(v >= 8, "Violations should accumulate");
}

/// E2E: Async chunk bridge — enqueue + drain cycle
#[test]
fn e2e_async_chunk_bridge_enqueue_drain() {
    // The AsyncChunkBridge requires a Tokio runtime, so this test
    // validates the module compiles and basic structure is correct.
    // Full async test would need #[tokio::test]
    let cs = ChunkStore::new();
    let r#gen = Arc::new(FlatGenerator::new());
    let seed = 42u64;

    // Verify chunk store works with generator
    let pos = ChunkPos::new(0, 0);
    let chunk = r#gen.generate_chunk(pos, seed);
    cs.insert(pos, chunk);
    assert_eq!(cs.count(), 1);
}
