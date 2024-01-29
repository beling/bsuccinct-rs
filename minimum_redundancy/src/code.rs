//! Tools to deal with codewords.

use std::iter::FusedIterator;
use crate::TreeDegree;

/// Represents a codeword.
#[derive(PartialEq, Eq, Debug, Clone, Copy, Default)]
pub struct Code {
    /// Concatenated fragments of the codeword.
    /// 
    /// If the code is too short to take up all the bits,
    /// only the least significant ones are used.
    /// The first fragments of the code are stored either on the most significant or,
    /// in the case of an reversed code, on the least significant, of these bits.
    /// 
    /// If the code is too long, only the last few fragments are explicitly stored.
    /// Fragments that do not fit contain zeros and are not explicitly stored.
    pub content: u32,
    /// Length of the code in fragments.
    pub len: u32
}

impl Code {
    /// Gets `fragment_nr`-th fragment from either the end or, if `self` is reversed, the beginning.
    /// 
    /// Result is undefined if `fragment_nr` is out of bounds (i.e. not less than `len`).
    #[inline(always)] pub unsafe fn get_rev_unchecked(&self, fragment_nr: u32, degree: impl TreeDegree) -> u32 {
        degree.get_fragment(self.content, fragment_nr)
    }

    /// Gets `fragment_nr`-th fragment from either the end or, if `self` is reversed, the beginning.
    /// 
    /// Returns [`None`] if `fragment_nr` is out of bounds (i.e. not less than `len`).
    #[inline] pub fn get_rev(&self, fragment_nr: u32, degree: impl TreeDegree) -> Option<u32> {
        (fragment_nr < self.len).then(|| degree.get_fragment(self.content, fragment_nr))
    }

    /// Gets `fragment_nr`-th fragment from either the beginning or, if `self` is reversed, the end.
    /// 
    /// Result is undefined if `fragment_nr` is out of bounds (i.e. not less than `len`).
    #[inline] pub unsafe fn get_unchecked(&self, fragment_nr: u32, degree: impl TreeDegree) -> u32 {
        self.get_rev_unchecked(self.len - fragment_nr - 1, degree)
    }

    /// Gets `fragment_nr`-th fragment from either the beginning or, if `self` is reversed, the end.
    /// 
    /// Returns [`None`] if `fragment_nr` is out of bounds (i.e. not less than `len`).
    #[inline] pub fn get(&self, fragment_nr: u32, degree: impl TreeDegree) -> Option<u32> {
        (fragment_nr < self.len).then(|| unsafe { self.get_unchecked(fragment_nr, degree) })
    }

    /// Extracts and returns the first remaining fragment of the unreversed `code`.
    /// 
    /// Return [`None`] if `code` is empty.
    pub fn extract_first(&mut self, degree: impl TreeDegree) -> Option<u32> {
        (self.len != 0).then(|| {
            self.len -= 1;
            unsafe { self.get_rev_unchecked(self.len, degree) }
        })
    }

    /// Extracts and returns the first remaining fragment of the reversed `code`.
    /// 
    /// Return [`None`] if `code` is empty.
    pub fn extract_rev_first(&mut self, degree: impl TreeDegree) -> Option<u32> {
        (self.len != 0).then(|| {
            self.len -= 1;
            degree.pop_front(&mut self.content)
        })
    }

    /// Returns whether `self` consists of zero fragments.
    #[inline] pub fn is_empty(&self) -> bool { self.len == 0 }

    /// Returns iterator over the fragments of unreversed code.
    #[inline] pub fn iter<D: TreeDegree>(&self, degree: D) -> CodeIterator<D> {
        CodeIterator { code: *self, degree }
    }

    /// Returns iterator over the fragments of reversed code.
    #[inline] pub fn iter_rev<D: TreeDegree>(&self, degree: D) -> ReversedCodeIterator<D> {
        ReversedCodeIterator { code: *self, degree }
    }
}

/// Iterator over the fragments of (unreversed) code.
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


/// Iterator over the fragments of reversed code.
pub struct ReversedCodeIterator<D: TreeDegree> {
    code: Code,
    degree: D
}

impl<D: TreeDegree> FusedIterator for ReversedCodeIterator<D> {}

impl<D: TreeDegree> ExactSizeIterator for ReversedCodeIterator<D> {
    #[inline] fn len(&self) -> usize {
        self.code.len as usize
    }
}

impl<D: TreeDegree> Iterator for ReversedCodeIterator<D> {
    type Item = u32;

    #[inline] fn next(&mut self) -> Option<Self::Item> {
        self.code.extract_rev_first(self.degree)
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

    /// Extracts and returns the first fragment of `self` or returns [`None`] of `self` is empty.
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