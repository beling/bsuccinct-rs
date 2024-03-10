#![doc = include_str!("../README.md")]

mod rank_select;

pub use rank_select::{RankSimple, ArrayWithRankSimple, RankSelect101111, ArrayWithRank101111,
     Rank, Select, Select0, SelectForRank101111, Select0ForRank101111, select64, optimal_combined_sampling,
     BinaryRankSearch, CombinedSampling, ConstCombinedSamplingDensity, AdaptiveCombinedSamplingDensity,
    utils::partition_point_with_index};

mod bitvec;
pub use bitvec::*;

/// Returns ceil of `n/d`.
#[inline(always)] pub const fn ceiling_div(n: usize, d: usize) -> usize { (n+d-1)/d }

/// Returns the largest `how_many`-bit number, i.e. 0..01..1 mask with `how_many` ones. `how_many` must be in range [0, 63].
#[inline(always)] pub const fn n_lowest_bits(how_many: u8) -> u64 { (1u64 << how_many).wrapping_sub(1) }

/// Returns the largest `how_many`-bit number, i.e. 0..01..1 mask with `how_many` ones. `how_many` must be in range [1, 64].
#[inline(always)] pub const fn n_lowest_bits_1_64(how_many: u8) -> u64 { u64::MAX >> (64-how_many) }

/// Returns the largest `how_many`-bit number, i.e. 0..01..1 mask with `how_many` ones. `how_many` must be in range [0, 64].
/// 
/// It is a bit slower than [`n_lowest_bits`] and [`n_lowest_bits_1_64`].
#[inline(always)] pub const fn n_lowest_bits_0_64(how_many: u8) -> u64 {
    // 1u64.checked_shl(how_many as u32).unwrap_or(0).wrapping_sub(1) gives the same assembly (as version with how_many >= 64) but is not allowed in const fn
    // version with how_many == 64 gives a bit different, very similar assembly
    if how_many >= 64 { return u64::MAX; }
    n_lowest_bits(how_many)
}

/// Calculates the minimal number of bits needed to store values from `0` to given `max_value`.
///
/// # Example
///
/// ```
/// use bitm::bits_to_store;
///
/// assert_eq!(bits_to_store(0u8), 0);
/// assert_eq!(bits_to_store(1u16), 1);
/// assert_eq!(bits_to_store(7u32), 3);
/// assert_eq!(bits_to_store(8u64), 4);
/// ```
#[inline] pub fn bits_to_store<V: Into<u64>>(max_value: V) -> u8 {
    /*let max_value: u64 = max_value.into();
    (if max_value.is_power_of_two() {
        max_value.trailing_zeros()+1
    } else {
        max_value.checked_next_power_of_two().unwrap_or(0).trailing_zeros()
    }) as u8*/
    max_value.into().checked_ilog2().map_or(0, |v| v as u8+1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_div_up() {
        assert_eq!(ceiling_div(7, 2), 4);
        assert_eq!(ceiling_div(8, 2), 4);
        assert_eq!(ceiling_div(9, 2), 5);
        assert_eq!(ceiling_div(10, 3), 4);
    }

    #[test]
    fn test_n_lowest() {
        assert_eq!(n_lowest_bits(63), u64::MAX>>1);
        assert_eq!(n_lowest_bits(3), 0b111);
        assert_eq!(n_lowest_bits(1), 0b1);
        assert_eq!(n_lowest_bits(0), 0);
    }

    #[test]
    fn test_bits_to_store() {
        assert_eq!(bits_to_store(0u32), 0);
        assert_eq!(bits_to_store(1u32), 1);
        assert_eq!(bits_to_store(2u32), 2);
        assert_eq!(bits_to_store(3u32), 2);
        assert_eq!(bits_to_store(4u32), 3);
        assert_eq!(bits_to_store(7u32), 3);
        assert_eq!(bits_to_store(8u32), 4);
        assert_eq!(bits_to_store(9u32), 4);
        assert_eq!(bits_to_store(15u32), 4);
        assert_eq!(bits_to_store(16u32), 5);
        assert_eq!(bits_to_store(u32::MAX-1), 32);
        assert_eq!(bits_to_store(u32::MAX), 32);
        assert_eq!(bits_to_store(u64::MAX), 64);
    }
}
