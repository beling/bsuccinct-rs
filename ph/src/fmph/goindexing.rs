//! Utils for indexing with group optimization.

use std::convert::{TryFrom, TryInto};
use std::ops::Mul;
use binout::{AsIs, Serializer};
use bitm::{BitAccess, ceiling_div};
use crate::seeds::{to_io_error, Bits, TwoToPowerBitsStatic};
use crate::utils::{map32_to_32, map64_to_64};

/// Calculates group number for a given `key` at level of the size `level_size_groups` groups, whose number is already hashed in `hasher`.
/// Modifies `hasher`, which can be farther used to calculate index in the group by just writing to it the seed of the group.
#[inline(always)]
pub fn group_nr(hash: u64, level_size_groups: usize) -> usize {
    //map64_to_32(hash, level_size_groups)
    //map32_to_32((hash >> 32) as u32, level_size_groups as u32) as u64
    //map48_to_64(hash >> 16, level_size_groups)
    map64_to_64(hash, level_size_groups as u64) as usize    // note that the lowest x bits of hash are ignored by map64_to_64 if level_size_groups < 2^(64-x) and it is save to use the lowest bits by in_group_index
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
    fn bit_index_for_seed(&self, hash: u64, group_seed: u16, group: usize) -> usize {
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

/// Size being the power of two.
#[derive(Copy, Clone)]
pub struct TwoToPowerBits {
    log2size: u8,
    mask: u8
}

impl TwoToPowerBits {
    /// Returns [`TwoToPowerBits`] that represent two to power of `log2size`.
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

impl GroupSize for Bits {
    fn validate(&self) -> Result<Self, &'static str> {
        if self.0 <= 63 { Ok(*self) } else { Err("group sizes grater than 63 are not supported by Bits") }
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