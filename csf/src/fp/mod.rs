//! Compressed static maps based on fingerprinting.

pub mod kvset;

mod map;
pub use map::{Map, MapConf};

mod cmap;
pub use cmap::{CMap, CMapConf};

mod gomap;
pub use gomap::{GOMap, GOMapConf};

mod gocmap;
pub use gocmap::{GOCMap, GOCMapConf};
pub use ph::fmph::{GroupSize, SeedSize, TwoToPowerBits, TwoToPowerBitsStatic, Bits, Bits8, GOConf};

pub mod level_sizer;
pub use level_sizer::{LevelSizer, ProportionalLevelSize, OptimalLevelSize, ResizedLevel};

pub mod collision_solver;
pub use collision_solver::{CollisionSolver, CollisionSolverBuilder, IsLossless, LoMemAcceptEquals};



mod common;