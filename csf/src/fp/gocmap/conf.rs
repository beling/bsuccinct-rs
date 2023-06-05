use crate::fp::OptimalLevelSize;
use ph::fmph::GOConf;
use ph::{BuildDefaultSeededHasher};
use ph::fmph::{GroupSize, SeedSize, TwoToPowerBitsStatic};
use crate::coding::BuildMinimumRedundancy;

#[derive(Clone)]
pub struct GOCMapConf<
    BC = BuildMinimumRedundancy,
    LSC = OptimalLevelSize,
    GS: GroupSize = TwoToPowerBitsStatic::<4>, SS: SeedSize = TwoToPowerBitsStatic<2>, S = BuildDefaultSeededHasher
> {
    pub coding: BC,
    /// Configuration of family of (group-optimized) hash functions (default: [`GOConf::default`]).
    pub goconf: GOConf<GS, SS, S>,
    /// Chooses the size of level for the given level input.
    pub level_size_chooser: LSC,
}

impl Default for GOCMapConf {
    fn default() -> Self { Self {
        coding: Default::default(),
        goconf: Default::default(),
        level_size_chooser: Default::default(),
    } }
}

impl GOCMapConf {
    pub fn bpf(bits_per_fragment: u8) -> Self {
        Self::coding(BuildMinimumRedundancy { bits_per_fragment })
    }
}

impl<BC> GOCMapConf<BC, OptimalLevelSize, TwoToPowerBitsStatic::<4>, TwoToPowerBitsStatic<2>, BuildDefaultSeededHasher> {
    pub fn coding(coding: BC) -> Self {
        Self {
            coding,
            goconf: Default::default(),
            level_size_chooser: Default::default(),
        }
    }
}

impl<GS: GroupSize, SS: SeedSize, S> GOCMapConf<BuildMinimumRedundancy, OptimalLevelSize, GS, SS, S> {
    pub fn groups(goconf: GOConf<GS, SS, S>) -> Self {
        Self {
            coding: Default::default(),
            goconf,
            level_size_chooser: Default::default(),
        }
    }
}

impl<BC, GS: GroupSize, SS: SeedSize, S> GOCMapConf<BC, OptimalLevelSize, GS, SS, S> {
    pub fn groups_coding(goconf: GOConf<GS, SS, S>, coding: BC) -> Self {
        Self {
            coding,
            goconf,
            level_size_chooser: Default::default(),
        }
    }
}

impl<BC, LSC> GOCMapConf<BC, LSC, TwoToPowerBitsStatic::<4>, TwoToPowerBitsStatic<2>, BuildDefaultSeededHasher> {
    pub fn lsize_coding(level_size_chooser: LSC, coding: BC) -> Self {
        Self {
            coding,
            goconf: Default::default(),
            level_size_chooser,
        }
    }
}

impl<LSC> GOCMapConf<BuildMinimumRedundancy, LSC, TwoToPowerBitsStatic::<4>, TwoToPowerBitsStatic<2>, BuildDefaultSeededHasher> {
    pub fn lsize(level_size_chooser: LSC) -> Self {
        Self::lsize_coding(level_size_chooser, BuildMinimumRedundancy::default())
    }
}

impl<BC, LSC, GS: GroupSize, SS: SeedSize, S> GOCMapConf<BC, LSC, GS, SS, S> {
    pub fn groups_lsize_coding(goconf: GOConf<GS, SS, S>, level_size_chooser: LSC, coding: BC) -> Self {
        Self { coding, goconf, level_size_chooser }
    }
}
