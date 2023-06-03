use crate::fp::level_size_chooser::OptimalLevelSize;
use ph::{BuildDefaultSeededHasher, BuildSeededHasher};
use crate::fp::collision_solver::{CollisionSolverBuilder, LoMemAcceptEquals};
use crate::coding::BuildMinimumRedundancy;

//#[derive(Default)]
#[derive(Copy, Clone)]
pub struct CMapConf<
    CB = BuildMinimumRedundancy,
    LSC = OptimalLevelSize,
    CSB: CollisionSolverBuilder = LoMemAcceptEquals,
    S: BuildSeededHasher = BuildDefaultSeededHasher
    /*, BS: stats::BuildStatsCollector = ()*/
> {
    pub coding: CB,
    pub level_size_chooser: LSC,
    pub collision_solver: CSB,
    /// The family of hash functions used by the constructed [`fp::CMap`](crate::fp::CMap). (default: [`BuildDefaultSeededHasher`])
    pub hash: S,
    //stats: BS
}

/*impl<LSC: LevelSizeChooser + Default, S: BuildHasher + Default> Default for Conf<LSC, S> {
    fn default() -> Self {
        Self { ..Default::default() }
    }
}*/

impl Default for CMapConf<BuildMinimumRedundancy, OptimalLevelSize, LoMemAcceptEquals, BuildDefaultSeededHasher> {
    //fn default() -> Self { Self { ..Default::default() } }
    fn default() -> Self { Self {
        coding: Default::default(), level_size_chooser: Default::default(),
        collision_solver: Default::default(), hash: Default::default()
    } }
}

impl CMapConf {
    pub fn bpf(bits_per_fragment: u8) -> Self {
        Self::coding(BuildMinimumRedundancy{ bits_per_fragment })
    }
}

impl<BC> CMapConf<BC, OptimalLevelSize, LoMemAcceptEquals, BuildDefaultSeededHasher> {
    pub fn coding(coding: BC) -> Self {
        Self { coding, level_size_chooser: Default::default(),
        collision_solver: Default::default(), hash: Default::default() }
    }
}

impl<CS: CollisionSolverBuilder> CMapConf<BuildMinimumRedundancy, OptimalLevelSize, CS, BuildDefaultSeededHasher> {
    pub fn cs(collision_solver: CS) -> Self {
        Self { coding: Default::default(), collision_solver, level_size_chooser: Default::default(), hash: Default::default() }
    }
}

impl<BC, CS: CollisionSolverBuilder> CMapConf<BC, OptimalLevelSize, CS, BuildDefaultSeededHasher> {
    pub fn cs_coding(collision_solver: CS, coding: BC) -> Self {
        Self { coding, collision_solver, level_size_chooser: Default::default(), hash: Default::default()}
    }
}

impl<LSC> CMapConf<BuildMinimumRedundancy, LSC, LoMemAcceptEquals, BuildDefaultSeededHasher> {
    pub fn lsize(level_size_chooser: LSC) -> Self {
        Self { coding: Default::default(), level_size_chooser, collision_solver: Default::default(), hash: Default::default() }
    }
    pub fn lsize_bpf(level_size_chooser: LSC, bits_per_fragment: u8) -> Self {
        Self::lsize_coding(level_size_chooser, BuildMinimumRedundancy{ bits_per_fragment })
    }
}

impl<BC, LSC> CMapConf<BC, LSC, LoMemAcceptEquals, BuildDefaultSeededHasher> {
    pub fn lsize_coding(level_size_chooser: LSC, coding: BC) -> Self {
        Self { coding, level_size_chooser, collision_solver: Default::default(), hash: Default::default() }
    }
}

impl<LSC, CS: CollisionSolverBuilder> CMapConf<BuildMinimumRedundancy, LSC, CS, BuildDefaultSeededHasher> {
    pub fn lsize_cs(level_size_chooser: LSC, collision_solver: CS) -> Self {
        Self { coding: Default::default(), level_size_chooser, collision_solver, hash: Default::default() }
    }
}

impl<BC, LSC, CS: CollisionSolverBuilder> CMapConf<BC, LSC, CS, BuildDefaultSeededHasher> {
    pub fn lsize_cs_coding(level_size_chooser: LSC, collision_solver: CS, coding: BC) -> Self {
        Self { coding, level_size_chooser, collision_solver, hash: Default::default() }
    }
}

impl<S: BuildSeededHasher> CMapConf<BuildMinimumRedundancy, OptimalLevelSize, LoMemAcceptEquals, S> {
    pub fn hash(hash: S) -> Self {
        Self { coding: Default::default(), level_size_chooser: Default::default(), collision_solver: Default::default(), hash }
    }
}

impl<BC, S: BuildSeededHasher> CMapConf<BC, OptimalLevelSize, LoMemAcceptEquals, S> {
    pub fn hash_coding(hash: S, coding: BC) -> Self {
        Self { coding, level_size_chooser: Default::default(), collision_solver: Default::default(), hash }
    }
}

impl<S: BuildSeededHasher, CS: CollisionSolverBuilder> CMapConf<BuildMinimumRedundancy, OptimalLevelSize, CS, S> {
    pub fn cs_hash(collision_solver: CS, hash: S) -> Self {
        Self { coding: Default::default(), level_size_chooser: Default::default(), collision_solver, hash }
    }
}

impl<BC, S: BuildSeededHasher, CS: CollisionSolverBuilder> CMapConf<BC, OptimalLevelSize, CS, S> {
    pub fn cs_hash_coding(collision_solver: CS, hash: S, coding: BC) -> Self {
        Self { coding, level_size_chooser: Default::default(), collision_solver, hash }
    }
}

impl<LSC, S: BuildSeededHasher> CMapConf<BuildMinimumRedundancy, LSC, LoMemAcceptEquals, S> {
    pub fn lsize_hash(level_size_chooser: LSC, hash: S) -> Self {
        Self { coding: Default::default(), level_size_chooser, collision_solver: Default::default(), hash }
    }
}

impl<BC, LSC, S: BuildSeededHasher> CMapConf<BC, LSC, LoMemAcceptEquals, S> {
    pub fn lsize_hash_coding(level_size_chooser: LSC, hash: S, coding: BC) -> Self {
        Self { coding, level_size_chooser, collision_solver: Default::default(), hash }
    }
}

impl<LSC, CS: CollisionSolverBuilder, S: BuildSeededHasher> CMapConf<BuildMinimumRedundancy, LSC, CS, S> {
    pub fn lsize_cs_hash(level_size_chooser: LSC, collision_solver: CS, hash: S) -> Self {
        Self { coding: Default::default(), level_size_chooser, collision_solver, hash }
    }
}

impl<BC, LSC, CS: CollisionSolverBuilder, S: BuildSeededHasher> CMapConf<BC, LSC, CS, S> {
    pub fn lsize_cs_hash_coding(level_size_chooser: LSC, collision_solver: CS, hash: S, coding: BC) -> Self {
        Self { coding, level_size_chooser, collision_solver, hash }
    }
}
