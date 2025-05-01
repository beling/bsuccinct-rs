use seedable_hash::map64_to_64;

use crate::seeds::{Bits, SeedSize};

use super::SeedChooser;

/// PHast map-or-bump function configuration.
#[derive(Clone, Copy)]
pub struct Conf<SS: SeedSize = Bits> {
    pub(crate) bits_per_seed: SS,  // seed size, K=2**bits_per_seed
    pub(crate) buckets_num: usize, // number of buckets, B
    pub(crate) slice_len_minus_one: u16,  // slice length L
    pub(crate) num_of_slices: usize,   // m-P
}

/*#[inline(always)]
const fn mix64(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9u64);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111ebu64);
    x ^ (x >> 31)
}*/

/*#[inline(always)]
fn mix16fast(mut x: u16) -> u16 {
    x += x << 7; x ^= x >> 8;
    x += x << 3; x ^= x >> 2;
    x += x << 4; x ^= x >> 8;
    x
}*/

#[inline(always)]
fn wymum(a: u64, b: u64) -> u64 {
    let r = (a as u128) * (b as u128);
    //((r >> 64) ^ r) as u64
    (r >> 64) as u64
}

//const SEEDS_MAP: [u64; 256] = std::array::from_fn(|i| mix64(i as u64));

/// Returns bucket size proper for given number of `bits_per_seed`.
#[inline]
pub const fn bits_per_seed_to_100_bucket_size(bits_per_seed: u8) -> u16 {
    match bits_per_seed {
        0..=4 => 250,
        5 => 290,
        6 => 320,
        7 => 370,
        8 => 450,
        9 => 530,
        10 => 590,
        11 => 650,
        12 => 720,
        13 => 770,
        _ => 830
    }
}

impl<SS: SeedSize> Conf<SS> {

    pub(crate) fn new(output_range: usize, bits_per_seed: SS, bucket_size_100: u16, max_shift: u16) -> Self {
        let max_shift = max_shift as usize;
        let slice_len = match output_range.wrapping_sub(max_shift) {
            n @ 0..64 => (n/2+1).next_power_of_two() as u16,
            64..1300 => 64,
            1300..1750 => 128,
            1750..7500 => 256,
            7500..150000 => 512,
            _ if bits_per_seed.into() < 7 => 512,
            _ => 1024
        };
        let bucket_size_100 = bucket_size_100 as usize;
        Self {
            bits_per_seed,
            buckets_num: 1.max((output_range * 100 + bucket_size_100/2) / bucket_size_100),
            slice_len_minus_one: slice_len - 1,
            num_of_slices: output_range + 1 - slice_len as usize - max_shift,
        }
    }

    /// Returns outpu range of the function.
    #[inline] pub fn output_range<SC: SeedChooser>(&self) -> usize {
        self.num_of_slices + self.slice_len_minus_one as usize + SC::extra_shift(self.bits_per_seed) as usize
    }

    /// Returns bucket assigned to the `key`.
    #[inline(always)]
    pub(crate) fn bucket_for(&self, key: u64) -> usize {
        map64_to_64(key, self.buckets_num as u64) as usize
    }

    /// Returns first value of slice assigned to the `key`.
    #[inline(always)]
    pub(crate) fn slice_begin(&self, key: u64) -> usize {
        map64_to_64(key, self.num_of_slices as u64) as usize
    }

    /// Returns index of `key` in its slice.
    #[inline(always)]
    pub(crate) fn in_slice(&self, key: u64, seed: u16) -> usize {
        (wymum((seed as u64).wrapping_mul(0x1d8e_4e27_c47d_124f), key) as u16 & self.slice_len_minus_one) as usize
        //((key.wrapping_add(seed as u64 * 2)) as u16 & self.slice_len_minus_one) as usize
        //((key.wrapping_mul(0x1d8e_4e27_c47d_124f).wrapping_add(seed as u64)) as u16 & self.slice_len_minus_one) as usize
        /*const P: u16 = 0;
        let seed_lo = (seed & ((1<<P)-1)) + 1;
        (wymum((seed_lo as u64).wrapping_mul(0x1d8e_4e27_c47d_124f), key).wrapping_add(3*(seed>>P) as u64) as u16 & self.slice_len_minus_one) as usize*/
    }

    #[inline(always)]
    pub(crate) fn in_slice_nobump(&self, key: u64, seed: u16) -> usize {
        (wymum((seed as u64 ^ 0xa076_1d64_78bd_642f).wrapping_mul(0x1d8e_4e27_c47d_124f), key) as u16 & self.slice_len_minus_one) as usize
    }

    /// Returns seed independent index of `key` in its partition.
    #[inline(always)]
    pub(crate) fn in_slice_noseed(&self, key: u64) -> usize {
        //(wymum(wymum(seed as u64, 0xe703_7ed1_a0b4_28db), key) as u16 & self.partition_size_minus_one) as usize
        (key as u16 & self.slice_len_minus_one) as usize
    }

    /// Returns the value of the function for given `key` and `seed`.
    #[inline(always)]
    pub(crate) fn f(&self, key: u64, seed: u16) -> usize {
        self.slice_begin(key) + self.in_slice(key, seed)
    }

    #[inline(always)]
    pub(crate) fn f_shift(&self, key: u64, shift: u16) -> usize {
        self.slice_begin(key) + self.in_slice_noseed(key) + shift as usize - 1
    }

    #[inline(always)]
    pub(crate) fn f_nobump(&self, key: u64, seed: u16) -> usize {
        self.slice_begin(key) + self.in_slice_nobump(key, seed)
    }

    #[inline] pub(crate) fn seeds_num(&self) -> u16 {
        1<<self.bits_per_seed.into()
    }

    #[inline] pub(crate) fn slice_len(&self) -> u16 {
        self.slice_len_minus_one + 1
    }

    #[inline(always)] pub(crate) fn bits_per_seed(&self) -> u8 {
        self.bits_per_seed.into()
    }

    #[inline] pub(crate) fn new_seeds_vec(&self) -> Box<[SS::VecElement]> {
        self.bits_per_seed.new_zeroed_seed_vec(self.buckets_num)
    }
}