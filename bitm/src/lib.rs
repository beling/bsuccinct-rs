#![doc = include_str!("../README.md")]

mod rank_select;
pub use rank_select::{RankSelect101111, ArrayWithRank101111, BitArrayWithRank};

mod bitvec;
pub use bitvec::*;

/// Returns ceil of `n/d`.
#[inline(always)] pub const fn ceiling_div(n: usize, d: usize) -> usize { (n+d-1)/d }

/// Returns the largest `how_many`-bit number, i.e. 0..01..1 mask with `how_many` ones.
#[inline(always)] pub const fn n_lowest_bits(how_many: u8) -> u64 { (1u64 << how_many).wrapping_sub(1) }

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
}
