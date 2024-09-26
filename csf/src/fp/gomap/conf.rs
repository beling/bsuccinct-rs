use crate::fp::collision_solver::{CollisionSolverBuilder, LoMemAcceptEquals};
use crate::fp::OptimalLevelSize;
use ph::BuildDefaultSeededHasher;
use ph::fmph::{GOConf, GroupSize, SeedSize, TwoToPowerBitsStatic};

/// Configuration accepted by [`fp::GOMap`](crate::fp::GOMap) constructors.
#[derive(Clone)]
pub struct GOMapConf<
    LSC = OptimalLevelSize,
    CSB: CollisionSolverBuilder = LoMemAcceptEquals,
    GS: GroupSize = TwoToPowerBitsStatic::<4>,
    SS: SeedSize = TwoToPowerBitsStatic<2>,
    S = BuildDefaultSeededHasher
> {
    /// Bits per each value, 0 for autodetect.
    pub bits_per_value: u8,
    /// Configuration of family of (group-optimized) hash functions (default: [`GOConf::default`]).
    pub goconf: GOConf<GS, SS, S>,
    /// Choose the size of each level.
    pub level_size_chooser: LSC,
    /// Constructs collision solver that decides which collisions are positive, and which are negative.
    pub collision_solver: CSB,
}

impl Default for GOMapConf {
    fn default() -> Self { Self {
        bits_per_value: Default::default(),
        goconf: Default::default(),
        level_size_chooser: Default::default(),
        collision_solver: Default::default(),
    } }
}

impl GOMapConf {
    pub fn bpv(bits_per_value: u8) -> Self {
        Self { bits_per_value, ..Default::default() }
    }
}

impl<CSB: CollisionSolverBuilder> GOMapConf<OptimalLevelSize, CSB, TwoToPowerBitsStatic::<4>, TwoToPowerBitsStatic<2>, BuildDefaultSeededHasher> {
    #[inline] pub fn cs(collision_solver: CSB) -> Self {
        Self::cs_bpv(collision_solver, Default::default())
    }

    pub fn cs_bpv(collision_solver: CSB, bits_per_value: u8) -> Self {
        Self {
            bits_per_value,
            goconf: Default::default(),
            level_size_chooser: Default::default(),
            collision_solver,
        }
    }
}

impl<GS: GroupSize, SS: SeedSize, S> GOMapConf<OptimalLevelSize, LoMemAcceptEquals, GS, SS, S> {
    #[inline] pub fn groups(goconf: GOConf<GS, SS, S>) -> Self {
        Self::groups_bpv(goconf, Default::default())
    }

    pub fn groups_bpv(goconf: GOConf<GS, SS, S>, bits_per_value: u8) -> Self {
        Self {
            bits_per_value,
            goconf,
            level_size_chooser: Default::default(),
            collision_solver: Default::default(),
        }
    }
}

impl<GS: GroupSize, SS: SeedSize, S> From<GOConf<GS, SS, S>> for GOMapConf<OptimalLevelSize, LoMemAcceptEquals, GS, SS, S> {
    #[inline] fn from(value: GOConf<GS, SS, S>) -> Self {
        Self::groups(value)
    }
}

impl<LSC> GOMapConf<LSC, LoMemAcceptEquals, TwoToPowerBitsStatic::<4>, TwoToPowerBitsStatic<2>, BuildDefaultSeededHasher> {
    pub fn lsize(level_size_chooser: LSC) -> Self {
        Self::lsize_bpv(level_size_chooser, Default::default())
    }

    pub fn lsize_bpv(level_size_chooser: LSC, bits_per_value: u8) -> Self {
        Self {
            bits_per_value,
            goconf: Default::default(),
            level_size_chooser,
            collision_solver: Default::default(),
        }
    }
}

impl<LSC, GS: GroupSize, SS: SeedSize, S> GOMapConf<LSC, LoMemAcceptEquals, GS, SS, S> {
    pub fn groups_lsize_bpv(goconf: GOConf<GS, SS, S>, level_size_chooser: LSC, bits_per_value: u8) -> Self {
        Self { bits_per_value, goconf, level_size_chooser, collision_solver: Default::default() }
    }
}

impl<LSC, CSB: CollisionSolverBuilder, GS: GroupSize, SS: SeedSize, S> GOMapConf<LSC, CSB, GS, SS, S> {
    pub fn groups_lsize_cs_bpv(goconf: GOConf<GS, SS, S>, level_size_chooser: LSC, collision_solver: CSB, bits_per_value: u8) -> Self {
        Self { bits_per_value, goconf, level_size_chooser, collision_solver }
    }
}
