use std::io;

use binout::{Serializer, VByte};
use seedable_hash::map64_to_64;

use crate::seeds::SeedSize;

use super::SeedChooser;

/// PHast map-or-bump function configuration.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Conf {
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

/// Returns most significant 64-bit half of `a*b`.
#[inline(always)]
pub(crate) fn mult_hi(a: u64, b: u64) -> u64 {
    let r = (a as u128) * (b as u128);
    //((r >> 64) ^ r) as u64
    (r >> 64) as u64
}

/// Returns the value that mix `key` and `seed`. Fast.
#[inline(always)]
pub(crate) fn mix_key_seed(key: u64, seed: u16) -> u16 {
    mult_hi((seed as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95 /*0x1d8e_4e27_c47d_124f*/), key) as u16
}   // 0x51_7c_c1_b7_27_22_0a_95 is from FXHash

/// Returns the value that mix `a` and `b` by multiplication and xoring.
#[inline(always)]
pub(crate) fn mix(a: u64, b: u64) -> u64 {
    let r = (a as u128) * (b as u128);
    ((r >> 64) ^ r) as u64
    //(r >> 64) as u64
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

impl Conf {

    pub(crate) fn new(output_range: usize, input_size: usize, bucket_size_100: u16, slice_len: u16, max_shift: u16) -> Self {
        let bucket_size_100 = bucket_size_100 as usize;
        Self {
            buckets_num: 1.max((input_size * 100 + bucket_size_100/2) / bucket_size_100),
            slice_len_minus_one: slice_len - 1,
            num_of_slices: output_range + 1 - slice_len as usize - max_shift as usize,
        }
    }

    // configuration for "turbo" function that ussume that input=output range and bucket_size_100 is about 400.
    /*pub(crate) fn turbo_new(output_range: usize, slice_len: u16, max_shift: u16) -> Self {
        let num_of_slices = output_range + 1 - slice_len as usize - max_shift as usize;
        Self {
            buckets_num: (num_of_slices-1)/4+1,
            slice_len_minus_one: slice_len - 1,
            num_of_slices,
        }
    }*/

    /// Returns output range of the function.
    #[inline] pub fn output_range<SC: SeedChooser>(&self, seed_chooser: SC, bits_per_seed: u8) -> usize {
        self.num_of_slices + self.slice_len_minus_one as usize + seed_chooser.extra_shift(bits_per_seed) as usize
    }

    /// Returns bucket assigned to the `key`.
    #[inline(always)]
    pub fn bucket_for(&self, key: u64) -> usize {
        map64_to_64(key, self.buckets_num as u64) as usize
    }

    /// Returns first value of slice assigned to the `key`.
    #[inline(always)]
    pub fn slice_begin(&self, key: u64) -> usize {
        map64_to_64(key, self.num_of_slices as u64) as usize
    }

    /// Returns index of `key` in its slice.
    #[inline(always)]
    pub fn in_slice(&self, key: u64, seed: u16) -> usize {
        (mult_hi((seed as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95 /*0x1d8e_4e27_c47d_124f*/), key) as u16 & self.slice_len_minus_one) as usize
        //((key.wrapping_add(seed as u64 * 2)) as u16 & self.slice_len_minus_one) as usize
        //((key.wrapping_mul(0x1d8e_4e27_c47d_124f).wrapping_add(seed as u64)) as u16 & self.slice_len_minus_one) as usize
        /*const P: u16 = 0;
        let seed_lo = (seed & ((1<<P)-1)) + 1;
        (wymum((seed_lo as u64).wrapping_mul(0x1d8e_4e27_c47d_124f), key).wrapping_add(3*(seed>>P) as u64) as u16 & self.slice_len_minus_one) as usize*/
    }

    /// Returns index of `key` in its slice.
    /*#[inline(always)]
    pub(crate) fn in_slice_seed_shift(&self, key: u64, seed: u16, shift: u16) -> usize {
        ((mult_hi((seed as u64).wrapping_mul(0x1d8e_4e27_c47d_124f), key) as u16 + shift as u16) & self.slice_len_minus_one) as usize
        //0x51_7c_c1_b7_27_22_0a_95
    }*/

    #[inline(always)]
    pub(crate) fn in_slice_nobump(&self, key: u64, seed: u16) -> usize {
        //(wymum((seed as u64 ^ 0xa076_1d64_78bd_642f).wrapping_mul(0x1d8e_4e27_c47d_124f), key) as u16 & self.slice_len_minus_one) as usize
        (mix(mix(seed as u64 ^ 0xa076_1d64_78bd_642f, 0x1d8e_4e27_c47d_124f), key) as u16 & self.slice_len_minus_one) as usize
    }

    /// Returns seed independent index of `key` in its partition.
    #[inline(always)]
    pub(crate) fn in_slice_noseed(&self, key: u64) -> usize {
        //(wymum_xor(key as u64, 0xe703_7ed1_a0b4_28db) as u16  & self.slice_len_minus_one) as usize
        (key as u16 & self.slice_len_minus_one) as usize
    }

    /// Returns the value of the function for given `key` and `seed`.
    #[inline(always)]
    pub fn f(&self, key: u64, seed: u16) -> usize {
        self.slice_begin(key) + self.in_slice(key, seed)
    }

    #[inline(always)]
    pub(crate) fn f_shift0(&self, key: u64) -> usize {
        self.slice_begin(key) + self.in_slice_noseed(key)
    }

    /*#[inline(always)]
    pub(crate) fn f_shift(&self, key: u64, shift: u16) -> usize {
        self.slice_begin(key) + self.in_slice_noseed(key) + shift as usize - 1
    }*/

    #[inline(always)]
    pub(crate) fn f_nobump(&self, key: u64, seed: u16) -> usize {
        self.slice_begin(key) + self.in_slice_nobump(key, seed)
    }



    #[inline] pub fn slice_len(&self) -> u16 {
        self.slice_len_minus_one + 1
    }

    #[inline] pub(crate) fn new_seeds_vec<SS: SeedSize>(&self, seed_size: SS) -> Box<[SS::VecElement]> {
        seed_size.new_zeroed_seed_vec(self.buckets_num)
    }

    // Returns bucket assigned to the `slice_begin` by "turbo" configuration.
    /*#[inline(always)]
    pub(crate) fn turbo_bucket_for_slice(&self, slice_begin: usize) -> usize {
        slice_begin / 4
    }*/

    // Returns bucket assigned to the `key` by "turbo" configuration, using `turbo_bucket_for_slice`.
    // It can be faster than bucket_for only if `slice_begin` is called near this call
    // and compiler optime out redundant `slice_begin` calculation.
    /*#[inline(always)]
    pub(crate) fn turbo_bucket_for_key(&self, key: u64) -> usize {
        self.turbo_bucket_for_slice(self.slice_begin(key))
    }*/

    /// Writes `self` to the `output`.
    pub fn write(&self, output: &mut dyn io::Write) -> io::Result<()>
    {
        VByte::write(output, self.buckets_num)?;
        VByte::write(output, self.slice_len_minus_one)?;
        VByte::write(output, self.num_of_slices)
    }

    /// Read `Self` from the `input`.
    pub fn read(input: &mut dyn io::Read) -> io::Result<Self>
    {
        let buckets_num = VByte::read(input)?;
        let slice_len_minus_one = VByte::read(input)?;
        let num_of_slices = VByte::read(input)?;
        Ok(Self {
            buckets_num,
            slice_len_minus_one,
            num_of_slices,
        })
    }
}

#[derive(Clone, Copy)]
pub struct Params<SS> {
    pub seed_size: SS,
    pub bucket_size100: u16,
    pub preferred_slice_len: u16
}

impl<SS> Params<SS> {
    #[inline]
    pub fn new(seed_size: SS, bucket_size100: u16) -> Self {
        Self { seed_size, bucket_size100, preferred_slice_len: 0 }
    }

    #[inline]
    pub fn new_psl(seed_size: SS, bucket_size100: u16, preferred_slice_len: u16) -> Self {
        Self { seed_size, bucket_size100, preferred_slice_len }
    }

    /*#[inline]
    pub fn slice_len(&self, default: u16) -> u16 {
        if self.preferred_slice_len == 0 { default } else { self.preferred_slice_len }
    }*/
}

impl<SS: Copy+Into<u8>> Params<SS> {
    #[inline(always)]
    pub fn bits_per_seed(&self) -> u8 { self.seed_size.into() }
}