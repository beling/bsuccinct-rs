mod utils;
mod select;
use std::ops::Deref;

use self::select::{U64_PER_L1_ENTRY, U64_PER_L2_ENTRY, U64_PER_L2_RECORDS};
pub use self::select::{Select, Select0, BinaryRankSearch, CombinedSampling,
     ConstCombinedSamplingDensity, AdaptiveCombinedSamplingDensity, SelectForRank101111, Select0ForRank101111,
     select64, optimal_combined_sampling};

use super::{ceiling_div, n_lowest_bits};
use dyn_size_of::GetSize;

/// Trait for rank queries on bit vector.
/// Rank query returns the number of ones (or zeros) in requested number of the first bits.
pub trait Rank {
    /// Returns the number of ones in first `index` bits or [`None`] if `index` is out of bounds.
    fn try_rank(&self, index: usize) -> Option<usize>;

    /// Returns the number of ones in first `index` bits or panics if `index` is out of bounds.
    #[inline] fn rank(&self, index: usize) -> usize {
        self.try_rank(index).expect("rank index out of bound")
    }

    /// Returns the number of ones in first `index` bits.
    /// The result is undefined if `index` is out of bounds.
    #[inline] unsafe fn rank_unchecked(&self, index: usize) -> usize {
        self.rank(index)
    }

    /// Returns the number of zeros in first `index` bits or [`None`] if `index` is out of bounds.
    #[inline] fn try_rank0(&self, index: usize) -> Option<usize> {
         self.try_rank(index).map(|r| index-r)
    }

    /// Returns the number of zeros in first `index` bits or panics if `index` is out of bounds.
    #[inline] fn rank0(&self, index: usize) -> usize { index - self.rank(index) }

    /// Returns the number of ones in first `index` bits.
    /// The result is undefined if `index` is out of bounds.
    #[inline] unsafe fn rank0_unchecked(&self, index: usize) -> usize {
        index - self.rank_unchecked(index)
    }

    #[inline]
    fn prefetch(&self, _index: usize) {}
}

/// Returns number of bits set (to one) in `content` whose length does not exceeds 8.
#[inline] fn count_bits_in(content: &[u64]) -> usize {
    let mut it = content.iter().map(|v| v.count_ones() as usize);
    let mut result = 0;
    if let Some(v) = it.next() { result += v; } else { return result; }
    if let Some(v) = it.next() { result += v; } else { return result; }
    if let Some(v) = it.next() { result += v; } else { return result; }
    if let Some(v) = it.next() { result += v; } else { return result; }
    if let Some(v) = it.next() { result += v; } else { return result; }
    if let Some(v) = it.next() { result += v; } else { return result; }
    if let Some(v) = it.next() { result += v; } else { return result; }
    if let Some(v) = it.next() { result += v; }
    return result;
}

/*#[inline] fn count_bits_in(content: &[u64]) -> usize {  // almost the same asm as above
    let l = content.len();
    let mut result = 0;
    for i in 0..8 {
        if i < l { result += unsafe{ content.get_unchecked(i) }.count_ones() as usize; }
    }
    return result;
}*/

/// Returns number of bits set (to one) in `content` whose length does not exceeds 8.
/*#[inline] fn count_bits_in(mut content: &[u64]) -> usize {
    let mut result = 0;
    if content.len() >= 3 {
        result += unsafe{ content.get_unchecked(0) }.count_ones() as usize +
            unsafe{ content.get_unchecked(1) }.count_ones() as usize +
            unsafe{ content.get_unchecked(2) }.count_ones() as usize;
        content = &content[3..];
        if content.len() >= 3 {
            result += unsafe{ content.get_unchecked(0) }.count_ones() as usize +
                unsafe{ content.get_unchecked(1) }.count_ones() as usize +
                unsafe{ content.get_unchecked(2) }.count_ones() as usize;
            content = &content[3..];
        }
    }
    // up to 2 elements
    let l = content.len();
    if l > 0 {
        result += unsafe{ content.get_unchecked(0) }.count_ones() as usize;
        if l > 1 {
            result += unsafe{ content.get_unchecked(1) }.count_ones() as usize;
        }
    }
    result
}*/

/// Returns number of bits set (to one) in `content` whose length does not exceeds 8.
/*#[inline] fn count_bits_in(mut content: &[u64]) -> usize {
    let mut result = 0;
    // up to 8 elements
    if content.len() >= 4 {
        result += unsafe{ content.get_unchecked(0) }.count_ones() as usize +
            unsafe{ content.get_unchecked(1) }.count_ones() as usize +
            unsafe{ content.get_unchecked(2) }.count_ones() as usize +
            unsafe{ content.get_unchecked(3) }.count_ones() as usize;
        content = &content[4..];
    }
    // up to 4 elements
    if content.len() >= 2 {
        result += unsafe{ content.get_unchecked(0) }.count_ones() as usize +
            unsafe{ content.get_unchecked(1) }.count_ones() as usize;
        content = &content[2..];
    }
    // up to 2 elements
    let l = content.len();
    if l > 0 {
        result += unsafe{ content.get_unchecked(0) }.count_ones() as usize;
        if l > 1 {
            result += unsafe{ content.get_unchecked(1) }.count_ones() as usize;
        }
    }
    result
}*/



/// The structure that holds bit vector `content` and `ranks` structure that takes no more than 3.125% extra space.
/// It can return the number of ones (or zeros) in first `index` bits of the `content` (see `rank` and `rank0` method) in *O(1)* time.
/// In addition, it supports select queries utilizing binary search over ranks (see [`BinaryRankSearch`])
/// or (optionally, at the cost of extra space overhead; about 0.39% with default settings)
/// combined sampling (which is usually faster; see [`CombinedSampling`]).
///
/// Any type that implements the [`Deref`] trait with `Target = [u64]` can be used as a bit vector.
/// It is recommended to use this structure with bit vectors allocated with alignment to the CPU cache line or 64 bytes.
/// Such a vector can be constructed, for example, by compiling `bitm` with the `aligned-vec` feature and using implementation
/// of [`crate::BitVec`] trait for `aligned_vec::ABox<[u64]>`, for example: `ABox::with_zeroed_bits(number_of_bits)`.
///
/// The structure supports vectors up to 2<sup>64</sup> bits and its design is based on a 3-level (compact due to relative addressing)
/// index that samples rank responses every 512 bits and is CPU cache friendly as the first level is small
/// (each its entry covers 2<sup>32</sup> bits) and the other two are interleaved.
///
/// It uses modified version of the structure described in the paper:
/// - Zhou D., Andersen D.G., Kaminsky M. (2013) "Space-Efficient, High-Performance Rank and Select Structures on Uncompressed Bit Sequences".
///   In: Bonifaci V., Demetrescu C., Marchetti-Spaccamela A. (eds) Experimental Algorithms. SEA 2013.
///   Lecture Notes in Computer Science, vol 7933. Springer, Berlin, Heidelberg. <https://doi.org/10.1007/978-3-642-38527-8_15>
/// 
/// The modification consists of different level 2 entries that hold 4 rank values (r0 <= r1 <= r2 <= r3) relative to level 1 entry.
/// The content of level 2 entry, listing from the least significant bits, is:
/// - original: r0 stored on 32 bits, r1-r0 on 10 bits, r2-r1 on 10 bits, r3-r2 on 10 bits;
/// - our: r0 stored on 32 bits, r3-r0 on 11 bits, r2-r0 on 11 bits, r1-r0 on 10 bits.
/// With this layout, we can read the corresponding value in the rank query without branching.
/// 
/// Another modification that makes our implementation unique is the ability of the select support structure to adapt
/// the sampling density to the content of the bit vector (see [`CombinedSampling`] and [`AdaptiveCombinedSamplingDensity`]).
/// 
/// For in-word selection, the structure uses the [`select64`] function.
#[derive(Clone)]
pub struct RankSelect101111<Select = BinaryRankSearch, Select0 = BinaryRankSearch, BV = Box::<[u64]>> {
    pub content: BV,  // bit vector
    #[cfg(target_pointer_width = "64")] pub l1ranks: Box<[usize]>,  // Each cell holds one rank using 64 bits
    pub l2ranks: Box<[u64]>,  // Each cell holds 4 ranks using [bits]: 32 (absolute), and, in reverse order (deltas): 10, 11, 11.
    select: Select,  // support for select (one)
    select0: Select0,  // support for select (zero)
}

impl<S, S0, BV> RankSelect101111<S, S0, BV> {
    /// Returns reference to structure that support select (one) operation.
    #[inline] pub fn select_support(&self) -> &S { &self.select }

    /// Returns reference to structure that support select zero operation.
    #[inline] pub fn select0_support(&self) -> &S0 { &self.select0 }
}

impl<S: GetSize, S0: GetSize, BV: GetSize> GetSize for RankSelect101111<S, S0, BV> {
    #[cfg(target_pointer_width = "64")]
    fn size_bytes_dyn(&self) -> usize {
        self.content.size_bytes_dyn() + self.l2ranks.size_bytes_dyn() + self.l1ranks.size_bytes_dyn() + self.select.size_bytes_dyn() + self.select0.size_bytes_dyn()
    }
    #[cfg(target_pointer_width = "32")]
    fn size_bytes_dyn(&self) -> usize {
        self.content.size_bytes_dyn() + self.l2ranks.size_bytes_dyn() + self.select.size_bytes_dyn() + self.select0.size_bytes_dyn()
    }
    const USES_DYN_MEM: bool = true;
}

impl<S: SelectForRank101111, S0: Select0ForRank101111, BV: Deref<Target = [u64]>> From<BV> for RankSelect101111<S, S0, BV> {
    #[inline] fn from(value: BV) -> Self { Self::build(value).0 }
}

impl<S: SelectForRank101111, S0, BV: Deref<Target = [u64]>> Select for RankSelect101111<S, S0, BV> {
    #[inline] fn try_select(&self, rank: usize) -> Option<usize> {
        self.select.select(&self.content, #[cfg(target_pointer_width = "64")] &self.l1ranks, &self.l2ranks, rank)
    }

    #[inline] unsafe fn select_unchecked(&self, rank: usize) -> usize {
        self.select.select_unchecked(&self.content, #[cfg(target_pointer_width = "64")] &self.l1ranks, &self.l2ranks, rank)
    }
}

impl<S, S0: Select0ForRank101111, BV: Deref<Target = [u64]>> Select0 for RankSelect101111<S, S0, BV> {
    #[inline] fn try_select0(&self, rank: usize) -> Option<usize> {
        self.select0.select0(&self.content, #[cfg(target_pointer_width = "64")] &self.l1ranks, &self.l2ranks, rank)
    }

    #[inline] unsafe fn select0_unchecked(&self, rank: usize) -> usize {
        self.select0.select0_unchecked(&self.content, #[cfg(target_pointer_width = "64")] &self.l1ranks, &self.l2ranks, rank)
    }
}

impl<S: SelectForRank101111, S0: Select0ForRank101111, BV: Deref<Target = [u64]>> Rank for RankSelect101111<S, S0, BV> {
    #[inline] fn try_rank(&self, index: usize) -> Option<usize> {
        let block = index / 512;
        let word_idx = index / 64;
        // we start from access to content, as if given index of content is not out of bounds,
        // then corresponding indices l1ranks and l2ranks are also not out of bound
        let mut r = (self.content.get(word_idx)? & n_lowest_bits(index as u8 % 64)).count_ones() as usize;
        let block_content = *unsafe{ self.l2ranks.get_unchecked(index/2048) };  // sound after returning Some by content.get(word_idx)
        #[cfg(target_pointer_width = "64")] { r += unsafe{ *self.l1ranks.get_unchecked(index >> 32) } + (block_content & 0xFFFFFFFFu64) as usize; } // 32 lowest bits   // for 34 bits: 0x3FFFFFFFFu64
        #[cfg(target_pointer_width = "32")] { r += (block_content & 0xFFFFFFFFu64) as usize; }

        r += (((block_content >> (11 * (!block & 3))) >> 32) & 0b1_11111_11111) as usize;

        //Some(r + count_bits_in(unsafe {self.content.get_unchecked(block * 8/*word_idx&!7*/..word_idx)}))
        Some(r + count_bits_in(unsafe {self.content.get_unchecked(word_idx&!7..word_idx)}))
    }

    #[inline] unsafe fn rank_unchecked(&self, index: usize) -> usize {
        let block = index / 512;
        let word_idx = index / 64;   
        let block_content = *unsafe{ self.l2ranks.get_unchecked(index/2048) };
        #[cfg(target_pointer_width = "64")] let mut r = *self.l1ranks.get_unchecked(index >> 32) + (block_content & 0xFFFFFFFFu64) as usize; // 32 lowest bits   // for 34 bits: 0x3FFFFFFFFu64
        #[cfg(target_pointer_width = "32")] let mut r = (block_content & 0xFFFFFFFFu64) as usize;
        r += (self.content.get_unchecked(word_idx) & n_lowest_bits(index as u8 % 64)).count_ones() as usize;

        //r += (((block_content>>32) >> (33 - 11 * (block & 3))) & 0b1_11111_11111) as usize;
        //r += (((block_content >> (33 - 11 * (block & 3))) >> 32) & 0b1_11111_11111) as usize;
        //if block & 3 != 0 { r += ((block_content >> ((32+33) - 11 * (block & 3))) & 0b1_11111_11111) as usize }
        //r += (((block_content >> 32) >> (11 * (3 - (block & 3)))) & 0b1_11111_11111) as usize;
        
        r += (((block_content >> (11 * (!block & 3))) >> 32) & 0b1_11111_11111) as usize;
        //r += (((block_content >> 32) >> (11 * (!block & 3))) & 0b1_11111_11111) as usize;

        //r + count_bits_in(self.content.get_unchecked(block * 8..word_idx))
        r + count_bits_in(self.content.get_unchecked(word_idx&!7..word_idx))
    }

    #[inline]
    fn prefetch(&self, index: usize) {
        let word_idx = index / 64;
        prefetch_index(&self.l2ranks, index / 2048);
        prefetch_index(&self.l1ranks, index >> 32);
        prefetch_index(&*self.content, word_idx);
    }
}

impl<S: SelectForRank101111, S0: Select0ForRank101111, BV: Deref<Target = [u64]>> RankSelect101111<S, S0, BV> {
    pub fn build(content: BV) -> (Self, usize) {
        #[cfg(target_pointer_width = "64")] let mut l1ranks = Vec::with_capacity(ceiling_div(content.len(), U64_PER_L1_ENTRY));
        let mut l2ranks = Vec::with_capacity(ceiling_div(content.len(), U64_PER_L2_ENTRY));
        let mut current_total_rank: usize = 0;
        for content in content.chunks(U64_PER_L1_ENTRY) {  // each l1 chunk has 1<<32 bits = (1<<32)/64 content elements
            #[cfg(target_pointer_width = "64")] l1ranks.push(current_total_rank);
            let mut current_rank: u64 = 0;
            for chunk in content.chunks(U64_PER_L2_ENTRY) {   // each chunk has 32*64 = 2048 bits
                let mut to_append = current_rank;
                let mut vals = chunk.chunks(U64_PER_L2_RECORDS).map(|c| count_bits_in(c)); // each val has 8*64 = 512 bits
                if let Some(v) = vals.next() {
                    let mut chunk_sum = v as u64;  // now chunk_sum uses up to 10 bits
                    to_append |= chunk_sum << (32+11+11);
                    if let Some(v) = vals.next() {
                        chunk_sum += v as u64;     // now chunk_sum uses up to 11 bits
                        to_append |= chunk_sum << (32+11);
                        if let Some(v) = vals.next() {
                            chunk_sum += v as u64;     // now chunk_sum uses up to 11 bits
                            to_append |= chunk_sum << 32;
                            if let Some(v) = vals.next() { chunk_sum += v as u64; }
                        } else {
                            to_append |= chunk_sum << 32;   // replication of the last chunk_sum in the last l2rank
                        }
                    } else {
                        to_append |= (chunk_sum << 32) | (chunk_sum << (32+11));    // replication of the last chunk_sum in the last l2rank
                    }
                    current_rank += chunk_sum;
                } //else { to_append |= (0 << 32) | (0 << (32+11)) | (0 << (32+22)); }
                l2ranks.push(to_append);
            }
            current_total_rank += current_rank as usize;
        }
        #[cfg(target_pointer_width = "64")] let l1ranks = l1ranks.into_boxed_slice();
        let l2ranks = l2ranks.into_boxed_slice();
        let select = S::new(&content, #[cfg(target_pointer_width = "64")] &l1ranks, &l2ranks, current_total_rank);
        let select0 = S0::new0(&content, #[cfg(target_pointer_width = "64")] &l1ranks, &l2ranks, current_total_rank);
        (Self{content, #[cfg(target_pointer_width = "64")] l1ranks, l2ranks, select, select0}, current_total_rank)
    }
}

impl<S: SelectForRank101111, S0: Select0ForRank101111, BV: Deref<Target = [u64]>> AsRef<[u64]> for RankSelect101111<S, S0, BV> {
    #[inline] fn as_ref(&self) -> &[u64] { &self.content }
}

/// Alias for backward compatibility. [`RankSelect101111`] with [`BinaryRankSearch`] (which does not introduce a space overhead) for select queries.
pub type ArrayWithRank101111 = RankSelect101111<BinaryRankSearch, BinaryRankSearch>;

/// The structure that holds array of bits `content` and `ranks` structure that takes no more than 6.25% extra space.
/// It can returns the number of ones in first `index` bits of the `content` (see `rank` method) in *O(1)* time.
/// Only `content` with less than 2<sup>32</sup> bit ones is supported.
/// Any type that implements the [`Deref`] trait with `Target = [u64]` can be used as a bit vector.
/// 
/// Usually [`RankSelect101111`] should be preferred to [`ArrayWithRankSimple`].
#[derive(Clone)]
pub struct RankSimple<BV = Box<[u64]>> {
    content: BV,
    ranks: Box<[u32]>,
}

impl<BV: GetSize> GetSize for RankSimple<BV> {
    fn size_bytes_dyn(&self) -> usize {
        self.content.size_bytes_dyn() + self.ranks.size_bytes_dyn()
    }
    const USES_DYN_MEM: bool = true;
}

impl<BV: Deref<Target = [u64]>> From<BV> for RankSimple<BV> {
    #[inline] fn from(value: BV) -> Self { Self::build(value).0 }
}

impl<BV: Deref<Target = [u64]>> RankSimple<BV> {

    /// Constructs `ArrayWithRankSimple` and count number of bits set in `content`. Returns both.
    pub fn build(content: BV) -> (Self, u32) {
        let mut result = Vec::with_capacity(ceiling_div(content.len(), 8usize));
        let mut current_rank: u32 = 0;
        for seg_nr in 0..content.len() {
            if seg_nr % 8 == 0 { result.push(current_rank); }
            current_rank += content[seg_nr].count_ones();
        }
        (Self{content, ranks: result.into_boxed_slice()}, current_rank)
    }

    pub fn try_rank(&self, index: usize) -> Option<u32> {
        let word_idx = index / 64;
        let word_offset = index as u8 % 64;
        let block = index / 512;
        let mut r = (self.content.get(word_idx)? & n_lowest_bits(word_offset)).count_ones() as u32;
        r += unsafe{self.ranks.get_unchecked(block)};   // sound after returning Some by content.get(word_idx)
        for w in block * (512 / 64)..word_idx {
            r += unsafe{self.content.get_unchecked(w)}.count_ones();    // sound after returning Some by content.get(word_idx)
        }
        Some(r)
    }

    pub fn rank(&self, index: usize) -> u32 {
        let word_idx = index / 64;
        let word_offset = index as u8 % 64;
        let block = index / 512;
        let mut r = self.ranks[block];
        for w in block * (512 / 64)..word_idx {
            r += self.content[w].count_ones();
        }
        r + (self.content[word_idx] & n_lowest_bits(word_offset)).count_ones() as u32
    }

    //pub fn select(&self, rank: u32) -> usize {}
}

impl<BV: Deref<Target = [u64]>> AsRef<[u64]> for RankSimple<BV> {
    #[inline] fn as_ref(&self) -> &[u64] { &self.content }
}

impl<BV: Deref<Target = [u64]>> Rank for RankSimple<BV> {
    #[inline] fn try_rank(&self, index: usize) -> Option<usize> {
        Self::try_rank(self, index).map(|r| r as usize)
    }

    #[inline] fn rank(&self, index: usize) -> usize {
        Self::rank(self, index) as usize
    }
}

/// Alias for backward compatibility.
pub type ArrayWithRankSimple = RankSimple;

//impl Select for ArrayWithRankSimple {}

#[cfg(test)]
mod tests {
    use crate::BitAccess;
    use super::*;

    fn check_all_ones<ArrayWithRank: AsRef<[u64]> + Rank + Select>(a: &ArrayWithRank) {
        let mut rank = 0;
        for index in a.as_ref().bit_ones() {
            assert_eq!(a.rank(index), rank, "rank({}) should be {}", index, rank);
            assert_eq!(a.select(rank), index, "select({}) should be {}", rank, index);
            assert_eq!(unsafe{a.rank_unchecked(index)}, rank, "rank({}) should be {}", index, rank);
            assert_eq!(unsafe{a.select_unchecked(rank)}, index, "select({}) should be {}", rank, index);
            //assert_eq!(a.try_rank(index), Some(rank), "rank({}) should be {}", index, rank);
            //assert_eq!(a.try_select(rank), Some(index), "select({}) should be {}", rank, index);
            rank += 1;
        }
        assert_eq!(a.try_select(rank), None, "select({}) should be None", rank);
    }

    fn check_all_zeros<ArrayWithRank: AsRef<[u64]> + Rank + Select0>(a: &ArrayWithRank) {
        let mut rank = 0;
        for index in a.as_ref().bit_zeros() {
            assert_eq!(a.rank0(index), rank, "rank0({}) should be {}", index, rank);
            assert_eq!(a.select0(rank), index, "select0({}) should be {}", rank, index);
            assert_eq!(unsafe{a.rank0_unchecked(index)}, rank, "rank0({}) should be {}", index, rank);
            assert_eq!(unsafe{a.select0_unchecked(rank)}, index, "select0({}) should be {}", rank, index);
            //assert_eq!(a.try_rank0(index), Some(rank), "rank0({}) should be {}", index, rank);
            //assert_eq!(a.try_select0(rank), Some(index), "select0({}) should be {}", rank, index);
            rank += 1;
        }
        assert_eq!(a.try_select0(rank), None, "select0({}) should be None", rank);
    }

    fn test_empty_array_rank<ArrayWithRank: From<Box<[u64]>> + AsRef<[u64]> + Rank + Select + Select0>() {
        let a: ArrayWithRank = vec![].into_boxed_slice().into();
        assert_eq!(a.try_rank(0), None);
        assert_eq!(a.try_select(0), None);
    }

    #[test]
    fn test_empty_array_rank_101111() {
        test_empty_array_rank::<ArrayWithRank101111>();
    }

    #[test]
    fn test_empty_array_rank_101111_combined() {
        test_empty_array_rank::<RankSelect101111::<CombinedSampling, CombinedSampling>>();
    }

    fn test_array_with_rank<ArrayWithRank: From<Box<[u64]>> + AsRef<[u64]> + Rank + Select + Select0>() {
        let a: ArrayWithRank = vec![0b1101, 0b110].into_boxed_slice().into();
        assert_eq!(a.try_select(0), Some(0));
        assert_eq!(a.try_select(1), Some(2));
        assert_eq!(a.try_select(2), Some(3));
        assert_eq!(a.try_select(3), Some(65));
        assert_eq!(a.try_select(4), Some(66));
        assert_eq!(a.try_select(5), None);
        #[cfg(target_pointer_width = "64")] assert_eq!(a.try_select(1+(1<<32)), None);
        #[cfg(target_pointer_width = "64")] assert_eq!(a.try_select(1+(1<<33)), None);
        assert_eq!(a.rank(0), 0);
        assert_eq!(a.rank(1), 1);
        assert_eq!(a.rank(2), 1);
        assert_eq!(a.rank(3), 2);
        assert_eq!(a.rank(4), 3);
        assert_eq!(a.rank(8), 3);
        assert_eq!(a.rank(64), 3);
        assert_eq!(a.rank(65), 3);
        assert_eq!(a.rank(66), 4);
        assert_eq!(a.rank(67), 5);
        assert_eq!(a.rank(70), 5);
        assert_eq!(a.try_rank(127), Some(5));
        assert_eq!(a.try_rank(128), None);
        #[cfg(target_pointer_width = "64")] assert_eq!(a.try_rank(1+(1<<32)), None);
        check_all_ones(&a);
        check_all_zeros(&a);
    }

    #[test]
    fn array_with_rank_101111() {
        test_array_with_rank::<ArrayWithRank101111>();
    }

    #[test]
    fn array_with_rank_101111_combined() {
        test_array_with_rank::<RankSelect101111::<CombinedSampling, CombinedSampling>>();
    }

    /*#[test]
    fn array_with_rank_simple() {
        test_array_with_rank::<ArrayWithRankSimple>();
    }*/

    fn test_big_array_with_rank<ArrayWithRank: From<Box<[u64]>> + AsRef<[u64]> + Rank + Select + Select0>() {
        let a: ArrayWithRank = vec![0b1101; 60].into_boxed_slice().into();
        assert_eq!(a.try_select0(488), Some(513));
        assert_eq!(a.try_select(0), Some(0));
        assert_eq!(a.try_select(1), Some(2));
        assert_eq!(a.try_select(2), Some(3));
        assert_eq!(a.try_select(3), Some(64));
        assert_eq!(a.try_select(4), Some(66));
        assert_eq!(a.try_select(5), Some(67));
        assert_eq!(a.try_select(6), Some(128));
        assert_eq!(a.try_select(7), Some(130));
        assert_eq!(a.try_select(3*8), Some(512));
        assert_eq!(a.try_select(3*8+1), Some(514));
        assert_eq!(a.try_select(2*6*8), Some(2*1024));
        assert_eq!(a.try_select(2*6*8+1), Some(2*1024+2));
        assert_eq!(a.try_select(2*6*8+2), Some(2*1024+3));
        assert_eq!(a.try_select(60*3), None);
        assert_eq!(a.rank(0), 0);
        assert_eq!(a.rank(1), 1);
        assert_eq!(a.rank(2), 1);
        assert_eq!(a.rank(3), 2);
        assert_eq!(a.rank(4), 3);
        assert_eq!(a.rank(8), 3);
        assert_eq!(a.rank(64), 3);
        assert_eq!(a.rank(65), 4);
        assert_eq!(a.rank(66), 4);
        assert_eq!(a.rank(67), 5);
        assert_eq!(a.rank(68), 6);
        assert_eq!(a.rank(69), 6);
        assert_eq!(a.rank(128), 6);
        assert_eq!(a.rank(129), 7);
        assert_eq!(a.rank(512), 3*8);
        assert_eq!(a.rank(513), 3*8+1);
        assert_eq!(a.rank(514), 3*8+1);
        assert_eq!(a.rank(515), 3*8+2);
        assert_eq!(a.rank(1024), 6*8);
        assert_eq!(a.rank(2*1024), 2*6*8);
        assert_eq!(a.rank(2*1024+1), 2*6*8+1);
        assert_eq!(a.rank(2*1024+2), 2*6*8+1);
        assert_eq!(a.rank(2*1024+3), 2*6*8+2);
        assert_eq!(a.try_rank(60*64), None);
        check_all_ones(&a);
        check_all_zeros(&a);
    }

    #[test]
    fn big_array_with_rank_101111() {
        test_big_array_with_rank::<ArrayWithRank101111>();
    }

    #[test]
    fn big_array_with_rank_101111_combined() {
        test_big_array_with_rank::<RankSelect101111::<CombinedSampling, CombinedSampling>>();
    }

    /*#[test]
    fn big_array_with_rank_simple() {
        test_big_array_with_rank::<ArrayWithRankSimple>();
    }*/

    fn test_content<ArrayWithRank: From<Box<[u64]>> + AsRef<[u64]> + Rank + Select + Select0>() {
        let a: ArrayWithRank = vec![u64::MAX; 35].into_boxed_slice().into();
        check_all_ones(&a);
        check_all_zeros(&a);
    }

    #[test]
    fn content_101111() {
        test_content::<ArrayWithRank101111>();
    }

    #[test]
    fn content_101111_combined() {
        test_content::<RankSelect101111::<CombinedSampling, CombinedSampling>>();
    }

    /*#[test]
    fn content_simple() {
        test_content::<ArrayWithRankSimple>();
    }*/

    #[cfg(target_pointer_width = "64")] 
    fn array_64bit<ArrayWithRank: From<Box<[u64]>> + AsRef<[u64]> + Rank + Select + Select0>() {
        const SEGMENTS: usize = (1<<32)/64 * 2;
        let a: ArrayWithRank = vec![0b01_01_01_01; SEGMENTS].into_boxed_slice().into();
        assert_eq!(a.try_select(268435456), Some(4294967296));
        assert_eq!(a.try_select(268435456+1), Some(4294967296+2));
        assert_eq!(a.try_select(268435456+2), Some(4294967296+4));
        assert_eq!(a.try_select(268435456+3), Some(4294967296+6));
        assert_eq!(a.try_select(0), Some(0));
        assert_eq!(a.try_select(1), Some(2));
        assert_eq!(a.rank(0), 0);
        assert_eq!(a.rank(1), 1);
        assert_eq!(a.rank(2), 1);
        assert_eq!(a.rank(1<<32), (1<<(32-6)) * 4);
        assert_eq!(a.rank((1<<32)+1), (1<<(32-6)) * 4 + 1);
        assert_eq!(a.rank((1<<32)+2), (1<<(32-6)) * 4 + 1);
        assert_eq!(a.rank((1<<32)+3), (1<<(32-6)) * 4 + 2);
        assert_eq!(a.try_rank(SEGMENTS*64), None);
        check_all_ones(&a);
        check_all_zeros(&a);
    }

    #[cfg(target_pointer_width = "64")] 
    #[test]
    #[ignore = "uses much memory and time"]
    fn array_64bit_101111_binary() {
        array_64bit::<ArrayWithRank101111>();
    }

    #[cfg(target_pointer_width = "64")] 
    #[test]
    #[ignore = "uses much memory and time"]
    fn array_64bit_101111_combined() {
        array_64bit::<RankSelect101111::<CombinedSampling, CombinedSampling>>();
    }

    #[cfg(target_pointer_width = "64")] 
    fn array_64bit_filled<ArrayWithRank: From<Box<[u64]>> + AsRef<[u64]> + Rank + Select>() {
        const SEGMENTS: usize = (1<<32)/64 * 2;
        let a: ArrayWithRank = vec![u64::MAX; SEGMENTS].into_boxed_slice().into();
        assert_eq!(a.select(4294965248), 4294965248);
        assert_eq!(a.rank(0), 0);
        assert_eq!(a.rank(1), 1);
        assert_eq!(a.rank(2), 2);
        for i in (1<<32)..(1<<32)+2048 {
            assert_eq!(a.rank(i), i);
            assert_eq!(a.select(i), i);
        }
        //check_all_ones(a);
    }

    #[cfg(target_pointer_width = "64")] 
    #[test]
    #[ignore = "uses much memory and time"]
    fn array_64bit_filled_101111() {
        array_64bit_filled::<ArrayWithRank101111>();
    }

    #[cfg(target_pointer_width = "64")] 
    #[test]
    #[ignore = "uses much memory and time"]
    fn array_64bit_filled_101111_combined() {
        array_64bit_filled::<RankSelect101111::<CombinedSampling, CombinedSampling>>();
    }

    #[cfg(target_pointer_width = "64")] 
    fn array_64bit_halffilled<ArrayWithRank: From<Box<[u64]>> + AsRef<[u64]> + Rank + Select + Select0>() {
        const SEGMENTS: usize = (1<<32)/64 * 2;
        let a: ArrayWithRank = vec![0x5555_5555_5555_5555; SEGMENTS].into_boxed_slice().into();
        check_all_ones(&a);
        check_all_zeros(&a);
    }

    #[cfg(target_pointer_width = "64")] 
    #[test]
    #[ignore = "uses much memory and time"]
    fn array_64bit_halffilled_101111_binary() {
        array_64bit_halffilled::<ArrayWithRank101111>();
    }

    #[cfg(target_pointer_width = "64")] 
    #[test]
    #[ignore = "uses much memory and time"]
    fn array_64bit_halffilled_101111_combined() {
        array_64bit_halffilled::<RankSelect101111::<CombinedSampling, CombinedSampling>>();
    }

    #[cfg(target_pointer_width = "64")] 
    fn array_64bit_zeroed_first<ArrayWithRank: From<Box<[u64]>> + AsRef<[u64]> + Rank + Select>() {
        const SEGMENTS: usize = (1<<32)/64 + 1;
        let mut content = vec![0; SEGMENTS].into_boxed_slice();
        content[SEGMENTS-1] = 0b11<<62;
        let a: ArrayWithRank = content.into();
        assert_eq!(a.rank(0), 0);
        assert_eq!(a.rank((1<<32)-1), 0);
        assert_eq!(a.rank(1<<32), 0);
        assert_eq!(a.rank((1<<32)+62), 0);
        assert_eq!(a.rank((1<<32)+63), 1);
        assert_eq!(a.select(0), (1<<32)+62);
        assert_eq!(a.select(1), (1<<32)+63);
        assert_eq!(a.try_select(2), None);
    }

    #[cfg(target_pointer_width = "64")] 
    #[test]
    #[ignore = "uses much memory and time"]
    fn array_64bit_zeroed_first_101111() {
        array_64bit_zeroed_first::<ArrayWithRank101111>();
    }

    #[cfg(target_pointer_width = "64")] 
    #[test]
    #[ignore = "uses much memory and time"]
    fn array_64bit_zeroed_first_101111_combined() {
        array_64bit_zeroed_first::<RankSelect101111::<CombinedSampling, CombinedSampling>>();
    }
}

/// Prefetch the cache line containing (the first byte of) `data[index]` into
/// all levels of the cache.
#[inline(always)]
fn prefetch_index<T>(data: impl AsRef<[T]>, index: usize) {
    let ptr = data.as_ref().as_ptr().wrapping_add(index) as *const i8;
    #[cfg(target_arch = "x86_64")]
    unsafe {
        std::arch::x86_64::_mm_prefetch(ptr, std::arch::x86_64::_MM_HINT_T0);
    }
    #[cfg(target_arch = "x86")]
    unsafe {
        std::arch::x86::_mm_prefetch(ptr, std::arch::x86::_MM_HINT_T0);
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        std::arch::aarch64::_prefetch(ptr, std::arch::aarch64::_PREFETCH_LOCALITY3);
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")))]
    {
        // Do nothing.
    }
}
