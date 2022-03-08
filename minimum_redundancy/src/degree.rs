use std::convert::TryFrom;
use std::ops::Mul;
use crate::{read_int, write_int};

/// Represents the degree of the Hufmann tree,
/// which is equal to the number of different
/// values of a single codeword fragment.
pub trait TreeDegree: Sized + Copy + Mul<u32, Output=u32> {
    /// Returns the degree of the Hufmann tree as u32.
    fn as_u32(&self) -> u32;

    /// Returns number of bites that `self.write` writes to the output.
    #[inline(always)] fn write_bytes(&self) -> usize {
        std::mem::size_of::<u32>()
    }

    /// Writes `self` to `output`.
    #[inline(always)] fn write(&self, output: &mut dyn std::io::Write) -> std::io::Result<()> {
        write_int!(output, self.as_u32())
    }

    /// Reads `Self` from `input`.
    fn read(input: &mut dyn std::io::Read) -> std::io::Result<Self>;

    /// Returns the `fragment_nr`-th fragment of `bits`. Result is less than `self.tree_degree()`.
    fn get_fragment(&self, bits: u32, fragment_nr: u32) -> u32;

    /// Appends the `fragment` (that must be less than `self.tree_degree`) to the lowest digits (bits) of `bits`.
    fn push_front(&self, bits: &mut u32, fragment: u32) {
        *bits = *self * *bits + fragment;
    }
}

/// `BitsPerFragment` represents the Hufmann's tree degree that is the power of two.
/// It represents number of bits needed to store the degree.
/// It can be used to construct minimum-redundancy coding whose
/// codeword lengths are a multiple of this number of bits.
/// It is faster than `Degree` and should be preferred
/// for degrees that are the powers of two.
#[derive(Copy, Clone)]
pub struct BitsPerFragment(pub u8);

impl Mul<u32> for BitsPerFragment {
    type Output = u32;

    #[inline(always)] fn mul(self, rhs: u32) -> Self::Output {
        rhs << self.0
    }
}

impl TreeDegree for BitsPerFragment {
    #[inline(always)] fn as_u32(&self) -> u32 { 1u32 << self.0 }

    #[inline(always)] fn write_bytes(&self) -> usize {
        std::mem::size_of::<u8>()
    }

    fn write(&self, output: &mut dyn std::io::Write) -> std::io::Result<()> {
        write_int!(output, self.0)
    }

    fn read(input: &mut dyn std::io::Read) -> std::io::Result<Self> {
        read_int!(input, u8).map(|v| Self(v))
    }

    fn get_fragment(&self, bits: u32, fragment_nr: u32) -> u32 {
        bits.checked_shr(self.0 as u32 * fragment_nr).map_or(0, |v| v & ((1u32 << self.0) - 1))
        //(bits >> (bits_per_fragment as u32 * fragment_nr as u32)) & ((1u32 << bits_per_fragment as u32) - 1)
    }

    fn push_front(&self, bits: &mut u32, fragment: u32) {
        *bits = *self * *bits | fragment;
    }
}

impl TryFrom<Degree> for BitsPerFragment {
    type Error = &'static str;

    fn try_from(value: Degree) -> Result<Self, Self::Error> {
        if value.0.is_power_of_two() {  // power of 2?
            Ok(Self(value.0.trailing_zeros() as u8))
        } else {
            Err("BitsPerFragment requires the tree degree to be a power of two")
        }
    }
}

/// `Degree` represents the degree of the Hufmann tree.
/// It is slower than `BitsPerFragment` and should be avoided
/// when the degree is the power of two.
#[derive(Copy, Clone)]
pub struct Degree(pub u32);

impl Mul<u32> for Degree {
    type Output = u32;

    #[inline(always)] fn mul(self, rhs: u32) -> Self::Output {
        self.0 * rhs
    }
}

impl TreeDegree for Degree {
    #[inline(always)] fn as_u32(&self) -> u32 {
        self.0
    }

    fn read(input: &mut dyn std::io::Read) -> std::io::Result<Self> {
        read_int!(input, u32).map(|v| Self(v))
    }

    fn get_fragment(&self, bits: u32, fragment_nr: u32) -> u32 {
        self.0.checked_pow(fragment_nr).map_or(0, |v| (bits/v) % self.0)
    }
}

impl From<BitsPerFragment> for Degree {
    fn from(bits_per_fragment: BitsPerFragment) -> Self {
        Self(bits_per_fragment.as_u32())
    }
}