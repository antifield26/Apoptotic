//! 类型化常量 — 消除硬编码魔法数字

/// 实体类型 ID (Minecraft Java Edition)
pub mod entity_type {
    // Passive mobs
    pub const COW: i32 = 11;
    pub const PIG: i32 = 12;
    pub const CHICKEN: i32 = 13;
    pub const SHEEP: i32 = 14;
    pub const RABBIT: i32 = 15;
    pub const BAT: i32 = 16;
    pub const SQUID: i32 = 17;
    pub const DOLPHIN: i32 = 18;
    pub const TURTLE: i32 = 19;
    pub const PUFFERFISH: i32 = 20;
    pub const TROPICAL_FISH: i32 = 21;
    pub const PHANTOM: i32 = 22;
    pub const COD: i32 = 23;
    pub const SALMON: i32 = 24;
    // Hostile/Boss
    pub const WITHER: i32 = 25;
    pub const GUARDIAN: i32 = 26;
    pub const GLOW_SQUID: i32 = 27;
    pub const POLAR_BEAR: i32 = 28;
    pub const PANDA: i32 = 29;
    pub const CREEPER: i32 = 33;
    pub const SLIME: i32 = 34;
    pub const SPIDER: i32 = 35;
    pub const ZOMBIE: i32 = 36;
    pub const SKELETON: i32 = 37;
    pub const ENDERMAN: i32 = 38;
    pub const BLAZE: i32 = 43;
    pub const FOX: i32 = 44;
    pub const DROWNED: i32 = 45;
    pub const CAVE_SPIDER: i32 = 46;
    pub const SILVERFISH: i32 = 47;
    pub const WITCH: i32 = 48;
    pub const WITHER_SKELETON: i32 = 49;
    pub const ZOMBIE_VILLAGER: i32 = 50;
    pub const VINDICATOR: i32 = 51;
    pub const EVOKER: i32 = 52;
    pub const ENDER_DRAGON: i32 = 53;
    pub const MAGMA_CUBE: i32 = 55;
    pub const GHAST: i32 = 56;
    pub const ZOMBIE_PIGMAN: i32 = 57;
    pub const HOGLIN: i32 = 58;
    pub const PIGLIN: i32 = 59;
    pub const PIGLIN_BRUTE: i32 = 60;
    pub const RAVAGER: i32 = 61;
    pub const SHULKER: i32 = 62;
    pub const WARDEN: i32 = 63;
    pub const ALLAY: i32 = 64;
    pub const BEE: i32 = 65;
    pub const CAMEL: i32 = 67;
    pub const SNIFFER: i32 = 70;
    pub const BREEZE: i32 = 71;
    pub const BOGGED: i32 = 130; // bogged skeleton variant (moved from 72 — collision with FIREWORK_ENTITY)
    // Variant hostile (unique IDs to avoid collision with COD/SALMON/SPIDER)
    pub const HUSK: i32 = 111;   // desert zombie variant
    pub const STRAY: i32 = 112;  // frozen skeleton variant
    pub const VEX: i32 = 113;    // evoker summon
    // Utility
    pub const VILLAGER: i32 = 92;
    pub const WANDERING_TRADER: i32 = 95;
    pub const IRON_GOLEM: i32 = 99;
    pub const SNOW_GOLEM: i32 = 105;
    pub const TADPOLE: i32 = 98;
    pub const FROG: i32 = 106;
    pub const ARMADILLO: i32 = 108;
    // Missing entity types (added for taming/breeding correctness)
    pub const WOLF: i32 = 114;
    pub const CAT: i32 = 115;
    pub const OCELOT: i32 = 116;
    pub const PARROT: i32 = 117;
    pub const HORSE: i32 = 118;
    pub const DONKEY: i32 = 119;
    pub const LLAMA: i32 = 120;
    pub const TRADER_LLAMA: i32 = 121;
    // Additional passive/hostile — added for AI completion
    pub const AXOLOTL: i32 = 123;
    pub const GOAT: i32 = 124;
    pub const STRIDER: i32 = 125;
    pub const SKELETON_HORSE: i32 = 126;
    pub const ZOMBIE_HORSE: i32 = 127;
    pub const MOOSHROOM: i32 = 128;
    pub const ELDER_GUARDIAN: i32 = 129;
    // Vehicles (IDs 10, 40-42 reserved for minecart variants)
    pub const MINECART: i32 = 10;
    pub const CHEST_MINECART: i32 = 40;
    pub const FURNACE_MINECART: i32 = 41;
    pub const HOPPER_MINECART: i32 = 42;
    pub const TNT_MINECART: i32 = 107;
    pub const BOAT: i32 = 41; // boat entity type (moved from 23 — collision with COD)
    // Projectiles/Items
    pub const ARROW_ENTITY: i32 = 7;
    pub const ITEM_ENTITY: i32 = 54;
    pub const XP_ORB: i32 = 2;  // experience orb entity type (correct vanilla ID)
    pub const SNOWBALL_ENTITY: i32 = 86;
    pub const EGG_ENTITY: i32 = 87;
    pub const ENDER_PEARL_ENTITY: i32 = 79;
    pub const FIREWORK_ENTITY: i32 = 72;
    pub const SULFUR_CUBE: i32 = 131; // 26.2 Chaos Cubed passive mob
    pub const AREA_EFFECT_CLOUD: i32 = 122;

    /// Check if mob type is hostile
    pub fn is_hostile(t: i32) -> bool {
        matches!(t,
            CREEPER | SLIME | SPIDER | ZOMBIE | SKELETON | ENDERMAN |
            CAVE_SPIDER | SILVERFISH | BLAZE | WITCH | DROWNED |
            WITHER_SKELETON | ZOMBIE_VILLAGER | VINDICATOR | EVOKER |
            MAGMA_CUBE | GHAST | ZOMBIE_PIGMAN | HOGLIN | PIGLIN |
            PIGLIN_BRUTE | RAVAGER | SHULKER | WARDEN | BREEZE | BOGGED |
            WITHER | ENDER_DRAGON | GUARDIAN | VEX | HUSK | STRAY |
            ZOMBIE_HORSE | ELDER_GUARDIAN
        )
    }

    /// Check if undead (Smite-affected)
    pub fn is_undead(t: i32) -> bool {
        matches!(t,
            ZOMBIE | SKELETON | DROWNED | WITHER_SKELETON |
            ZOMBIE_VILLAGER | WITHER | BOGGED | ZOMBIE_PIGMAN | HUSK | STRAY |
            SKELETON_HORSE | ZOMBIE_HORSE
        )
    }

    /// Check if arthropod (BaneOfArthropods-affected)
    pub fn is_arthropod(t: i32) -> bool {
        matches!(t, SPIDER | CAVE_SPIDER | SILVERFISH | ENDERMAN | BEE)
    }
}

/// 方块 ID 常量
pub mod block_id {
    pub const AIR: u32 = 0;
    pub const STONE: u32 = 1;
    pub const GRASS: u32 = 2;
    pub const SAND: u32 = 12;
    pub const GRAVEL: u32 = 13;
    pub const GLASS: u32 = 20;
    pub const DISPENSER: u32 = 23;
    pub const TNT: u32 = 25;
    pub const POWERED_RAIL: u32 = 27;
    pub const OAK_LOG: u32 = 34;
    pub const BOOKSHELF: u32 = 47;
    pub const OBSIDIAN: u32 = 49;
    pub const TORCH: u32 = 50;
    pub const FIRE: u32 = 51;
    pub const CHEST: u32 = 54;
    pub const FURNACE: u32 = 61;
    pub const CRAFTING_TABLE: u32 = 113;
    pub const ENCHANTING_TABLE: u32 = 151;
    pub const BREWING_STAND: u32 = 117;
    pub const BEACON: u32 = 167;
    pub const GRINDSTONE: u32 = 169;
    pub const SMITHING_TABLE: u32 = 455;
    pub const STONECUTTER: u32 = 456;
    pub const LOOM: u32 = 457;
    pub const CARTOGRAPHY_TABLE: u32 = 458;
    pub const LECTERN: u32 = 459;
    pub const CRAFTER: u32 = 364;
    pub const HOPPER: u32 = 154;
    pub const NOTE_BLOCK: u32 = 74;
    pub const JUKEBOX: u32 = 84;
    pub const PISTON: u32 = 137;
    pub const STICKY_PISTON: u32 = 138;
    pub const OBSERVER: u32 = 317;
    pub const COMPARATOR: u32 = 149;
    pub const REDSTONE_WIRE: u32 = 993;
    pub const REDSTONE_TORCH: u32 = 994;
    pub const REDSTONE_BLOCK: u32 = 152;
    pub const WATER: u32 = 267;
    pub const LAVA: u32 = 268;
    pub const PORTAL: u32 = 90;
    pub const BEDROCK: u32 = 266;
    pub const SOUL_SAND: u32 = 85;
    pub const MAGMA_BLOCK: u32 = 213;
    pub const ICE: u32 = 79;
    pub const CACTUS: u32 = 81;
    pub const SUGAR_CANE: u32 = 83;
    pub const MELON: u32 = 103;
    pub const WHEAT: u32 = 59;
    pub const CARROT: u32 = 141;
    pub const POTATO: u32 = 142;
    pub const BEETROOT: u32 = 207;
}

/// 物品 ID 常量
pub mod item_id {
    pub const STICK: u32 = 794;
    pub const SHIELD: u32 = 895;
    pub const FISHING_ROD: u32 = 844;
    pub const BOW: u32 = 773;
    pub const ARROW: u32 = 774;
    pub const LEAD: u32 = 966;
    pub const SHEARS: u32 = 845;
    pub const ENCHANTED_BOOK: u32 = 1050;
    pub const LAPIS_LAZULI: u32 = 571;
    pub const PAPER: u32 = 339;
    pub const GLASS_PANE: u32 = 102;
    // Swords
    pub const WOODEN_SWORD: u32 = 780;
    pub const STONE_SWORD: u32 = 785;
    pub const IRON_SWORD: u32 = 792;
    pub const DIAMOND_SWORD: u32 = 797;
    // Axes
    pub const WOODEN_AXE: u32 = 770;
    pub const STONE_AXE: u32 = 788;
    pub const IRON_AXE: u32 = 791;
}
