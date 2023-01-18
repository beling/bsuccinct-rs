mod hash;
pub use hash::{FPHash, FPHashConf};

pub mod indexing2;
pub use indexing2::{GroupSize, SeedSize, TwoToPowerBits, TwoToPowerBitsStatic, Bits, Bits8};
mod hash2;
pub use hash2::{FPHash2, FPHash2Conf, FPHash2Builder};

pub mod keyset;