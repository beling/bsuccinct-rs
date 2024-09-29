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
    /// Configuration of family of (group-optimized) hash functions (default: [`GOConf::default`]).
    pub goconf: GOConf<GS, SS, S>,
    /// Choose the size of each level.
    pub level_size_chooser: LSC,
    /// Constructs collision solver that decides which collisions are positive, and which are negative.
    pub collision_solver: CSB,
}

impl Default for GOMapConf {
    fn default() -> Self { Self {
        goconf: Default::default(),
        level_size_chooser: Default::default(),
        collision_solver: Default::default(),
    } }
}

impl<CSB: CollisionSolverBuilder> GOMapConf<OptimalLevelSize, CSB, TwoToPowerBitsStatic::<4>, TwoToPowerBitsStatic<2>, BuildDefaultSeededHasher> {
    pub fn cs(collision_solver: CSB) -> Self {
        Self {
            goconf: Default::default(),
            level_size_chooser: Default::default(),
            collision_solver,
        }
    }
}

impl<GS: GroupSize, SS: SeedSize, S> GOMapConf<OptimalLevelSize, LoMemAcceptEquals, GS, SS, S> {
    pub fn groups(goconf: GOConf<GS, SS, S>) -> Self {
        Self {
            goconf,
            level_size_chooser: Default::default(),
            collision_solver: Default::default(),
        }
    }
}

impl<CSB: CollisionSolverBuilder, GS: GroupSize, SS: SeedSize, S> GOMapConf<OptimalLevelSize, CSB, GS, SS, S> {
    pub fn groups_cs(goconf: GOConf<GS, SS, S>, collision_solver: CSB) -> Self {
        Self {
            goconf,
            level_size_chooser: Default::default(),
            collision_solver,
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
        Self {
            goconf: Default::default(),
            level_size_chooser,
            collision_solver: Default::default(),
        }
    }
}

impl<LSC, CSB: CollisionSolverBuilder> GOMapConf<LSC, CSB, TwoToPowerBitsStatic::<4>, TwoToPowerBitsStatic<2>, BuildDefaultSeededHasher> {
    pub fn lsize_cs(level_size_chooser: LSC, collision_solver: CSB) -> Self {
        Self {
            goconf: Default::default(),
            level_size_chooser,
            collision_solver,
        }
    }
}

impl<LSC, GS: GroupSize, SS: SeedSize, S> GOMapConf<LSC, LoMemAcceptEquals, GS, SS, S> {
    pub fn groups_lsize(goconf: GOConf<GS, SS, S>, level_size_chooser: LSC) -> Self {
        Self { goconf, level_size_chooser, collision_solver: Default::default() }
    }
}

impl<LSC, CSB: CollisionSolverBuilder, GS: GroupSize, SS: SeedSize, S> GOMapConf<LSC, CSB, GS, SS, S> {
    pub fn groups_lsize_cs(goconf: GOConf<GS, SS, S>, level_size_chooser: LSC, collision_solver: CSB) -> Self {
        Self { goconf, level_size_chooser, collision_solver }
    }
}
