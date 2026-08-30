//! Perfect Hashing with fast evaluation.
//!
//! This module implements PHast – a family of (minimal, k-)perfect hash functions
//! with very fast evaluation, developed by Piotr Beling and Peter Sanders
//! (see Piotr Beling, Peter Sanders, *PHast - Perfect Hashing made fast*, 2025,
//! <https://arxiv.org/abs/2504.17918>).
//!
//! The concrete function types are:
//! - [`Function`] – minimal perfect hash function,
//! - [`Function2`] – minimal perfect hash function compatible with almost all seed choosers,
//! - [`Perfect`] – (k-)perfect (not necessarily minimal) hash function,
//! - [`KFunction`] – k-perfect hash function,
//! - [`Partial`] – map-or-bump function (assigns values to some keys only),
//! - [`NBFunction`] – a no-bumping variant with single-cache-miss evaluation.
//!
//! The particular PHast variant is selected by a *seed chooser*
//! (see the [`SeedChooserConf`] implementations, e.g. [`ShiftOnlyWrapped`], [`SeedOnly`]),
//! and the core (range partitioning) configuration by [`CoreConf`] implementations
//! ([`Generic`], [`Turbo`]).

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

/// Window size (in buckets) used during construction; must be a power of two.
/// It is the maximum number of buckets held in the priority queue of already-processed buckets.
#[cfg(feature = "W256")]
const MAX_WINDOW_SIZE: usize = 256;
#[cfg(not(feature = "W256"))]
const MAX_WINDOW_SIZE: usize = 512;

/// Number of output values (must be a power of two) covered by the cyclic sets
/// ([`crate::phast::cyclic::CyclicSet`], [`crate::phast::cyclic::CyclicArray`])
/// used during construction. It must be at least as large as the widest slice (of output values)
/// processed in a single window, so it limits the maximum slice length.
//const MAX_VALUES: usize = 4096;
const MAX_VALUES: usize = 4096 * 2  *2    *2/* for l=16384 */; // TODO only MT require last *2; maybe switch to dynamic allocation?

/// Window size (in buckets) used during construction;
/// the maximum number of buckets held in the priority queue.
pub const WINDOW_SIZE: u16 = MAX_WINDOW_SIZE as u16;
