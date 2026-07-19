//! 生物群系类型 — 54 种群系 + 温度/湿度噪声采样

use crate::block::BlockState;

/// 生物群系 ID (54 total in this implementation)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BiomeId {
    // Overworld — temperate (0-19)
    Plains = 0, SunflowerPlains = 1, Forest = 2, FlowerForest = 3,
    BirchForest = 4, OldGrowthBirchForest = 5, DarkForest = 6,
    Swamp = 7, MangroveSwamp = 8, Jungle = 9, SparseJungle = 10, BambooJungle = 11,
    Beach = 12, MushroomFields = 13,
    River = 14, Ocean = 15, DeepOcean = 16, WarmOcean = 17, ColdOcean = 18, FrozenOcean = 19,
    // Overworld — cold (20-29)
    Taiga = 20, OldGrowthPineTaiga = 21, OldGrowthSpruceTaiga = 22,
    SnowyPlains = 23, IceSpikes = 24, SnowyTaiga = 25,
    SnowyBeach = 26, FrozenRiver = 27,
    // Overworld — arid (30-39)
    Desert = 30, Savanna = 31, SavannaPlateau = 32,
    Badlands = 33, ErodedBadlands = 34, WoodedBadlands = 35,
    // Overworld — mountain (40-49)
    WindsweptHills = 40, WindsweptGravellyHills = 41, WindsweptForest = 42,
    Meadow = 43, Grove = 44, SnowySlopes = 45,
    JaggedPeaks = 46, FrozenPeaks = 47, StonyPeaks = 48,
    // Caves
    DripstoneCaves = 49, LushCaves = 50, DeepDark = 51,
    // 26.2 Chaos Cubed
    SulfurCaves = 54,
    // Cherry
    CherryGrove = 52, PaleGarden = 53,
    // Nether (60-64)
    NetherWastes = 60, SoulSandValley = 61, CrimsonForest = 62,
    WarpedForest = 63, BasaltDeltas = 64,
    // End (70-74)
    TheEnd = 70, EndHighlands = 71, EndMidlands = 72,
    SmallEndIslands = 73, EndBarrens = 74,
}

impl BiomeId {
    pub fn from_id(id: u32) -> Option<Self> {
        match id {
            0 => Some(Self::Plains), 1 => Some(Self::SunflowerPlains),
            2 => Some(Self::Forest), 3 => Some(Self::FlowerForest),
            4 => Some(Self::BirchForest), 5 => Some(Self::OldGrowthBirchForest),
            6 => Some(Self::DarkForest), 7 => Some(Self::Swamp),
            8 => Some(Self::MangroveSwamp), 9 => Some(Self::Jungle),
            10 => Some(Self::SparseJungle), 11 => Some(Self::BambooJungle),
            12 => Some(Self::Beach), 13 => Some(Self::MushroomFields),
            14 => Some(Self::River), 15 => Some(Self::Ocean),
            16 => Some(Self::DeepOcean), 17 => Some(Self::WarmOcean),
            18 => Some(Self::ColdOcean), 19 => Some(Self::FrozenOcean),
            20 => Some(Self::Taiga), 21 => Some(Self::OldGrowthPineTaiga),
            22 => Some(Self::OldGrowthSpruceTaiga), 23 => Some(Self::SnowyPlains),
            24 => Some(Self::IceSpikes), 25 => Some(Self::SnowyTaiga),
            26 => Some(Self::SnowyBeach), 27 => Some(Self::FrozenRiver),
            30 => Some(Self::Desert), 31 => Some(Self::Savanna),
            32 => Some(Self::SavannaPlateau), 33 => Some(Self::Badlands),
            34 => Some(Self::ErodedBadlands), 35 => Some(Self::WoodedBadlands),
            40 => Some(Self::WindsweptHills), 41 => Some(Self::WindsweptGravellyHills),
            42 => Some(Self::WindsweptForest), 43 => Some(Self::Meadow),
            44 => Some(Self::Grove), 45 => Some(Self::SnowySlopes),
            46 => Some(Self::JaggedPeaks), 47 => Some(Self::FrozenPeaks),
            48 => Some(Self::StonyPeaks), 49 => Some(Self::DripstoneCaves),
            50 => Some(Self::LushCaves), 51 => Some(Self::DeepDark),
            52 => Some(Self::CherryGrove), 53 => Some(Self::PaleGarden),
            54 => Some(Self::SulfurCaves),
            60 => Some(Self::NetherWastes), 61 => Some(Self::SoulSandValley),
            62 => Some(Self::CrimsonForest), 63 => Some(Self::WarpedForest),
            64 => Some(Self::BasaltDeltas),
            70 => Some(Self::TheEnd), 71 => Some(Self::EndHighlands),
            72 => Some(Self::EndMidlands), 73 => Some(Self::SmallEndIslands),
            74 => Some(Self::EndBarrens),
            _ => None,
        }
    }

    pub fn id(self) -> u32 { self as u32 }

    /// 返回 (表土, 中层, 深层) 方块
    pub fn surface_blocks(self) -> (BlockState, BlockState, BlockState) {
        match self {
            // Temperate
            Self::Plains | Self::SunflowerPlains | Self::Forest | Self::FlowerForest |
            Self::BirchForest | Self::OldGrowthBirchForest | Self::DarkForest |
            Self::Swamp | Self::MangroveSwamp | Self::Jungle | Self::SparseJungle |
            Self::BambooJungle | Self::CherryGrove | Self::PaleGarden |
            Self::Meadow | Self::Grove =>
                (BlockState::new(8), BlockState::new(9), BlockState::new(1)),
            // Arid
            Self::Desert | Self::Savanna | Self::SavannaPlateau |
            Self::Badlands | Self::ErodedBadlands | Self::WoodedBadlands =>
                (BlockState::new(24), BlockState::new(24), BlockState::new(1)),
            // Cold
            Self::Taiga | Self::OldGrowthPineTaiga | Self::OldGrowthSpruceTaiga |
            Self::SnowyTaiga => (BlockState::new(8), BlockState::new(9), BlockState::new(1)),
            Self::SnowyPlains => (BlockState::new(78), BlockState::new(9), BlockState::new(1)),
            Self::IceSpikes => (BlockState::new(78), BlockState::new(78), BlockState::new(1)),
            // Mountain
            Self::WindsweptHills | Self::WindsweptGravellyHills |
            Self::WindsweptForest | Self::JaggedPeaks | Self::StonyPeaks =>
                (BlockState::new(1), BlockState::new(1), BlockState::new(1)),
            Self::SnowySlopes | Self::FrozenPeaks =>
                (BlockState::new(78), BlockState::new(1), BlockState::new(1)),
            // Beach/Water
            Self::Beach | Self::SnowyBeach =>
                (BlockState::new(24), BlockState::new(24), BlockState::new(1)),
            Self::River | Self::FrozenRiver =>
                (BlockState::new(26), BlockState::new(9), BlockState::new(1)),
            Self::Ocean | Self::DeepOcean | Self::WarmOcean | Self::ColdOcean |
            Self::FrozenOcean =>
                (BlockState::new(26), BlockState::new(1), BlockState::new(1)),
            Self::MushroomFields =>
                (BlockState::new(110), BlockState::new(9), BlockState::new(1)),
            // Caves
            Self::DripstoneCaves => (BlockState::new(1), BlockState::new(1), BlockState::new(1)),
            Self::LushCaves => (BlockState::new(9), BlockState::new(1), BlockState::new(1)),
            Self::DeepDark => (BlockState::new(1), BlockState::new(269), BlockState::new(1)),
            // 26.2 Sulfur Caves — sulfur surface, stone mid, deepslate deep with cinnabar veins
            Self::SulfurCaves => (BlockState::new(1240), BlockState::new(1), BlockState::new(269)),
            // Nether
            Self::NetherWastes | Self::CrimsonForest | Self::WarpedForest =>
                (BlockState::new(87), BlockState::new(87), BlockState::new(87)),
            Self::SoulSandValley => (BlockState::new(88), BlockState::new(88), BlockState::new(88)),
            Self::BasaltDeltas => (BlockState::new(87), BlockState::new(87), BlockState::new(268)),
            // End
            _ => (BlockState::new(121), BlockState::new(121), BlockState::new(121)),
        }
    }

    pub fn is_nether(self) -> bool { matches!(self, Self::NetherWastes | Self::SoulSandValley | Self::CrimsonForest | Self::WarpedForest | Self::BasaltDeltas) }
    pub fn is_end(self) -> bool { matches!(self, Self::TheEnd | Self::EndHighlands | Self::EndMidlands | Self::SmallEndIslands | Self::EndBarrens) }
    pub fn is_ocean(self) -> bool { matches!(self, Self::Ocean | Self::DeepOcean | Self::WarmOcean | Self::ColdOcean | Self::FrozenOcean) }
}
