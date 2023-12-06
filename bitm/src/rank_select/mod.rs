mod select;
use self::select::{SimpleSelect, ArrayWithRank101111Select, U64_PER_L1_ENTRY, U64_PER_L2_ENTRY, U64_PER_L2_RECORDS};

use super::{ceiling_div, n_lowest_bits};
use dyn_size_of::GetSize;

/// The trait implemented by the types which holds the array of bits and the rank structure for this array.
/// Thanks to the rank structure, the implementor can quickly return the number of ones
/// in requested number of the first bits of the stored array (see `rank` method).
pub trait BitArrayWithRank {
    /// Returns `Self` (that stores `content` and the rank structure) and
    /// the number of bits set in the whole `content`.
    fn build(content: Box<[u64]>) -> (Self, u64) where Self: Sized;

    /// Returns the number of ones in first `index` bits of the `content`.
    fn rank(&self, index: usize) -> u64;
}

/// Returns number of bits set (to one) in `content`.
#[inline(always)] fn count_bits_in(content: &[u64]) -> u64 {
    content.iter().map(|v| v.count_ones() as u64).sum()
}

/// The structure that holds array of bits `content` and `ranks` structure that takes no more than 3.125% extra space.
/// It can return the number of ones in first `index` bits of the `content` (see `rank` method) in *O(1)* time.
///
/// It uses modified version of the structure described in the paper:
/// - Zhou D., Andersen D.G., Kaminsky M. (2013) "Space-Efficient, High-Performance Rank and Select Structures on Uncompressed Bit Sequences".
///   In: Bonifaci V., Demetrescu C., Marchetti-Spaccamela A. (eds) Experimental Algorithms. SEA 2013.
///   Lecture Notes in Computer Science, vol 7933. Springer, Berlin, Heidelberg. <https://doi.org/10.1007/978-3-642-38527-8_15>
/// 
/// The modification consists of different level 2 entries that hold 4 rank values (r0 <= r1 <= r2 <= r3) relative to level 1 entry.
/// The content of level 2 entry, listing from the least significant bits, is:
/// - original: r0 stored on 32 bits, r1-r0 on 10 bits, r2-r1 on 10 bits, r3-r2 on 10 bits;
/// - our: r0 stored on 32 bits, r3-r0 on 11 bits, r2-r0 on 11 bits, r1-r0 on 10 bits
///        (and unused fields in the last entry are filled with bit ones).
//TODO filled with bit ones??
#[derive(Clone)]
pub struct RankSelect101111<Select = SimpleSelect> {
    pub content: Box<[u64]>,  // BitVec
    pub l1ranks: Box<[u64]>,  // Each cell holds one rank using 64 bits
    pub l2ranks: Box<[u64]>,  // Each cell holds 4 ranks using [bits]: 32 (absolute), and, in reverse order (deltas): 10, 11, 11.
    select: Select  // support for select
}

impl<S: GetSize> GetSize for RankSelect101111<S> {
    fn size_bytes_dyn(&self) -> usize {
        self.content.size_bytes_dyn() + self.l2ranks.size_bytes_dyn() + self.l1ranks.size_bytes_dyn() + self.select.size_bytes_dyn()
    }
    const USES_DYN_MEM: bool = true;
}

impl<S: ArrayWithRank101111Select> BitArrayWithRank for RankSelect101111<S> {
    fn build(content: Box<[u64]>) -> (Self, u64) {
        let mut l1ranks = Vec::with_capacity(ceiling_div(content.len(), U64_PER_L1_ENTRY));
        let mut l2ranks = Vec::with_capacity(ceiling_div(content.len(), U64_PER_L2_ENTRY));
        let mut current_total_rank: u64 = 0;
        for content in content.chunks(U64_PER_L1_ENTRY) {  // each l1 chunk has 1<<32 bits = (1<<32)/64 content elements
            l1ranks.push(current_total_rank);
            let mut current_rank: u64 = 0;
            for chunk in content.chunks(U64_PER_L2_ENTRY) {   // each chunk has 32*64 = 2048 bits
                let mut to_append = current_rank;
                let mut vals = chunk.chunks(U64_PER_L2_RECORDS).map(|c| count_bits_in(c)); // each val has 8*64 = 512 bits
                if let Some(v) = vals.next() {
                    let mut chunk_sum = v;  // now chunk_sum uses up to 10 bits
                    to_append |= chunk_sum << (32+11+11);
                    if let Some(v) = vals.next() {
                        chunk_sum += v;     // now chunk_sum uses up to 11 bits
                        to_append |= chunk_sum << (32+11);
                        if let Some(v) = vals.next() {
                            chunk_sum += v;     // now chunk_sum uses up to 11 bits
                            to_append |= chunk_sum << 32;
                            if let Some(v) = vals.next() { chunk_sum += v; }
                        } else {
                            to_append |= ((1<<11)-1) << 32; // TODO powielic chunk_sum??
                        }
                    } else {
                        to_append |= ((1<<22)-1) << 32; // TODO powielic chunk_sum??
                    }
                    current_rank += chunk_sum;
                } else {
                    to_append |= 0xFF_FF_FF_FF << 32;   // TODO powielic chunk_sum??
                }
                l2ranks.push(to_append);
            }
            current_total_rank += current_rank;
        }
        let l1ranks = l1ranks.into_boxed_slice();
        let l2ranks = l2ranks.into_boxed_slice();
        let select = S::new(&content, &l1ranks, &l2ranks, current_total_rank);
        (Self{content, l1ranks, l2ranks, select}, current_total_rank)
    }

    fn rank(&self, index: usize) -> u64 {
        let block = index / 512;
        let mut block_content =  self.l2ranks[index/2048];//self.ranks[block/4];
        let mut r = unsafe{ *self.l1ranks.get_unchecked(index >> 32) } + (block_content & 0xFFFFFFFFu64); // 32 lowest bits   // for 34 bits: 0x3FFFFFFFFu64
        block_content >>= 32;   // remove the lowest 32 bits
        r += (block_content >> (33 - 11 * (block & 3))) & 0b1_11111_11111;
        let word_idx = index / 64;
        r += count_bits_in(&self.content[block * 8..word_idx]);
        /*for w in block * (512 / 64)..word_idx {
            r += self.content[w].count_ones() as u64;
        }*/
        r + (self.content[word_idx] & n_lowest_bits(index as u8 % 64)).count_ones() as u64
    }
}

pub type ArrayWithRank101111 = RankSelect101111<SimpleSelect>;

#[cfg(test)]
mod tests {
    use super::*;

    fn test_array_with_rank<ArrayWithRank: BitArrayWithRank>() {
        let (a, c) = ArrayWithRank::build(vec![0b1101, 0b110].into_boxed_slice());
        assert_eq!(c, 5);
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
    }

    #[test]
    fn array_with_rank_101111() {
        test_array_with_rank::<ArrayWithRank101111>();
    }

    fn test_big_array_with_rank<ArrayWithRank: BitArrayWithRank>() {
        let (a, c) = ArrayWithRank::build(vec![0b1101; 60].into_boxed_slice());
        assert_eq!(c, 60*3);
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
    }

    #[test]
    fn big_array_with_rank_101111() {
        test_big_array_with_rank::<ArrayWithRank101111>();
    }

    fn test_content<ArrayWithRank: BitArrayWithRank>() {
        let (a, c) = ArrayWithRank::build(vec![u64::MAX; 35].into_boxed_slice());
        assert_eq!(c, 35*64);
        for i in 0..35*64 {
            assert_eq!(i, a.rank(i) as usize);
        }
    }

    #[test]
    fn content_101111() {
        test_content::<ArrayWithRank101111>();
    }

    #[test]
    #[ignore = "uses much memory and time"]
    fn array_64bit() {
        const SEGMENTS: usize = 1<<(33-6);
        let (a, c) = ArrayWithRank101111::build(vec![0b01_01_01_01; SEGMENTS].into_boxed_slice());
        assert_eq!(c as usize, SEGMENTS * 4);
        assert_eq!(a.rank(0), 0);
        assert_eq!(a.rank(1), 1);
        assert_eq!(a.rank(2), 1);
        assert_eq!(a.rank(1<<32), (1<<(32-6)) * 4);
        assert_eq!(a.rank((1<<32)+1), (1<<(32-6)) * 4 + 1);
        assert_eq!(a.rank((1<<32)+2), (1<<(32-6)) * 4 + 1);
        assert_eq!(a.rank((1<<32)+3), (1<<(32-6)) * 4 + 2);
    }

    #[test]
    #[ignore = "uses much memory and time"]
    fn array_64bit_filled() {
        const SEGMENTS: usize = 1<<(33-6);
        let (a, c) = ArrayWithRank101111::build(vec![u64::MAX; SEGMENTS].into_boxed_slice());
        assert_eq!(c as usize, SEGMENTS * 64);
        assert_eq!(a.rank(0), 0);
        assert_eq!(a.rank(1), 1);
        assert_eq!(a.rank(2), 2);
        for i in (1<<32)..(1<<32)+2048 {
            assert_eq!(a.rank(i), i as u64);    
        }
    }
}
