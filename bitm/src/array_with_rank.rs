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

/// Returns the position of the `rank`-th one in the bit representation of `n`, i.e. the index of one with the given rank.
/// 
/// On x86-64 CPU with the BMI2 instruction set, it uses the method described in:
/// - Prashant Pandey, Michael A. Bender, Rob Johnson, and Rob Patro,
///   "A General-Purpose Counting Filter: Making Every Bit Count",
///   In Proceedings of the 2017 ACM International Conference on Management of Data (SIGMOD '17).
///   Association for Computing Machinery, New York, NY, USA, 775–787. https://doi.org/10.1145/3035918.3035963
/// - Prashant Pandey, Michael A. Bender, Rob Johnson, "A Fast x86 Implementation of Select", arXiv:1706.00990
/// 
/// If BMI2 is not available, the implementation uses the broadword selection algorithm by Vigna, improved by Gog and Petri, and Vigna:
/// - Sebastiano Vigna, "Broadword Implementation of Rank/Select Queries", WEA, 2008
/// - Simon Gog, Matthias Petri, "Optimized succinct data structures for massive data". Softw. Pract. Exper., 2014
/// - Sebastiano Vigna. MG4J 5.2.1. http://mg4j.di.unimi.it/ and SUX https://sux.di.unimi.it/
/// 
/// The implementation is based on the one contained in folly library by Meta.
#[inline] pub fn select64(n: u64, rank: u8) -> u8 {
    #[cfg(target_feature = "bmi2")]
    { unsafe { core::arch::x86_64::_pdep_u64(1u64 << rank, n) }.trailing_zeros() as u8 }
    #[cfg(not(target_feature = "bmi2"))] {
        use std::num::Wrapping as W;

        let rank = W(rank as u64);
        const ONES_STEP4: W<u64> = W(0x1111111111111111);
        const ONES_STEP8: W<u64> = W(0x0101010101010101);
        const MSB_STEP8: W<u64> = W(0x80 * ONES_STEP8.0);
    
        let mut s = W(n);
        s = s - ((s & W(0xA) * ONES_STEP4) >> 1);
        s = (s & W(0x3) * ONES_STEP4) + ((s >> 2) & W(0x3) * ONES_STEP4);
        s = (s + (s >> 4)) & W(0xF) * ONES_STEP8;
        let byte_sums = s * ONES_STEP8;
    
        let step8 = rank * ONES_STEP8;
        let geq_step8 = ((step8 | MSB_STEP8) - byte_sums) & MSB_STEP8;
        let place = geq_step8.0.count_ones() as u8 * 8;
        let byte_rank = rank.0 - (((byte_sums.0 << 8) >> place) & 0xFF);
        place + unsafe { SELECT_U8.get_unchecked((((n >> place) & 0xFF) | (byte_rank << 8)) as usize) } 

        /*let mut res = 0;
        sel_step(&mut res, &mut n, &mut r, 32);
        sel_step(&mut res, &mut n, &mut r, 16);
        sel_step(&mut res, &mut n, &mut r, 8);
        sel_step(&mut res, &mut n, &mut r, 4);
        sel_step(&mut res, &mut n, &mut r, 2);
        sel_step(&mut res, &mut n, &mut r, 1); res // OR let n = n as u8; res + (n & !(n>>1)) OR res + (n ^ r == 2) as u8*/
    }
}

/*#[inline(always)] fn select_step(res: &mut u8, n: &mut u64, r: &mut u8, shift: u8) {
    let mask = (1u64<<shift)-1;
    let o = (*n & mask).count_ones() as u8;
    if o < *r {
        *r -= o;
        *n >>= shift;
        *res += o;
    } else {
        *n &= mask;
    }
}*/
trait ArrayWithRank101111Select {
    fn new(content: &[u64], l1ranks: &[u64], l2ranks: &[u64], total_rank: u64) -> Self;
    fn select(&self, content: &[u64], l1ranks: &[u64], l2ranks: &[u64], rank: u64) -> u64;
}

struct SimpleSelect;

impl GetSize for SimpleSelect {}

/// Find index of L1 chunk that contains `rank`-th one and decrease `rank` by number of ones in previous chunks.
#[inline] fn select_l1(l1ranks: &[u64], rank: &mut u64) -> usize {
    for (i, v) in l1ranks.into_iter().copied().enumerate().rev() {
        if v <= *rank { // this must be true at least for i == 0, as then v == 0
            *rank -= v;
            return i;
        }
    }
    unreachable!()
}

impl ArrayWithRank101111Select for SimpleSelect {
    #[inline] fn new(_content: &[u64], _l1ranks: &[u64], _l2ranks: &[u64], _total_rank: u64) -> Self { Self }

    #[inline] fn select(&self, content: &[u64], l1ranks: &[u64], l2ranks: &[u64], mut rank: u64) -> u64 {
        let l1_index = select_l1(l1ranks, &mut rank);
        let l2_begin = l1_index * L2_ENTRIES_PER_L1_ENTRY;
        let l2_index = l2ranks[l2_begin..l2ranks.len().min(l2_begin+L2_ENTRIES_PER_L1_ENTRY)].partition_point(|v| v&0xFFFFFFFFu64 > rank) - 1;
        // note: partition_point cannot return 0 as at index 0, v>>32 is 0, and the condition is false for any rank
        let mut l2_entry = *unsafe { l2ranks.get_unchecked(l2_index) };  // safe as we subtracted 1 from partition_point result
        rank -= l2_entry & 0xFFFFFFFFu64;
        l2_entry >>= 32;
        let mut c = l2_index * U64_PER_L2_ENTRY;
        if rank >= l2_entry & 0b1_11111_11111 {
            rank -= l2_entry & 0b1_11111_11111;
            c += 3 * U64_PER_L2_RECORDS;
        } else {
            l2_entry >>= 11;
            if rank >= l2_entry & 0b1_11111_11111 {
                rank -= l2_entry & 0b1_11111_11111;
                c += 2 * U64_PER_L2_RECORDS;
            } else {
                l2_entry >>= 11;
                if rank >= l2_entry {
                    rank -= l2_entry;
                    c += U64_PER_L2_RECORDS;
                }
            }
        };
        for (i, v) in content[c..].iter().enumerate() {
            let ones = v.count_ones() as u64;
            if ones <= rank {
                rank -= ones;
            } else {
                return (i+c) as u64 * 64 + select64(*v, rank as u8) as u64;
            }
        }
        panic!()    // ??
    }
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
pub struct ArrayWithRank101111<Select = SimpleSelect> {
    pub content: Box<[u64]>,  // BitVec
    pub l1ranks: Box<[u64]>,  // Each cell holds one rank using 64 bits
    pub l2ranks: Box<[u64]>,  // Each cell holds 4 ranks using [bits]: 32 (absolute), and, in reverse order (deltas): 10, 11, 11.
    select: Select  // support for select
}

impl<S: GetSize> GetSize for ArrayWithRank101111<S> {
    fn size_bytes_dyn(&self) -> usize {
        self.content.size_bytes_dyn() + self.l2ranks.size_bytes_dyn() + self.l1ranks.size_bytes_dyn() + self.select.size_bytes_dyn()
    }
    const USES_DYN_MEM: bool = true;
}

const U64_PER_L1_ENTRY: usize = 1<<(32-6);    // each l1 chunk has 1<<32 bits = (1<<32)/64 content (u64) elements
const U64_PER_L2_ENTRY: usize = 32;   // each l2 chunk has 32 content (u64) elements = 32*64 = 2048 bits
const U64_PER_L2_RECORDS: usize = 8; // each l2 entry is splitted to 4, 8*64=512 bits records
const L2_ENTRIES_PER_L1_ENTRY: usize = U64_PER_L1_ENTRY / U64_PER_L2_ENTRY;

impl<S: ArrayWithRank101111Select> BitArrayWithRank for ArrayWithRank101111<S> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select() {
        assert_eq!(select64(1<<0, 0), 0);
        assert_eq!(select64(1<<1, 0), 1);
        assert_eq!(select64(1<<7, 0), 7);
        assert_eq!(select64(1<<12, 0), 12);
        assert_eq!(select64(1<<23, 0), 23);
        assert_eq!(select64(1<<31, 0), 31);
        assert_eq!(select64(1<<46, 0), 46);
        assert_eq!(select64(1<<53, 0), 53);
        assert_eq!(select64(1<<63, 0), 63);
        const N: u64 = (1<<2) | (1<<7) | (1<<15) | (1<<25) | (1<<33) | (1<<47) | (1<<60) | (1<<61);
        assert_eq!(select64(N, 0), 2);
        assert_eq!(select64(N, 1), 7);
        assert_eq!(select64(N, 2), 15);
        assert_eq!(select64(N, 3), 25);
        assert_eq!(select64(N, 4), 33);
        assert_eq!(select64(N, 5), 47);
        assert_eq!(select64(N, 6), 60);
        assert_eq!(select64(N, 6), 61);
    }

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


/// For any n<256 and rank<8, the value at index 256*rank+n is the index of the (rank+1)-th one in the bit representation of n, or 8.
#[cfg(not(target_feature = "bmi2"))] const SELECT_U8: [u8; 2048] = [
    8,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,4,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,5,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,4,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,
    6,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,4,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,5,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,4,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,
    7,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,4,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,5,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,4,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,
    6,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,4,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,5,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,4,0,1,0,2,0,1,0,3,0,1,0,2,0,1,0,
    8,8,8,1,8,2,2,1,8,3,3,1,3,2,2,1,8,4,4,1,4,2,2,1,4,3,3,1,3,2,2,1,8,5,5,1,5,2,2,1,5,3,3,1,3,2,2,1,5,4,4,1,4,2,2,1,4,3,3,1,3,2,2,1,
    8,6,6,1,6,2,2,1,6,3,3,1,3,2,2,1,6,4,4,1,4,2,2,1,4,3,3,1,3,2,2,1,6,5,5,1,5,2,2,1,5,3,3,1,3,2,2,1,5,4,4,1,4,2,2,1,4,3,3,1,3,2,2,1,
    8,7,7,1,7,2,2,1,7,3,3,1,3,2,2,1,7,4,4,1,4,2,2,1,4,3,3,1,3,2,2,1,7,5,5,1,5,2,2,1,5,3,3,1,3,2,2,1,5,4,4,1,4,2,2,1,4,3,3,1,3,2,2,1,
    7,6,6,1,6,2,2,1,6,3,3,1,3,2,2,1,6,4,4,1,4,2,2,1,4,3,3,1,3,2,2,1,6,5,5,1,5,2,2,1,5,3,3,1,3,2,2,1,5,4,4,1,4,2,2,1,4,3,3,1,3,2,2,1,
    8,8,8,8,8,8,8,2,8,8,8,3,8,3,3,2,8,8,8,4,8,4,4,2,8,4,4,3,4,3,3,2,8,8,8,5,8,5,5,2,8,5,5,3,5,3,3,2,8,5,5,4,5,4,4,2,5,4,4,3,4,3,3,2,
    8,8,8,6,8,6,6,2,8,6,6,3,6,3,3,2,8,6,6,4,6,4,4,2,6,4,4,3,4,3,3,2,8,6,6,5,6,5,5,2,6,5,5,3,5,3,3,2,6,5,5,4,5,4,4,2,5,4,4,3,4,3,3,2,
    8,8,8,7,8,7,7,2,8,7,7,3,7,3,3,2,8,7,7,4,7,4,4,2,7,4,4,3,4,3,3,2,8,7,7,5,7,5,5,2,7,5,5,3,5,3,3,2,7,5,5,4,5,4,4,2,5,4,4,3,4,3,3,2,
    8,7,7,6,7,6,6,2,7,6,6,3,6,3,3,2,7,6,6,4,6,4,4,2,6,4,4,3,4,3,3,2,7,6,6,5,6,5,5,2,6,5,5,3,5,3,3,2,6,5,5,4,5,4,4,2,5,4,4,3,4,3,3,2,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,3,8,8,8,8,8,8,8,4,8,8,8,4,8,4,4,3,8,8,8,8,8,8,8,5,8,8,8,5,8,5,5,3,8,8,8,5,8,5,5,4,8,5,5,4,5,4,4,3,
    8,8,8,8,8,8,8,6,8,8,8,6,8,6,6,3,8,8,8,6,8,6,6,4,8,6,6,4,6,4,4,3,8,8,8,6,8,6,6,5,8,6,6,5,6,5,5,3,8,6,6,5,6,5,5,4,6,5,5,4,5,4,4,3,
    8,8,8,8,8,8,8,7,8,8,8,7,8,7,7,3,8,8,8,7,8,7,7,4,8,7,7,4,7,4,4,3,8,8,8,7,8,7,7,5,8,7,7,5,7,5,5,3,8,7,7,5,7,5,5,4,7,5,5,4,5,4,4,3,
    8,8,8,7,8,7,7,6,8,7,7,6,7,6,6,3,8,7,7,6,7,6,6,4,7,6,6,4,6,4,4,3,8,7,7,6,7,6,6,5,7,6,6,5,6,5,5,3,7,6,6,5,6,5,5,4,6,5,5,4,5,4,4,3,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,4,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,5,8,8,8,8,8,8,8,5,8,8,8,5,8,5,5,4,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,6,8,8,8,8,8,8,8,6,8,8,8,6,8,6,6,4,8,8,8,8,8,8,8,6,8,8,8,6,8,6,6,5,8,8,8,6,8,6,6,5,8,6,6,5,6,5,5,4,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,7,8,8,8,8,8,8,8,7,8,8,8,7,8,7,7,4,8,8,8,8,8,8,8,7,8,8,8,7,8,7,7,5,8,8,8,7,8,7,7,5,8,7,7,5,7,5,5,4,
    8,8,8,8,8,8,8,7,8,8,8,7,8,7,7,6,8,8,8,7,8,7,7,6,8,7,7,6,7,6,6,4,8,8,8,7,8,7,7,6,8,7,7,6,7,6,6,5,8,7,7,6,7,6,6,5,7,6,6,5,6,5,5,4,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,5,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,6,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,6,8,8,8,8,8,8,8,6,8,8,8,6,8,6,6,5,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,7,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,7,8,8,8,8,8,8,8,7,8,8,8,7,8,7,7,5,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,7,8,8,8,8,8,8,8,7,8,8,8,7,8,7,7,6,8,8,8,8,8,8,8,7,8,8,8,7,8,7,7,6,8,8,8,7,8,7,7,6,8,7,7,6,7,6,6,5,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,6,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,7,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,7,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,7,8,8,8,8,8,8,8,7,8,8,8,7,8,7,7,6,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,
    8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,8,7
];  // code for calculation is at https://github.com/facebook/folly/blob/main/folly/experimental/Select64.cpp