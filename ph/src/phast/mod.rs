//! Perfect Hashing with fast evaluation.

pub mod compressed_array;
pub use compressed_array::{CompressedArray, CompressedBuilder, DefaultCompressedArray};

mod builder;
mod conf;
pub use conf::bits_per_seed_to_100_bucket_size;

mod cyclic;
mod evaluator;

mod function;
pub use function::Function;

mod seed_chooser;
pub use seed_chooser::{SeedChooser, SeedOnly};

/// Power of two grater or equal than `WINDOW_SIZE`.
const MAX_WINDOW_SIZE: usize = 256;

/// Power of two grater or equal then range of values covered by the window.
const MAX_VALUES: usize = 4096;

/// Window size. Maximum number of elements in the priority queue.
const WINDOW_SIZE: u16 = 256;