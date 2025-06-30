//! Perfect Hashing with fast evaluation.

pub mod compressed_array;
pub use compressed_array::{CompressedArray, CompressedBuilder, DefaultCompressedArray};

mod builder;
mod conf;
pub use conf::bits_per_seed_to_100_bucket_size;

mod cyclic;
mod evaluator;
pub use evaluator::{BucketToActivateEvaluator, Weights};

mod function;
pub use function::Function;

mod function2;
pub use function2::Function2;

mod perfect;
pub use perfect::Perfect;

mod partial;
pub use partial::Partial;

mod seed_chooser;
mod seed_chooser_k;
pub use seed_chooser::{SeedChooser, SeedOnly, ShiftOnly, ShiftOnlyWrapped, ShiftOnlyX1, ShiftOnlyX2, ShiftOnlyX3, ShiftOnlyX4, ShiftSeedWrapped};
pub use seed_chooser_k::{SeedOnlyK};

/// Power of two grater or equal than `WINDOW_SIZE`.
const MAX_WINDOW_SIZE: usize = 256;

/// Power of two grater or equal then range of values covered by the window.
//const MAX_VALUES: usize = 4096;
const MAX_VALUES: usize = 4096 * 2;

/// Window size. Maximum number of elements in the priority queue.
const WINDOW_SIZE: u16 = 256;