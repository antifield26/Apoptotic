//! 状态效果类型 — Minecraft status effects (protocol 776)
//!
//! 来源: PrismarineJS minecraft-data effects.json (1.21.5)

use std::fmt;

/// Minecraft 状态效果 (33 total in vanilla 1.21.5)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectType {
    Speed,            // 1
    Slowness,         // 2
    Haste,            // 3
    MiningFatigue,    // 4
    Strength,         // 5
    InstantHealth,    // 6
    InstantDamage,    // 7
    JumpBoost,        // 8
    Nausea,           // 9
    Regeneration,     // 10
    Resistance,       // 11
    FireResistance,   // 12
    WaterBreathing,   // 13
    Invisibility,     // 14
    Blindness,        // 15
    NightVision,      // 16
    Hunger,           // 17
    Weakness,         // 18
    Poison,           // 19
    Wither,           // 20
    HealthBoost,      // 21
    Absorption,       // 22
    Saturation,       // 23
    Glowing,          // 24
    Levitation,       // 25
    Luck,             // 26
    Unluck,           // 27
    SlowFalling,      // 28
    ConduitPower,     // 29
    DolphinGrace,     // 30
    BadOmen,          // 31
    HeroOfTheVillage, // 32
    Darkness,         // 33
}

impl EffectType {
    /// 从协议 ID 解析效果类型
    pub fn from_id(id: u8) -> Option<Self> {
        match id {
            1 => Some(Self::Speed),
            2 => Some(Self::Slowness),
            3 => Some(Self::Haste),
            4 => Some(Self::MiningFatigue),
            5 => Some(Self::Strength),
            6 => Some(Self::InstantHealth),
            7 => Some(Self::InstantDamage),
            8 => Some(Self::JumpBoost),
            9 => Some(Self::Nausea),
            10 => Some(Self::Regeneration),
            11 => Some(Self::Resistance),
            12 => Some(Self::FireResistance),
            13 => Some(Self::WaterBreathing),
            14 => Some(Self::Invisibility),
            15 => Some(Self::Blindness),
            16 => Some(Self::NightVision),
            17 => Some(Self::Hunger),
            18 => Some(Self::Weakness),
            19 => Some(Self::Poison),
            20 => Some(Self::Wither),
            21 => Some(Self::HealthBoost),
            22 => Some(Self::Absorption),
            23 => Some(Self::Saturation),
            24 => Some(Self::Glowing),
            25 => Some(Self::Levitation),
            26 => Some(Self::Luck),
            27 => Some(Self::Unluck),
            28 => Some(Self::SlowFalling),
            29 => Some(Self::ConduitPower),
            30 => Some(Self::DolphinGrace),
            31 => Some(Self::BadOmen),
            32 => Some(Self::HeroOfTheVillage),
            33 => Some(Self::Darkness),
            _ => None,
        }
    }

    /// 协议 ID
    pub fn id(&self) -> u8 {
        match self {
            Self::Speed => 1,
            Self::Slowness => 2,
            Self::Haste => 3,
            Self::MiningFatigue => 4,
            Self::Strength => 5,
            Self::InstantHealth => 6,
            Self::InstantDamage => 7,
            Self::JumpBoost => 8,
            Self::Nausea => 9,
            Self::Regeneration => 10,
            Self::Resistance => 11,
            Self::FireResistance => 12,
            Self::WaterBreathing => 13,
            Self::Invisibility => 14,
            Self::Blindness => 15,
            Self::NightVision => 16,
            Self::Hunger => 17,
            Self::Weakness => 18,
            Self::Poison => 19,
            Self::Wither => 20,
            Self::HealthBoost => 21,
            Self::Absorption => 22,
            Self::Saturation => 23,
            Self::Glowing => 24,
            Self::Levitation => 25,
            Self::Luck => 26,
            Self::Unluck => 27,
            Self::SlowFalling => 28,
            Self::ConduitPower => 29,
            Self::DolphinGrace => 30,
            Self::BadOmen => 31,
            Self::HeroOfTheVillage => 32,
            Self::Darkness => 33,
        }
    }

    /// 是否是即时效果 (无持续时间)
    pub fn is_instant(&self) -> bool {
        matches!(self, Self::InstantHealth | Self::InstantDamage)
    }

    /// 是否是正面效果
    pub fn is_beneficial(&self) -> bool {
        !matches!(self,
            Self::Slowness | Self::MiningFatigue | Self::InstantDamage |
            Self::Nausea | Self::Blindness | Self::Hunger |
            Self::Weakness | Self::Poison | Self::Wither |
            Self::Unluck | Self::Darkness | Self::BadOmen
        )
    }
}

impl fmt::Display for EffectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// 按名称查找效果 (不区分大小写)
pub fn resolve_effect(name: &str) -> Option<EffectType> {
    match name.to_lowercase().as_str() {
        "speed" => Some(EffectType::Speed),
        "slowness" => Some(EffectType::Slowness),
        "haste" => Some(EffectType::Haste),
        "mining_fatigue" | "miningfatigue" => Some(EffectType::MiningFatigue),
        "strength" => Some(EffectType::Strength),
        "instant_health" | "instanthealth" | "heal" => Some(EffectType::InstantHealth),
        "instant_damage" | "instantdamage" | "harm" => Some(EffectType::InstantDamage),
        "jump_boost" | "jumpboost" => Some(EffectType::JumpBoost),
        "nausea" => Some(EffectType::Nausea),
        "regeneration" | "regen" => Some(EffectType::Regeneration),
        "resistance" => Some(EffectType::Resistance),
        "fire_resistance" | "fireresistance" => Some(EffectType::FireResistance),
        "water_breathing" | "waterbreathing" => Some(EffectType::WaterBreathing),
        "invisibility" | "invis" => Some(EffectType::Invisibility),
        "blindness" => Some(EffectType::Blindness),
        "night_vision" | "nightvision" => Some(EffectType::NightVision),
        "hunger" => Some(EffectType::Hunger),
        "weakness" => Some(EffectType::Weakness),
        "poison" => Some(EffectType::Poison),
        "wither" => Some(EffectType::Wither),
        "health_boost" | "healthboost" => Some(EffectType::HealthBoost),
        "absorption" => Some(EffectType::Absorption),
        "saturation" => Some(EffectType::Saturation),
        "glowing" => Some(EffectType::Glowing),
        "levitation" => Some(EffectType::Levitation),
        "luck" => Some(EffectType::Luck),
        "unluck" => Some(EffectType::Unluck),
        "slow_falling" | "slowfalling" => Some(EffectType::SlowFalling),
        "conduit_power" | "conduitpower" => Some(EffectType::ConduitPower),
        "dolphin_grace" | "dolphinsgrace" => Some(EffectType::DolphinGrace),
        "bad_omen" | "badomen" => Some(EffectType::BadOmen),
        "hero_of_the_village" | "heroofthevillage" => Some(EffectType::HeroOfTheVillage),
        "darkness" => Some(EffectType::Darkness),
        _ => None,
    }
}

/// 玩家身上的活跃效果
#[derive(Debug, Clone)]
pub struct ActiveEffect {
    pub effect: EffectType,
    pub amplifier: u8,  // 0 = level I, 1 = level II, ...
    pub duration_ticks: u32, // remaining duration in ticks (0 = permanent/instant)
}

impl ActiveEffect {
    pub fn new(effect: EffectType, amplifier: u8, duration_ticks: u32) -> Self {
        Self { effect, amplifier, duration_ticks }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_ids() {
        assert_eq!(EffectType::Speed.id(), 1);
        assert_eq!(EffectType::InstantHealth.id(), 6);
        assert_eq!(EffectType::Regeneration.id(), 10);
        assert_eq!(EffectType::Darkness.id(), 33);
    }

    #[test]
    fn test_from_id_roundtrip() {
        for id in 1..=33u8 {
            let effect = EffectType::from_id(id);
            assert!(effect.is_some(), "Effect ID {} should resolve", id);
            assert_eq!(effect.unwrap().id(), id);
        }
    }

    #[test]
    fn test_resolve_effect_names() {
        assert_eq!(resolve_effect("speed"), Some(EffectType::Speed));
        assert_eq!(resolve_effect("regen"), Some(EffectType::Regeneration));
        assert_eq!(resolve_effect("invis"), Some(EffectType::Invisibility));
        assert_eq!(resolve_effect("SLOWNESS"), Some(EffectType::Slowness));
        assert!(resolve_effect("nonexistent").is_none());
    }

    #[test]
    fn test_instant_effects() {
        assert!(EffectType::InstantHealth.is_instant());
        assert!(EffectType::InstantDamage.is_instant());
        assert!(!EffectType::Speed.is_instant());
        assert!(!EffectType::Regeneration.is_instant());
    }

    #[test]
    fn test_beneficial_effects() {
        assert!(EffectType::Speed.is_beneficial());
        assert!(EffectType::Regeneration.is_beneficial());
        assert!(!EffectType::Poison.is_beneficial());
        assert!(!EffectType::Wither.is_beneficial());
    }
}
