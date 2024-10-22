/// Maps 32-bit `hash` to the range `[0, n)`, where `n` is a 32-bit integer.
///
/// Uses the algorithm described in: Daniel Lemire, *A fast alternative to the modulo reduction*,
/// <https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/>
#[inline(always)]
pub fn map32_to_32(hash: u32, n: u32) -> u32 {
    (((hash as u64) * (n as u64)) >> 32) as u32
}

/// Maps 16-bit `hash` to the range `[0, n)`, where `n` is a 16-bit integer.
///
/// Uses the algorithm described in: Daniel Lemire, *A fast alternative to the modulo reduction*,
/// <https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/>
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

/// Maps `hash` to the range `[0, n)` using either [`map64_to_64`] or [`map32_to_32`] (depended on platform).
#[inline(always)]
pub fn map_usize(hash: usize, n: usize) -> usize {
    #[cfg(target_pointer_width = "64")] { map64_to_64(hash as u64, n as u64) as usize }
    #[cfg(target_pointer_width = "32")] { map32_to_32(hash as u32, n as u32) as usize }
}