use dyn_size_of::GetSize;

#[cfg(all(target_arch = "x86", target_feature = "bmi2"))] use core::arch::x86 as arch;
#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))] use core::arch::x86_64 as arch;
//#[cfg(target_arch = "x86")] use core::arch::x86 as arch;
//#[cfg(target_arch = "x86_64")] use core::arch::x86_64 as arch;

use super::utils::partition_point_with_index;

pub const BITS_PER_L1_ENTRY: usize = 1<<32;
pub const U64_PER_L1_ENTRY: usize = 1<<(32-6);    // each l1 chunk has 1<<32 bits = (1<<32)/64 content (u64) elements
pub const U64_PER_L2_ENTRY: usize = 32;   // each l2 chunk has 32 content (u64) elements = 32*64 = 2048 bits
pub const BITS_PER_L2_ENTRY: usize = U64_PER_L2_ENTRY*64;   // each l2 chunk has 32 content (u64) elements = 32*64 = 2048 bits
pub const U64_PER_L2_RECORDS: usize = 8; // each l2 entry is splitted to 4, 8*64=512 bits records
pub const BITS_PER_L2_RECORDS: u64 = U64_PER_L2_RECORDS as u64 * 64; // each l2 entry is splitted to 4, 8*64=512 bits records
pub const L2_ENTRIES_PER_L1_ENTRY: usize = U64_PER_L1_ENTRY / U64_PER_L2_ENTRY;

/// Trait implemented by the types that support select (one) queries,
/// i.e. can (quickly) find the position of the n-th one in the bitmap.
pub trait Select {
    /// Returns the position of the `rank`-th one (counting from 0) in `self` or [`None`] if there are no such many ones in `self`.
    fn try_select(&self, rank: usize) -> Option<usize>;

    /// Returns the position of the `rank`-th one (counting from 0) in `self` or panics if there are no such many ones in `self`.
    #[inline(always)] fn select(&self, rank: usize) -> usize {
        self.try_select(rank).expect("cannot select rank-th one as there are no such many ones")
    }

    /// Returns the position of the `rank`-th one (counting from 0) in `self`.
    /// The result is undefined if there are no such many ones in `self`.
    #[inline(always)] unsafe fn select_unchecked(&self, rank: usize) -> usize {
        self.select(rank)
    }
}

/// Trait implemented by the types that support select zero queries,
/// i.e. can (quickly) find the position of the n-th zero in the bitmap.
pub trait Select0 {
    /// Returns the position of the `rank`-th zero (counting from 0) in `self` or [`None`] if there are no such many zeros in `self`.
    fn try_select0(&self, rank: usize) -> Option<usize>;

    /// Returns the position of the `rank`-th zero (counting from 0) in `self` or panics if there are no such many zeros in `self`.
    #[inline(always)] fn select0(&self, rank: usize) -> usize {
        self.try_select0(rank).expect("cannot select rank-th zero as there are no such many zeros")
    }

    /// Returns the position of the `rank`-th zero (counting from 0) in `self`.
    /// The result is undefined if there are no such many zeros in `self`.
    #[inline(always)] unsafe fn select0_unchecked(&self, rank: usize) -> usize {
        self.select0(rank)
    }
}

/// Trait implemented by strategies for select (ones) operations for `ArrayWithRank101111`.
pub trait SelectForRank101111 {
    fn new(content: &[u64], l1ranks: &[usize], l2ranks: &[u64], total_rank: usize) -> Self;

    fn select(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], rank: usize) -> Option<usize>;

    #[inline(always)] unsafe fn select_unchecked(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], rank: usize) -> usize {
        self.select(content, l1ranks, l2ranks, rank).unwrap_unchecked()
    }
}

/// Trait implemented by strategies for select zeros operations for `ArrayWithRank101111`.
pub trait Select0ForRank101111 {
    fn new0(content: &[u64], l1ranks: &[usize], l2ranks: &[u64], total_rank: usize) -> Self;

    fn select0(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], rank: usize) -> Option<usize>;

    #[inline(always)] unsafe fn select0_unchecked(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], rank: usize) -> usize {
        self.select0(content, l1ranks, l2ranks, rank).unwrap_unchecked()
    }
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
/// - Sebastiano Vigna, The selection problem <https://sux4j.di.unimi.it/select.php>
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
/// that does not introduce any overhead (on space or construction speed)
/// and is based on a binary search over ranks.
#[derive(Clone, Copy)]
pub struct BinaryRankSearch;

impl GetSize for BinaryRankSearch {}

/// Find index of L1 chunk that contains `rank`-th one (or zero if `ONE` is `false`)
/// and decrease `rank` by number of ones (or zeros) in previous chunks.
#[inline] fn select_l1<const ONE: bool>(l1ranks: &[usize], rank: &mut usize) -> usize {
    if ONE {    // select 1:
        let i = l1ranks.partition_point(|v| v <= rank) - 1;
        *rank -= unsafe{l1ranks.get_unchecked(i)};
        return i;
    } else {    // select 0:
        let i = partition_point_with_index(&l1ranks, |v, i| i * BITS_PER_L1_ENTRY - *v <= *rank) - 1;
        *rank -= i as usize * BITS_PER_L1_ENTRY - unsafe{l1ranks.get_unchecked(i)};
        return i;
    }
}

/*#[inline(always)] fn consider_l2entry<const ONE: bool>(mut l2_entry: u64, rank: &mut usize) -> usize {
    l2_entry >>= 32;
    let to_subtract = if ONE { l2_entry & 0b1_11111_11111 } else { (3*BITS_PER_L2_RECORDS).wrapping_sub(l2_entry & 0b1_11111_11111) } as usize;
    return if *rank >= to_subtract {
        *rank -= to_subtract;
         3 * U64_PER_L2_RECORDS
    } else {
        l2_entry >>= 11;
        let to_subtract = if ONE { l2_entry & 0b1_11111_11111 } else { (2*BITS_PER_L2_RECORDS).wrapping_sub(l2_entry & 0b1_11111_11111) } as usize;
        if *rank >= to_subtract {
            *rank -= to_subtract;
            2 * U64_PER_L2_RECORDS
        } else {
            l2_entry >>= 11;
            let to_subtract = if ONE { l2_entry } else { BITS_PER_L2_RECORDS.wrapping_sub(l2_entry) } as usize;
            if *rank >= to_subtract {
                *rank -= to_subtract;
                U64_PER_L2_RECORDS
            } else { 0 }
        }
    };
}*/

#[inline(always)] fn consider_l2entry<const ONE: bool>(l2_index: usize, l2_entry: u64, rank: &mut usize) -> usize {
    if ONE {
        *rank -= (l2_entry & 0xFFFFFFFF) as usize;
    } else {
        *rank -= (l2_index % L2_ENTRIES_PER_L1_ENTRY) * BITS_PER_L2_ENTRY - (l2_entry & 0xFFFFFFFF) as usize;
    }
    let to_subtract = if ONE { (l2_entry>>(32+11)) & 0b1_11111_11111 }
        else { (2*BITS_PER_L2_RECORDS).wrapping_sub((l2_entry>>(32+11)) & 0b1_11111_11111) } as usize;
    if *rank >= to_subtract {
        let to_subtract_more = if ONE { (l2_entry>>32) & 0b1_11111_11111 }
            else { (3*BITS_PER_L2_RECORDS).wrapping_sub((l2_entry>>32) & 0b1_11111_11111) } as usize;        
        if *rank >= to_subtract_more {
            *rank -= to_subtract_more;
            3 * U64_PER_L2_RECORDS
        } else {
            *rank -= to_subtract;
            2 * U64_PER_L2_RECORDS
        }
    } else {
        let to_subtract = if ONE { l2_entry>>(32+22) }
            else { BITS_PER_L2_RECORDS.wrapping_sub(l2_entry>>(32+22)) } as usize;
        if *rank >= to_subtract {
            *rank -= to_subtract;
            U64_PER_L2_RECORDS
        } else { 0 }
    }
}

/// Select from `l2ranks` entry pointed by `l2_index`, without `l2_entry` entry bounds checking.
#[inline(always)] unsafe fn select_from_l2_unchecked<const ONE: bool>(content: &[u64], l2ranks: &[u64], l2_index: usize, mut rank: usize) -> usize {
    let l2_entry = *l2ranks.get_unchecked(l2_index);
    let mut c = l2_index * U64_PER_L2_ENTRY + consider_l2entry::<ONE>(l2_index, l2_entry, &mut rank);
    
    let v = unsafe{content.get_unchecked(c)};   // 0
    let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
    if ones > rank { return c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize; }
    rank -= ones; c += 1;

    let v = unsafe{content.get_unchecked(c)};   // 1
    let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
    if ones > rank { return c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize; }
    rank -= ones; c += 1;

    let v = unsafe{content.get_unchecked(c)};   // 2
    let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
    if ones > rank { return c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize; }
    rank -= ones; c += 1;

    let v = unsafe{content.get_unchecked(c)};   // 3
    let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
    if ones > rank { return c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize; }
    rank -= ones; c += 1;

    let v = unsafe{content.get_unchecked(c)};   // 4
    let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
    if ones > rank { return c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize; }
    rank -= ones; c += 1;

    let v = unsafe{content.get_unchecked(c)};   // 5
    let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
    if ones > rank { return c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize; }
    rank -= ones; c += 1;

    let v = unsafe{content.get_unchecked(c)};   // 6
    let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
    if ones > rank { return c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize; }
    rank -= ones; c += 1;

    let v = unsafe{content.get_unchecked(c)};   // 7
    c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize
}

/// Select from `l2ranks` entry pointed by `l2_index`, without `l2_entry` entry bounds checking.
#[inline(always)] unsafe fn select_from_l2<const ONE: bool>(content: &[u64], l2ranks: &[u64], l2_index: usize, mut rank: usize) -> Option<usize> {
    let l2_entry = *l2ranks.get_unchecked(l2_index);
    let mut c = l2_index * U64_PER_L2_ENTRY + consider_l2entry::<ONE>(l2_index, l2_entry, &mut rank);
    /*#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse"))] { unsafe {
         arch::_mm_prefetch(content.as_ptr().wrapping_add(c) as *const i8, arch::_MM_HINT_NTA);
    } }*/
    //for (i, v) in content.get(c..)?.iter().enumerate() {    // TODO select512, here we can unroll the loop for upto 7 iterations
    /*for i in 0..8 {
        let v = content.get(c+i)?;
        let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
        if ones <= rank { rank -= ones;
        } else { return Some((c+i) * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize); }
    }
    None*/
    let v = content.get(c)?;    // 0
    let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
    if ones > rank { return Some(c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize); }
    rank -= ones; c += 1;

    let v = content.get(c)?;    // 1
    let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
    if ones > rank { return Some(c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize); }
    rank -= ones; c += 1;

    let v = content.get(c)?;    // 2
    let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
    if ones > rank { return Some(c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize); }
    rank -= ones; c += 1;

    let v = content.get(c)?;    // 3
    let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
    if ones > rank { return Some(c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize); }
    rank -= ones; c += 1;

    let v = content.get(c)?;    // 4
    let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
    if ones > rank { return Some(c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize); }
    rank -= ones; c += 1;

    let v = content.get(c)?;    // 5
    let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
    if ones > rank { return Some(c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize); }
    rank -= ones; c += 1;

    let v = content.get(c)?;    // 6
    let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
    if ones > rank { return Some(c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize); }
    rank -= ones; c += 1;

    let v = content.get(c)?;    // 7
    let ones = if ONE { v.count_ones() } else { v.count_zeros() } as usize;
    if ones > rank { return Some(c * 64 + select64(if ONE { *v } else { !*v }, rank as u8) as usize); }
    
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

    #[inline] unsafe fn select_unchecked(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], mut rank: usize) -> usize {
        let l1_index = select_l1::<true>(l1ranks, &mut rank);
        let l2_begin = l1_index * L2_ENTRIES_PER_L1_ENTRY;
        let l2_index = l2_begin +
            l2ranks[l2_begin..l2ranks.len().min(l2_begin+L2_ENTRIES_PER_L1_ENTRY)]
                .partition_point(|v| (v&0xFFFFFFFF) as usize <= rank) - 1;
        select_from_l2_unchecked::<true>(content, l2ranks, l2_index, rank)
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

    #[inline] unsafe fn select0_unchecked(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], mut rank: usize) -> usize {
        let l1_index = select_l1::<false>(l1ranks, &mut rank);
        let l2_begin = l1_index * L2_ENTRIES_PER_L1_ENTRY;
        let l2_index = l2_begin +
            super::utils::partition_point_with_index(
                &l2ranks[l2_begin..l2ranks.len().min(l2_begin+L2_ENTRIES_PER_L1_ENTRY)],
                |v, i| i * BITS_PER_L2_ENTRY - (v&0xFFFFFFFF) as usize <= rank) - 1;
        select_from_l2_unchecked::<false>(content, l2ranks, l2_index, rank)
    }
}

/// Calculates such a sampling density of select values that provides an approximately constant space overhead
/// (independent of set/unset bits ratio in the vector) of [`CombinedSampling`].
/// 
/// The parameters describe the bit vector and the desired result range:
/// - `n` -- numbers of ones (for select) or zeros (for select0) in the vector,
/// - `len` -- length of the vector in bits,
/// - `max_result` -- the largest possible result, returned only and always for n >= 75%len
///                   (must be in range [6, 31]).
/// 
/// The result is the base 2 logarithm of the recommended sampling, and it is always in range [6, `max_result`].
/// (The minimum value is 6, as we never sample two bits in the same 64-bit word.)
/// 
/// A good value for `max_result` is 13, which leads to about 0.39% space overhead,
/// and, for n = 50%u, results in sampling positions of every 2^12=4096 ones (or zeros for select0).
/// As `max_result` decreases, the speed of select queries increases
/// at the cost of higher space overhead (which doubles with each decrease by 1).
pub const fn optimal_combined_sampling(mut n: usize, len: usize, max_result: u8) -> u8 {
    if 4*n >= 3*len { return max_result; }
    if 2*n > len { n = len - n; }
    if n == 0 { return 6; }
    let r = max_result.saturating_sub((len/n).ilog2() as u8); // note: len/n >= 2
    return if r <= 6 { 6 } else { r }   // max is not allowed in const
}

/// Trait that determines the sampling density of select values by [`CombinedSampling`].
pub trait CombinedSamplingDensity: Copy {
    type SamplingDensity: Copy;

    /// Returns density for given parameters of bit vector:
    /// - `number_of_items` -- numbers of ones (for select) or zeros (for select0) in the vector,
    /// - `len` -- length of the vector in bits.
    fn density_for(number_of_items: usize, len: usize) -> Self::SamplingDensity;

    /// Returns number of bit ones or zeros (in the case of select0) per each sample (entry).
    fn items_per_sample(density: Self::SamplingDensity) -> u32;

    /// Returns `index` divided by [`Self::items_per_sample(density)`].
    fn divide_by_density(index: usize, density: Self::SamplingDensity) -> usize;

    /// Returns `index` divided by [`Self::items_per_sample(density)`].
    fn ceiling_divide_by_density(index: usize, density: Self::SamplingDensity) -> usize {
        Self::divide_by_density(index+Self::items_per_sample(density) as usize-1, density)
    }
}

/// Specifies a constant sampling of select values by [`CombinedSampling`], given as
/// the base 2 logarithm (which can be calculated by [`optimal_combined_sampling`] function).
/// 
/// `VALUE_LOG2` must be in range [6, 31].
/// Default value 13 means sampling positions of every 2^13=8192 ones (or zeros for select0),
/// which leads to about 0.20% space overhead in vectors filled with bit ones in about half.
/// As sampling decreases, the speed of select queries increases at the expense of higher
/// space overhead (which doubles with each decrease by 1).
#[derive(Clone, Copy)]
pub struct ConstCombinedSamplingDensity<const VALUE_LOG2: u8 = 13>;

impl<const VALUE_LOG2: u8> CombinedSamplingDensity for ConstCombinedSamplingDensity<VALUE_LOG2> {
    type SamplingDensity = ();
    #[inline(always)] fn density_for(_number_of_items: usize, _len: usize) -> Self::SamplingDensity { () }
    #[inline(always)] fn items_per_sample(_density: Self::SamplingDensity) -> u32 { 1<<VALUE_LOG2 }
    #[inline(always)] fn divide_by_density(index: usize, _density: Self::SamplingDensity) -> usize { index>>VALUE_LOG2 }
}

/// Specifies adaptive sampling of select values by [`CombinedSampling`].
/// 
/// The sampling density is calculated based on the content of the bit vector
/// using the [`optimal_combined_sampling`] function, with the given `MAX_RESULT` parameter.
/// `MAX_RESULT` must be in range [6, 31].
/// As `MAX_RESULT` decreases, the speed of select queries increases
/// at the cost of higher space overhead (which doubles with each decrease by 1).
/// Its value 13 leads to about 0.39% space overhead, and, for n = 50%u, results in sampling
/// positions of every 2^12=4096 ones (or zeros for select0).
#[derive(Clone, Copy)]
pub struct AdaptiveCombinedSamplingDensity<const MAX_RESULT: u8 = 13>;

impl<const MAX_RESULT: u8> CombinedSamplingDensity for AdaptiveCombinedSamplingDensity<MAX_RESULT> {
    type SamplingDensity = u8;
    #[inline(always)] fn density_for(number_of_items: usize, len: usize) -> Self::SamplingDensity {
        optimal_combined_sampling(number_of_items, len, MAX_RESULT)
    }
    #[inline(always)] fn items_per_sample(density: Self::SamplingDensity) -> u32 { 1 << density }
    #[inline(always)] fn divide_by_density(index: usize, density: Self::SamplingDensity) -> usize { index >> density }
}

/// Fast select strategy for [`ArrayWithRankSelect101111`](crate::ArrayWithRankSelect101111) with about 0.39% space overhead.
/// 
/// Space/speed trade-off can be adjusted by the template parameter, by giving one of:
/// - [`AdaptiveCombinedSamplingDensity`] (default) -- works well with a wide range of bit vectors,
/// - [`ConstCombinedSamplingDensity`] -- recommended for vectors with a known ratio of set/unset bits;
///                with default parameters, recommended for vectors filled with bit ones in about half.
/// 
/// The implementation generally follows the paper:
/// - Zhou D., Andersen D.G., Kaminsky M. (2013) "Space-Efficient, High-Performance Rank and Select Structures on Uncompressed Bit Sequences".
///   In: Bonifaci V., Demetrescu C., Marchetti-Spaccamela A. (eds) Experimental Algorithms. SEA 2013.
///   Lecture Notes in Computer Science, vol 7933. Springer, Berlin, Heidelberg. <https://doi.org/10.1007/978-3-642-38527-8_15>
/// However, our implementation can automatically adjust the sampling density according to the content of the vector
/// (see [`AdaptiveCombinedSamplingDensity`]).
/// 
/// Combined Sampling was proposed in:
/// - G. Navarro, E. Providel, "Fast, small, simple rank/select on bitmaps",
///   in: R. Klasing (Ed.), Experimental Algorithms, Springer Berlin Heidelberg, Berlin, Heidelberg, 2012, pp. 295–306
#[derive(Clone)]
pub struct CombinedSampling<D: CombinedSamplingDensity = /*ConstCombinedSamplingDensity*/AdaptiveCombinedSamplingDensity> {
    /// Bit indices (relative to level 1) of every [`ONES_PER_SELECT_ENTRY`]-th one (or zero in the case of select 0) in content, starting from the first one.
    select: Box<[u32]>,
    /// [`select_begin`] indices that begin descriptions of subsequent first-level entries.
    select_begin: Box<[usize]>,
    /// Sampling density (ZST for const density).
    density: D::SamplingDensity,
}

impl<D: CombinedSamplingDensity> GetSize for CombinedSampling<D> where D::SamplingDensity: GetSize {
    fn size_bytes_dyn(&self) -> usize {
        self.select.size_bytes_dyn() + self.select_begin.size_bytes_dyn() + self.density.size_bytes_dyn()
    }
    const USES_DYN_MEM: bool = true;
}

impl<D: CombinedSamplingDensity> CombinedSampling<D> {
    #[inline]
    fn new<const ONE: bool>(content: &[u64], l1ranks: &[usize], total_rank: usize) -> Self {
        let density = D::density_for(
            if ONE { total_rank } else { content.len()*64-total_rank },
            content.len()*64
        );
        if content.is_empty() { return Self{ select: Default::default(), select_begin: Default::default(), density } }
        let mut ones_positions_begin = Vec::with_capacity(l1ranks.len());
        let mut ones_positions_len = 0;
        ones_positions_begin.push(0);
        for ones in l1ranks.windows(2) {
            let chunk_len = D::ceiling_divide_by_density(
                if ONE {ones[1] - ones[0]} else {BITS_PER_L1_ENTRY-(ones[1] - ones[0])},
                density);
            ones_positions_len += chunk_len;
            ones_positions_begin.push(ones_positions_len);
        }
        ones_positions_len += D::ceiling_divide_by_density(
            if ONE {(total_rank - l1ranks.last().unwrap()) as usize }
            else { ((content.len()-1)%U64_PER_L1_ENTRY+1)*64 - (total_rank - l1ranks.last().unwrap()) as usize },
            density);
        let mut ones_positions = Vec::with_capacity(ones_positions_len);
        for content in content.chunks(U64_PER_L1_ENTRY) {
            //TODO use l2ranks for faster reducing rank
            let mut bit_index = 0;
            let mut rank = 0; /*ONES_PER_SELECT_ENTRY as u16 - 1;*/    // we scan for 1 with this rank, to find its bit index in content
            for c in content.iter() {
                let c_ones = if ONE { c.count_ones() } else { c.count_zeros() };
                if c_ones <= rank {
                    rank -= c_ones;
                } else {
                    let new_rank = D::items_per_sample(density) - c_ones + rank;
                    ones_positions.push((bit_index + select64(if ONE {*c} else {!c}, rank as u8) as u32) >> 11);    // each l2 entry covers 2^11 bits
                    rank = new_rank;
                }
                bit_index = bit_index.wrapping_add(64);
            }
        }
        debug_assert_eq!(ones_positions.len(), ones_positions_len);
        Self { select: ones_positions.into_boxed_slice(), select_begin: ones_positions_begin.into_boxed_slice(), density }
    }

    #[inline(always)]
    fn select<const ONE: bool>(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], mut rank: usize) -> Option<usize> {
        if l1ranks.is_empty() { return None; }
        let l1_index = select_l1::<ONE>(l1ranks, &mut rank);
        let l2_begin = l1_index * L2_ENTRIES_PER_L1_ENTRY;
        //let l2ranks = &l2ranks[l2_begin..l2ranks.len().min(l2_begin+L2_ENTRIES_PER_L1_ENTRY)];
        let mut l2_index = l2_begin + *self.select.get(unsafe{self.select_begin.get_unchecked(l1_index)} + D::divide_by_density(rank as usize, self.density))? as usize;
        let l2_chunk_end = l2ranks.len().min(l2_begin+L2_ENTRIES_PER_L1_ENTRY);
        while l2_index+1 < l2_chunk_end &&
             if ONE {(unsafe{l2ranks.get_unchecked(l2_index+1)} & 0xFF_FF_FF_FF) as usize}
             else {(l2_index+1-l2_begin) * BITS_PER_L2_ENTRY - (unsafe{l2ranks.get_unchecked(l2_index+1)} & 0xFF_FF_FF_FF) as usize} <= rank
        {
            l2_index += 1;
        }
        unsafe { select_from_l2::<ONE>(content, l2ranks, l2_index, rank) }
    }

    #[inline(always)]
    unsafe fn select_unchecked<const ONE: bool>(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], mut rank: usize) -> usize {
        let l1_index = select_l1::<ONE>(l1ranks, &mut rank);
        let l2_begin = l1_index * L2_ENTRIES_PER_L1_ENTRY;
        //let l2ranks = &l2ranks[l2_begin..l2ranks.len().min(l2_begin+L2_ENTRIES_PER_L1_ENTRY)];
        let mut l2_index = l2_begin + *self.select.get_unchecked(self.select_begin.get_unchecked(l1_index) + D::divide_by_density(rank as usize, self.density)) as usize;
        let l2_chunk_end = l2ranks.len().min(l2_begin+L2_ENTRIES_PER_L1_ENTRY);
        while l2_index+1 < l2_chunk_end &&
             if ONE {(l2ranks.get_unchecked(l2_index+1) & 0xFF_FF_FF_FF) as usize}
             else {(l2_index+1-l2_begin) * BITS_PER_L2_ENTRY - (l2ranks.get_unchecked(l2_index+1) & 0xFF_FF_FF_FF) as usize} <= rank
        {
            l2_index += 1;
        }
        unsafe { select_from_l2_unchecked::<ONE>(content, l2ranks, l2_index, rank) }
    }
}

impl<D: CombinedSamplingDensity> SelectForRank101111 for CombinedSampling<D> {
    fn new(content: &[u64], l1ranks: &[usize], _l2ranks: &[u64], total_rank: usize) -> Self {
        Self::new::<true>(content, l1ranks, total_rank)
    }

    fn select(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], rank: usize) -> Option<usize> {
        Self::select::<true>(&self, content, l1ranks, l2ranks, rank)
    }

    unsafe fn select_unchecked(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], rank: usize) -> usize {
        Self::select_unchecked::<true>(&self, content, l1ranks, l2ranks, rank)
    }
}

impl<D: CombinedSamplingDensity> Select0ForRank101111 for CombinedSampling<D> {
    fn new0(content: &[u64], l1ranks: &[usize], _l2ranks: &[u64], total_rank: usize) -> Self {
        Self::new::<false>(content, l1ranks, total_rank)
    }

    fn select0(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], rank: usize) -> Option<usize> {
        Self::select::<false>(&self, content, l1ranks, l2ranks, rank)
    }

    unsafe fn select0_unchecked(&self, content: &[u64], l1ranks: &[usize], l2ranks: &[u64], rank: usize) -> usize {
        Self::select_unchecked::<false>(&self, content, l1ranks, l2ranks, rank)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimal_combined_sampling_14() {
        assert_eq!(optimal_combined_sampling(4, 5, 14), 14);
        assert_eq!(optimal_combined_sampling(3, 4, 14), 14);
        assert_eq!(optimal_combined_sampling(299, 400, 14), 13);
        assert_eq!(optimal_combined_sampling(1, 2, 14), 13);
        assert_eq!(optimal_combined_sampling(101, 200, 14), 13);
        assert_eq!(optimal_combined_sampling(99, 200, 14), 13);
        assert_eq!(optimal_combined_sampling(3, 8, 14), 13);
        assert_eq!(optimal_combined_sampling(1, 4, 14), 12);
        assert_eq!(optimal_combined_sampling(10, 41, 14), 12);
        assert_eq!(optimal_combined_sampling(9, 41, 14), 12);
        assert_eq!(optimal_combined_sampling(1, 8, 14), 11);
        assert_eq!(optimal_combined_sampling(10, 81, 14), 11);
        assert_eq!(optimal_combined_sampling(9, 81, 14), 11);
        assert_eq!(optimal_combined_sampling(1, 32, 14), 9);
        assert_eq!(optimal_combined_sampling(10, 321, 14), 9);
        assert_eq!(optimal_combined_sampling(9, 319, 14), 9);
        assert_eq!(optimal_combined_sampling(1, 10, 14), 11);
        assert_eq!(optimal_combined_sampling(1, 100000000, 14), 6);
        assert_eq!(optimal_combined_sampling(1, 1000000000000000000, 14), 6);
    }

    #[test]
    fn test_optimal_combined_sampling_15() {
        assert_eq!(optimal_combined_sampling(4, 5, 15), 15);
        assert_eq!(optimal_combined_sampling(3, 4, 15), 15);
        assert_eq!(optimal_combined_sampling(299, 400, 15), 14);
        assert_eq!(optimal_combined_sampling(1, 2, 15), 14);
        assert_eq!(optimal_combined_sampling(101, 200, 15), 14);
        assert_eq!(optimal_combined_sampling(99, 200, 15), 14);
        assert_eq!(optimal_combined_sampling(3, 8, 15), 14);
        assert_eq!(optimal_combined_sampling(1, 4, 15), 13);
        assert_eq!(optimal_combined_sampling(1, 10, 15), 12);
    }

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
#[cfg(not(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "bmi2")))] const SELECT_U8: [u8; 2048] = [
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
