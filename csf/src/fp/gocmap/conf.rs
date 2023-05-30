use crate::fp::OptimalLevelSize;
use ph::{BuildDefaultSeededHasher, BuildSeededHasher};
use ph::fmph::{GroupSize, SeedSize, TwoToPowerBits, TwoToPowerBitsStatic};
use crate::coding::BuildMinimumRedundancy;

#[derive(Copy, Clone)]
pub struct GOCMapConf<
    GS: GroupSize = TwoToPowerBits,
    SS: SeedSize = TwoToPowerBitsStatic<2>,
    BC = BuildMinimumRedundancy,
    LSC = OptimalLevelSize,
    S = BuildDefaultSeededHasher
> {
    pub coding: BC,
    pub bits_per_seed: SS,
    pub bits_per_group: GS,
    pub level_size_chooser: LSC,
    pub hash_builder: S,
}

impl Default for GOCMapConf {
    fn default() -> Self { Self {
        coding: Default::default(),
        bits_per_seed: Default::default(), bits_per_group: TwoToPowerBits::new(4),
        level_size_chooser: Default::default(),
        hash_builder: Default::default(),
    } }
}

impl GOCMapConf {
    pub fn bpf(bits_per_fragment: u8) -> Self {
        Self::coding(BuildMinimumRedundancy { bits_per_fragment })
    }
}

impl<BC> GOCMapConf<TwoToPowerBits, TwoToPowerBitsStatic<2>, BC, OptimalLevelSize, BuildDefaultSeededHasher> {
    pub fn coding(coding: BC) -> Self {
        Self {
            coding,
            bits_per_seed: Default::default(),
            bits_per_group: TwoToPowerBits::new(4),
            level_size_chooser: Default::default(),
            hash_builder: Default::default(),
        }
    }
}

impl<SS: SeedSize> GOCMapConf<TwoToPowerBits, SS, BuildMinimumRedundancy, OptimalLevelSize, BuildDefaultSeededHasher> {
    pub fn bps(bits_per_seed: SS) -> Self {
        bits_per_seed.validate().unwrap();
        Self {
            coding: Default::default(),
            bits_per_seed,
            bits_per_group: TwoToPowerBits::new(4),
            level_size_chooser: Default::default(),
            hash_builder: Default::default(),
        }
    }
}

impl<SS: SeedSize, BC> GOCMapConf<TwoToPowerBits, SS, BC, OptimalLevelSize, BuildDefaultSeededHasher> {
    pub fn coding_bps(coding: BC, bits_per_seed: SS) -> Self {
        bits_per_seed.validate().unwrap();
        Self {
            coding,
            bits_per_seed,
            bits_per_group: TwoToPowerBits::new(4),
            level_size_chooser: Default::default(),
            hash_builder: Default::default(),
        }
    }
}

impl<GS: GroupSize> GOCMapConf<GS, TwoToPowerBitsStatic<2>, BuildMinimumRedundancy, OptimalLevelSize, BuildDefaultSeededHasher> {
    pub fn bpg(bits_per_group: GS) -> Self {
        bits_per_group.validate().unwrap();
        Self {
            coding: Default::default(),
            bits_per_seed: Default::default(),
            bits_per_group,
            level_size_chooser: Default::default(),
            hash_builder: Default::default(),
        }
    }
}

impl<GS: GroupSize, BC> GOCMapConf<GS, TwoToPowerBitsStatic<2>, BC, OptimalLevelSize, BuildDefaultSeededHasher> {
    pub fn coding_bpg(coding: BC, bits_per_group: GS) -> Self {
        bits_per_group.validate().unwrap();
        Self {
            coding,
            bits_per_seed: Default::default(),
            bits_per_group,
            level_size_chooser: Default::default(),
            hash_builder: Default::default(),
        }
    }
}

impl<GS: GroupSize, SS: SeedSize> GOCMapConf<GS, SS, BuildMinimumRedundancy, OptimalLevelSize, BuildDefaultSeededHasher> {
    pub fn bps_bpg(bits_per_seed: SS, bits_per_group: GS) -> Self {
        bits_per_seed.validate().unwrap();
        bits_per_group.validate().unwrap();
        Self {
            coding: Default::default(),
            bits_per_seed,
            bits_per_group,
            level_size_chooser: Default::default(),
            hash_builder: Default::default(),
        }
    }
}

impl<GS: GroupSize, SS: SeedSize, BC> GOCMapConf<GS, SS, BC, OptimalLevelSize, BuildDefaultSeededHasher> {
    pub fn coding_bps_bpg(coding: BC, bits_per_seed: SS, bits_per_group: GS) -> Self {
        bits_per_seed.validate().unwrap();
        bits_per_group.validate().unwrap();
        Self {
            coding,
            bits_per_seed,
            bits_per_group,
            level_size_chooser: Default::default(),
            hash_builder: Default::default(),
        }
    }
}

impl<BC, LSC> GOCMapConf<TwoToPowerBits, TwoToPowerBitsStatic<2>, BC, LSC, BuildDefaultSeededHasher> {
    pub fn lsize_coding(level_size_chooser: LSC, coding: BC) -> Self {
        Self {
            coding,
            bits_per_seed: Default::default(),
            bits_per_group: TwoToPowerBits::new(4),
            level_size_chooser,
            hash_builder: Default::default(),
        }
    }
}

impl<LSC> GOCMapConf<TwoToPowerBits, TwoToPowerBitsStatic<2>, BuildMinimumRedundancy, LSC> {
    pub fn lsize(level_size_chooser: LSC) -> Self {
        Self::lsize_coding(level_size_chooser, BuildMinimumRedundancy::default())
    }
}

impl<BC, S: BuildSeededHasher> GOCMapConf<TwoToPowerBits, TwoToPowerBitsStatic<2>, BC, OptimalLevelSize, S> {
    pub fn hash_coding(hash_builder: S, coding: BC) -> Self {
        Self {
            coding,
            bits_per_seed: Default::default(),
            bits_per_group: TwoToPowerBits::new(4),
            level_size_chooser: Default::default(),
            hash_builder
        }
    }
}

impl<S: BuildSeededHasher> GOCMapConf<TwoToPowerBits, TwoToPowerBitsStatic<2>, BuildMinimumRedundancy, OptimalLevelSize, S> {
    pub fn hash(hash_builder: S) -> Self {
        Self::hash_coding(hash_builder, BuildMinimumRedundancy::default())
    }
}

impl<BC, LSC, S: BuildSeededHasher> GOCMapConf<TwoToPowerBits, TwoToPowerBitsStatic<2>, BC, LSC, S> {
    pub fn lsize_hash_coding(level_size_chooser: LSC, hash_builder: S, coding: BC) -> Self {
        Self {
            coding,
            bits_per_seed: Default::default(),
            bits_per_group: TwoToPowerBits::new(4),
            level_size_chooser,
            hash_builder
        }
    }
}

impl<LSC, S: BuildSeededHasher> GOCMapConf<TwoToPowerBits, TwoToPowerBitsStatic<2>, BuildMinimumRedundancy, LSC, S> {
    pub fn lsize_hash(level_size_chooser: LSC, hash_builder: S) -> Self {
        Self::lsize_hash_coding(level_size_chooser, hash_builder, BuildMinimumRedundancy::default())
    }
}