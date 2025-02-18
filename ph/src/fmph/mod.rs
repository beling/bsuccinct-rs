//! Fingerprint-based minimal perfect hashing.

mod function;
pub use function::{Function, BuildConf};

pub mod goindexing;
pub use goindexing::{GroupSize, TwoToPowerBits};
mod gofunction;
pub use gofunction::{GOFunction, GOConf, GOBuildConf};

// For backward compatibility: 
pub use crate::seeds::{SeedSize, Bits8, TwoToPowerBitsStatic, Bits};

pub mod keyset;