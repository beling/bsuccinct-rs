use dyn_size_of::GetSize;

use crate::ceiling_div;
#[cfg(target_arch = "x86")] use core::arch::x86 as arch;
#[cfg(target_arch = "x86_64")] use core::arch::x86_64 as arch;

pub const U64_PER_L1_ENTRY: usize = 1<<(32-6);    // each l1 chunk has 1<<32 bits = (1<<32)/64 content (u64) elements
pub const U64_PER_L2_ENTRY: usize = 32;   // each l2 chunk has 32 content (u64) elements = 32*64 = 2048 bits
pub const U64_PER_L2_RECORDS: usize = 8; // each l2 entry is splitted to 4, 8*64=512 bits records
pub const L2_ENTRIES_PER_L1_ENTRY: usize = U64_PER_L1_ENTRY / U64_PER_L2_ENTRY;

/// Trait implemented by types that support select operation,
/// i.e. can quickly find the position of the n-th one in the bitmap.
pub trait Select {
    /// Returns the position of the `rank`-th one in `self` or `None` if there are no such many ones in `self`.
    fn try_select(&self, rank: u64) -> Option<u64>;

    /// Returns the position of the `rank`-th one in `self` or panics if there are no such many ones in `self`.
    fn select(&self, rank: u64) -> u64 {
        self.try_select(rank).expect("cannot select rank-th one as there are no such many ones")
    }
}

/// Trait implemented by strategies for select operations for `ArrayWithRank101111`.
pub trait ArrayWithRank101111Select {
    fn new(content: &[u64], l1ranks: &[u64], l2ranks: &[u64], total_rank: u64) -> Self;
    fn select(&self, content: &[u64], l1ranks: &[u64], l2ranks: &[u64], rank: u64) -> Option<u64>;
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
    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "bmi2"))]
    { unsafe { arch::_pdep_u64(1u64 << rank, n) }.trailing_zeros() as u8 }
    #[cfg(not(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "bmi2")))] {
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

/// A select strategy for [`RankSelect101111`] that does not introduce any overhead
/// and is based on a binary search of the rank structure.
#[derive(Clone, Copy)]
pub struct BinarySearchSelect;

impl GetSize for BinarySearchSelect {}

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

/// Select from `l2ranks` entry pointed by `l2_index`, without bounds checking.
#[inline] unsafe fn select_from_l2(content: &[u64], l2ranks: &[u64], l2_index: usize, mut rank: u64) -> Option<u64> {
    let mut l2_entry = *l2ranks.get_unchecked(l2_index);
    rank -= l2_entry & 0xFFFFFFFF;
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
    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse"))] { unsafe {
         arch::_mm_prefetch(content.as_ptr().wrapping_add(c) as *const i8, arch::_MM_HINT_NTA);
    } }
    for (i, v) in content.get(c..)?.iter().enumerate() {
        let ones = v.count_ones() as u64;
        if ones <= rank {
            rank -= ones;
        } else {
            return Some((i+c) as u64 * 64 + select64(*v, rank as u8) as u64);
        }
    }
    None
}

impl ArrayWithRank101111Select for BinarySearchSelect {
    #[inline] fn new(_content: &[u64], _l1ranks: &[u64], _l2ranks: &[u64], _total_rank: u64) -> Self { Self }

    #[inline] fn select(&self, content: &[u64], l1ranks: &[u64], l2ranks: &[u64], mut rank: u64) -> Option<u64> {
        let l1_index = select_l1(l1ranks, &mut rank);
        let l2_begin = l1_index * L2_ENTRIES_PER_L1_ENTRY;
        //let l2ranks = &l2ranks[l2_begin..l2ranks.len().min(l2_begin+L2_ENTRIES_PER_L1_ENTRY)];
        //let l2_index = l2ranks.partition_point(|v| v&0xFFFFFFFF <= rank) - 1;
        //unsafe { select_from_l2(content, l2ranks, l2_index, rank) }

        let l2_index = l2_begin+l2ranks[l2_begin..l2ranks.len().min(l2_begin+L2_ENTRIES_PER_L1_ENTRY)].partition_point(|v| v&0xFFFFFFFF <= rank) - 1;
        // note: partition_point cannot return 0 as at index 0, v&0xFFFFFFFF is 0, and the condition is true for any rank
        // so l2_index is in bound, as we subtracted 1 from partition_point result
        unsafe { select_from_l2(content, l2ranks, l2_index, rank) }
    }
}

pub const ONES_PER_SELECT_ENTRY: usize = 8192;

/// A select strategy for [`RankSelect101111`] proposed in:
/// - Zhou D., Andersen D.G., Kaminsky M. (2013) "Space-Efficient, High-Performance Rank and Select Structures on Uncompressed Bit Sequences".
///   In: Bonifaci V., Demetrescu C., Marchetti-Spaccamela A. (eds) Experimental Algorithms. SEA 2013.
///   Lecture Notes in Computer Science, vol 7933. Springer, Berlin, Heidelberg. <https://doi.org/10.1007/978-3-642-38527-8_15>
#[derive(Clone)]
pub struct CombinedSamplingSelect {
    /// Bit indices (relative to level 1) of every [`ONES_PER_SELECT_ENTRY`]-th one in content.
    ones_positions: Box<[u32]>,
    /// `ones_positions` indices that begin descriptions of subsequent first-level entries.
    ones_positions_begin: Box<[usize]>,
}

impl GetSize for CombinedSamplingSelect {}

impl ArrayWithRank101111Select for CombinedSamplingSelect {
    fn new(content: &[u64], l1ranks: &[u64], _l2ranks: &[u64], total_rank: u64) -> Self {
        let mut ones_positions_begin = Vec::with_capacity(l1ranks.len());
        let mut ones_positions_len = 0;
        ones_positions_begin.push(0);
        for ones in l1ranks.windows(2) {
            let chunk_len = ceiling_div((ones[1] - ones[0]) as usize, ONES_PER_SELECT_ENTRY);
            ones_positions_len += chunk_len;
            ones_positions_begin.push(ones_positions_len);
        }
        ones_positions_len += ceiling_div((total_rank - l1ranks.last().unwrap()) as usize, ONES_PER_SELECT_ENTRY);
        let mut ones_positions = Vec::with_capacity(ones_positions_len);
        for content in content.chunks(U64_PER_L1_ENTRY) {
            let mut bit_index = 0;
            let mut rank = 0; /*ONES_PER_SELECT_ENTRY as u16 - 1;*/    // we scan for 1 with this rank, to find its bit index in content
            for c in content.iter().copied() {
                let c_ones = c.count_ones() as u16;
                if c_ones <= rank {
                    rank -= c_ones;
                } else {
                    let new_rank = ONES_PER_SELECT_ENTRY as u16 - c_ones + rank;
                    ones_positions.push((bit_index + select64(c, rank as u8) as u32) >> 11);    // each l2 entry covers 2^11 bits
                    rank = new_rank;
                }
                bit_index = bit_index.wrapping_add(64);
            }
        }
        debug_assert_eq!(ones_positions.len(), ones_positions_len);
        Self { ones_positions: ones_positions.into_boxed_slice(), ones_positions_begin: ones_positions_begin.into_boxed_slice() }
    }

    fn select(&self, content: &[u64], l1ranks: &[u64], l2ranks: &[u64], mut rank: u64) -> Option<u64> {
        let l1_index = select_l1(l1ranks, &mut rank);
        let l2_begin = l1_index * L2_ENTRIES_PER_L1_ENTRY;
        //let l2ranks = &l2ranks[l2_begin..l2ranks.len().min(l2_begin+L2_ENTRIES_PER_L1_ENTRY)];
        let mut l2_index = l2_begin + self.ones_positions[self.ones_positions_begin[l1_index] + rank as usize / ONES_PER_SELECT_ENTRY] as usize;
        debug_assert!(l2ranks[l2_index] & 0xFF_FF_FF_FF <= rank, "{} {} {} {}", l2_begin, l2_index, l2ranks[l2_index] & 0xFF_FF_FF_FF, rank);
        while l2_index+1 < l2ranks.len() && (l2ranks[l2_index+1] & 0xFF_FF_FF_FF) <= rank {
            l2_index += 1;
        }
        unsafe { select_from_l2(content, l2ranks, l2_index, rank) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select64() {
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
        assert_eq!(select64(N, 7), 61);
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