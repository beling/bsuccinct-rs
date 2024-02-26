use std::convert::TryFrom;
use std::ops::Mul;
use binout::{AsIs, VByte, Serializer};

/// Represents the degree of the Huffman tree,
/// which is equal to the number of different
/// values of a single codeword fragment.
pub trait TreeDegree: Sized + Copy + Mul<u32, Output=u32> {
    /// Returns the degree of the Huffman tree as `u32`.
    fn as_u32(&self) -> u32;

    /// Returns number of bytes that `self.write` writes to the output.
    #[inline(always)] fn write_size_bytes(&self) -> usize {
        VByte::size(self.as_u32())
    }

    /// Writes `self` to `output`.
    #[inline(always)] fn write(&self, output: &mut dyn std::io::Write) -> std::io::Result<()> {
        VByte::write(output, self.as_u32())
    }

    /// Reads `Self` from `input`.
    fn read(input: &mut dyn std::io::Read) -> std::io::Result<Self>;

    /// Returns the `fragment_nr`-th fragment of `bits`. Result is less than `self.tree_degree()`.
    fn get_fragment(&self, bits: u32, fragment_nr: u32) -> u32;

    /// Appends the `fragment` (that must be less than `self.tree_degree`)
    /// to the least significant digit (bits) of `bits`.
    fn push_front(&self, bits: &mut u32, fragment: u32) {
        *bits = *self * *bits + fragment;
    }

    /// Removes from `bits` and returns its fragment stored on least significant digit (bits).
    fn pop_front(&self, bits: &mut u32) -> u32;

    /// Returns the largest number of fragments that can be explicitly stored in the code.
    /// Longer codes begin with a sequence of zeros and only their last fragments are explicitly represented.
    fn code_capacity(&self) -> u8;

    fn reverse_code(&self, bits: u32, len: u32) -> u32;

    //type MULTIPLIER; ??

    //fn first_multiplier(&self) -> u32;

    //fn copy_fragment_mult(&self, src_code: u32, src_divider: u32, dst_bits: &mut u32, dst_multiplier: u32);

    //fn push_at_mult(&self, multiplier: u32, bits: &mut u32, fragment: u32);

    //fn next_multiplier(multiplier: u32) -> u32; // multiplier + multiplier??
    //fn inc_multiplier(multiplier: &mut u32);

    // Returns the `fragment_nr`-th fragment of reversed `bits`.
    // Result is less than `self.tree_degree()`.
    //fn reversed_get_fragment(&self, bits: u32, len: u32, fragment_nr: u32) -> u32;

    // Appends the `fragment` (that must be less than `self.tree_degree`)
    // to the highest digits (bits) of `bits`.
    //fn reversed_push_front(&self, bits: u32, len: u32, fragment_nr: u32);
}

/// `BitsPerFragment` represents the Huffman's tree degree that is the power of two.
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

    #[inline(always)] fn write_size_bytes(&self) -> usize {
        std::mem::size_of::<u8>()
    }

    fn write(&self, output: &mut dyn std::io::Write) -> std::io::Result<()> {
        AsIs::write(output, self.0)
    }

    fn read(input: &mut dyn std::io::Read) -> std::io::Result<Self> {
        AsIs::read(input).map(|v| Self(v))
    }

    fn get_fragment(&self, bits: u32, fragment_nr: u32) -> u32 {
        bits.checked_shr(self.0 as u32 * fragment_nr).map_or(0, |v| v & ((1u32 << self.0) - 1))
        //(bits >> (bits_per_fragment as u32 * fragment_nr as u32)) & ((1u32 << bits_per_fragment as u32) - 1)
    }

    #[inline] fn push_front(&self, bits: &mut u32, fragment: u32) {
        *bits = *self * *bits | fragment;
    }

    #[inline] fn pop_front(&self, bits: &mut u32) -> u32 {
        let result = *bits & (self.as_u32() - 1);
        *bits >>= self.0;
        result
    }

    #[inline(always)] fn code_capacity(&self) -> u8 { 32 / self.0 }

    #[inline] fn reverse_code(&self, mut bits: u32, len: u32) -> u32 {
        //TODO faster code for self.0 == 1 or self.0 being power of 2
        if self.0 == 1 {    // very common and the slowest case
            return bits.reverse_bits() >> 32u32.saturating_sub(len);
        }
        let mut len = len.min(self.code_capacity() as u32) as u8;
        let mask = self.as_u32() - 1;
        let mut result = 0;
        while bits != 0 {
            result <<= self.0;
            result |= bits & mask;
            bits >>= self.0;
            len -= 1;
        }
        result << (self.0 * len)
    }

    /*fn reversed_get_fragment(&self, bits: u32, len: u32, fragment_nr: u32) -> u32 {
        todo!()
        let capacity = 32 / self.0;
        self.get_fragment(bits, len - fragment_nr)
    }

    fn reversed_push_front(&self, bits: u32, len: u32, fragment_nr: u32) {
        todo!()
    }*/
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

/// `Degree` represents the degree of the Huffman tree.
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
        VByte::read(input).map(|v| Self(v))
    }

    #[inline] fn get_fragment(&self, bits: u32, fragment_nr: u32) -> u32 {
        self.0.checked_pow(fragment_nr).map_or(0, |v| (bits/v) % self.0)
    }

    #[inline] fn code_capacity(&self) -> u8 {
        (1u64<<32).ilog(self.0 as u64) as u8
    }

    #[inline] fn pop_front(&self, bits: &mut u32) -> u32 {
        let result = *bits % self.0;
        *bits /= self.0;
        result
    }
    
    #[inline] fn reverse_code(&self, mut bits: u32, mut len: u32) -> u32 {
        //TODO use some lib for fast dividing
        let mut result = 0;
        let mut representable = ((1u64<<32) / self.0 as u64) as u32; // >0 only if we can add next digit to result
        while bits != 0 {
            result *= self.0;
            result += bits % self.0;
            bits /= self.0;
            len -= 1;
            if representable == 0 { return result; }
            representable /= self.0;
        }
        while len != 0 && representable != 0 {
            result *= self.0;
            len -= 1;
            representable /= self.0;
        }        
        result
    }

    
}

impl From<BitsPerFragment> for Degree {
    fn from(bits_per_fragment: BitsPerFragment) -> Self {
        Self(bits_per_fragment.as_u32())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_degree_2(d: impl TreeDegree) {
        assert_eq!(d.get_fragment(0b1011, 0), 1);
        assert_eq!(d.get_fragment(0b1011, 1), 1);
        assert_eq!(d.get_fragment(0b1011, 2), 0);
        assert_eq!(d.get_fragment(0b1011, 3), 1);
        assert_eq!(d.get_fragment(0b1011, 4), 0);
        assert_eq!(d.get_fragment(0b1011, 17), 0);
        assert_eq!(d.get_fragment(0b1011, 31), 0);
        assert_eq!(d.get_fragment(0b1011, 32), 0);
        assert_eq!(d.get_fragment(0b1011, 33), 0);
        assert_eq!(d.code_capacity(), 32);
        assert_eq!(d.reverse_code(0b1011, 4), 0b1101);
        assert_eq!(d.reverse_code(0b1011, 5), 0b11010);
        assert_eq!(d.reverse_code(0b1011, 7), 0b1101000);
        assert_eq!(d.reverse_code(0b1011, 31), 0b01101000_00000000_00000000_00000000);
        assert_eq!(d.reverse_code(0b1011, 32), 0b11010000_00000000_00000000_00000000);
        assert_eq!(d.reverse_code(0b1011, 33), 0b11010000_00000000_00000000_00000000);
        assert_eq!(d.reverse_code(0b1011, 44), 0b11010000_00000000_00000000_00000000);
    }

    #[test]
    fn bits_per_fragment_1() {
        check_degree_2(BitsPerFragment(1))
    }
    
    #[test]
    fn degree_2() {
        check_degree_2(Degree(2));
    }

    fn check_degree_4(d: impl TreeDegree) {
        assert_eq!(d.get_fragment(0b10_00_11, 0), 0b11);
        assert_eq!(d.get_fragment(0b10_00_11, 1), 0b00);
        assert_eq!(d.get_fragment(0b10_00_11, 2), 0b10);
        assert_eq!(d.get_fragment(0b10_00_11, 3), 0b00);
        assert_eq!(d.get_fragment(0b10_00_11, 8), 0b00);
        assert_eq!(d.get_fragment(0b10_00_11, 15), 0b00);
        assert_eq!(d.get_fragment(0b10_00_11, 16), 0b00);
        assert_eq!(d.get_fragment(0b10_00_11, 17), 0b00);
        assert_eq!(d.get_fragment(0b10_00_11, 33), 0b00);
        assert_eq!(d.code_capacity(), 16);
        assert_eq!(d.reverse_code(0b10_00_11, 3), 0b11_00_10);
        assert_eq!(d.reverse_code(0b10_00_11, 4), 0b11_00_10_00);
        assert_eq!(d.reverse_code(0b10_00_11, 5), 0b11_00_10_00_00);
        assert_eq!(d.reverse_code(0b10_00_11, 15), 0b11_00_10_00_00__00000_00000_00000_00000);
        assert_eq!(d.reverse_code(0b10_00_11, 16), 0b11_00_10_00_00_00__00000_00000_00000_00000);
        assert_eq!(d.reverse_code(0b10_00_11, 17), 0b11_00_10_00_00_00__00000_00000_00000_00000);
        assert_eq!(d.reverse_code(0b10_00_11, 20), 0b11_00_10_00_00_00__00000_00000_00000_00000);
    }

    #[test]
    fn bits_per_fragment_2() {
        check_degree_4(BitsPerFragment(2));
    }

    #[test]
    fn degree_4() {
        check_degree_4(Degree(4));
    }

    fn check_degree_8(d: impl TreeDegree) {
        assert_eq!(d.get_fragment(0b100_000_111, 0), 0b111);
        assert_eq!(d.get_fragment(0b100_000_111, 1), 0b000);
        assert_eq!(d.get_fragment(0b100_000_111, 2), 0b100);
        assert_eq!(d.get_fragment(0b100_000_111, 3), 0b000);
        assert_eq!(d.get_fragment(0b100_000_111, 10), 0b000);
        assert_eq!(d.get_fragment(0b100_000_111, 20), 0b000);
        assert_eq!(d.code_capacity(), 10);
        assert_eq!(d.reverse_code(0b100_000_111, 3), 0b111_000_100);
        assert_eq!(d.reverse_code(0b100_000_111, 4), 0b111_000_100_000);
        assert_eq!(d.reverse_code(0b100_000_111, 5), 0b111_000_100_000_000);
        assert_eq!(d.reverse_code(0b100_000_111, 9), 0b111_000_100_000_000_000_000_000_000);
        assert_eq!(d.reverse_code(0b100_000_111, 10), 0b111_000_100_000_000_000_000_000_000_000);
        assert_eq!(d.reverse_code(0b100_000_111, 11), 0b111_000_100_000_000_000_000_000_000_000);
        assert_eq!(d.reverse_code(0b100_000_111, 12), 0b111_000_100_000_000_000_000_000_000_000);
        assert_eq!(d.reverse_code(0b100_000_111, 19), 0b111_000_100_000_000_000_000_000_000_000);
    }
    
    #[test]
    fn bits_per_fragment_3() {
        check_degree_8(BitsPerFragment(3));
    }

    #[test]
    fn degree_8() {
        check_degree_8(Degree(8));
    }

    #[test]
    fn degree_27() {
        let d = Degree(27);
        let bits = 19 * (27*27) + 5 * 27 + 26;
        assert_eq!(d.get_fragment(bits, 0), 26);
        assert_eq!(d.get_fragment(bits, 1), 5);
        assert_eq!(d.get_fragment(bits, 2), 19);
        assert_eq!(d.get_fragment(bits, 3), 0);
        assert_eq!(d.get_fragment(bits, 5), 0);
        assert_eq!(d.get_fragment(bits, 6), 0);
        assert_eq!(d.get_fragment(bits, 11), 0);
        assert_eq!(d.code_capacity(), 6);
        let reversed = 26 * (27*27) + 5 * 27 + 19;
        assert_eq!(d.reverse_code(bits, 3), reversed);
        assert_eq!(d.reverse_code(bits, 4), reversed * 27);
        assert_eq!(d.reverse_code(bits, 5), reversed * 27 * 27);
        assert_eq!(d.reverse_code(bits, 6), reversed * 27 * 27 * 27);
        assert_eq!(d.reverse_code(bits, 7), reversed * 27 * 27 * 27);
        assert_eq!(d.reverse_code(bits, 11), reversed * 27 * 27 * 27);
    }
}