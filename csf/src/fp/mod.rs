//! Compressed static maps based on fingerprinting.

mod map;
pub use map::{Map, MapConf};

mod cmap;
pub use cmap::{CMap, CMapConf};

mod gocmap;
pub use gocmap::{GOCMap, GOCMapConf};

pub mod level_size_chooser;
pub use level_size_chooser::{LevelSizeChooser, SimpleLevelSizeChooser, ProportionalLevelSize, OptimalLevelSize};

pub mod collision_solver;
pub use collision_solver::{CollisionSolver, CollisionSolverBuilder, IsLossless, LoMemAcceptEquals};

mod common;