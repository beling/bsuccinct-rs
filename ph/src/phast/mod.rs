pub mod compressed_array;
pub use compressed_array::{CompressedArray, DefaultCompressedArray};

mod builder;
mod conf;
pub use conf::bits_per_seed_to_100_bucket_size;

mod cyclic;
mod evaluator;

mod function;
pub use function::Function;

pub const MAX_SPAN: usize = 256;
pub const MAX_VALUES: usize = 4096;
//pub const MAX_VALUES: usize = 2048; // OK?