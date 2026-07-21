//! 信标系统 — 金字塔检测、效果选择与范围应用
//!
//! 支持 1-4 层金字塔, 5 种主效果 + 副效果

use std::collections::HashMap;

/// 信标数据
#[derive(Debug, Clone)]
pub struct BeaconData {
    pub pos: (i32, i32, i32),
    pub pyramid_level: u8,         // 0-4
    pub primary_effect: Option<u32>,   // 效果 ID
    pub secondary_effect: Option<u32>, // 副效果 (仅 4 层)
    pub payment_item: Option<u32>, // 玩家放入的支付物品
}

impl BeaconData {
    pub fn new(pos: (i32, i32, i32)) -> Self {
        Self {
            pos,
            pyramid_level: 0,
            primary_effect: None,
            secondary_effect: None,
            payment_item: None,
        }
    }

    /// 可用的主效果 (按金字塔等级)
    pub fn available_effects(level: u8) -> &'static [(u32, &'static str)] {
        match level {
            1 => &[(1, "Speed"), (3, "Haste")],
            2 => &[(1, "Speed"), (3, "Haste"), (11, "Resistance"), (8, "Jump Boost")],
            3 => &[(1, "Speed"), (3, "Haste"), (11, "Resistance"), (8, "Jump Boost"), (5, "Strength")],
            _ => &[(1, "Speed"), (3, "Haste"), (11, "Resistance"), (8, "Jump Boost"), (5, "Strength")],
        }
    }

    /// 副效果 (仅 4 层可选, 与主效果不同)
    pub fn available_secondary(level: u8) -> &'static [(u32, &'static str)] {
        if level >= 4 {
            &[(1, "Speed"), (3, "Haste"), (11, "Resistance"), (8, "Jump Boost"), (5, "Strength"), (10, "Regeneration")]
        } else {
            &[]
        }
    }
}

/// 信标管理器
pub struct BeaconManager {
    pub beacons: HashMap<(i32, i32, i32), BeaconData>,
    pub newly_activated: bool,
}

impl BeaconManager {
    pub fn has_newly_activated(&mut self) -> bool {
        let v = self.newly_activated;
        self.newly_activated = false;
        v
    }
}

impl Default for BeaconManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BeaconManager {
    pub fn new() -> Self {
        Self { beacons: HashMap::new(), newly_activated: false }
    }

    /// 获取或创建信标
    pub fn get_or_create(&mut self, pos: (i32, i32, i32)) -> &mut BeaconData {
        self.beacons.entry(pos).or_insert_with(|| BeaconData::new(pos))
    }

    /// 移除信标
    pub fn remove(&mut self, pos: (i32, i32, i32)) {
        self.beacons.remove(&pos);
    }

    /// 设置支付物品
    pub fn set_payment(&mut self, pos: (i32, i32, i32), item: Option<u32>) {
        if let Some(beacon) = self.beacons.get_mut(&pos) {
            beacon.payment_item = item;
        }
    }

    /// 选择效果
    pub fn select_effect(&mut self, pos: (i32, i32, i32), primary: Option<u32>, secondary: Option<u32>) -> bool {
        if let Some(beacon) = self.beacons.get_mut(&pos) {
            if beacon.pyramid_level == 0 { return false; }
            let avail = Self::available_effects_for_level(beacon.pyramid_level, primary, secondary);
            if avail {
                beacon.primary_effect = primary;
                beacon.secondary_effect = secondary;
                return true;
            }
        }
        false
    }

    fn available_effects_for_level(level: u8, primary: Option<u32>, secondary: Option<u32>) -> bool {
        let p_ok = if let Some(p) = primary {
            BeaconData::available_effects(level).iter().any(|(id, _)| *id == p)
        } else { true };
        let s_ok = if let Some(s) = secondary {
            BeaconData::available_secondary(level).iter().any(|(id, _)| *id == s)
        } else { true };
        p_ok && s_ok
    }

    /// 检测金字塔等级 (从信标位置向下检测)
    pub fn detect_pyramid(
        chunk_store: &mc_world::chunk_store::ChunkStore,
        x: i32, y: i32, z: i32,
    ) -> u8 {
        // 信标在金字塔顶部, y 已在地上
        // 金字塔层从信标下方开始: layer 1 at y-1, layer 2 at y-2, etc.
        let beacon_blocks: &[u32] = &[42, 41, 57, 133, 996]; // iron, gold, diamond, emerald, netherite

        for level in 1u8..=4u8 {
            let size = level as i32;
            let base_y = y - level as i32;

            for dx in -size..=size {
                for dz in -size..=size {
                    let bx = x + dx;
                    let bz = z + dz;
                    // 跳过信标柱正上方 (如果是 level 1, size=1, 只有边框)
                    // 简化: 检查所有 9x9, 7x7, 5x5, 3x3 的顶部层
                    if level == 1 {
                        // 3x3 完整检查
                        let cp = mc_core::position::ChunkPos::new(bx >> 4, bz >> 4);
                        if let Some(chunk) = chunk_store.get(&cp) {
                            let block = chunk.get_block((bx & 0xF) as usize, base_y, (bz & 0xF) as usize);
                            if !block.is_air() && !beacon_blocks.contains(&block.id) {
                                return level - 1;
                            }
                            if block.is_air() {
                                return level - 1;
                            }
                        } else {
                            return level - 1;
                        }
                    } else {
                        // 2+ layers: only check the outer ring (the visible part)
                        if dx.abs() == size || dz.abs() == size {
                            let cp = mc_core::position::ChunkPos::new(bx >> 4, bz >> 4);
                            if let Some(chunk) = chunk_store.get(&cp) {
                                let block = chunk.get_block((bx & 0xF) as usize, base_y, (bz & 0xF) as usize);
                                if block.is_air() || !beacon_blocks.contains(&block.id) {
                                    return level - 1;
                                }
                            } else {
                                return level - 1;
                            }
                        }
                    }
                }
            }
        }

        4 // 满级金字塔
    }

    /// 每 80 tick 重新应用信标效果
    pub fn tick(
        &mut self,
        chunk_store: &mc_world::chunk_store::ChunkStore,
        player_manager: &crate::player::PlayerManager,
    ) {
        for (pos, beacon) in self.beacons.iter_mut() {
            let old_level = beacon.pyramid_level;
            beacon.pyramid_level = Self::detect_pyramid(chunk_store, pos.0, pos.1, pos.2);
            // Track newly activated beacons
            if old_level == 0 && beacon.pyramid_level > 0 && beacon.payment_item.is_some() {
                self.newly_activated = true;
            }

            if beacon.pyramid_level == 0 { continue; }
            if beacon.payment_item.is_none() { continue; }
            if beacon.primary_effect.is_none() { continue; }

            let range = 10.0 + beacon.pyramid_level as f64 * 10.0;

            // 向范围内玩家应用效果
            let players = player_manager.all_players();
            for player in &players {
                let dx = player.position.x - pos.0 as f64;
                let dy = player.position.y - pos.1 as f64;
                let dz = player.position.z - pos.2 as f64;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                if dist <= range {
                    // 应用主效果
                    if let Some(effect_id) = beacon.primary_effect {
                        let amplifier = 0u8;
                        if let Some(effect) = mc_core::effect::EffectType::from_id(effect_id as u8) {
                            let active = mc_core::effect::ActiveEffect::new(effect, amplifier, 240);
                            let _ = player_manager.add_effect(&player.uuid, active);
                        }
                    }
                    // 应用副效果 (仅 4 层)
                    if beacon.pyramid_level >= 4
                        && let Some(effect_id) = beacon.secondary_effect {
                            let amplifier = if effect_id == 10 { 1u8 } else { 0u8 };
                            if let Some(effect) = mc_core::effect::EffectType::from_id(effect_id as u8) {
                                let active = mc_core::effect::ActiveEffect::new(effect, amplifier, 240);
                                let _ = player_manager.add_effect(&player.uuid, active);
                            }
                        }
                }
            }
        }
    }
}
