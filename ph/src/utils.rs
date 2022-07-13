pub use binout::{read_int, write_int};
use std::num::NonZeroUsize;
use std::thread::available_parallelism;
#[cfg(feature = "simple_rank")] use bitm::ArrayWithRankSimple;
#[cfg(not(feature = "simple_rank"))] use bitm::ArrayWithRank101111;

#[cfg(feature = "simple_rank")] pub type ArrayWithRank = ArrayWithRankSimple;
#[cfg(not(feature = "simple_rank"))] pub type ArrayWithRank = ArrayWithRank101111;

/// Calculates the number of bits needed to store values from `0` up to given one (works only for non-negative integers).
///
/// # Example
///
/// ```
/// use ph::bits_to_store;
///
/// assert_eq!(bits_to_store!(0u32), 0);
/// assert_eq!(bits_to_store!(1u32), 1);
/// assert_eq!(bits_to_store!(7u32), 3);
/// assert_eq!(bits_to_store!(8u32), 4);
/// ```
#[macro_export]
macro_rules! bits_to_store {
    ($value:expr) => {{
        let v = $value;
        (if v.is_power_of_two() {
            v.trailing_zeros()+1
        } else {
            v.checked_next_power_of_two().unwrap_or(0).trailing_zeros()
        }) as u8
    }};
}

/// Reads array of integers from given `input`.
#[macro_export]
macro_rules! read_array {
    ([$cell_t:ty; $len:expr] from $input:ident to $vec:ident) => {{
        for _ in 0..$len { $vec.push(read_int!($input, $cell_t)?) }
    }};

    ([$cell_t:ty; read $len_t:ty] from $input:ident) => {{
        //read_array!(read_int!($input, $len_t) $cell_t cells from $input)
        read_array!([$cell_t; read_int!($input, $len_t)?] from $input)
    }};

    ([$cell_t:ty; $len:expr] from $input:ident) => {{
        let len = ($len) as usize;
        let mut result = Vec::<$cell_t>::with_capacity(len);
        read_array!([$cell_t; {len}] from $input to result);
        result
    }};

    ($len:expr; bits from $input:ident) => {{
        read_array!([u64; bitm::ceiling_div($len, 64)] from $input)
    }};
}

/// Maps 32-bit `hash` to the range `[0, n)`, where `n` is a 32-bit integer.
///
/// Uses the algorithm described in: Daniel Lemire, *A fast alternative to the modulo reduction*,
/// <https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/>
#[inline(always)]
pub fn map32_to_32(hash: u32, n: u32) -> u32 {
    (((hash as u64) * (n as u64)) >> 32) as u32
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
    /*if n > (u32::MAX as u64) {
        hash % n
    } else {
        map64_to_32(hash, n as u32) as u64
    }*/
    (((hash as u128) * (n as u128)) >> 64) as u64
}

/// Test given `mphf`, assuming that it is built for given set of keys.
#[cfg(test)]
pub fn test_mphf<K: std::fmt::Display, G: Fn(&K)->Option<usize>>(mphf_keys: &[K], mphf: G) {
    use bitm::{BitAccess, BitVec};
    let mut seen = Box::<[u64]>::with_zeroed_bits(mphf_keys.len());
    for key in mphf_keys {
        let index = mphf(key);
        assert!(index.is_some(), "MPHF does not assign the value for the key {} which is in the input", key);
        let index = index.unwrap() as usize;
        assert!(index < mphf_keys.len(), "MPHF assigns too large value for the key {}: {}>{}.", key, index, mphf_keys.len());
        assert!(!seen.get_bit(index), "MPHF assigns the same value to two keys of input, including {}.", key);
        seen.set_bit(index);
    }
}

/// Returns `conf` if it is greater than `0`, or `max(1, available parallelism + conf)` otherwise.
pub fn threads_count(conf: isize) -> NonZeroUsize {
    if conf > 0 {
        unsafe { NonZeroUsize::new_unchecked(conf as usize) }
    } else {
        unsafe { available_parallelism().map_or(NonZeroUsize::new_unchecked(1), |v| {
            NonZeroUsize::new_unchecked(v.get().saturating_sub((-conf) as usize).max(1))
        }) }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_bits_to_store() {
        assert_eq!(bits_to_store!(0u32), 0);
        assert_eq!(bits_to_store!(1u32), 1);
        assert_eq!(bits_to_store!(2u32), 2);
        assert_eq!(bits_to_store!(3u32), 2);
        assert_eq!(bits_to_store!(4u32), 3);
        assert_eq!(bits_to_store!(7u32), 3);
        assert_eq!(bits_to_store!(8u32), 4);
        assert_eq!(bits_to_store!(u32::MAX-1), 32);
        assert_eq!(bits_to_store!(u32::MAX), 32);
    }
}


