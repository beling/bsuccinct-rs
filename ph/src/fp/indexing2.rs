use std::convert::{TryFrom, TryInto};
use std::hash::{Hasher, Hash};
use std::io::{Read, Write};
use std::ops::Mul;
use bitm::{BitAccess, BitVec, ceiling_div};
use dyn_size_of::GetSize;
use binout::{read_int, write_int};
use crate::read_array;
use crate::utils::map64_to_32;

/// Calculates group number for a given `key` at level of the size `level_size_groups` groups, whose number is already hashed in `hasher`.
/// Modifies `hasher`, which can be farther used to calculate index in the group by just writing to it the seed of the group.
#[inline]
pub(super) fn group_nr(hasher: &mut impl Hasher, key: &impl Hash, level_size_groups: u32) -> u32 {
    key.hash(hasher);
    map64_to_32(hasher.finish(), level_size_groups)
}

/// Implementations of `GroupSize` represent group size in fingerprinting-based minimal perfect hashing with group optimization.
pub trait GroupSize: Sized + Mul<usize, Output=usize> + Copy + Into<u8> + TryFrom<u8, Error=&'static str> {

    fn validate(&self) -> Result<Self, &'static str> { Ok(*self) }

    /// Returns `hash` modulo `self`.
    #[inline]
    fn hash_to_group(&self, hash: u64) -> u8 {
        map64_to_32(hash, Into::<u8>::into(*self) as u32) as u8
    }

    /// Returns index in the group with given seed `group_seed` using `hasher`
    /// which must be modified earlier by `group_nr` function.
    #[inline]
    fn in_group_index(&self, mut hasher: impl Hasher, group_seed: u16) -> u8 {
        hasher.write_u16(group_seed);
        self.hash_to_group(hasher.finish())
    }

    /// Returns bit index inside the group with number `group` and seed `group_seed`,
    /// assigned to the key hashed by the `hasher`.
    #[inline]
    fn bit_index_for_seed(&self, hasher: impl Hasher, group_seed: u16, group: u32) -> usize {
        (*self * group as usize) + self.in_group_index(hasher, group_seed) as usize
    }

    /// Returns number of groups and 64-bit segments for given `desired_total_size`.
    fn level_size_groups_segments(&self, mut desired_total_size: usize) -> (usize, usize) {
        let remainder = desired_total_size % 64;
        if remainder != 0 { desired_total_size += 64 - remainder; } // round up to multiple of 64
        let group_size = Into::<u8>::into(*self) as usize;
        while desired_total_size % group_size != 0 { desired_total_size += 64; } // and also to multiple of group_size
        return (desired_total_size / group_size, desired_total_size / 64);
    }

    #[inline] fn ones_in_group(&self, arr: &[u64], group_index: usize) -> u8 {
        arr.get_fragment(group_index, (*self).into()).count_ones() as u8
    }

    /// Returns number of bites that `self.write` writes to the output.
    #[inline] fn write_size_bytes(&self) -> usize {
        std::mem::size_of::<u8>()
    }

    /// Writes `self` to `output`.
    fn write(&self, output: &mut dyn std::io::Write) -> std::io::Result<()> {
        write_int!(output, (*self).into())
    }

    /// Reads `Self` from `input`.
    fn read(input: &mut dyn std::io::Read) -> std::io::Result<Self> {
        let group_size = read_int!(input, u8)?;
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

    fn concatenate_seed_vecs(&self, level_sizes: &[u32], group_seeds: Vec<Box<[Self::VecElement]>>) -> Box<[Self::VecElement]> {
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
    fn write(&self, output: &mut dyn std::io::Write) -> std::io::Result<()> {
        write_int!(output, (*self).into())
    }

    /// Reads `Self` from `input`.
    fn read(input: &mut dyn std::io::Read) -> std::io::Result<Self> {
        let seed_size = read_int!(input, u8)?;
        let result = TryInto::<Self>::try_into(seed_size).map_err(to_io_error)?;
        result.validate().map_err(to_io_error)
    }

    fn write_seed_vec(&self, output: &mut dyn std::io::Write, seeds: &[Self::VecElement]) -> std::io::Result<()>;

    fn read_seed_vec(input: &mut dyn std::io::Read, number_of_seeds: usize) -> std::io::Result<(Self, Box<[Self::VecElement]>)>;
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

    #[inline] fn mul(self, rhs: usize) -> Self::Output {
        rhs << self.log2size
    }
}

impl Into<u8> for TwoToPowerBits {
    #[inline] fn into(self) -> u8 {
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
    #[inline] fn hash_to_group(&self, hash: u64) -> u8 {
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

    fn ones_in_group(&self, arr: &[u64], group_index: usize) -> u8 {
        if self.log2size >= 6 {
            let log2_segments_per_group = self.log2size - 6;
            let segments_per_group = 1 << log2_segments_per_group;
            let begin_segment = group_index << log2_segments_per_group; // * segments_per_group
            arr[begin_segment..begin_segment+segments_per_group].iter().map(|v| v.count_ones() as u8).sum()
        } else {
            arr.get_fragment(group_index, (*self).into()).count_ones() as u8
        }
    }
}

#[derive(Copy, Clone)]
pub struct Bits(pub u8);

impl Mul<usize> for Bits {
    type Output = usize;

    #[inline] fn mul(self, rhs: usize) -> Self::Output {
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
        seeds.iter().try_for_each(|v| write_int!(output, v))
    }

    fn read_seed_vec(input: &mut dyn Read, number_of_seeds: usize) -> std::io::Result<(Self, Box<[Self::VecElement]>)> {
        let bits_per_group_seed = SeedSize::read(input)?;
        Ok((bits_per_group_seed, read_array!(number_of_seeds * bits_per_group_seed.0 as usize; bits from input).into_boxed_slice()))
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
        Ok((bits_per_group_seed, read_array!([u8; number_of_seeds] from input).into_boxed_slice()))
    }

    fn write_seed_vec(&self, output: &mut dyn Write, seeds: &[Self::VecElement]) -> std::io::Result<()> {
        SeedSize::write(self, output)?;
        seeds.iter().try_for_each(|v| write_int!(output, v))
    }

    fn concatenate_seed_vecs(&self, _level_sizes: &[u32], group_seeds: Vec<Box<[Self::VecElement]>>) -> Box<[Self::VecElement]> {
        group_seeds.concat().into_boxed_slice()
    }
}

/// Seed size given as a power of two.
#[derive(Copy, Clone, Default)]
pub struct TwoToPowerBitsStatic<const LOG2_BITS: u8>;

impl<const LOG2_BITS: u8> TwoToPowerBitsStatic<LOG2_BITS> {
    const BITS: u8 = 1 << LOG2_BITS;
    const VALUES_PER_64: u8 = 64 >> LOG2_BITS;
    const MASK: u16 = ((1u64 << Self::BITS) - 1) as u16;
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
        (vec[index / Self::VALUES_PER_64 as usize] >> Self::shift_for(index)) as u16 & Self::MASK
    }

    #[inline] fn set_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16) {
        let v = &mut vec[index / Self::VALUES_PER_64 as usize];
        let s = Self::shift_for(index);
        *v &= !((Self::MASK as u64) << s);
        *v |= (seed as u64) << s;
    }

    fn init_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16) {
        vec[index / Self::VALUES_PER_64 as usize] |= (seed as u64) << Self::shift_for(index);
    }

    fn write_seed_vec(&self, output: &mut dyn Write, seeds: &[Self::VecElement]) -> std::io::Result<()> {
        SeedSize::write(self, output)?;
        seeds.iter().try_for_each(|v| write_int!(output, v))
    }

    fn read_seed_vec(input: &mut dyn Read, number_of_seeds: usize) -> std::io::Result<(Self, Box<[Self::VecElement]>)> {
        let bits_per_group_seed = SeedSize::read(input)?;   // mainly for validation
        Ok((bits_per_group_seed, read_array!(number_of_seeds * Self::BITS as usize; bits from input).into_boxed_slice()))
    }
}



