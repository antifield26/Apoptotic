//! Sound effect ID registry for protocol 776 (1.21.5)
//!
//! Maps common gameplay sounds to their protocol numeric IDs.
//! IDs are from the minecraft:sound_event registry built into the client.

/// Common sound effect IDs used for gameplay feedback
pub struct SoundIds;

impl SoundIds {
    // Block interactions
    pub const BLOCK_STONE_BREAK: i32 = 1;
    pub const BLOCK_WOOD_BREAK: i32 = 2;
    pub const BLOCK_GRAVEL_BREAK: i32 = 3;
    pub const BLOCK_GRASS_BREAK: i32 = 4;
    pub const BLOCK_STONE_PLACE: i32 = 5;
    pub const BLOCK_WOOD_PLACE: i32 = 6;
    pub const BLOCK_GRAVEL_PLACE: i32 = 7;
    pub const BLOCK_GRASS_PLACE: i32 = 8;
    pub const BLOCK_SAND_BREAK: i32 = 9;
    pub const BLOCK_SAND_PLACE: i32 = 10;

    // Entity sounds
    pub const ENTITY_PLAYER_HURT: i32 = 100;
    pub const ENTITY_PLAYER_DEATH: i32 = 101;
    pub const ENTITY_PLAYER_ATTACK_SWEEP: i32 = 102;
    pub const ENTITY_PLAYER_ATTACK_KNOCKBACK: i32 = 103;
    pub const ENTITY_PLAYER_ATTACK_STRONG: i32 = 104;
    pub const ENTITY_PLAYER_ATTACK_WEAK: i32 = 105;
    pub const ENTITY_PLAYER_ATTACK_NODAMAGE: i32 = 106;

    // Generic entity
    pub const ENTITY_GENERIC_HURT: i32 = 50;
    pub const ENTITY_GENERIC_DEATH: i32 = 51;
    pub const ENTITY_GENERIC_EXPLODE: i32 = 52;

    // Ambient
    pub const AMBIENT_CAVE: i32 = 200;

    // Weather
    pub const WEATHER_RAIN: i32 = 300;
    pub const WEATHER_THUNDER: i32 = 301;

    // Blocks
    pub const BLOCK_BELL: i32 = 400;
    pub const BLOCK_CAMPFIRE_CRACKLE: i32 = 401;

    /// Get a break sound ID for a block type (approximate mapping)
    pub fn break_sound_for_block(block_id: u32) -> i32 {
        match block_id {
            // Sand (before stone family to get priority)
            25 => Self::BLOCK_SAND_BREAK,
            // Dirt/grass
            8..=11 => Self::BLOCK_GRASS_BREAK,
            // Wood/log family
            56..=65 => Self::BLOCK_WOOD_BREAK,
            // Stone family (default for most mineral blocks)
            _ => Self::BLOCK_STONE_BREAK,
        }
    }
}

/// Sound categories (sent with SoundEffect packet)
pub struct SoundCategory;
impl SoundCategory {
    pub const MASTER: i32 = 0;
    pub const MUSIC: i32 = 1;
    pub const RECORDS: i32 = 2;
    pub const WEATHER: i32 = 3;
    pub const BLOCKS: i32 = 4;
    pub const HOSTILE: i32 = 5;
    pub const NEUTRAL: i32 = 6;
    pub const PLAYERS: i32 = 7;
    pub const AMBIENT: i32 = 8;
    pub const VOICE: i32 = 9;
}
