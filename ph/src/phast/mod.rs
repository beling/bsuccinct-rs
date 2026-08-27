//! Perfect Hashing with fast evaluation.

pub mod compressed_array;
pub use compressed_array::{CompressedArray, CompressedBuilder, DefaultCompressedArray};

mod builder;
mod conf;
pub use conf::{Generic, GenericCore, Turbo, TurboCore, Placement, FastPlacement, RandomPlacement, bits_per_seed_to_100_bucket_size, Core, CoreConf, Conf};

mod cyclic;
pub use cyclic::FreeValueMultiSetU16;
mod evaluator;
pub use evaluator::{BucketEvaluator, Weights};

mod function;
pub use function::Function;

mod function2;
pub use function2::Function2;

mod perfect;
pub use perfect::Perfect;

mod partial;
pub use partial::Partial;

mod kfunction;
pub use kfunction::KFunction;

mod nbfunction;
pub use nbfunction::NBFunction;

mod seed_chooser;
pub use seed_chooser::{SeedChooser, SeedChooserConf, SeedChooserCore, SeedEvaluator, ProdOfValues, SeedOnly, SeedOnlyCore, ShiftOnly, ShiftCore,
    ShiftOnlyWrapped, ShiftOnlyProdWrapped, ShiftWrappedCore, ShiftSeedWrapped, ShiftSeedCore, SeedOnlyK, SeedOnlyKCore, KSeedEvaluator, ProdOfValuesKEval, SumOfValues,
    KSeedEvaluatorConf, bucket_size_normalization_multiplier, space_lower_bound, ComparableF64, ProdCmp};

/// Power of two grater or equal than `WINDOW_SIZE`.
#[cfg(feature = "W256")]
const MAX_WINDOW_SIZE: usize = 256;
#[cfg(not(feature = "W256"))]
const MAX_WINDOW_SIZE: usize = 512;

/// Power of two grater or equal then range of values covered by the window.
//const MAX_VALUES: usize = 4096;
const MAX_VALUES: usize = 4096 * 2  *2    *2/* for l=16384 */; // TODO only MT require last *2; maybe switch to dynamic allocation?

/// Window size. Maximum number of elements in the priority queue.
pub const WINDOW_SIZE: u16 = MAX_WINDOW_SIZE as u16;
