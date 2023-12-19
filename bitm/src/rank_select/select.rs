use dyn_size_of::GetSize;

use crate::ceiling_div;
#[cfg(target_arch = "x86")] use core::arch::x86 as arch;
#[cfg(target_arch = "x86_64")] use core::arch::x86_64 as arch;

use super::utils::partition_point_with_index;

pub const BITS_PER_L1_ENTRY: usize = 1<<32;
pub const U64_PER_L1_ENTRY: usize = 1<<(32-6);    // each l1 chunk has 1<<32 bits = (1<<32)/64 content (u64) elements
pub const U64_PER_L2_ENTRY: usize = 32;   // each l2 chunk has 32 content (u64) elements = 32*64 = 2048 bits
pub const BITS_PER_L2_ENTRY: usize = U64_PER_L2_ENTRY*64;   // each l2 chunk has 32 content (u64) elements = 32*64 = 2048 bits
pub const U64_PER_L2_RECORDS: usize = 8; // each l2 entry is splitted to 4, 8*64=512 bits records
pub const BITS_PER_L2_RECORDS: u64 = U64_PER_L2_RECORDS as u64 * 64; // each l2 entry is splitted to 4, 8*64=512 bits records
pub const L2_ENTRIES_PER_L1_ENTRY: usize = U64_PER_L1_ENTRY / U64_PER_L2_ENTRY;

/// Trait implemented by types that support select (one) operation,
/// i.e. can (quickly) find the position of the n-th one in the bitmap.
pub trait Select {
    /// Returns the position of the `rank`-th one (counting from 0) in `self` or [`None`] if there are no such many ones in `self`.
    fn try_select(&self, rank: usize) -> Option<usize>;

    /// Returns the position of the `rank`-th one (counting from 0) in `self` or panics if there are no such many ones in `self`.
    fn select(&self, rank: usize) -> usize {
        self.try_select(rank).expect("cannot select rank-th one as there are no such many ones")
    }

    /// Returns the position of the `rank`-th one (counting from 0) in `self`.
    /// The result is undefined if there are no such many ones in `self`.
    unsafe fn select_unchecked(&self, rank: usize) -> usize {
        self.select(rank)
    }
}

/// Trait implemented by types that support select zero operation,
/// i.e. can (quickly) find the position of the n-th zero in the bitmap.
pub trait Select0 {
    /// Returns the position of the `rank`-th zero (counting from 0) in `self` or [`None`] if there are no such many zeros in `self`.
    fn try_select0(&self, rank: usize) -> Option<usize>;

    /// Returns the position of the `rank`-th zero (counting from 0) in `self` or panics if there are no such many zeros in `self`.
    fn select0(&self, rank: usize) -> usize {
        self.try_select0(rank).expect("cannot select rank-th zero as there are no such many zeros")
    }

    /// Returns the position of the `rank`-th zero (counting from 0) in `self`.
    /// The result is undefined if there are no such many zeros in `self`.
    unsafe fn select0_unchecked(&self, rank: usize) -> usize {
        self.select0(rank)
    }
}

/// Trait implemented by strategies for select (ones) operations for `ArrayWithRank101111`.
pub trait SelectForRank101111 {
    fn new(content: &[u64], l1ranks: &[usize], l2ranks: &[u64], total_rank: usize) -> Self;
    fn select(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], rank: usize) -> Option<usize>;
}

/// Trait implemented by strategies for select zeros operations for `ArrayWithRank101111`.
pub trait Select0ForRank101111 {
    fn new0(content: &[u64], l1ranks: &[usize], l2ranks: &[u64], total_rank: usize) -> Self;
    fn select0(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], rank: usize) -> Option<usize>;
}

/// Returns the position of the `rank`-th (counting from 0) one in the bit representation of `n`,
/// i.e. the index of the one with the given rank.
/// 
/// On x86-64 CPU with the BMI2 instruction set, it uses the method described in:
/// - Prashant Pandey, Michael A. Bender, Rob Johnson, and Rob Patro,
///   "A General-Purpose Counting Filter: Making Every Bit Count",
///   In Proceedings of the 2017 ACM International Conference on Management of Data (SIGMOD '17).
///   Association for Computing Machinery, New York, NY, USA, 775–787. <https://doi.org/10.1145/3035918.3035963>
/// - Prashant Pandey, Michael A. Bender, Rob Johnson, "A Fast x86 Implementation of Select", arXiv:1706.00990
/// 
/// If BMI2 is not available, the implementation uses the broadword selection algorithm by Vigna, improved by Gog and Petri, and Vigna:
/// - Sebastiano Vigna, "Broadword Implementation of Rank/Select Queries", WEA, 2008
/// - Simon Gog, Matthias Petri, "Optimized succinct data structures for massive data". Software: Practice and Experience 44, 2014
/// - Sebastiano Vigna, The selection problem <https://sux4j.di.unimi.it/select.php> MG4J <http://mg4j.di.unimi.it/> and SUX <https://sux.di.unimi.it/>
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

/// A select strategy for [`ArrayWithRankSelect101111`](crate::ArrayWithRankSelect101111)
/// that does not introduce any overhead and is based on a binary search of the entries of rank structure.
#[derive(Clone, Copy)]
pub struct BinaryRankSearch;

impl GetSize for BinaryRankSearch {}

/// Find index of L1 chunk that contains `rank`-th one (or zero if `ONE` is `false`)
/// and decrease `rank` by number of ones (or zeros) in previous chunks.
#[inline] fn select_l1<const ONE: bool>(l1ranks: &[usize], rank: &mut usize) -> usize {
    if ONE {    // select 1:
        let i = l1ranks.partition_point(|v| v <= rank) - 1;
        *rank -= l1ranks[i];
        return i;
    } else {    // select 0:
        let i = partition_point_with_index(&l1ranks, |v, i| i * BITS_PER_L1_ENTRY - *v <= *rank) - 1;
        *rank -= i as usize * BITS_PER_L1_ENTRY - l1ranks[i];
        return i;
    }
}

/// Select from `l2ranks` entry pointed by `l2_index`, without bounds checking.
#[inline] unsafe fn select_from_l2<const ONE: bool>(content: &[u64], l2ranks: &[u64], l2_index: usize, mut rank: usize) -> Option<usize> {
    let mut l2_entry = *l2ranks.get_unchecked(l2_index);
    if ONE {
        rank -= (l2_entry & 0xFFFFFFFF) as usize;
    } else {
        rank -= (l2_index % L2_ENTRIES_PER_L1_ENTRY) * BITS_PER_L2_ENTRY - (l2_entry & 0xFFFFFFFF) as usize;
    }
    l2_entry >>= 32;
    let mut c = l2_index * U64_PER_L2_ENTRY;
    let to_subtract = if ONE { l2_entry & 0b1_11111_11111 } else { (3*BITS_PER_L2_RECORDS).wrapping_sub(l2_entry & 0b1_11111_11111) } as usize;
    if rank >= to_subtract {
        rank -= to_subtract;
        c += 3 * U64_PER_L2_RECORDS;
    } else {
        l2_entry >>= 11;
        let to_subtract = if ONE { l2_entry & 0b1_11111_11111 } else { (2*BITS_PER_L2_RECORDS).wrapping_sub(l2_entry & 0b1_11111_11111) } as usize;
        if rank >= to_subtract {
            rank -= to_subtract;
            c += 2 * U64_PER_L2_RECORDS;
        } else {
            l2_entry >>= 11;
            let to_subtract = if ONE { l2_entry } else { BITS_PER_L2_RECORDS.wrapping_sub(l2_entry) } as usize;
            if rank >= to_subtract {
                rank -= to_subtract;
                c += U64_PER_L2_RECORDS;
            }
        }
    };
    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse"))] { unsafe {
         arch::_mm_prefetch(content.as_ptr().wrapping_add(c) as *const i8, arch::_MM_HINT_NTA);
    } }
    for (i, v) in content.get(c..)?.iter().enumerate() {
        let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
        if ones <= rank {
            rank -= ones;
        } else {
            return Some((i+c) * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize);
        }
    }
    None
}

impl SelectForRank101111 for BinaryRankSearch {
    #[inline] fn new(_content: &[u64], _l1ranks: &[usize], _l2ranks: &[u64], _total_rank: usize) -> Self { Self }

    #[inline] fn select(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], mut rank: usize) -> Option<usize> {
        if l1ranks.is_empty() { return None; }
        let l1_index = select_l1::<true>(l1ranks, &mut rank);
        let l2_begin = l1_index * L2_ENTRIES_PER_L1_ENTRY;
        let l2_index = l2_begin +
            l2ranks[l2_begin..l2ranks.len().min(l2_begin+L2_ENTRIES_PER_L1_ENTRY)]
                .partition_point(|v| (v&0xFFFFFFFF) as usize <= rank) - 1;
        unsafe { select_from_l2::<true>(content, l2ranks, l2_index, rank) }
    }
}

impl Select0ForRank101111 for BinaryRankSearch {
    #[inline] fn new0(_content: &[u64], _l1ranks: &[usize], _l2ranks: &[u64], _total_rank: usize) -> Self { Self }

    #[inline] fn select0(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], mut rank: usize) -> Option<usize> {
        if l1ranks.is_empty() { return None; }
        let l1_index = select_l1::<false>(l1ranks, &mut rank);
        let l2_begin = l1_index * L2_ENTRIES_PER_L1_ENTRY;
        let l2_index = l2_begin +
            super::utils::partition_point_with_index(
                &l2ranks[l2_begin..l2ranks.len().min(l2_begin+L2_ENTRIES_PER_L1_ENTRY)],
                |v, i| i * BITS_PER_L2_ENTRY - (v&0xFFFFFFFF) as usize <= rank) - 1;
        unsafe { select_from_l2::<false>(content, l2ranks, l2_index, rank) }
    }
}

pub const ONES_PER_SELECT_ENTRY: usize = 8192;

/// A select strategy for [`ArrayWithRankSelect101111`](crate::ArrayWithRankSelect101111) with about 0.39% space overhead,
/// implemented according to the paper:
/// - Zhou D., Andersen D.G., Kaminsky M. (2013) "Space-Efficient, High-Performance Rank and Select Structures on Uncompressed Bit Sequences".
///   In: Bonifaci V., Demetrescu C., Marchetti-Spaccamela A. (eds) Experimental Algorithms. SEA 2013.
///   Lecture Notes in Computer Science, vol 7933. Springer, Berlin, Heidelberg. <https://doi.org/10.1007/978-3-642-38527-8_15>
#[derive(Clone)]
pub struct CombinedSampling {
    /// Bit indices (relative to level 1) of every [`ONES_PER_SELECT_ENTRY`]-th one (or zero in the case of select 0) in content, starting from the first one.
    select: Box<[u32]>,
    /// [`select_begin`] indices that begin descriptions of subsequent first-level entries.
    select_begin: Box<[usize]>,
}

impl GetSize for CombinedSampling {
    fn size_bytes_dyn(&self) -> usize { self.select.size_bytes_dyn() + self.select_begin.size_bytes_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl CombinedSampling {
    #[inline]
    fn new<const ONE: bool>(content: &[u64], l1ranks: &[usize], total_rank: usize) -> Self {
        if content.is_empty() { return Self{ select: Default::default(), select_begin: Default::default() } }
        let mut ones_positions_begin = Vec::with_capacity(l1ranks.len());
        let mut ones_positions_len = 0;
        ones_positions_begin.push(0);
        for ones in l1ranks.windows(2) {
            let chunk_len = ceiling_div(
                if ONE {ones[1] - ones[0]} else {BITS_PER_L1_ENTRY-(ones[1] - ones[0])},
                ONES_PER_SELECT_ENTRY);
            ones_positions_len += chunk_len;
            ones_positions_begin.push(ones_positions_len);
        }
        ones_positions_len += ceiling_div(
            if ONE {(total_rank - l1ranks.last().unwrap()) as usize }
            else { ((content.len()-1)%U64_PER_L1_ENTRY+1)*64 - (total_rank - l1ranks.last().unwrap()) as usize },
            ONES_PER_SELECT_ENTRY);
        let mut ones_positions = Vec::with_capacity(ones_positions_len);
        for content in content.chunks(U64_PER_L1_ENTRY) {
            //TODO use l2ranks for faster reducing rank
            let mut bit_index = 0;
            let mut rank = 0; /*ONES_PER_SELECT_ENTRY as u16 - 1;*/    // we scan for 1 with this rank, to find its bit index in content
            for c in content.iter().copied() {
                let c_ones = if ONE { c.count_ones() } else { c.count_zeros() } as u16;
                if c_ones <= rank {
                    rank -= c_ones;
                } else {
                    let new_rank = ONES_PER_SELECT_ENTRY as u16 - c_ones + rank;
                    ones_positions.push((bit_index + select64(if ONE {c} else {!c}, rank as u8) as u32) >> 11);    // each l2 entry covers 2^11 bits
                    rank = new_rank;
                }
                bit_index = bit_index.wrapping_add(64);
            }
        }
        debug_assert_eq!(ones_positions.len(), ones_positions_len);
        Self { select: ones_positions.into_boxed_slice(), select_begin: ones_positions_begin.into_boxed_slice() }
    }

    #[inline]
    fn select<const ONE: bool>(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], mut rank: usize) -> Option<usize> {
        if l1ranks.is_empty() { return None; }
        let l1_index = select_l1::<ONE>(l1ranks, &mut rank);
        let l2_begin = l1_index * L2_ENTRIES_PER_L1_ENTRY;
        //let l2ranks = &l2ranks[l2_begin..l2ranks.len().min(l2_begin+L2_ENTRIES_PER_L1_ENTRY)];
        let mut l2_index = l2_begin + self.select[self.select_begin[l1_index] + rank as usize / ONES_PER_SELECT_ENTRY] as usize;
        let l2_chunk_end = l2ranks.len().min(l2_begin+L2_ENTRIES_PER_L1_ENTRY);
        while l2_index+1 < l2_chunk_end &&
             if ONE {(l2ranks[l2_index+1] & 0xFF_FF_FF_FF) as usize}
             else {(l2_index+1-l2_begin) * BITS_PER_L2_ENTRY - (l2ranks[l2_index+1] & 0xFF_FF_FF_FF) as usize} <= rank
        {
            l2_index += 1;
        }
        unsafe { select_from_l2::<ONE>(content, l2ranks, l2_index, rank) }
    }
}

impl SelectForRank101111 for CombinedSampling {
    fn new(content: &[u64], l1ranks: &[usize], _l2ranks: &[u64], total_rank: usize) -> Self {
        Self::new::<true>(content, l1ranks, total_rank)
    }

    fn select(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], rank: usize) -> Option<usize> {
        Self::select::<true>(&self, content, l1ranks, l2ranks, rank)
    }
}

impl Select0ForRank101111 for CombinedSampling {
    fn new0(content: &[u64], l1ranks: &[usize], _l2ranks: &[u64], total_rank: usize) -> Self {
        Self::new::<false>(content, l1ranks, total_rank)
    }

    fn select0(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], rank: usize) -> Option<usize> {
        Self::select::<false>(&self, content, l1ranks, l2ranks, rank)
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