mod hash;
pub use hash::{FPHash, FPHashConf};

mod indexing2;
pub use indexing2::{GroupSize, SeedSize, TwoToPowerBits, TwoToPowerBitsStatic, Bits, Bits8};
mod hash2;
pub use hash2::{FPHash2, FPHash2Conf};

pub mod keyset;