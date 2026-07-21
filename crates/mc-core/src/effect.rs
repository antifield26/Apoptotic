//! 状态效果类型 — Minecraft 26.2 status effects (protocol 776)
//!
//! Source: Official Minecraft 26.2 registry dump (mob_effect registry, 40 total)

use std::fmt;

/// Minecraft 状态效果 (40 total in vanilla 26.2)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectType {
    Speed,            // 0
    Slowness,         // 1
    Haste,            // 2
    MiningFatigue,    // 3
    Strength,         // 4
    InstantHealth,    // 5
    InstantDamage,    // 6
    JumpBoost,        // 7
    Nausea,           // 8
    Regeneration,     // 9
    Resistance,       // 10
    FireResistance,   // 11
    WaterBreathing,   // 12
    Invisibility,     // 13
    Blindness,        // 14
    NightVision,      // 15
    Hunger,           // 16
    Weakness,         // 17
    Poison,           // 18
    Wither,           // 19
    HealthBoost,      // 20
    Absorption,       // 21
    Saturation,       // 22
    Glowing,          // 23
    Levitation,       // 24
    Luck,             // 25
    Unluck,           // 26
    SlowFalling,      // 27
    ConduitPower,     // 28
    DolphinGrace,     // 29
    BadOmen,          // 30
    HeroOfTheVillage, // 31
    Darkness,         // 32
    TrialOmen,        // 33 (1.21.5+)
    RaidOmen,         // 34 (1.21.5+)
    WindCharged,      // 35 (1.21.5+)
    Weaving,          // 36 (1.21.5+)
    Oozing,           // 37 (1.21.5+)
    Infested,         // 38 (1.21.5+)
    BreathOfTheNautilus, // 39 (26.2)
}

impl EffectType {
    pub fn from_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::Speed), 1 => Some(Self::Slowness), 2 => Some(Self::Haste),
            3 => Some(Self::MiningFatigue), 4 => Some(Self::Strength),
            5 => Some(Self::InstantHealth), 6 => Some(Self::InstantDamage),
            7 => Some(Self::JumpBoost), 8 => Some(Self::Nausea),
            9 => Some(Self::Regeneration), 10 => Some(Self::Resistance),
            11 => Some(Self::FireResistance), 12 => Some(Self::WaterBreathing),
            13 => Some(Self::Invisibility), 14 => Some(Self::Blindness),
            15 => Some(Self::NightVision), 16 => Some(Self::Hunger),
            17 => Some(Self::Weakness), 18 => Some(Self::Poison),
            19 => Some(Self::Wither), 20 => Some(Self::HealthBoost),
            21 => Some(Self::Absorption), 22 => Some(Self::Saturation),
            23 => Some(Self::Glowing), 24 => Some(Self::Levitation),
            25 => Some(Self::Luck), 26 => Some(Self::Unluck),
            27 => Some(Self::SlowFalling), 28 => Some(Self::ConduitPower),
            29 => Some(Self::DolphinGrace), 30 => Some(Self::BadOmen),
            31 => Some(Self::HeroOfTheVillage), 32 => Some(Self::Darkness),
            33 => Some(Self::TrialOmen), 34 => Some(Self::RaidOmen),
            35 => Some(Self::WindCharged), 36 => Some(Self::Weaving),
            37 => Some(Self::Oozing), 38 => Some(Self::Infested),
            39 => Some(Self::BreathOfTheNautilus),
            _ => None,
        }
    }

    pub fn id(&self) -> u8 {
        match self {
            Self::Speed => 0, Self::Slowness => 1, Self::Haste => 2,
            Self::MiningFatigue => 3, Self::Strength => 4,
            Self::InstantHealth => 5, Self::InstantDamage => 6,
            Self::JumpBoost => 7, Self::Nausea => 8, Self::Regeneration => 9,
            Self::Resistance => 10, Self::FireResistance => 11,
            Self::WaterBreathing => 12, Self::Invisibility => 13,
            Self::Blindness => 14, Self::NightVision => 15,
            Self::Hunger => 16, Self::Weakness => 17, Self::Poison => 18,
            Self::Wither => 19, Self::HealthBoost => 20,
            Self::Absorption => 21, Self::Saturation => 22,
            Self::Glowing => 23, Self::Levitation => 24,
            Self::Luck => 25, Self::Unluck => 26, Self::SlowFalling => 27,
            Self::ConduitPower => 28, Self::DolphinGrace => 29,
            Self::BadOmen => 30, Self::HeroOfTheVillage => 31,
            Self::Darkness => 32, Self::TrialOmen => 33, Self::RaidOmen => 34,
            Self::WindCharged => 35, Self::Weaving => 36, Self::Oozing => 37,
            Self::Infested => 38, Self::BreathOfTheNautilus => 39,
        }
    }

    pub fn is_instant(&self) -> bool {
        matches!(self, Self::InstantHealth | Self::InstantDamage)
    }

    pub fn is_beneficial(&self) -> bool {
        !matches!(self,
            Self::Slowness | Self::MiningFatigue | Self::InstantDamage |
            Self::Nausea | Self::Blindness | Self::Hunger |
            Self::Weakness | Self::Poison | Self::Wither |
            Self::Unluck | Self::Darkness | Self::BadOmen |
            Self::TrialOmen | Self::RaidOmen | Self::WindCharged |
            Self::Weaving | Self::Oozing | Self::Infested
        )
    }
}

impl fmt::Display for EffectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{:?}", self) }
}

pub fn resolve_effect(name: &str) -> Option<EffectType> {
    match name.to_lowercase().as_str() {
        "speed" => Some(EffectType::Speed), "slowness" => Some(EffectType::Slowness),
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
        "hunger" => Some(EffectType::Hunger), "weakness" => Some(EffectType::Weakness),
        "poison" => Some(EffectType::Poison), "wither" => Some(EffectType::Wither),
        "health_boost" | "healthboost" => Some(EffectType::HealthBoost),
        "absorption" => Some(EffectType::Absorption),
        "saturation" => Some(EffectType::Saturation),
        "glowing" => Some(EffectType::Glowing),
        "levitation" => Some(EffectType::Levitation),
        "luck" => Some(EffectType::Luck), "unluck" => Some(EffectType::Unluck),
        "slow_falling" | "slowfalling" => Some(EffectType::SlowFalling),
        "conduit_power" | "conduitpower" => Some(EffectType::ConduitPower),
        "dolphin_grace" | "dolphinsgrace" => Some(EffectType::DolphinGrace),
        "bad_omen" | "badomen" => Some(EffectType::BadOmen),
        "hero_of_the_village" | "heroofthevillage" => Some(EffectType::HeroOfTheVillage),
        "darkness" => Some(EffectType::Darkness),
        "trial_omen" => Some(EffectType::TrialOmen),
        "raid_omen" => Some(EffectType::RaidOmen),
        "wind_charged" => Some(EffectType::WindCharged),
        "weaving" => Some(EffectType::Weaving), "oozing" => Some(EffectType::Oozing),
        "infested" => Some(EffectType::Infested),
        "breath_of_the_nautilus" => Some(EffectType::BreathOfTheNautilus),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct ActiveEffect {
    pub effect: EffectType,
    pub amplifier: u8,
    pub duration_ticks: u32,
}

impl ActiveEffect {
    pub fn new(effect: EffectType, amplifier: u8, duration_ticks: u32) -> Self {
        Self { effect, amplifier, duration_ticks }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_effect_ids() {
        assert_eq!(EffectType::Speed.id(), 0);
        assert_eq!(EffectType::InstantHealth.id(), 5);
        assert_eq!(EffectType::Darkness.id(), 32);
        assert_eq!(EffectType::BreathOfTheNautilus.id(), 39);
    }
    #[test] fn test_from_id_roundtrip() {
        for id in 0..=39u8 {
            let e = EffectType::from_id(id);
            assert!(e.is_some(), "Effect ID {} should resolve", id);
            assert_eq!(e.unwrap().id(), id);
        }
    }
    #[test] fn test_resolve() {
        assert_eq!(resolve_effect("speed"), Some(EffectType::Speed));
        assert_eq!(resolve_effect("trial_omen"), Some(EffectType::TrialOmen));
        assert!(resolve_effect("nonexistent").is_none());
    }
}
