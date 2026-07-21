//! 类型化常量 — 官方 Minecraft 26.2 protocol IDs

/// 实体类型 ID (Minecraft 26.2 — 158 types)
pub mod entity_type {

    // Passive mobs
    pub const COW: i32 = 30;
    pub const PIG: i32 = 100;
    pub const CHICKEN: i32 = 26;
    pub const SHEEP: i32 = 111;
    pub const RABBIT: i32 = 108;
    pub const BAT: i32 = 10;
    pub const SQUID: i32 = 127;
    pub const DOLPHIN: i32 = 35;
    pub const TURTLE: i32 = 138;
    pub const PUFFERFISH: i32 = 107;
    pub const TROPICAL_FISH: i32 = 137;
    pub const COD: i32 = 27;
    pub const SALMON: i32 = 110;
    pub const GLOW_SQUID: i32 = 61;
    pub const POLAR_BEAR: i32 = 104;
    pub const PANDA: i32 = 96;
    pub const FOX: i32 = 54;
    pub const BEE: i32 = 11;
    pub const CAMEL: i32 = 19;
    pub const SNIFFER: i32 = 119;
    pub const ARMADILLO: i32 = 4;
    pub const WOLF: i32 = 149;
    pub const CAT: i32 = 21;
    pub const OCELOT: i32 = 91;
    pub const PARROT: i32 = 98;
    pub const HORSE: i32 = 66;
    pub const DONKEY: i32 = 36;
    pub const MOOSHROOM: i32 = 86;
    pub const GOAT: i32 = 62;
    pub const AXOLOTL: i32 = 7;
    pub const FROG: i32 = 55;
    pub const TADPOLE: i32 = 131;
    pub const STRIDER: i32 = 129;
    pub const CREAKING: i32 = 31;

    // Hostile
    pub const CREEPER: i32 = 32;
    pub const SLIME: i32 = 117;
    pub const SPIDER: i32 = 124;
    pub const ZOMBIE: i32 = 151;
    pub const SKELETON: i32 = 115;
    pub const ENDERMAN: i32 = 41;
    pub const BLAZE: i32 = 14;
    pub const DROWNED: i32 = 38;
    pub const CAVE_SPIDER: i32 = 22;
    pub const SILVERFISH: i32 = 114;
    pub const WITCH: i32 = 145;
    pub const WITHER_SKELETON: i32 = 147;
    pub const ZOMBIE_VILLAGER: i32 = 154;
    pub const VINDICATOR: i32 = 141;
    pub const EVOKER: i32 = 46;
    pub const ENDER_DRAGON: i32 = 43;
    pub const MAGMA_CUBE: i32 = 80;
    pub const GHAST: i32 = 57;
    pub const ZOMBIFIED_PIGLIN: i32 = 155;
    pub const HOGLIN: i32 = 64;
    pub const PIGLIN: i32 = 101;
    pub const PIGLIN_BRUTE: i32 = 102;
    pub const RAVAGER: i32 = 109;
    pub const SHULKER: i32 = 112;
    pub const WARDEN: i32 = 143;
    pub const BREEZE: i32 = 17;
    pub const BOGGED: i32 = 16;
    pub const HUSK: i32 = 67;
    pub const STRAY: i32 = 128;
    pub const VEX: i32 = 139;
    pub const GUARDIAN: i32 = 63;
    pub const ELDER_GUARDIAN: i32 = 40;
    pub const PHANTOM: i32 = 99;
    pub const ENDERMITE: i32 = 42;
    pub const ZOGLIN: i32 = 150;
    pub const ILLUSIONER: i32 = 68;
    pub const WITHER: i32 = 146;
    pub const PARCHED: i32 = 97;

    // Vehicles
    pub const MINECART: i32 = 85;
    pub const CHEST_MINECART: i32 = 25;
    pub const FURNACE_MINECART: i32 = 56;
    pub const HOPPER_MINECART: i32 = 65;
    pub const TNT_MINECART: i32 = 134;
    pub const SPAWNER_MINECART: i32 = 122;
    pub const COMMAND_BLOCK_MINECART: i32 = 29;
    pub const OAK_BOAT: i32 = 89;
    pub const SPRUCE_BOAT: i32 = 125;
    pub const BIRCH_BOAT: i32 = 12;
    pub const JUNGLE_BOAT: i32 = 74;
    pub const ACACIA_BOAT: i32 = 0;
    pub const CHERRY_BOAT: i32 = 23;
    pub const DARK_OAK_BOAT: i32 = 33;
    pub const MANGROVE_BOAT: i32 = 81;
    pub const BAMBOO_RAFT: i32 = 9;
    pub const PALE_OAK_BOAT: i32 = 94;
    pub const OAK_CHEST_BOAT: i32 = 90;
    pub const SPRUCE_CHEST_BOAT: i32 = 126;
    pub const BIRCH_CHEST_BOAT: i32 = 13;
    pub const JUNGLE_CHEST_BOAT: i32 = 75;
    pub const ACACIA_CHEST_BOAT: i32 = 1;
    pub const CHERRY_CHEST_BOAT: i32 = 24;
    pub const DARK_OAK_CHEST_BOAT: i32 = 34;
    pub const MANGROVE_CHEST_BOAT: i32 = 82;
    pub const BAMBOO_CHEST_RAFT: i32 = 8;
    pub const PALE_OAK_CHEST_BOAT: i32 = 95;

    // Projectiles
    pub const ARROW: i32 = 6;
    pub const SPECTRAL_ARROW: i32 = 123;
    pub const DRAGON_FIREBALL: i32 = 37;
    pub const FIREBALL: i32 = 52;
    pub const SMALL_FIREBALL: i32 = 118;
    pub const WITHER_SKULL: i32 = 148;
    pub const SHULKER_BULLET: i32 = 113;
    pub const WIND_CHARGE: i32 = 144;
    pub const BREEZE_WIND_CHARGE: i32 = 18;
    pub const SNOWBALL: i32 = 120;
    pub const EGG: i32 = 39;
    pub const ENDER_PEARL: i32 = 44;
    pub const EYE_OF_ENDER: i32 = 50;
    pub const EXPERIENCE_BOTTLE: i32 = 48;
    pub const SPLASH_POTION: i32 = 105;
    pub const LINGERING_POTION: i32 = 106;
    pub const TRIDENT: i32 = 136;
    pub const FIREWORK_ROCKET: i32 = 53;
    pub const LLAMA_SPIT: i32 = 79;

    // Utility/Display
    pub const VILLAGER: i32 = 140;
    pub const WANDERING_TRADER: i32 = 142;
    pub const IRON_GOLEM: i32 = 70;
    pub const SNOW_GOLEM: i32 = 121;
    pub const ALLAY: i32 = 2;
    pub const TRADER_LLAMA: i32 = 135;
    pub const LLAMA: i32 = 78;
    pub const MULE: i32 = 87;
    pub const SKELETON_HORSE: i32 = 116;
    pub const ZOMBIE_HORSE: i32 = 152;
    pub const CAMEL_HUSK: i32 = 20;
    pub const ARMOR_STAND: i32 = 5;
    pub const ITEM_FRAME: i32 = 73;
    pub const GLOW_ITEM_FRAME: i32 = 60;
    pub const PAINTING: i32 = 93;
    pub const ITEM: i32 = 71;
    pub const EXPERIENCE_ORB: i32 = 49;
    pub const AREA_EFFECT_CLOUD: i32 = 3;
    pub const FALLING_BLOCK: i32 = 51;
    pub const TNT: i32 = 133;
    pub const END_CRYSTAL: i32 = 45;
    pub const LIGHTNING_BOLT: i32 = 77;
    pub const FISHING_BOBBER: i32 = 157;
    pub const LEASH_KNOT: i32 = 76;
    pub const MARKER: i32 = 84;
    pub const INTERACTION: i32 = 69;
    pub const ITEM_DISPLAY: i32 = 72;
    pub const BLOCK_DISPLAY: i32 = 15;
    pub const TEXT_DISPLAY: i32 = 132;
    pub const MANNEQUIN: i32 = 83;
    pub const COPPER_GOLEM: i32 = 28;
    pub const OMINOUS_ITEM_SPAWNER: i32 = 92;

    // 26.2 Chaos Cubed
    pub const SULFUR_CUBE: i32 = 130;

    // Aquatic
    pub const NAUTILUS: i32 = 88;
    pub const ZOMBIE_NAUTILUS: i32 = 153;
    pub const HAPPY_GHAST: i32 = 58;

    // Misc
    pub const PLAYER: i32 = 156;

    /// Check if mob type is hostile
    pub fn is_hostile(t: i32) -> bool {
        matches!(t, CREEPER | SLIME | SPIDER | ZOMBIE | SKELETON | ENDERMAN | BLAZE | DROWNED | CAVE_SPIDER | SILVERFISH | WITCH | WITHER_SKELETON | ZOMBIE_VILLAGER | VINDICATOR | EVOKER | ENDER_DRAGON | MAGMA_CUBE | GHAST | ZOMBIFIED_PIGLIN | HOGLIN | PIGLIN | PIGLIN_BRUTE | RAVAGER | SHULKER | WARDEN | BREEZE | BOGGED | HUSK | STRAY | VEX | GUARDIAN | ELDER_GUARDIAN | PHANTOM | ENDERMITE | ZOGLIN | ILLUSIONER | WITHER | PARCHED)
    }

    /// Check if undead (Smite-affected)
    pub fn is_undead(t: i32) -> bool {
        matches!(t, ZOMBIE | SKELETON | DROWNED | WITHER_SKELETON | ZOMBIE_VILLAGER | WITHER | BOGGED | ZOMBIFIED_PIGLIN | HUSK | STRAY | SKELETON_HORSE | ZOMBIE_HORSE | ZOGLIN | ZOMBIE_NAUTILUS | PARCHED)
    }

    /// Check if arthropod (BaneOfArthropods-affected)
    pub fn is_arthropod(t: i32) -> bool {
        matches!(t, SPIDER | CAVE_SPIDER | SILVERFISH | ENDERMAN | BEE | ENDERMITE)
    }
}

/// 方块 ID 常量 (official 26.2 IDs)
pub mod block_id {
    pub const AIR: u32 = 0;
    pub const STONE: u32 = 1;
    pub const GRASS: u32 = 8;
    pub const SAND: u32 = 37;
    pub const GRAVEL: u32 = 40;
    pub const GLASS: u32 = 101;
    pub const DISPENSER: u32 = 105;
    pub const TNT: u32 = 177;
    pub const POWERED_RAIL: u32 = 126;
    pub const OAK_LOG: u32 = 49;
    pub const BOOKSHELF: u32 = 178;
    pub const OBSIDIAN: u32 = 193;
    pub const TORCH: u32 = 194;
    pub const FIRE: u32 = 196;
    pub const CHEST: u32 = 201;
    pub const FURNACE: u32 = 209;
    pub const CRAFTING_TABLE: u32 = 206;
    pub const ENCHANTING_TABLE: u32 = 385;
    pub const BREWING_STAND: u32 = 386;
    pub const BEACON: u32 = 408;
    pub const GRINDSTONE: u32 = 844;
    pub const SMITHING_TABLE: u32 = 846;
    pub const STONECUTTER: u32 = 847;
    pub const LOOM: u32 = 838;
    pub const CARTOGRAPHY_TABLE: u32 = 842;
    pub const LECTERN: u32 = 845;
    pub const CRAFTER: u32 = 1184;
    pub const HOPPER: u32 = 477;
    pub const NOTE_BLOCK: u32 = 109;
    pub const JUKEBOX: u32 = 283;
    pub const PISTON: u32 = 138;
    pub const STICKY_PISTON: u32 = 128;
    pub const OBSERVER: u32 = 676;
    pub const COMPARATOR: u32 = 473;
    pub const REDSTONE_WIRE: u32 = 202;
    pub const REDSTONE_TORCH: u32 = 273;
    pub const REDSTONE_BLOCK: u32 = 475;
    pub const WATER: u32 = 35;
    pub const LAVA: u32 = 36;
    pub const NETHER_PORTAL: u32 = 295;
    pub const BEDROCK: u32 = 34;
    pub const SOUL_SAND: u32 = 286;
    pub const MAGMA_BLOCK: u32 = 671;
    pub const ICE: u32 = 277;
    pub const CACTUS: u32 = 279;
    pub const SUGAR_CANE: u32 = 282;
    pub const MELON: u32 = 361;
    pub const WHEAT: u32 = 207;
    pub const CARROTS: u32 = 441;
    pub const POTATOES: u32 = 442;
    pub const BEETROOTS: u32 = 665;
    pub const SULFUR_BLOCK: u32 = 998;
    pub const POTENT_SULFUR: u32 = 999;
    pub const SULFUR_SPIKE: u32 = 1134;
    pub const CINNABAR_BLOCK: u32 = 1012;
}

/// 物品 ID 常量 (official 26.2 IDs)
pub mod item_id {
    pub const STICK: u32 = 974;
    pub const SHIELD: u32 = 1325;
    pub const FISHING_ROD: u32 = 1082;
    pub const BOW: u32 = 922;
    pub const ARROW: u32 = 923;
    pub const LEAD: u32 = 1291;
    pub const SHEARS: u32 = 1134;
    pub const ENCHANTED_BOOK: u32 = 1274;
    pub const LAPIS_LAZULI: u32 = 928;
    pub const PAPER: u32 = 1057;
    pub const GLASS_PANE: u32 = 436;
    pub const WOODEN_SWORD: u32 = 939;
    pub const STONE_SWORD: u32 = 949;
    pub const IRON_SWORD: u32 = 959;
    pub const DIAMOND_SWORD: u32 = 964;
    pub const WOODEN_AXE: u32 = 942;
    pub const STONE_AXE: u32 = 952;
    pub const IRON_AXE: u32 = 962;
    pub const DIAMOND_AXE: u32 = 967;
    pub const DIAMOND_PICKAXE: u32 = 966;
    pub const GOLDEN_APPLE: u32 = 1014;
    pub const ENCHANTED_GOLDEN_APPLE: u32 = 1015;
    pub const BUCKET: u32 = 1040;
    pub const WATER_BUCKET: u32 = 1041;
    pub const LAVA_BUCKET: u32 = 1042;
    pub const MILK_BUCKET: u32 = 1046;
    pub const BREAD: u32 = 981;
    pub const COOKED_BEEF: u32 = 1140;
    pub const CROSSBOW: u32 = 1370;
    pub const TRIDENT: u32 = 1362;
    pub const SADDLE: u32 = 865;
    pub const NAME_TAG: u32 = 1292;
    pub const ENDER_PEARL: u32 = 1144;
    pub const MUSIC_DISC_BOUNCE: u32 = 1342;
    pub const SULFUR_CUBE_SPAWN_EGG: u32 = 1195;
    pub const BUCKET_OF_SULFUR_CUBE: u32 = 1052;
}
