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

const MAX_SPAN: usize = 64 * 1024 * 1024;
const MAX_VALUES: usize = 64 * 1024 * 1024;