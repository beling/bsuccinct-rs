use std::convert::{TryFrom, TryInto};
use std::io::{Read, Write};
use std::ops::Mul;
use binout::{AsIs, Serializer};
use bitm::{BitAccess, BitVec, ceiling_div};
use dyn_size_of::GetSize;
use crate::utils::{map32_to_32, map64_to_64, read_bits};

/// Calculates group number for a given `key` at level of the size `level_size_groups` groups, whose number is already hashed in `hasher`.
/// Modifies `hasher`, which can be farther used to calculate index in the group by just writing to it the seed of the group.
#[inline(always)]
pub fn group_nr(hash: u64, level_size_groups: u64) -> u64 {
    //map64_to_32(hash, level_size_groups)
    //map32_to_32((hash >> 32) as u32, level_size_groups as u32) as u64
    //map48_to_64(hash >> 16, level_size_groups)
    map64_to_64(hash, level_size_groups)    // note that the lowest x bits of hash are ignored by map64_to_64 if level_size_groups < 2^(64-x) and it is save to use the lowest bits by in_group_index
}

/*#[inline]
fn mix64(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9u64);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111ebu64);
    x ^ (x >> 31)
}*/

#[inline(always)]
fn mix32(mut x: u32) -> u32 {
    x = (x ^ (x >> 16)).wrapping_mul(0x21f0aaad);
    x = (x ^ (x >> 15)).wrapping_mul(0xd35a2d97);
    x ^ (x >> 15)
}

/*#[inline(always)]
fn mix16(mut x: u16) -> u16 {
    x = (x ^ (x >> 8)).wrapping_mul(0xa3d3);
    x = (x ^ (x >> 7)).wrapping_mul(0x4b2d);
    x ^ (x >> 9)
}

#[inline(always)]
fn mix16fast(mut x: u16) -> u16 {
    x += x << 7; x ^= x >> 8;
    x += x << 3; x ^= x >> 2;
    x += x << 4; x ^= x >> 8;
    x
}*/

/// Implementations of `GroupSize` represent group size in fingerprinting-based minimal perfect hashing with group optimization.
pub trait GroupSize: Sized + Mul<usize, Output=usize> + Copy + Into<u8> + TryFrom<u8, Error=&'static str> {

    fn validate(&self) -> Result<Self, &'static str> { Ok(*self) }

    /// Returns `hash` modulo `self`.
    #[inline(always)]
    fn hash_to_group(&self, hash: u32) -> u8 {
        map32_to_32(hash as u32, Into::<u8>::into(*self) as u32) as u8
        //map16_to_16(hash as u16, Into::<u8>::into(*self) as u16) as u8
    }

    /// Returns index in the group with given seed `group_seed` using `hasher`
    /// which must be modified earlier by `group_nr` function.
    #[inline(always)]
    fn in_group_index(&self, hash: u64, group_seed: u16) -> u8 {
        //self.hash_to_group(mix16((hash as u16) ^ group_seed as u16))
        self.hash_to_group(mix32((hash as u32) ^ (group_seed as u32)))

        //self.hash_to_group(mix32((hash as u32).wrapping_add(group_seed as u32)))
        //self.hash_to_group(mix32(((hash as u32) & 0xFFFF) | ((group_seed as u32) << 16)))
        //self.hash_to_group(mix64(hash ^ group_seed as u64))
    }

    /// Returns bit index inside the group with number `group` and seed `group_seed`,
    /// assigned to the key hashed by the `hasher`.
    #[inline]
    fn bit_index_for_seed(&self, hash: u64, group_seed: u16, group: u64) -> usize {
        (*self * group as usize) + self.in_group_index(hash, group_seed) as usize
    }

    /// Returns number of groups and 64-bit segments for given `desired_total_size`.
    fn level_size_groups_segments(&self, mut desired_total_size: usize) -> (usize, usize) {
        let remainder = desired_total_size % 64;
        if remainder != 0 { desired_total_size += 64 - remainder; } // round up to multiple of 64
        let group_size = Into::<u8>::into(*self) as usize;
        while desired_total_size % group_size != 0 { desired_total_size += 64; } // and also to multiple of group_size
        return (desired_total_size / group_size, desired_total_size / 64);
    }

    /// Returns number of ones in the group.
    /*#[inline] fn ones_in_group(&self, arr: &[u64], group_index: usize) -> u8 {
        arr.get_fragment(group_index, (*self).into()).count_ones() as u8
    }*/

    // If `predicate` is `true`, copy group with given index, from `src` to `dst`.
    // Predicate is called with the content of the groups of, successively, `dst` and `src`.
    //#[inline] fn conditionally_copy_group<Pred>(&self, dst: &mut [u64], src: &[u64], group_index: usize, predicate: Pred)
    //    where Pred: FnOnce(u64, u64) -> bool {
    //    dst.conditionally_copy_fragment(src, predicate, group_index, (*self).into())
    //}

    /// If group with given index has more ones in `dst` than in `src`, then copy it from `src` to `dst` and call `callback`.
    #[inline(always)] fn copy_group_if_better<CB>(&self, dst: &mut [u64], src: &[u64], group_index: usize, callback: CB)
        where CB: FnOnce()
    {
        dst.conditionally_copy_fragment(src, |best, new|
            if best.count_ones() < new.count_ones() {
                callback();
                true
            } else { false }, group_index, (*self).into())
    }

    /// Returns number of bytes that `self.write` writes to the output.
    #[inline] fn write_size_bytes(&self) -> usize {
        std::mem::size_of::<u8>()
    }

    /// Writes `self` to `output`.
    fn write(&self, output: &mut dyn std::io::Write) -> std::io::Result<()> {
        AsIs::write(output, (*self).into())
    }

    /// Reads `Self` from `input`.
    fn read(input: &mut dyn std::io::Read) -> std::io::Result<Self> {
        let group_size: u8 = AsIs::read(input)?;
        let result = TryInto::<Self>::try_into(group_size).map_err(to_io_error)?;
        result.validate().map_err(to_io_error)
    }
}

fn to_io_error<E>(err: E) -> std::io::Error
where E: Into<Box<dyn std::error::Error + Send + Sync>> {
    std::io::Error::new(std::io::ErrorKind::InvalidData, err)
}

/// Implementations of `SeedSize` represent seed size in fingerprinting-based minimal perfect hashing with group optimization.
pub trait SeedSize: Copy + Into<u8> + Sync + TryFrom<u8, Error=&'static str> {
    type VecElement: Copy + Send + Sync + Sized + GetSize;

    fn validate(&self) -> Result<Self, &'static str> { Ok(*self) }

    #[inline] fn new_zeroed_seed_vec(&self, number_of_seeds: usize) -> Box<[Self::VecElement]> {
        self.new_seed_vec(0, number_of_seeds)
    }

    fn new_seed_vec(&self, seed: u16, number_of_seeds: usize) -> Box<[Self::VecElement]>;

    fn get_seed(&self, vec: &[Self::VecElement], index: usize) -> u16;

    fn set_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16);

    #[inline] fn init_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16) {
        self.set_seed(vec, index, seed)
    }

    fn concatenate_seed_vecs(&self, level_sizes: &[u64], group_seeds: Vec<Box<[Self::VecElement]>>) -> Box<[Self::VecElement]> {
        let mut group_seeds_concatenated = self.new_zeroed_seed_vec(level_sizes.iter().map(|v| *v as usize).sum::<usize>());
        let mut dst_group = 0;
        for (l_size, l_seeds) in level_sizes.iter().zip(group_seeds.into_iter()) {
            for index in 0..*l_size {
                self.init_seed(&mut group_seeds_concatenated, dst_group, self.get_seed(&l_seeds, index as usize));
                dst_group += 1;
            }
        }
        group_seeds_concatenated
    }

    /// Writes `self` to `output`.
    fn write(&self, output: &mut dyn Write) -> std::io::Result<()> {
        AsIs::write(output, (*self).into())
    }

    /// Reads `Self` from `input`.
    fn read(input: &mut dyn Read) -> std::io::Result<Self> {
        let seed_size: u8 = AsIs::read(input)?;
        let result = TryInto::<Self>::try_into(seed_size).map_err(to_io_error)?;
        result.validate().map_err(to_io_error)
    }

    fn write_seed_vec(&self, output: &mut dyn Write, seeds: &[Self::VecElement]) -> std::io::Result<()>;

    fn read_seed_vec(input: &mut dyn Read, number_of_seeds: usize) -> std::io::Result<(Self, Box<[Self::VecElement]>)>;
}

#[derive(Copy, Clone)]
pub struct TwoToPowerBits {
    log2size: u8,
    mask: u8
}

impl TwoToPowerBits {
    pub fn new(log2size: u8) -> Self {
        assert!(log2size <= 7);
        Self { log2size, mask: (1u8<<log2size)-1 }
    }

    /*pub fn get_fragment(&self, arr: &[u64], index: usize) -> u64 {
        let bit_index = index << self.log2size;
        (arr[bit_index / 64] >> (bit_index%64)) & nie ten self.mask
    }*/
}

impl Mul<usize> for TwoToPowerBits {
    type Output = usize;

    #[inline(always)] fn mul(self, rhs: usize) -> Self::Output {
        rhs << self.log2size
    }
}

impl Into<u8> for TwoToPowerBits {
    #[inline(always)] fn into(self) -> u8 {
        1<<self.log2size
    }
}

impl TryFrom<u8> for TwoToPowerBits {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value.is_power_of_two() && value <= 128 {
            Ok(Self::new(value.trailing_zeros() as u8))
        } else {
            Err("group size must be the power of two, not greater than 128")
        }
    }
}

impl GroupSize for TwoToPowerBits {
    #[inline(always)] fn hash_to_group(&self, hash: u32) -> u8 {
        hash as u8 & self.mask
    }

    fn level_size_groups_segments(&self, desired_total_size: usize) -> (usize, usize) {
        let level_size_segments;
        let level_size_groups;
        if self.log2size > 6 {
            //level_size_segments = div_up(input_size << segments_per_group_log2, 64) << segments_per_group_log2;
            //level_size_groups = (level_size_segments >> segments_per_group_log2) as u32;
            level_size_groups = ceiling_div(desired_total_size, 1usize<<self.log2size);
            level_size_segments = (level_size_groups as usize) << (self.log2size - 6);
        } else {
            level_size_segments = ceiling_div(desired_total_size, 64);
            level_size_groups = level_size_segments << (6 - self.log2size);
        }
        (level_size_groups, level_size_segments)
    }

    /*fn ones_in_group(&self, arr: &[u64], group_index: usize) -> u8 {
        if self.log2size >= 6 {
            let log2_segments_per_group = self.log2size - 6;
            let segments_per_group = 1 << log2_segments_per_group;
            let begin_segment = group_index << log2_segments_per_group; // * segments_per_group
            arr[begin_segment..begin_segment+segments_per_group].iter().map(|v| v.count_ones() as u8).sum()
        } else {
            arr.get_fragment(group_index, (*self).into()).count_ones() as u8
        }
    }*/
}

#[derive(Copy, Clone)]
pub struct Bits(pub u8);

impl Mul<usize> for Bits {
    type Output = usize;

    #[inline(always)] fn mul(self, rhs: usize) -> Self::Output {
        self.0 as usize * rhs
    }
}

impl Into<u8> for Bits {
    #[inline(always)] fn into(self) -> u8 { self.0 }
}

impl GroupSize for Bits {
    fn validate(&self) -> Result<Self, &'static str> {
        if self.0 <= 63 { Ok(*self) } else { Err("group sizes grater than 63 are not supported by Bits") }
    }
}

impl TryFrom<u8> for Bits {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(Self(value))
    }
}

impl SeedSize for Bits {
    type VecElement = u64;

    #[inline(always)] fn new_zeroed_seed_vec(&self, number_of_seeds: usize) -> Box<[Self::VecElement]> {
        Box::<[u64]>::with_zeroed_bits(number_of_seeds * self.0 as usize)
    }

    #[inline(always)] fn new_seed_vec(&self, seed: u16, number_of_seeds: usize) -> Box<[Self::VecElement]> {
        Box::<[u64]>::with_bitwords(seed as u64, self.0, number_of_seeds)
    }

    #[inline(always)] fn get_seed(&self, vec: &[Self::VecElement], index: usize) -> u16 {
        vec.get_fragment(index, self.0) as u16
    }

    #[inline(always)] fn set_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16) {
        vec.set_fragment(index, seed as u64, self.0)
    }

    #[inline(always)] fn init_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16) {
        vec.init_fragment(index, seed as u64, self.0)
    }

    fn write_seed_vec(&self, output: &mut dyn Write, seeds: &[Self::VecElement]) -> std::io::Result<()> {
        SeedSize::write(self, output)?;
        AsIs::write_all(output, seeds)
    }

    fn read_seed_vec(input: &mut dyn Read, number_of_seeds: usize) -> std::io::Result<(Self, Box<[Self::VecElement]>)> {
        let bits_per_group_seed = SeedSize::read(input)?;
        Ok((bits_per_group_seed, read_bits(input, number_of_seeds * bits_per_group_seed.0 as usize)?))
    }
}

/// Seed size of 8 bits.
#[derive(Copy, Clone, Default)]
pub struct Bits8;

impl Into<u8> for Bits8 {
    #[inline] fn into(self) -> u8 { 8 }
}

impl TryFrom<u8> for Bits8 {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value == 8 { Ok(Self) } else { Err("Bits8 supports only 8-bit seeds.") }
    }
}

impl SeedSize for Bits8 {
    type VecElement = u8;

    fn new_seed_vec(&self, seed: u16, number_of_seeds: usize) -> Box<[Self::VecElement]> {
        vec![seed as u8; number_of_seeds].into_boxed_slice()
    }

    #[inline] fn get_seed(&self, vec: &[Self::VecElement], index: usize) -> u16 {
        vec[index] as u16
    }

    #[inline] fn set_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16) {
        vec[index] = seed as u8
    }

    fn read_seed_vec(input: &mut dyn Read, number_of_seeds: usize) -> std::io::Result<(Self, Box<[Self::VecElement]>)> {
        let bits_per_group_seed = SeedSize::read(input)?;   // mainly for validation
        Ok((bits_per_group_seed, AsIs::read_n(input, number_of_seeds)?))
    }

    fn write_seed_vec(&self, output: &mut dyn Write, seeds: &[Self::VecElement]) -> std::io::Result<()> {
        SeedSize::write(self, output)?;
        AsIs::write_all(output, seeds)
    }

    fn concatenate_seed_vecs(&self, _level_sizes: &[u64], group_seeds: Vec<Box<[Self::VecElement]>>) -> Box<[Self::VecElement]> {
        group_seeds.concat().into_boxed_slice()
    }
}

/// Seed size given as a power of two.
#[derive(Copy, Clone, Default)]
pub struct TwoToPowerBitsStatic<const LOG2_BITS: u8>;

impl<const LOG2_BITS: u8> TwoToPowerBitsStatic<LOG2_BITS> {
    const BITS: u8 = 1 << LOG2_BITS;
    const LOG2_MASK: u8 = Self::BITS-1;
    const VALUES_PER_64: u8 = 64 >> LOG2_BITS;
    const MASK16: u16 = ((1u64 << Self::BITS) - 1) as u16;
    const MASK64: u64 = ((1u128 << Self::BITS) - 1) as u64;
    #[inline(always)] const fn shift_for(index: usize) -> u8 {
        ((index % Self::VALUES_PER_64 as usize) as u8) << LOG2_BITS
    }
}

impl<const LOG2_BITS: u8> Into<u8> for TwoToPowerBitsStatic<LOG2_BITS> {
    #[inline] fn into(self) -> u8 { Self::BITS }
}

impl<const LOG2_BITS: u8> TryFrom<u8> for TwoToPowerBitsStatic<LOG2_BITS> {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value == Self::BITS { Ok(Self) } else { Err("Number of bits per seed differs from TwoToPowerBitsStatic parameter.") }
    }
}

impl<const LOG2_BITS: u8> SeedSize for TwoToPowerBitsStatic<LOG2_BITS> {
    type VecElement = u64;

    #[inline(always)] fn new_zeroed_seed_vec(&self, number_of_seeds: usize) -> Box<[Self::VecElement]> {
        Box::<[u64]>::with_zeroed_bits(number_of_seeds << LOG2_BITS)
    }

    #[inline(always)] fn new_seed_vec(&self, seed: u16, number_of_seeds: usize) -> Box<[Self::VecElement]> {
        let mut w = 0;
        for _ in 0..Self::VALUES_PER_64 { w <<= Self::BITS; w |= seed as u64; }
        vec![w; ceiling_div(number_of_seeds, Self::VALUES_PER_64 as usize)].into_boxed_slice()
        //Box::<[u64]>:: with_bitwords(seed as u64, Self::BITS, number_of_seeds)
    }

    #[inline(always)] fn get_seed(&self, vec: &[Self::VecElement], index: usize) -> u16 {
        (vec[index / Self::VALUES_PER_64 as usize] >> Self::shift_for(index)) as u16 & Self::MASK16
    }

    #[inline] fn set_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16) {
        let v = &mut vec[index / Self::VALUES_PER_64 as usize];
        let s = Self::shift_for(index);
        *v &= !((Self::MASK16 as u64) << s);
        *v |= (seed as u64) << s;
    }

    fn init_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16) {
        vec[index / Self::VALUES_PER_64 as usize] |= (seed as u64) << Self::shift_for(index);
    }

    fn write_seed_vec(&self, output: &mut dyn Write, seeds: &[Self::VecElement]) -> std::io::Result<()> {
        SeedSize::write(self, output)?;
        AsIs::write_all(output, seeds)
    }

    fn read_seed_vec(input: &mut dyn Read, number_of_seeds: usize) -> std::io::Result<(Self, Box<[Self::VecElement]>)> {
        let bits_per_group_seed = SeedSize::read(input)?;   // mainly for validation
        Ok((bits_per_group_seed, read_bits(input, number_of_seeds * Self::BITS as usize)?))
    }
}

impl<const LOG2_BITS: u8> Mul<usize> for TwoToPowerBitsStatic<LOG2_BITS> {
    type Output = usize;

    #[inline(always)] fn mul(self, rhs: usize) -> Self::Output {
        rhs << LOG2_BITS
    }
}

impl<const LOG2_BITS: u8> GroupSize for TwoToPowerBitsStatic<LOG2_BITS> {
    #[inline(always)] fn hash_to_group(&self, hash: u32) -> u8 {
        hash as u8 & Self::LOG2_MASK
    }

    fn level_size_groups_segments(&self, desired_total_size: usize) -> (usize, usize) {
        let level_size_segments;
        let level_size_groups;
        if LOG2_BITS > 6 {
            //level_size_segments = div_up(input_size << segments_per_group_log2, 64) << segments_per_group_log2;
            //level_size_groups = (level_size_segments >> segments_per_group_log2) as u32;
            level_size_groups = ceiling_div(desired_total_size, 1usize<<LOG2_BITS);
            level_size_segments = (level_size_groups as usize) << (LOG2_BITS - 6);
        } else {
            level_size_segments = ceiling_div(desired_total_size, 64);
            level_size_groups = level_size_segments << (6 - LOG2_BITS);
        }
        (level_size_groups, level_size_segments)
    }

    #[inline(always)] fn copy_group_if_better<CB>(&self, dst: &mut [u64], src: &[u64], group_index: usize, callback: CB)
        where CB: FnOnce()
    {
        let vec_index = group_index / Self::VALUES_PER_64 as usize;
        let shift = Self::shift_for(group_index);
        let dst_v = &mut dst[vec_index];
        let best = (*dst_v >> shift) & Self::MASK64;
        let new = (src[vec_index] >> shift) & Self::MASK64;
        if best.count_ones() < new.count_ones() {
            callback();
            *dst_v &= !(Self::MASK64 << shift);
            *dst_v |= new << shift;
        }
    }
}