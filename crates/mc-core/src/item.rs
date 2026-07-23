//! 物品名称注册表 — Minecraft 26.2 "Chaos Cubed" (protocol 776)
//!
//! Source: Official Minecraft 26.2 registry (extracted from local client JAR).
//! Blocks: 1,196 | Items: 1,537 | Total unique: 1691
//!
//! **Portability**: The registry data is checked into Git in `item_registry.in.rs` —
//! no Minecraft client is needed on deploy devices (RPi 5, CI, etc.).
//! Build-time integrity is verified by `build.rs` (entry count + structure).
//!
//! **Updating for new Minecraft versions**:
//!   ./scripts/update-minecraft-data.sh <version> --apply
//! This regenerates `item_registry.in.rs` from either:
//!   - Local client JAR (most accurate, requires installed Minecraft)
//!   - PrismineJS/minecraft-data npm package (fallback, no client needed)

use crate::block::BlockState;
use std::collections::HashMap;
use std::sync::LazyLock;

mod item_registry_data {
    include!("item_registry.in.rs");
}
static ITEM_REGISTRY: LazyLock<HashMap<&'static str, u32>> = LazyLock::new(|| {
    item_registry_data::build_registry()
});

/// Resolve item name to BlockState ID.
pub fn resolve_item(name: &str) -> Option<BlockState> {
    let normalized = name.to_lowercase();
    let normalized = normalized.strip_prefix("minecraft:").unwrap_or(&normalized);
    ITEM_REGISTRY.get(normalized).map(|&id| BlockState::new(id))
}

pub fn resolve_item_id(name: &str) -> u32 {
    let normalized = name.to_lowercase();
    let normalized = normalized.strip_prefix("minecraft:").unwrap_or(&normalized);
    ITEM_REGISTRY.get(normalized).copied().unwrap_or(0)
}

pub fn item_count() -> usize { ITEM_REGISTRY.len() }
pub fn item_names() -> Vec<&'static str> {
    let mut names: Vec<_> = ITEM_REGISTRY.keys().copied().collect();
    names.sort(); names
}
pub fn is_known_id(id: u32) -> bool { ITEM_REGISTRY.values().any(|&v| v == id) }
pub fn known_item_ids() -> std::collections::HashSet<u32> {
    ITEM_REGISTRY.values().copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_item_count() { assert!(item_count() >= 1600); }
    #[test] fn test_resolve_stone() { assert_eq!(resolve_item("stone").unwrap().id, 1); }
    #[test] fn test_resolve_sulfur() {
        assert!(resolve_item("sulfur").is_some());
        assert!(resolve_item("sulfur_block").is_some()); // legacy alias
        assert!(resolve_item("sulfur_spike").is_some());
    }
    #[test] fn test_26_2_blocks() {
        assert_eq!(resolve_item("potent_sulfur").unwrap().id, 999);
        assert_eq!(resolve_item("music_disc_bounce").unwrap().id, 1342);
    }
}
