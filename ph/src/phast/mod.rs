//! Perfect Hashing with fast evaluation.

pub mod compressed_array;
pub use compressed_array::{CompressedArray, CompressedBuilder, DefaultCompressedArray};

mod builder;
mod conf;
pub use conf::{Params, bits_per_seed_to_100_bucket_size, Conf};

mod cyclic;
pub use cyclic::UsedValueMultiSetU8;
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
pub use seed_chooser::{SeedChooser, SeedOnly, ShiftOnly, ShiftOnlyWrapped, ShiftSeedWrapped, SeedOnlyK, KSeedEvaluator, SumOfValues, SumOfWeightedValues,
    bucket_size_normalization_multiplier, space_lower_bound};

/// Power of two grater or equal than `WINDOW_SIZE`.
const MAX_WINDOW_SIZE: usize = 256;

/// Power of two grater or equal then range of values covered by the window.
//const MAX_VALUES: usize = 4096;
const MAX_VALUES: usize = 4096 * 2  *2; // TODO only MT require last *2; maybe switch to dynamic allocation?

/// Window size. Maximum number of elements in the priority queue.
const WINDOW_SIZE: u16 = 256;