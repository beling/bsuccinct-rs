use std::convert::{TryFrom, TryInto};
use std::io::{Read, Write};
use std::ops::Mul;
use binout::{AsIs, Serializer};
use bitm::{BitAccess, BitVec, ceiling_div};
use dyn_size_of::GetSize;
use crate::utils::read_bits;

pub fn to_io_error<E>(err: E) -> std::io::Error
where E: Into<Box<dyn std::error::Error + Send + Sync>> {
    std::io::Error::new(std::io::ErrorKind::InvalidData, err)
}

/// Implementations of `SeedSize` represent seed size in fingerprinting-based minimal perfect hashing with group optimization.
pub trait SeedSize: Copy + Into<u8> + Sync + TryFrom<u8, Error=&'static str> {
    type VecElement: Copy + Send + Sync + Sized + GetSize;
    const VEC_ELEMENT_BIT_SIZE: usize = size_of::<Self::VecElement>() * 8;

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

    fn concatenate_seed_vecs<LSI, LS>(&self, level_sizes: LS, seeds: Vec<Box<[Self::VecElement]>>) -> Box<[Self::VecElement]> 
        where LSI: IntoIterator<Item = usize>, LS: Fn() -> LSI
    {
        let mut group_seeds_concatenated = self.new_zeroed_seed_vec(level_sizes().into_iter().sum::<usize>());
        let mut dst_group = 0;
        for (l_size, l_seeds) in level_sizes().into_iter().zip(seeds.into_iter()) {
            for index in 0..l_size {
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

/// Size in bits.
#[cfg_attr(feature = "epserde", derive(epserde::Epserde), epserde_deep_copy)]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg, mem_dbg::MemSize))]
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
        //vec.get_fragment(index, self.0) as u16
        unsafe{ vec.get_fragment_unchecked(index, self.0) as u16 }
    }

    #[inline(always)] fn set_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16) {
        debug_assert!(seed < (1<<self.0));
        //vec.set_fragment(index, seed as u64, self.0)
        unsafe { vec.set_fragment_unchecked(index, seed as u64, self.0); }
    }

    #[inline(always)] fn init_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16) {
        debug_assert!(seed < (1<<self.0));
        unsafe { vec.init_fragment_unchecked(index, seed as u64, self.0) }
    }

    fn write_seed_vec(&self, output: &mut dyn Write, seeds: &[Self::VecElement]) -> std::io::Result<()> {
        SeedSize::write(self, output)?;
        AsIs::write_all(output, seeds)
    }

    fn read_seed_vec(input: &mut dyn Read, number_of_seeds: usize) -> std::io::Result<(Self, Box<[Self::VecElement]>)> {
        let bits_per_seed = SeedSize::read(input)?;
        Ok((bits_per_seed, read_bits(input, number_of_seeds * bits_per_seed.0 as usize)?))
    }
}

/// Size in bits.
/// 
/// Uses unaligned reads/writes to access data in SeedSize implementation.
#[cfg_attr(feature = "epserde", derive(epserde::Epserde), epserde_deep_copy)]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg, mem_dbg::MemSize))]
#[derive(Copy, Clone)]
pub struct BitsFast(pub u8);

impl Mul<usize> for BitsFast {
    type Output = usize;

    #[inline(always)] fn mul(self, rhs: usize) -> Self::Output {
        self.0 as usize * rhs
    }
}

impl Into<u8> for BitsFast {
    #[inline(always)] fn into(self) -> u8 { self.0 }
}

impl TryFrom<u8> for BitsFast {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(Self(value))
    }
}

impl BitsFast {
    #[inline]
    fn vec_len(&self, number_of_seeds: usize) -> usize {
        ceiling_div(number_of_seeds * self.0 as usize, 8) + 3
    }
}

impl SeedSize for BitsFast {
    type VecElement = u8;

    #[inline(always)] fn new_zeroed_seed_vec(&self, number_of_seeds: usize) -> Box<[Self::VecElement]> {
        vec![0; self.vec_len(number_of_seeds)].into_boxed_slice()
    }

    #[inline(always)] fn new_seed_vec(&self, seed: u16, number_of_seeds: usize) -> Box<[Self::VecElement]> {
        let mut vec = Self::new_zeroed_seed_vec(&self, number_of_seeds);
        for index in 0..number_of_seeds { self.init_seed(&mut vec, index, seed); }
        vec
    }

    #[inline(always)] fn get_seed(&self, vec: &[Self::VecElement], index: usize) -> u16 {
        (unsafe{ bitm::get_bits25(vec.as_ptr(), index * self.0 as usize) } & ((1<<self.0)-1)) as u16
    }

    #[inline(always)] fn set_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16) {
        debug_assert!(seed < (1<<self.0));
        unsafe { bitm::set_bits25(vec.as_mut_ptr(), index * self.0 as usize, seed as u32, (1<<self.0)-1) }
    }

    #[inline(always)] fn init_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16) {
        debug_assert!(seed < (1<<self.0));
        unsafe { bitm::init_bits25(vec.as_mut_ptr(), index * self.0 as usize, seed as u32) }
    }

    fn write_seed_vec(&self, output: &mut dyn Write, seeds: &[Self::VecElement]) -> std::io::Result<()> {
        SeedSize::write(self, output)?;
        AsIs::write_all(output, seeds)
    }

    fn read_seed_vec(input: &mut dyn Read, number_of_seeds: usize) -> std::io::Result<(Self, Box<[Self::VecElement]>)> {
        let bits_per_seed = SeedSize::read(input)?;
        Ok((bits_per_seed, AsIs::read_n(input, bits_per_seed.vec_len(number_of_seeds))?))
    }
}

/// Seed size of 8 bits.
#[cfg_attr(feature = "epserde", derive(epserde::Epserde), epserde_deep_copy)]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg, mem_dbg::MemSize))]
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
        //vec[index] as u16
        unsafe { *vec.get_unchecked(index) as u16 }
    }

    #[inline] fn set_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16) {
        debug_assert!(seed < 256);
        //vec[index] = seed as u8
        unsafe { *vec.get_unchecked_mut(index) = seed as u8 }
    }

    fn read_seed_vec(input: &mut dyn Read, number_of_seeds: usize) -> std::io::Result<(Self, Box<[Self::VecElement]>)> {
        let bits_per_group_seed = SeedSize::read(input)?;   // mainly for validation
        Ok((bits_per_group_seed, AsIs::read_n(input, number_of_seeds)?))
    }

    fn write_seed_vec(&self, output: &mut dyn Write, seeds: &[Self::VecElement]) -> std::io::Result<()> {
        SeedSize::write(self, output)?;
        AsIs::write_all(output, seeds)
    }

    fn concatenate_seed_vecs<LSI, LS>(&self, _level_sizes: LS, group_seeds: Vec<Box<[Self::VecElement]>>) -> Box<[Self::VecElement]> 
        where LSI: IntoIterator<Item = usize>, LS: Fn() -> LSI
    {
        group_seeds.concat().into_boxed_slice()
    }
}

/// Seed size given as a power of two (knowing at compile time).
#[cfg_attr(feature = "epserde", derive(epserde::Epserde), epserde_deep_copy)]
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemDbg, mem_dbg::MemSize))]
#[derive(Copy, Clone, Default)]
pub struct TwoToPowerBitsStatic<const LOG2_BITS: u8>;

impl<const LOG2_BITS: u8> TwoToPowerBitsStatic<LOG2_BITS> {
    pub const BITS: u8 = 1 << LOG2_BITS;
    pub const LOG2_MASK: u8 = Self::BITS-1;
    pub const VALUES_PER_64: u8 = 64 >> LOG2_BITS;
    pub const MASK16: u16 = ((1u64 << Self::BITS) - 1) as u16;
    pub const MASK64: u64 = ((1u128 << Self::BITS) - 1) as u64;
    #[inline(always)] pub const fn shift_for(index: usize) -> u8 {
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
        //(vec[index / Self::VALUES_PER_64 as usize] >> Self::shift_for(index)) as u16 & Self::MASK16
        unsafe {
            (*vec.get_unchecked(index / Self::VALUES_PER_64 as usize) >> Self::shift_for(index)) as u16 & Self::MASK16
        }
    }

    #[inline] fn set_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16) {
        debug_assert!(seed < (1<<Self::BITS));
        //let v = &mut vec[index / Self::VALUES_PER_64 as usize];
        let v = unsafe { vec.get_unchecked_mut(index / Self::VALUES_PER_64 as usize) };
        let s = Self::shift_for(index);
        *v &= !((Self::MASK16 as u64) << s);
        *v |= (seed as u64) << s;
    }

    #[inline] fn init_seed(&self, vec: &mut [Self::VecElement], index: usize, seed: u16) {
        debug_assert!(seed < (1<<Self::BITS));
        //vec[index / Self::VALUES_PER_64 as usize] |= (seed as u64) << Self::shift_for(index);
        unsafe{ *vec.get_unchecked_mut(index / Self::VALUES_PER_64 as usize) |= (seed as u64) << Self::shift_for(index); }
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