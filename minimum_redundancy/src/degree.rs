use std::convert::TryFrom;
use crate::{read_int, write_int};

pub trait FragmentSize: Sized + Copy {
    /// Returns the range of a single fragment.
    fn tree_degree(&self) -> u32;

    /// Returns `tree_degree() * rhs`.
    #[inline(always)] fn tree_degree_times(&self, rhs: u32) -> u32 {
        self.tree_degree() * rhs
    }

    /// Returns number of bites that `self.write` writes to the output.
    #[inline(always)] fn write_bytes(&self) -> usize {
        std::mem::size_of::<u32>()
    }

    /// Writes `self` to `output`.
    #[inline(always)] fn write(&self, output: &mut dyn std::io::Write) -> std::io::Result<()> {
        write_int!(output, self.tree_degree())
    }

    /// Reads `Self` from `input`.
    fn read(input: &mut dyn std::io::Read) -> std::io::Result<Self>;

    /// Returns the `fragment_nr`-th fragment of `bits`. Result is less than `self.tree_degree()`.
    fn get_fragment(&self, bits: u32, fragment_nr: u32) -> u32;

    /// Appends the `fragment` (that must be less than `self.tree_degree`) to the lowest digits (bits) of `bits`.
    fn push_front(&self, bits: &mut u32, fragment: u32) {
        *bits = self.tree_degree_times(*bits) + fragment;
    }
}

/// Number of bits per code fragment.
/// Codewords assigned to values have bit-lengths dividable by `bits_per_fragment`.
#[derive(Copy, Clone)]
pub struct BitsPerFragment(pub u8);

impl FragmentSize for BitsPerFragment {
    #[inline(always)] fn tree_degree(&self) -> u32 { 1u32 << self.0 }

    #[inline(always)] fn tree_degree_times(&self, rhs: u32) -> u32 {
        rhs << self.0
    }

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
        *bits = self.tree_degree_times(*bits) | fragment;
    }
}

impl TryFrom<TreeDegree> for BitsPerFragment {
    type Error = &'static str;

    fn try_from(value: TreeDegree) -> Result<Self, Self::Error> {
        if value.0.is_power_of_two() {  // power of 2?
            Ok(Self(value.0.trailing_zeros() as u8))
        } else {
            Err("BitsPerFragment requires the tree degree to be a power of two")
        }
    }
}

/// Degree of the tree.
/// Each internal node (excepting at most one at the lowest level) has `tree_degree` children.
#[derive(Copy, Clone)]
pub struct TreeDegree(pub u32);

impl FragmentSize for TreeDegree {
    #[inline(always)] fn tree_degree(&self) -> u32 {
        self.0
    }

    fn read(input: &mut dyn std::io::Read) -> std::io::Result<Self> {
        read_int!(input, u32).map(|v| Self(v))
    }

    fn get_fragment(&self, bits: u32, fragment_nr: u32) -> u32 {
        self.0.checked_pow(fragment_nr).map_or(0, |v| (bits/v) % self.0)
    }
}

impl From<BitsPerFragment> for TreeDegree {
    fn from(bits_per_fragment: BitsPerFragment) -> Self {
        Self(bits_per_fragment.tree_degree())
    }
}