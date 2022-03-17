//! Tools to deal with codewords.

use std::iter::FusedIterator;
use crate::TreeDegree;

/// Represents a codeword.
#[derive(PartialEq, Eq, Debug, Clone, Copy, Default)]
pub struct Code {
    /// Concatenated fragments of the codeword. The most significant bits contain the first fragment.
    /// It stores only few last fragments, and can represent the code with length > 32 bits, with zeroed few first fragments.
    pub content: u32,
    /// Length of the code in fragments.
    pub len: u32
}

impl Code {
    /// Appends the `fragment` (that must be less than `degree.as_u32()`) to the end of `self`.
    #[inline] pub fn push(&mut self, fragment: u32, degree: impl TreeDegree) {
        degree.push_front(&mut self.content, fragment)
    }

    /// Gets `fragment_nr`-th fragment from the end.
    #[inline(always)] pub unsafe fn get_rev_unchecked(&self, fragment_nr: u32, degree: impl TreeDegree) -> u32 {
        degree.get_fragment(self.content, fragment_nr)
    }

    /// Gets `fragment_nr`-th fragment from the end.
    #[inline] pub fn get_rev(&self, fragment_nr: u32, degree: impl TreeDegree) -> Option<u32> {
        (fragment_nr < self.len).then(|| degree.get_fragment(self.content, fragment_nr))
    }

    /// Gets `fragment_nr`-th fragment.
    #[inline] pub unsafe fn get_unchecked(&self, fragment_nr: u32, degree: impl TreeDegree) -> u32 {
        self.get_rev_unchecked(self.len - fragment_nr - 1, degree)
    }

    #[inline] pub fn get(&self, fragment_nr: u32, degree: impl TreeDegree) -> Option<u32> {
        (fragment_nr < self.len).then(|| unsafe { self.get_unchecked(fragment_nr, degree) })
    }

    /// Extracts and returns first, remaining code fragment.
    pub fn extract_first(&mut self, degree: impl TreeDegree) -> Option<u32> {
        (self.len != 0).then(|| {
            self.len -= 1;
            unsafe { self.get_rev_unchecked(self.len, degree) }
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
    #[inline] pub fn is_empty(&self) -> bool { self.len == 0 }

    /// Returns iterator over the fragments of code.
    #[inline] pub fn iter<D: TreeDegree>(&self, degree: D) -> CodeIterator<D> {
        CodeIterator { code: *self, degree }
    }
}

/// Iterator over the fragments of code.
pub struct CodeIterator<D: TreeDegree> {
    code: Code,
    degree: D
}

impl<D: TreeDegree> FusedIterator for CodeIterator<D> {}

impl<D: TreeDegree> ExactSizeIterator for CodeIterator<D> {
    #[inline] fn len(&self) -> usize {
        self.code.len as usize
    }
}

impl<D: TreeDegree> Iterator for CodeIterator<D> {
    type Item = u32;

    #[inline] fn next(&mut self) -> Option<Self::Item> {
        self.code.extract_first(self.degree)
    }

    #[inline] fn size_hint(&self) -> (usize, Option<usize>) {
        let l = self.len(); (l, Some(l))
    }
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
        let mut code = Code { content: 0b_11_10_01, len: 3 };
        assert_eq!(code.get_rev(0, BitsPerFragment(2)).unwrap(), 0b01);
        assert_eq!(code.get_rev(1, BitsPerFragment(2)).unwrap(), 0b10);
        assert_eq!(code.get_rev(2, BitsPerFragment(2)).unwrap(), 0b11);
        assert_eq!(code.get(0, BitsPerFragment(2)).unwrap(), 0b11);
        assert_eq!(code.get(2, BitsPerFragment(2)).unwrap(), 0b01);
        assert_eq!(code.iter(BitsPerFragment(2)).collect::<Vec<_>>(), [0b11, 0b10, 0b01]);
        assert_eq!(code.len, 3);
        assert_eq!(code.extract_first(BitsPerFragment(2)), Some(0b11));
        assert_eq!(code.len, 2);
        assert_eq!(code.extract_first(BitsPerFragment(2)), Some(0b10));
        assert_eq!(code.len, 1);
        assert_eq!(code.extract_first(BitsPerFragment(2)), Some(0b01));
        assert_eq!(code.len, 0);
        assert_eq!(code.extract_first(BitsPerFragment(2)), None);
    }

    #[test]
    fn code_tree_degree3() {
        let mut code = Code { content: 1*3*3 + 0*3 + 2, len: 3 };
        assert_eq!(code.get_rev(0, Degree(3)).unwrap(), 2);
        assert_eq!(code.get_rev(1, Degree(3)).unwrap(), 0);
        assert_eq!(code.get_rev(2, Degree(3)).unwrap(), 1);
        assert_eq!(code.get(2, Degree(3)).unwrap(), 2);
        assert_eq!(code.get(1, Degree(3)).unwrap(), 0);
        assert_eq!(code.get(0, Degree(3)).unwrap(), 1);
        assert_eq!(code.iter(Degree(3)).collect::<Vec<_>>(), [1, 0, 2]);
        assert_eq!(code.len, 3);
        assert_eq!(code.extract_first(Degree(3)), Some(1));
        assert_eq!(code.len, 2);
        assert_eq!(code.extract_first(Degree(3)), Some(0));
        assert_eq!(code.len, 1);
        assert_eq!(code.extract_first(Degree(3)), Some(2));
        assert_eq!(code.len, 0);
        assert_eq!(code.extract_first(Degree(3)), None);
    }
}