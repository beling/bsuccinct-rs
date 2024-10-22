//! Utility functions.

use binout::{AsIs, Serializer};
use bitm::{ArrayWithRank101111, ceiling_div};
pub use seedable_hash::{map64_to_64, map32_to_32};

pub type ArrayWithRank = ArrayWithRank101111;

/// Reads `number_of_bits` bits, rounded up to multiple of 64, from `input`.
pub fn read_bits<R: std::io::Read + ?Sized>(input: &mut R, number_of_bits: usize) -> std::io::Result<Box<[u64]>> {
    AsIs::read_n(input, ceiling_div(number_of_bits, 64))
}

