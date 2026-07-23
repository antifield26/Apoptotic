#!/bin/bash
# Fix all packet IDs in play.rs to match protocol 776
# Generated from official protocol.json at:
# MC_Protocol_Data-master/java_edition/indexed_data/1073742149/protocol.json

F="crates/mc-protocol/src/packets/play.rs"

echo "=== Fixing S2C packet IDs for protocol 776 ==="

# Format: struct_name → old_id → new_id
# Each fix: find the struct's packet_id function and update the return value
fix_s2c() {
    local struct="$1" old_hex="$2" new_hex="$3"
    # Find the struct and fix its packet_id
    sed -i "/pub struct ${struct}/,/^}/{
        s/fn packet_id.*-> i32 { ${old_hex} }/fn packet_id(\&self) -> i32 { ${new_hex} }/
        s/fn packet_id().*-> i32 { ${old_hex} }/fn packet_id() -> i32 { ${new_hex} }/
    }" "$F"
}

# ── S2C (server→client) PacketEncoder ──
fix_s2c "KeepAlive"              "0x26" "0x2C"
fix_s2c "PlayDisconnect"         "0x1D" "0x20"
fix_s2c "SystemChatMessage"      "0x6C" "0x7A"
fix_s2c "PlayerPosition"         "0x3E" "0x48"
fix_s2c "RemoveEntities"         "0x42" "0x4D"
fix_s2c "BlockUpdate"            "0x0C" "0x08"
fix_s2c "SoundEffect"            "0x68" "0x76"
fix_s2c "SetHealth"              "0x5F" "0x69"
fix_s2c "SetExperience"          "0x5F" "0x68"
fix_s2c "PlayerAbilities"        "0x38" "0x40"
fix_s2c "UpdateAttributes"       "0x70" "0x84"
fix_s2c "OpenScreen"             "0x33" "0x3B"
fix_s2c "ContainerSetContent"    "0x14" "0x12"
fix_s2c "SetSlot"                "0x16" "0x14"
fix_s2c "TeleportEntity"         "0x6E" "0x7E"
fix_s2c "UpdateRecipes"          "0x72" "0x86"
fix_s2c "SetHeldItemS2C"         "0x53" "0x6A"
fix_s2c "BossEvent"              "0x0B" "0x09"
fix_s2c "SectionBlocksUpdate"    "0x47" "0x55"
fix_s2c "SpawnEntity"            "0x01" "0x01"
fix_s2c "SetEntityMetadata"      "0x5A" "0x5E"
fix_s2c "EntityEvent"            "0x1C" "0x1E"
fix_s2c "SetDefaultSpawnPosition" "0x52" "0x62"
fix_s2c "GameEvent"              "0x22" "0x26"
fix_s2c "WorldEvent"             "0x24" "0x28"

echo "=== S2C packet IDs fixed ==="
echo "=== Protocol 776 chunk format needs manual implementation ==="
