//! Fingerprint-based minimal perfect hashing.

mod function;
pub use function::{FPHash, FPHashConf};

pub mod indexing2;
pub use indexing2::{GroupSize, SeedSize, TwoToPowerBits, TwoToPowerBitsStatic, Bits, Bits8};
mod gofunction;
pub use gofunction::{FPHash2, FPHash2Conf, FPHash2Builder};

pub mod keyset;