//! Registry Codec — 生成 JoinGame 包所需的 NBT 注册表数据
//!
//! Minecraft 1.21.x 客户端要求 JoinGame 包含完整的维度和生物群系注册表。
//! 此模块生成最小可行注册表，包含 "minecraft:overworld" 维度和 "minecraft:plains" 生物群系。

use serde::Serialize;

/// 生成默认注册表 NBT（1.21.x 兼容）
pub fn default_registry_codec() -> Vec<u8> {
    let registry = build_registry();
    let mut buf = Vec::new();
    fastnbt::to_writer(&mut buf, &registry).expect("registry codec serialization must succeed");
    buf
}

#[derive(Serialize)]
struct RegistryCodec {
    #[serde(rename = "minecraft:dimension_type")]
    dimension_type: RegistryEntry<DimensionTypeList>,
    #[serde(rename = "minecraft:worldgen/biome")]
    biome: RegistryEntry<BiomeList>,
}

#[derive(Serialize)]
struct RegistryEntry<T: Serialize> {
    #[serde(rename = "type")]
    reg_type: String,
    value: T,
}

// ═════════════════════════════════════════════════
// Dimension Type
// ═════════════════════════════════════════════════

#[derive(Serialize)]
struct DimensionTypeList {
    list: Vec<DimensionTypeEntry>,
}

#[derive(Serialize)]
struct DimensionTypeEntry {
    name: String,
    id: i32,
    element: DimensionElement,
}

#[derive(Serialize)]
struct DimensionElement {
    // Basic properties
    #[serde(rename = "has_skylight")]
    has_skylight: i8,
    #[serde(rename = "has_ceiling")]
    has_ceiling: i8,
    ultrawarm: i8,
    natural: i8,
    coordinate_scale: f64,
    bed_works: i8,
    respawn_anchor_works: i8,
    min_y: i32,
    height: i32,
    logical_height: i32,
    infiniburn: String,
    effects: String,
    ambient_light: f32,
    piglin_safe: i8,
    has_raids: i8,
    monster_spawn_light_level: MonsterSpawnLight,
    monster_spawn_block_light_limit: i32,
}

#[derive(Serialize)]
struct MonsterSpawnLight {
    #[serde(rename = "type")]
    ty: String,
    value: MonsterSpawnValue,
}

#[derive(Serialize)]
struct MonsterSpawnValue {
    min_inclusive: i32,
    max_inclusive: i32,
}

fn dimension_type_list() -> DimensionTypeList {
    DimensionTypeList {
        list: vec![
            // ── Overworld (id=0) ──
            DimensionTypeEntry {
                name: "minecraft:overworld".into(),
                id: 0,
                element: DimensionElement {
                    has_skylight: 1,
                    has_ceiling: 0,
                    ultrawarm: 0,
                    natural: 1,
                    coordinate_scale: 1.0,
                    bed_works: 1,
                    respawn_anchor_works: 0,
                    min_y: -64,
                    height: 384,
                    logical_height: 384,
                    infiniburn: "#minecraft:infiniburn_overworld".into(),
                    effects: "minecraft:overworld".into(),
                    ambient_light: 0.0,
                    piglin_safe: 0,
                    has_raids: 1,
                    monster_spawn_light_level: MonsterSpawnLight {
                        ty: "minecraft:uniform".into(),
                        value: MonsterSpawnValue {
                            min_inclusive: 0,
                            max_inclusive: 7,
                        },
                    },
                    monster_spawn_block_light_limit: 0,
                },
            },
            // ── The Nether (id=1) ──
            DimensionTypeEntry {
                name: "minecraft:the_nether".into(),
                id: 1,
                element: DimensionElement {
                    has_skylight: 0,
                    has_ceiling: 1,
                    ultrawarm: 1,
                    natural: 0,
                    coordinate_scale: 8.0,
                    bed_works: 0,
                    respawn_anchor_works: 1,
                    min_y: 0,
                    height: 256,
                    logical_height: 128,
                    infiniburn: "#minecraft:infiniburn_nether".into(),
                    effects: "minecraft:the_nether".into(),
                    ambient_light: 0.1,
                    piglin_safe: 1,
                    has_raids: 0,
                    monster_spawn_light_level: MonsterSpawnLight {
                        ty: "minecraft:uniform".into(),
                        value: MonsterSpawnValue {
                            min_inclusive: 0,
                            max_inclusive: 7,
                        },
                    },
                    monster_spawn_block_light_limit: 15,
                },
            },
            // ── The End (id=2) ──
            DimensionTypeEntry {
                name: "minecraft:the_end".into(),
                id: 2,
                element: DimensionElement {
                    has_skylight: 0,
                    has_ceiling: 0,
                    ultrawarm: 0,
                    natural: 0,
                    coordinate_scale: 1.0,
                    bed_works: 0,
                    respawn_anchor_works: 0,
                    min_y: 0,
                    height: 256,
                    logical_height: 256,
                    infiniburn: "#minecraft:infiniburn_end".into(),
                    effects: "minecraft:the_end".into(),
                    ambient_light: 0.0,
                    piglin_safe: 0,
                    has_raids: 0,
                    monster_spawn_light_level: MonsterSpawnLight {
                        ty: "minecraft:uniform".into(),
                        value: MonsterSpawnValue {
                            min_inclusive: 0,
                            max_inclusive: 7,
                        },
                    },
                    monster_spawn_block_light_limit: 0,
                },
            },
        ],
    }
}

// ═════════════════════════════════════════════════
// Biome
// ═════════════════════════════════════════════════

#[derive(Serialize)]
struct BiomeList {
    list: Vec<BiomeEntry>,
}

#[derive(Serialize)]
struct BiomeEntry {
    name: String,
    id: i32,
    element: BiomeElement,
}

#[derive(Serialize)]
struct BiomeElement {
    has_precipitation: i8,
    temperature: f32,
    temperature_modifier: Option<String>,
    downfall: f32,
    effects: BiomeEffects,
}

#[derive(Serialize)]
struct BiomeEffects {
    fog_color: i32,
    water_color: i32,
    water_fog_color: i32,
    sky_color: i32,
    foliage_color: Option<i32>,
    grass_color: Option<i32>,
    grass_color_modifier: Option<String>,
    music: Option<MusicEffect>,
    ambient_sound: Option<String>,
    additions_sound: Option<SoundEvent>,
    mood_sound: Option<MoodSound>,
    particle: Option<BiomeParticle>,
}

#[derive(Serialize)]
struct MusicEffect {
    sound: String,
    min_delay: i32,
    max_delay: i32,
    replace_current_music: i8,
}

#[derive(Serialize)]
struct SoundEvent {
    sound: String,
    tick_chance: f64,
}

#[derive(Serialize)]
struct MoodSound {
    sound: String,
    tick_delay: i32,
    offset: f64,
    block_search_extent: i32,
}

#[derive(Serialize)]
struct BiomeParticle {
    options: ParticleOptions,
    probability: f32,
}

#[derive(Serialize)]
struct ParticleOptions {
    #[serde(rename = "type")]
    ty: String,
}

fn biome_list() -> BiomeList {
    BiomeList {
        list: vec![
            // ═══ Overworld Biomes ═══
            BiomeEntry {
                name: "minecraft:plains".into(),
                id: 0,
                element: plains_biome(),
            },
            BiomeEntry {
                name: "minecraft:the_void".into(),
                id: 1,
                element: BiomeElement {
                    has_precipitation: 0,
                    temperature: 0.5,
                    temperature_modifier: None,
                    downfall: 0.5,
                    effects: BiomeEffects {
                        fog_color: 0xC0D8FF,
                        water_color: 0x3F76E4,
                        water_fog_color: 0x050533,
                        sky_color: 0x6EB1FF,
                        foliage_color: None,
                        grass_color: None,
                        grass_color_modifier: None,
                        music: None,
                        ambient_sound: None,
                        additions_sound: None,
                        mood_sound: None,
                        particle: None,
                    },
                },
            },
            BiomeEntry {
                name: "minecraft:forest".into(),
                id: 2,
                element: plain_style_biome(0.7, 0.8, 0x79C05A),
            },
            BiomeEntry {
                name: "minecraft:ocean".into(),
                id: 3,
                element: plain_style_biome(0.5, 0.5, 0x3F76E4),
            },
            BiomeEntry {
                name: "minecraft:desert".into(),
                id: 4,
                element: plain_style_biome(2.0, 0.0, 0xBFB755),
            },
            // ═══ Nether Biomes ═══
            BiomeEntry {
                name: "minecraft:nether_wastes".into(),
                id: 5,
                element: nether_biome(2.0, 0.0, 0x330808),
            },
            BiomeEntry {
                name: "minecraft:soul_sand_valley".into(),
                id: 6,
                element: nether_biome(2.0, 0.0, 0x1B4745),
            },
            BiomeEntry {
                name: "minecraft:crimson_forest".into(),
                id: 7,
                element: nether_biome(2.0, 0.0, 0x330303),
            },
            BiomeEntry {
                name: "minecraft:warped_forest".into(),
                id: 8,
                element: nether_biome(2.0, 0.0, 0x03303),
            },
            BiomeEntry {
                name: "minecraft:basalt_deltas".into(),
                id: 9,
                element: nether_biome(2.0, 0.0, 0x40332E),
            },
            // ═══ End Biomes ═══
            BiomeEntry {
                name: "minecraft:the_end".into(),
                id: 10,
                element: end_biome(),
            },
            BiomeEntry {
                name: "minecraft:end_highlands".into(),
                id: 11,
                element: end_biome(),
            },
            BiomeEntry {
                name: "minecraft:end_midlands".into(),
                id: 12,
                element: end_biome(),
            },
            BiomeEntry {
                name: "minecraft:small_end_islands".into(),
                id: 13,
                element: end_biome(),
            },
            BiomeEntry {
                name: "minecraft:end_barrens".into(),
                id: 14,
                element: end_biome(),
            },
            // ═══ Overworld biomes (expanded — IDs 15-53) ═══
            BiomeEntry { name: "minecraft:taiga".into(), id: 15, element: plain_style_biome(0.25, 0.8, 0x0B6659) },
            BiomeEntry { name: "minecraft:snowy_plains".into(), id: 16, element: plain_style_biome(0.0, 0.5, 0xC0D8FF) },
            BiomeEntry { name: "minecraft:ice_spikes".into(), id: 17, element: plain_style_biome(0.0, 0.5, 0xC0D8FF) },
            BiomeEntry { name: "minecraft:badlands".into(), id: 18, element: plain_style_biome(2.0, 0.0, 0x90814D) },
            BiomeEntry { name: "minecraft:wooded_badlands".into(), id: 19, element: plain_style_biome(2.0, 0.0, 0x9E814D) },
            BiomeEntry { name: "minecraft:eroded_badlands".into(), id: 20, element: plain_style_biome(2.0, 0.0, 0xA0814D) },
            BiomeEntry { name: "minecraft:swamp".into(), id: 21, element: plain_style_biome(0.8, 0.9, 0x4C763C) },
            BiomeEntry { name: "minecraft:mangrove_swamp".into(), id: 22, element: plain_style_biome(0.8, 0.9, 0x5A7A3C) },
            BiomeEntry { name: "minecraft:jungle".into(), id: 23, element: plain_style_biome(0.95, 0.9, 0x2B820) },
            BiomeEntry { name: "minecraft:sparse_jungle".into(), id: 24, element: plain_style_biome(0.95, 0.8, 0x3E9320) },
            BiomeEntry { name: "minecraft:bamboo_jungle".into(), id: 25, element: plain_style_biome(0.95, 0.9, 0x2C820) },
            BiomeEntry { name: "minecraft:savanna".into(), id: 26, element: plain_style_biome(1.2, 0.2, 0xBDB25F) },
            BiomeEntry { name: "minecraft:savanna_plateau".into(), id: 27, element: plain_style_biome(1.0, 0.2, 0xADA15F) },
            BiomeEntry { name: "minecraft:windswept_hills".into(), id: 28, element: plain_style_biome(0.2, 0.3, 0x8AB689) },
            BiomeEntry { name: "minecraft:windswept_gravelly_hills".into(), id: 29, element: plain_style_biome(0.2, 0.3, 0x8AB689) },
            BiomeEntry { name: "minecraft:windswept_forest".into(), id: 30, element: plain_style_biome(0.2, 0.3, 0x7AA679) },
            BiomeEntry { name: "minecraft:dark_forest".into(), id: 31, element: plain_style_biome(0.7, 0.8, 0x334F1F) },
            BiomeEntry { name: "minecraft:birch_forest".into(), id: 32, element: plain_style_biome(0.6, 0.6, 0x88BB67) },
            BiomeEntry { name: "minecraft:old_growth_birch_forest".into(), id: 33, element: plain_style_biome(0.6, 0.6, 0x78AB57) },
            BiomeEntry { name: "minecraft:sunflower_plains".into(), id: 34, element: plain_style_biome(0.8, 0.4, 0x79C05A) },
            BiomeEntry { name: "minecraft:flower_forest".into(), id: 35, element: plain_style_biome(0.7, 0.8, 0x79C05A) },
            BiomeEntry { name: "minecraft:beach".into(), id: 36, element: plain_style_biome(0.8, 0.4, 0xFADE55) },
            BiomeEntry { name: "minecraft:snowy_beach".into(), id: 37, element: plain_style_biome(0.05, 0.3, 0xFAF0C0) },
            BiomeEntry { name: "minecraft:stony_shore".into(), id: 38, element: plain_style_biome(0.2, 0.3, 0x8AB689) },
            BiomeEntry { name: "minecraft:river".into(), id: 39, element: plain_style_biome(0.5, 0.5, 0x3F76E4) },
            BiomeEntry { name: "minecraft:frozen_river".into(), id: 40, element: plain_style_biome(0.0, 0.5, 0xA0D8FF) },
            BiomeEntry { name: "minecraft:mushroom_fields".into(), id: 41, element: plain_style_biome(0.9, 1.0, 0xC0D8FF) },
            BiomeEntry { name: "minecraft:warm_ocean".into(), id: 42, element: plain_style_biome(0.5, 0.5, 0x43D5EE) },
            BiomeEntry { name: "minecraft:lukewarm_ocean".into(), id: 43, element: plain_style_biome(0.5, 0.5, 0x3F76E4) },
            BiomeEntry { name: "minecraft:cold_ocean".into(), id: 44, element: plain_style_biome(0.5, 0.5, 0x3D57D6) },
            BiomeEntry { name: "minecraft:frozen_ocean".into(), id: 45, element: plain_style_biome(0.0, 0.5, 0x3938C9) },
            BiomeEntry { name: "minecraft:deep_ocean".into(), id: 46, element: plain_style_biome(0.5, 0.5, 0x3F76E4) },
            BiomeEntry { name: "minecraft:deep_lukewarm_ocean".into(), id: 47, element: plain_style_biome(0.5, 0.5, 0x3F76E4) },
            BiomeEntry { name: "minecraft:deep_cold_ocean".into(), id: 48, element: plain_style_biome(0.5, 0.5, 0x3D57D6) },
            BiomeEntry { name: "minecraft:deep_frozen_ocean".into(), id: 49, element: plain_style_biome(0.0, 0.5, 0x3938C9) },
            // Mountain biomes
            BiomeEntry { name: "minecraft:meadow".into(), id: 50, element: plain_style_biome(0.3, 0.4, 0x88BB67) },
            BiomeEntry { name: "minecraft:grove".into(), id: 51, element: plain_style_biome(-0.2, 0.6, 0x88BB67) },
            BiomeEntry { name: "minecraft:snowy_slopes".into(), id: 52, element: plain_style_biome(-0.3, 0.5, 0xC0D8FF) },
            BiomeEntry { name: "minecraft:jagged_peaks".into(), id: 53, element: plain_style_biome(-0.7, 0.5, 0xABBCD6) },
            BiomeEntry { name: "minecraft:frozen_peaks".into(), id: 54, element: plain_style_biome(-0.7, 0.5, 0xC0D8FF) },
            BiomeEntry { name: "minecraft:stony_peaks".into(), id: 55, element: plain_style_biome(1.0, 0.3, 0x8AB689) },
            // Cave biomes
            BiomeEntry { name: "minecraft:dripstone_caves".into(), id: 56, element: plain_style_biome(0.8, 0.4, 0x4C3A3A) },
            BiomeEntry { name: "minecraft:lush_caves".into(), id: 57, element: plain_style_biome(0.5, 0.5, 0x8BA030) },
            BiomeEntry { name: "minecraft:deep_dark".into(), id: 58, element: plain_style_biome(0.8, 0.4, 0x000000) },
            // 1.19-1.21 biomes
            BiomeEntry { name: "minecraft:cherry_grove".into(), id: 59, element: plain_style_biome(0.5, 0.6, 0xF2C4E0) },
            BiomeEntry { name: "minecraft:pale_garden".into(), id: 60, element: plain_style_biome(0.5, 0.5, 0xBFBFBF) },
        ],
    }
}

fn plains_biome() -> BiomeElement {
    BiomeElement {
        has_precipitation: 1,
        temperature: 0.8,
        temperature_modifier: None,
        downfall: 0.4,
        effects: BiomeEffects {
            fog_color: 0xC0D8FF,
            water_color: 0x3F76E4,
            water_fog_color: 0x050533,
            sky_color: 0x78A7FF,
            foliage_color: None,
            grass_color: None,
            grass_color_modifier: None,
            music: Some(MusicEffect {
                sound: "minecraft:music.overworld.plains".into(),
                min_delay: 12000,
                max_delay: 24000,
                replace_current_music: 0,
            }),
            ambient_sound: None,
            additions_sound: None,
            mood_sound: Some(MoodSound {
                sound: "minecraft:ambient.cave".into(),
                tick_delay: 6000,
                offset: 2.0,
                block_search_extent: 8,
            }),
            particle: None,
        },
    }
}

fn plain_style_biome(temp: f32, down: f32, fog: i32) -> BiomeElement {
    BiomeElement {
        has_precipitation: 1,
        temperature: temp,
        temperature_modifier: None,
        downfall: down,
        effects: BiomeEffects {
            fog_color: fog,
            water_color: 0x3F76E4,
            water_fog_color: 0x050533,
            sky_color: 0x78A7FF,
            foliage_color: None,
            grass_color: None,
            grass_color_modifier: None,
            music: None,
            ambient_sound: None,
            additions_sound: None,
            mood_sound: None,
            particle: None,
        },
    }
}

fn nether_biome(temp: f32, down: f32, fog: i32) -> BiomeElement {
    BiomeElement {
        has_precipitation: 0,
        temperature: temp,
        temperature_modifier: None,
        downfall: down,
        effects: BiomeEffects {
            fog_color: fog,
            water_color: 0x3F76E4,
            water_fog_color: 0x050533,
            sky_color: 0x330808,
            foliage_color: None,
            grass_color: None,
            grass_color_modifier: None,
            music: Some(MusicEffect {
                sound: "minecraft:music.nether.nether_wastes".into(),
                min_delay: 12000,
                max_delay: 24000,
                replace_current_music: 0,
            }),
            ambient_sound: Some("minecraft:ambient.nether_wastes.loop".into()),
            additions_sound: None,
            mood_sound: Some(MoodSound {
                sound: "minecraft:ambient.nether_wastes.mood".into(),
                tick_delay: 6000,
                offset: 2.0,
                block_search_extent: 8,
            }),
            particle: None,
        },
    }
}

fn end_biome() -> BiomeElement {
    BiomeElement {
        has_precipitation: 0,
        temperature: 0.5,
        temperature_modifier: None,
        downfall: 0.5,
        effects: BiomeEffects {
            fog_color: 0xA080A0,
            water_color: 0x3F76E4,
            water_fog_color: 0x050533,
            sky_color: 0x000000,
            foliage_color: None,
            grass_color: None,
            grass_color_modifier: None,
            music: Some(MusicEffect {
                sound: "minecraft:music.end".into(),
                min_delay: 6000,
                max_delay: 12000,
                replace_current_music: 0,
            }),
            ambient_sound: None,
            additions_sound: None,
            mood_sound: None,
            particle: None,
        },
    }
}

// ═════════════════════════════════════════════════
// Build
// ═════════════════════════════════════════════════

fn build_registry() -> RegistryCodec {
    RegistryCodec {
        dimension_type: RegistryEntry {
            reg_type: "minecraft:dimension_type".into(),
            value: dimension_type_list(),
        },
        biome: RegistryEntry {
            reg_type: "minecraft:worldgen/biome".into(),
            value: biome_list(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_codec_serializes() {
        let data = default_registry_codec();
        assert!(!data.is_empty(), "registry codec must not be empty");
        // Should be a valid NBT compound
        assert!(data.len() > 100, "registry codec should be substantial");
    }

    #[test]
    fn test_registry_codec_deserializable() {
        let data = default_registry_codec();
        let result: Result<fastnbt::Value, _> = fastnbt::from_bytes(&data);
        assert!(result.is_ok(), "registry codec must be valid NBT");
    }
}
