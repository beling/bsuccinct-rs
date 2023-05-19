//! Utility functions.

use binout::{AsIs, Serializer};
#[cfg(feature = "simple_rank")] use bitm::ArrayWithRankSimple;
#[cfg(not(feature = "simple_rank"))] use bitm::ArrayWithRank101111;
use bitm::ceiling_div;

#[cfg(feature = "simple_rank")] pub type ArrayWithRank = ArrayWithRankSimple;
#[cfg(not(feature = "simple_rank"))] pub type ArrayWithRank = ArrayWithRank101111;

/// Reads `number_of_bits` bits, rounded up to multiple of 64, from `input`.
pub fn read_bits<R: std::io::Read + ?Sized>(input: &mut R, number_of_bits: usize) -> std::io::Result<Box<[u64]>> {
    AsIs::read_n(input, ceiling_div(number_of_bits, 64))
}

/// Maps 32-bit `hash` to the range `[0, n)`, where `n` is a 32-bit integer.
///
/// Uses the algorithm described in: Daniel Lemire, *A fast alternative to the modulo reduction*,
/// <https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/>
#[inline(always)]
pub fn map32_to_32(hash: u32, n: u32) -> u32 {
    (((hash as u64) * (n as u64)) >> 32) as u32
}

#[inline(always)]
pub fn map16_to_16(hash: u16, n: u16) -> u16 {
    (((hash as u32) * (n as u32)) >> 16) as u16
}

/// Maps 64-bit `hash` to the range `[0, n)`, where `n` is a 32-bit integer.
///
/// Uses the algorithm described in: Daniel Lemire, *A fast alternative to the modulo reduction*,
/// <https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/>
#[inline(always)]
pub fn map64_to_32(hash: u64, n: u32) -> u32 {
    map32_to_32((hash ^ (hash>>32)) as u32, n)
}

/// Maps 64-bit `hash` to the range `[0, n)`, where `n` is a 64-bit integer.
///
/// Uses the algorithm described in: Daniel Lemire, *A fast alternative to the modulo reduction*,
/// <https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/>
#[inline(always)]
pub fn map64_to_64(hash: u64, n: u64) -> u64 {
    (((hash as u128) * (n as u128)) >> 64) as u64
}

// Maps 48-bit `hash` to the range `[0, n)`, where `n` is a 64-bit integer.
//
// Uses slightly modified version of the algorithm described in:
// Daniel Lemire, *A fast alternative to the modulo reduction*,
// <https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/>
/*#[inline(always)]
pub fn map48_to_64(hash: u64, n: u64) -> u64 {
    ((((hash << 16) as u128) * (n as u128)) >> 64) as u64
}*/ // the function is fine, but not needed
