use std::io;

use binout::{Serializer, VByte};
use seedable_hash::map64_to_64;

use crate::{phast::SeedChooserCore, seeds::SeedSize};

/// The PHast core which is responsible for mapping key hashes to buckets and slices.
pub trait Core: Copy+Sync {

    /// Returns the number of buckets.
    fn buckets_num(&self) -> usize;

    /// Returns slice length L - 1.
    fn slice_len_minus_one(&self) -> u16;

    #[inline(always)] fn slice_len(&self) -> u16 {
        self.slice_len_minus_one() + 1
    }

    /// Returns number of slices = output range - slice_len_minus_one
    fn num_of_slices(&self) -> usize;

    // Returns output range of the function.
    /*#[inline] fn output_range<SC: SeedChooser>(&self, seed_chooser: &SC, bits_per_seed: u8) -> usize {
        self.num_of_slices + self.slice_len_minus_one as usize + seed_chooser.extra_shift(bits_per_seed) as usize
    }*/

    /// Returns first value of slice assigned to the `key`.
    #[inline(always)]
    fn slice_begin(&self, key: u64) -> usize {
        map64_to_64(key, self.num_of_slices() as u64) as usize
    }

    /// Returns the largest value which is lower than or equal to the value of any key in the `bucket`.
    #[inline(always)]
    fn slice_begin_for_bucket(&self, bucket: usize) -> usize {
        // The lowest hash code in the bucket is ⌈(bucket<<64)/buckets_num⌉
        self.slice_begin(((bucket as u128) << 64).div_ceil(self.buckets_num() as u128) as u64)
        // Proof:
        // We look for the lowest c such that b=bucket=⌊(B*c)>>64⌋=⌊BC/U⌋ (*).
        // b = ⌊Bc/U⌋  =>  b ≤ ⌊Bc/U⌋  <=>  b ≤ Bc/U  <=>  bU ≤ Bc  <=>  bU/B ≤ c  <=>  ⌈bU/B⌉ ≤ c
        // for the lowest c fulfilling (*), we have b > B(c-1)/U  <=>  bU/B > c-1  <=>  c < bU/B + 1  <=>  c < ⌈bU/B⌉ + 1
        // So the lowest c fulfilling (*) meets:  ⌈bU/B⌉ ≤ c < ⌈bU/B⌉ + 1  <=>  c = ⌈bU/B⌉.
    }

    /// Returns bucket assigned to the `key`.
    #[inline(always)]
    fn bucket_for(&self, key: u64) -> usize {
        map64_to_64(key, self.buckets_num() as u64) as usize
    }

    /// Returns index of `key` in its slice.
    #[inline(always)]
    fn in_slice(&self, key: u64, seed: u16) -> usize {
        (mult_hi((seed as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95 /*0x1d8e_4e27_c47d_124f*/), key) as u16 & self.slice_len_minus_one()) as usize
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
    fn in_slice_nobump(&self, key: u64, seed: u16) -> usize {
        //(wymum((seed as u64 ^ 0xa076_1d64_78bd_642f).wrapping_mul(0x1d8e_4e27_c47d_124f), key) as u16 & self.slice_len_minus_one) as usize
        (mix(mix(seed as u64 ^ 0xa076_1d64_78bd_642f, 0x1d8e_4e27_c47d_124f), key) as u16 & self.slice_len_minus_one()) as usize
    }

    /// Returns seed independent index of `key` in its partition.
    #[inline(always)]
    fn in_slice_noseed(&self, key: u64) -> usize {
        //(wymum_xor(key as u64, 0xe703_7ed1_a0b4_28db) as u16  & self.slice_len_minus_one) as usize
        (key as u16 & self.slice_len_minus_one()) as usize
    }

    /// Returns the value of the function for given `key` and `seed`.
    #[inline(always)]
    fn f(&self, key: u64, seed: u16) -> usize {
        self.slice_begin(key) + self.in_slice(key, seed)
    }

    #[inline(always)]
    fn try_f<SS>(&self, seed_size: SS, seeds: &[SS::VecElement], key: u64) -> Option<usize> where SS: SeedSize {
        let seed = unsafe { seed_size.get_seed(seeds, self.bucket_for(key)) };
        (seed != 0).then(|| self.f(key, seed))
    }

    #[inline(always)]
    fn f_shift0(&self, key: u64) -> usize {
        self.slice_begin(key) + self.in_slice_noseed(key)
    }

    /*#[inline(always)]
    pub(crate) fn f_shift(&self, key: u64, shift: u16) -> usize {
        self.slice_begin(key) + self.in_slice_noseed(key) + shift as usize - 1
    }*/

    #[inline(always)]
    fn f_nobump(&self, key: u64, seed: u16) -> usize {
        self.slice_begin(key) + self.in_slice_nobump(key, seed)
    } 

    /// Returns output range of the function.
    #[inline] fn output_range<SCC: SeedChooserCore>(&self, seed_chooser_core: SCC, bits_per_seed: u8) -> usize {
        self.num_of_slices() + self.slice_len_minus_one() as usize + seed_chooser_core.extra_shift(bits_per_seed) as usize
    }

    #[inline] fn new_seeds_vec<SS: SeedSize>(&self, seed_size: SS) -> Box<[SS::VecElement]> {
        seed_size.new_zeroed_seed_vec(self.buckets_num())
    }

    /// Writes `self` to the `output`.
    fn write(&self, output: &mut dyn io::Write) -> io::Result<()>;

    /// Returns number of bytes which `write` will write.
    fn write_bytes(&self) -> usize;

    /// Read `Self` from the `input`.
    fn read(input: &mut dyn io::Read) -> io::Result<Self>;
}

/// Generic PHast core that supports many configurations.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GenericCore {
    pub(crate) buckets_num: usize, // number of buckets, B
    pub(crate) slice_len_minus_one: u16,  // slice length L - 1
    pub(crate) num_of_slices: usize,   // output range - slice_len_minus_one
}

impl Core for GenericCore {

    #[inline(always)]
    fn buckets_num(&self) -> usize {
        self.buckets_num
    }

    #[inline(always)]
    fn slice_len_minus_one(&self) -> u16 {
        self.slice_len_minus_one
    }
    
    #[inline(always)]
    fn num_of_slices(&self) -> usize {
        self.num_of_slices
    }

    fn write(&self, output: &mut dyn io::Write) -> io::Result<()> {
        VByte::write(output, self.buckets_num)?;
        VByte::write(output, self.slice_len_minus_one)?;
        VByte::write(output, self.num_of_slices)
    }

    fn write_bytes(&self) -> usize {
        VByte::size(self.buckets_num)
         + VByte::size(self.slice_len_minus_one)
         + VByte::size(self.num_of_slices)
    }

    fn read(input: &mut dyn io::Read) -> io::Result<Self>
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

impl GenericCore {

    pub(crate) fn new(output_range: usize, num_of_keys: usize, bucket_size_100: u16, slice_len: u16, max_shift: u16) -> Self {
        let bucket_size_100 = bucket_size_100 as usize;
        Self {
            buckets_num: 1.max((num_of_keys * 100 + bucket_size_100/2) / bucket_size_100),
            slice_len_minus_one: slice_len - 1,
            num_of_slices: output_range + 1 - slice_len as usize - max_shift as usize,
        }
    }

    // configuration for "turbo" function that assume that input=output range and bucket_size_100 is about 400.
    /*pub(crate) fn turbo_new(output_range: usize, slice_len: u16, max_shift: u16) -> Self {
        let num_of_slices = output_range + 1 - slice_len as usize - max_shift as usize;
        Self {
            buckets_num: (num_of_slices-1)/4+1,
            slice_len_minus_one: slice_len - 1,
            num_of_slices,
        }
    }*/

    /// Returns bucket assigned to the `key`.
    #[inline(always)]
    pub fn bucket_for(&self, key: u64) -> usize {
        map64_to_64(key, self.buckets_num as u64) as usize
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
}

pub trait Conf: Sync {
    /// PHast Core to use.
    type Core: Core;

    /// Type of seed size to use.
    type SeedSize: SeedSize;

    fn core(&self, output_range: usize, num_of_keys: usize, slice_len: u16, max_shift: u16) -> Self::Core;

    //fn bucket_size100(&self) -> u16;
    fn preferred_slice_len(&self) -> u16;

    fn seed_size(&self) -> Self::SeedSize;

    /// Returns number of bits used to store each seed.
    fn bits_per_seed(&self) -> u8;
}


/// PHast map-or-bump turbo function configuration.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TurboCore {
    pub(crate) slice_len_minus_one: u16,  // slice length L - 1
    pub(crate) num_of_slices: usize,   // output range - slice_len_minus_one
}

impl TurboCore {

    pub(crate) fn new(output_range: usize, mut slice_len: u16, max_shift: u16) -> Self {
        let max_allowed_slice_len = output_range/5+1;   // or output_range/4+1;
        if slice_len as usize > max_allowed_slice_len { slice_len = (max_allowed_slice_len as u16).next_power_of_two(); }
        Self {
            slice_len_minus_one: slice_len - 1,
            num_of_slices: output_range + 1 - slice_len as usize - max_shift as usize,
        }
    }
}

impl Core for TurboCore {

    #[inline(always)]
    fn try_f<SS>(&self, seed_size: SS, seeds: &[SS::VecElement], key: u64) -> Option<usize> where SS: SeedSize {
        let slice_begin = self.slice_begin(key);
        let seed = unsafe { seed_size.get_seed(seeds, slice_begin/4) };
        (seed != 0).then(|| slice_begin + self.in_slice(key, seed))
    }

    #[inline(always)]
    fn slice_begin_for_bucket(&self, bucket: usize) -> usize {
        bucket * 4
    }

    #[inline(always)]
    fn buckets_num(&self) -> usize {
        (self.num_of_slices-1)/4+1
    }

    #[inline(always)]
    fn slice_len_minus_one(&self) -> u16 {
        self.slice_len_minus_one
    }
    
    #[inline(always)]
    fn num_of_slices(&self) -> usize {
        self.num_of_slices
    }

    #[inline(always)]
    fn bucket_for(&self, key: u64) -> usize {
        self.slice_begin(key) / 4
    }

    fn write(&self, output: &mut dyn io::Write) -> io::Result<()> {
        VByte::write(output, self.slice_len_minus_one)?;
        VByte::write(output, self.num_of_slices)
    }

    fn write_bytes(&self) -> usize {
         VByte::size(self.slice_len_minus_one)
         + VByte::size(self.num_of_slices)
    }

    fn read(input: &mut dyn io::Read) -> io::Result<Self>
    {
        let slice_len_minus_one = VByte::read(input)?;
        let num_of_slices = VByte::read(input)?;
        Ok(Self {
            slice_len_minus_one,
            num_of_slices,
        })
    }
}




#[derive(Clone, Copy)]
pub struct Generic<SS> {
    pub seed_size: SS,
    pub bucket_size100: u16,
    pub preferred_slice_len: u16
}

impl<SS> Generic<SS> {
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

impl<SS: SeedSize> Conf for Generic<SS> {
    
    type Core = GenericCore;
    type SeedSize = SS;
    
    fn core(&self, output_range: usize, num_of_keys: usize, slice_len: u16, max_shift: u16) -> Self::Core {
        GenericCore::new(output_range, num_of_keys, self.bucket_size100, slice_len, max_shift)
    }

    #[inline(always)] fn preferred_slice_len(&self) -> u16 {
        self.preferred_slice_len
    }
    
    #[inline(always)] fn seed_size(&self) -> Self::SeedSize {
        self.seed_size
    }

    #[inline(always)] fn bits_per_seed(&self) -> u8 { self.seed_size.into() }
}



#[derive(Clone, Copy)]
pub struct Turbo<SS> {
    pub seed_size: SS,
    pub preferred_slice_len: u16
}

impl<SS> Turbo<SS> {
    #[inline]
    pub fn new(seed_size: SS) -> Self {
        Self { seed_size, preferred_slice_len: 0 }
    }

    #[inline]
    pub fn new_psl(seed_size: SS, preferred_slice_len: u16) -> Self {
        Self { seed_size, preferred_slice_len }
    }

    /*#[inline]
    pub fn slice_len(&self, default: u16) -> u16 {
        if self.preferred_slice_len == 0 { default } else { self.preferred_slice_len }
    }*/
}

impl<SS: SeedSize> Conf for Turbo<SS> {
    
    type Core = TurboCore;
    type SeedSize = SS;
    
    fn core(&self, output_range: usize, _num_of_keys: usize, slice_len: u16, max_shift: u16) -> Self::Core {
        TurboCore::new(output_range, slice_len, max_shift)
    }
    
    #[inline(always)] fn preferred_slice_len(&self) -> u16 {
        self.preferred_slice_len
    }
    
    #[inline(always)] fn seed_size(&self) -> Self::SeedSize {
        self.seed_size
    }

    #[inline(always)] fn bits_per_seed(&self) -> u8 { self.seed_size.into() }
}