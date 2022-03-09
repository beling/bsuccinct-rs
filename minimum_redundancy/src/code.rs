//! Tools to deal with codewords.

use crate::TreeDegree;

/// Represents a codeword.
#[derive(PartialEq, Eq, Debug, Clone, Copy, Default)]
pub struct Code {
    /// Concatenated fragments of the codeword. The lowest bits contain the first fragment.
    /// It stores only few last fragments, and can represent the code with length > 32 bits, with zeroed few first fragments.
    pub bits: u32,
    /// Number of fragments.
    pub fragments: u32
}

impl Code {
    /// Appends the `fragment` (that must be less than `degree.as_u32()`) to the end of `self`.
    #[inline] pub fn push(&mut self, fragment: u32, degree: impl TreeDegree) {
        degree.push_front(&mut self.bits, fragment)
    }

    /// Gets `fragment_nr`-th fragment from the end.
    #[inline] pub fn get_r(&self, fragment_nr: u32, degree: impl TreeDegree) -> u32 {
        degree.get_fragment(self.bits, fragment_nr)
        //get_u32_fragment(self.bits, fragment_nr, self.bits_per_fragment)
        //(self.bits >> (self.bits_per_fragment as u32 * fragment_nr as u32)) & ((1u32 << self.bits_per_fragment as u32) - 1)
    }

    /// Gets `fragment_nr`-th fragment.
    #[inline] pub fn get(&self, fragment_nr: u32, degree: impl TreeDegree) -> u32 {
        degree.get_fragment(self.bits, self.fragments - fragment_nr - 1)
    }

    /// Extracts and returns first, remaining code fragment.
    pub fn extract_first(&mut self, degree: impl TreeDegree) -> Option<u32> {
        (self.fragments != 0).then(|| {
            self.fragments -= 1;
            self.get_r(self.fragments, degree)
        })

        /*self.fragments -= 1;
        let shift = self.bits_per_fragment as u32 * self.fragments as u32;
        return if let Some(shifted) = self.bits.checked_shr(shift) {
            let result = shifted & ((1u32 << self.bits_per_fragment as u32) - 1);
            self.bits ^= result << shift;
            result
        } else {
            0
        }*/
    }

    /// Returns whether `self` consists of zero fragments.
    #[inline] pub fn is_empty(&self) -> bool { self.fragments == 0 }
}

/*impl ExactSizeIterator for Code {
    /// Returns the length of `self` in fragments.
    fn len(&self) -> usize { self.fragments as usize }
}

impl Iterator for Code {
    type Item = u32;

    /// Extracts and returns the first fragment of `self` or returns `None` of `self` is empty.
    fn next(&mut self) -> Option<u32> {
        (self.fragments != 0).then(|| self.extract_first())
        //if self.fragments == 0 { None } else { Some(self.extract_first()) }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.fragments as usize, Some(self.fragments as usize))
    }
}*/

#[cfg(test)]
mod tests {
    use crate::{BitsPerFragment, Degree};
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn code_2bits() {
        let mut code = Code { bits: 0b_11_10_01, fragments: 3 };
        assert_eq!(code.get_r(0, BitsPerFragment(2)), 0b01);
        assert_eq!(code.get_r(1, BitsPerFragment(2)), 0b10);
        assert_eq!(code.get_r(2, BitsPerFragment(2)), 0b11);
        assert_eq!(code.get(0, BitsPerFragment(2)), 0b11);
        assert_eq!(code.get(2, BitsPerFragment(2)), 0b01);
        assert_eq!(code.fragments, 3);
        assert_eq!(code.extract_first(BitsPerFragment(2)), Some(0b11));
        assert_eq!(code.fragments, 2);
        assert_eq!(code.extract_first(BitsPerFragment(2)), Some(0b10));
        assert_eq!(code.fragments, 1);
        assert_eq!(code.extract_first(BitsPerFragment(2)), Some(0b01));
        assert_eq!(code.fragments, 0);
        assert_eq!(code.extract_first(BitsPerFragment(2)), None);
    }

    #[test]
    fn code_tree_degree3() {
        let mut code = Code { bits: 1*3*3 + 0*3 + 2, fragments: 3 };
        assert_eq!(code.get_r(0, Degree(3)), 2);
        assert_eq!(code.get_r(1, Degree(3)), 0);
        assert_eq!(code.get_r(2, Degree(3)), 1);
        assert_eq!(code.get(2, Degree(3)), 2);
        assert_eq!(code.get(1, Degree(3)), 0);
        assert_eq!(code.get(0, Degree(3)), 1);
        assert_eq!(code.fragments, 3);
        assert_eq!(code.extract_first(Degree(3)), Some(1));
        assert_eq!(code.fragments, 2);
        assert_eq!(code.extract_first(Degree(3)), Some(0));
        assert_eq!(code.fragments, 1);
        assert_eq!(code.extract_first(Degree(3)), Some(2));
        assert_eq!(code.fragments, 0);
        assert_eq!(code.extract_first(Degree(3)), None);
    }
}