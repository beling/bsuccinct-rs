use crate::fp::collision_solver::{CollisionSolverBuilder, LoMemAcceptEquals};
use crate::fp::OptimalLevelSize;
use ph::{BuildDefaultSeededHasher, BuildSeededHasher};

/// Configuration accepted by [`fp::Map`](crate::fp::Map) constructors.
//#[derive(Default)]
#[derive(Clone)]
pub struct MapConf<
    LSC = OptimalLevelSize,
    CSB: CollisionSolverBuilder = LoMemAcceptEquals,
    S: BuildSeededHasher = BuildDefaultSeededHasher
> {
    /// Choose the size of each level.
    pub level_sizer: LSC,
    /// Constructs collision solver that decides which collisions are positive, and which are negative.
    pub collision_solver: CSB,
    /// The family of hash functions used by the constructed [`fp::Map`](crate::fp::Map). (default: [`BuildDefaultSeededHasher`])
    pub hash: S,
}

/*impl<LSC: LevelSizeChooser + Default, S: BuildHasher + Default> Default for Conf<LSC, S> {
    fn default() -> Self {
        Self { ..Default::default() }
    }
}*/

impl Default for MapConf<OptimalLevelSize, LoMemAcceptEquals, BuildDefaultSeededHasher> {
    //fn default() -> Self { Self { ..Default::default() } }
    fn default() -> Self { Self {
        level_sizer: Default::default(),
        collision_solver: Default::default(), hash: Default::default()
    } }
}

impl<CS: CollisionSolverBuilder> MapConf<OptimalLevelSize, CS, BuildDefaultSeededHasher> {
    pub fn cs(collision_solver: CS) -> Self {
        Self { collision_solver, level_sizer: Default::default(), hash: Default::default()}
    }
}

impl<LSC> MapConf<LSC, LoMemAcceptEquals, BuildDefaultSeededHasher> {
    pub fn lsize(level_size_chooser: LSC) -> Self {
        Self { level_sizer: level_size_chooser, collision_solver: Default::default(), hash: Default::default() }
    }
    pub fn lsize_bpv(level_size_chooser: LSC) -> Self {
        Self { level_sizer: level_size_chooser, collision_solver: Default::default(), hash: Default::default() }
    }
}

impl<LSC, CS: CollisionSolverBuilder> MapConf<LSC, CS, BuildDefaultSeededHasher> {
    pub fn lsize_cs(level_size_chooser: LSC, collision_solver: CS) -> Self {
        Self { level_sizer: level_size_chooser, collision_solver, hash: Default::default() }
    }
}

impl<S: BuildSeededHasher> MapConf<OptimalLevelSize, LoMemAcceptEquals, S> {
    pub fn hash(hash: S) -> Self {
        Self { level_sizer: Default::default(), collision_solver: Default::default(), hash }
    }
}

impl<S: BuildSeededHasher, CS: CollisionSolverBuilder> MapConf<OptimalLevelSize, CS, S> {
    pub fn cs_hash(collision_solver: CS, hash: S) -> Self {
        Self { level_sizer: Default::default(), collision_solver, hash }
    }
}

impl<LSC, S: BuildSeededHasher> MapConf<LSC, LoMemAcceptEquals, S> {
    pub fn lsize_hash(level_size_chooser: LSC, hash: S) -> Self {
        Self { level_sizer: level_size_chooser, collision_solver: Default::default(), hash }
    }
}

impl<LSC, CS: CollisionSolverBuilder, S: BuildSeededHasher> MapConf<LSC, CS, S> {
    pub fn lsize_cs_hash(level_size_chooser: LSC, collision_solver: CS, hash: S) -> Self {
        Self { level_sizer: level_size_chooser, collision_solver, hash }
    }
}

