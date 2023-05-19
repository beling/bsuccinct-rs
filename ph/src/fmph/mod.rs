//! Fingerprint-based minimal perfect hashing.

mod function;
pub use function::{Function, BuildConf};

pub mod goindexing;
pub use goindexing::{GroupSize, SeedSize, TwoToPowerBits, TwoToPowerBitsStatic, Bits, Bits8};
mod gofunction;
pub use gofunction::{GOFunction, GOConf, GOBuildConf};

pub mod keyset;