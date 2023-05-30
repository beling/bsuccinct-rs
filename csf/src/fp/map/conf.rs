use crate::fp::collision_solver::{CollisionSolverBuilder, LoMemAcceptEquals};
use crate::fp::OptimalLevelSize;
use ph::{BuildDefaultSeededHasher, BuildSeededHasher};

//#[derive(Default)]
#[derive(Clone)]
pub struct MapConf<
    LSC = OptimalLevelSize,
    CSB: CollisionSolverBuilder = LoMemAcceptEquals,
    S: BuildSeededHasher = BuildDefaultSeededHasher
    /*, BS: stats::BuildStatsCollector = ()*/
> {
    pub bits_per_value: u8,
    pub level_size_chooser: LSC,
    pub collision_solver: CSB,
    pub hash: S,
    //stats: BS
}

/*impl<LSC: LevelSizeChooser + Default, S: BuildHasher + Default> Default for Conf<LSC, S> {
    fn default() -> Self {
        Self { ..Default::default() }
    }
}*/

impl Default for MapConf<OptimalLevelSize, LoMemAcceptEquals, BuildDefaultSeededHasher> {
    //fn default() -> Self { Self { ..Default::default() } }
    fn default() -> Self { Self {
        bits_per_value: Default::default(), level_size_chooser: Default::default(),
        collision_solver: Default::default(), hash: Default::default()
    } }
}

impl MapConf<OptimalLevelSize, LoMemAcceptEquals, BuildDefaultSeededHasher> {
    pub fn bpv(bits_per_value: u8) -> Self {
        Self { bits_per_value, ..Default::default() }
    }
}

impl<CS: CollisionSolverBuilder> MapConf<OptimalLevelSize, CS, BuildDefaultSeededHasher> {
    pub fn cs(collision_solver: CS) -> Self {
        Self { bits_per_value: Default::default(), collision_solver, level_size_chooser: Default::default(), hash: Default::default()}
    }

    pub fn cs_bpv(collision_solver: CS, bits_per_value: u8) -> Self {
        Self { bits_per_value, collision_solver, level_size_chooser: Default::default(), hash: Default::default()}
    }
}

impl<LSC> MapConf<LSC, LoMemAcceptEquals, BuildDefaultSeededHasher> {
    pub fn lsize(level_size_chooser: LSC) -> Self {
        Self { bits_per_value: Default::default(), level_size_chooser, collision_solver: Default::default(), hash: Default::default() }
    }
    pub fn lsize_bpv(level_size_chooser: LSC, bits_per_value: u8) -> Self {
        Self { bits_per_value, level_size_chooser, collision_solver: Default::default(), hash: Default::default() }
    }
}

impl<LSC, CS: CollisionSolverBuilder> MapConf<LSC, CS, BuildDefaultSeededHasher> {
    pub fn lsize_cs(level_size_chooser: LSC, collision_solver: CS) -> Self {
        Self { bits_per_value: Default::default(), level_size_chooser, collision_solver, hash: Default::default() }
    }
    pub fn lsize_cs_bpv(level_size_chooser: LSC, collision_solver: CS, bits_per_value: u8) -> Self {
        Self { bits_per_value, level_size_chooser, collision_solver, hash: Default::default() }
    }
}

impl<S: BuildSeededHasher> MapConf<OptimalLevelSize, LoMemAcceptEquals, S> {
    pub fn hash(hash: S) -> Self {
        Self { bits_per_value: Default::default(), level_size_chooser: Default::default(), collision_solver: Default::default(), hash }
    }
    pub fn hash_bpv(hash: S, bits_per_value: u8) -> Self {
        Self { bits_per_value, level_size_chooser: Default::default(), collision_solver: Default::default(), hash }
    }
}

impl<S: BuildSeededHasher, CS: CollisionSolverBuilder> MapConf<OptimalLevelSize, CS, S> {
    pub fn cs_hash(collision_solver: CS, hash: S) -> Self {
        Self { bits_per_value: Default::default(), level_size_chooser: Default::default(), collision_solver, hash }
    }
    pub fn cs_hash_bpv(collision_solver: CS, hash: S, bits_per_value: u8) -> Self {
        Self { bits_per_value, level_size_chooser: Default::default(), collision_solver, hash }
    }
}

impl<LSC, S: BuildSeededHasher> MapConf<LSC, LoMemAcceptEquals, S> {
    pub fn lsize_hash(level_size_chooser: LSC, hash: S) -> Self {
        Self { bits_per_value: Default::default(), level_size_chooser, collision_solver: Default::default(), hash }
    }
    pub fn lsize_hash_bpv(level_size_chooser: LSC, hash: S, bits_per_value: u8) -> Self {
        Self { bits_per_value, level_size_chooser, collision_solver: Default::default(), hash }
    }
}

impl<LSC, CS: CollisionSolverBuilder, S: BuildSeededHasher> MapConf<LSC, CS, S> {
    pub fn lsize_cs_hash(level_size_chooser: LSC, collision_solver: CS, hash: S) -> Self {
        Self { bits_per_value: Default::default(), level_size_chooser, collision_solver, hash }
    }
    pub fn lsize_cs_hash_bpv(level_size_chooser: LSC, collision_solver: CS, hash: S, bits_per_value: u8) -> Self {
        Self { bits_per_value, level_size_chooser, collision_solver, hash }
    }
}

