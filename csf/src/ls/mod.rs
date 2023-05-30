//! Compressed static maps based on solving linear systems.

pub mod graph3;
mod map;
mod conf;
pub use conf::{MapConf, BufferManager, FillRandomly, FillWithPattern};
pub use map::Map;
mod cmap;
pub use cmap::CMap;


