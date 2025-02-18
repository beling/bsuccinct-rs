
use bitm::BitAccess;
use std::{iter::FusedIterator, ops::{Index, IndexMut}};

use super::MAX_SPAN;

/// SIZE must the power of two
pub struct CyclicSet<const SIZE: usize>([u64; SIZE]);  // filled in pseudo-code

impl<const SIZE: usize> CyclicSet<SIZE> {
    const MASK: usize = SIZE-1;

    #[inline]
    pub fn contain(&self, value: usize) -> bool {
        unsafe{ self.0.get_bit_unchecked(value & Self::MASK) }
    }

    #[inline]
    pub fn add(&mut self, value: usize) {
        unsafe{ self.0.set_bit_unchecked(value & Self::MASK) }
    }

    /*#[inline]
    pub fn contain_add(&mut self, value: usize) -> bool {
        let cell = unsafe{ self.0.get_unchecked_mut((value & Self::MASK) / 64) };
        let bit = 1u64 << (value % 64);
        if *cell & bit != 0 { return true };
        *cell |= bit;
        return false;
    }*/

    #[inline]
    pub fn remove(&mut self, value: usize) {
        unsafe{ self.0.clear_bit_unchecked(value & Self::MASK) }
    }
}

impl<const SIZE: usize> Default for CyclicSet<SIZE> {
    #[inline] fn default() -> Self {
        Self(std::array::from_fn(|_| 0))
    }
}

/// SIZE must be the power of 2
pub struct CyclicArray<T, const SIZE: usize = MAX_SPAN>(pub [T; SIZE]);

impl<T: Default, const SIZE: usize> Default for CyclicArray<T, SIZE> {
    #[inline(always)]
    fn default() -> Self {
        Self(std::array::from_fn(|_| Default::default()))
    }
}

/*impl<T, const SIZE: usize> CyclicArray<T, SIZE> {
    #[inline]
    pub fn new<F: FnMut(usize) -> T>(cb: F) -> Self {
        Self(std::array::from_fn(cb))
    }
}*/

impl<T, const SIZE: usize> Index<usize> for CyclicArray<T, SIZE> {
    type Output = T;

    #[inline] fn index(&self, index: usize) -> &Self::Output {
        unsafe { self.0.get_unchecked(index & (SIZE-1)) }
    }
}

impl<T, const SIZE: usize> IndexMut<usize> for CyclicArray<T, SIZE> {
    #[inline] fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe { self.0.get_unchecked_mut(index & (SIZE-1)) }
    }
}

#[allow(dead_code)]
pub trait SeedSet {
    type Iterator<'s>: Iterator<Item=u8> where Self: 's;

    fn iter<'s>(&'s self) -> Self::Iterator<'s>;
    fn contain(&self, seed: u8) -> bool;
    fn add(&mut self, seed: u8);
    fn remove(&mut self, seed: u8);
    fn clear(&mut self);
}

pub struct U64BitOnesIter(u64);

impl Iterator for U64BitOnesIter {
    type Item = u8;

    #[inline] fn next(&mut self) -> Option<Self::Item> {
        if self.0 == 0 { return None; }
        let result = self.0.trailing_zeros() as u8;
        self.0 ^= 1 << result;
        Some(result)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl FusedIterator for U64BitOnesIter {}

impl ExactSizeIterator for U64BitOnesIter {
    #[inline] fn len(&self) -> usize { self.0.count_ones() as usize }
    //#[inline] fn is_empty(&self) -> bool { self.0 == 0 }
}



/// Iterator over indices of bits set to 1 (if `B` is `true`) or 0 (if `B` is `false`) in slice of `u64`.
pub struct U64ArrBitOnesIterator<'a> {
    /// Iterator over 64-bit segments.
    segment_iter: std::slice::Iter<'a, u64>,
    /// 64 * index of the current segment.
    first_segment_bit: u8,
    /// Copy of the current segment (or its negation if `!B`) with zeroed already exposed bits.
    current_segment: u64
}

impl<'a> U64ArrBitOnesIterator<'a> {
    /// Constructs iterator over bits set in the given `slice`.
    pub fn new(slice: &'a [u64]) -> Self {
        let mut segment_iter = slice.into_iter();
        let current_segment = segment_iter.next().copied().unwrap_or(0);
        Self {
            segment_iter,
            first_segment_bit: 0,
            current_segment
        }
    }
}

impl<'a> Iterator for U64ArrBitOnesIterator<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current_segment == 0 {
            self.current_segment = *self.segment_iter.next()?;
            self.first_segment_bit += 64;
        }
        let result = self.current_segment.trailing_zeros();
        self.current_segment ^= 1<<result;
        Some(self.first_segment_bit + (result as u8))
    }

    #[inline] fn size_hint(&self) -> (usize, Option<usize>) {
        let result = self.len();
        (result, Some(result))
    }
}

impl<'a> ExactSizeIterator for U64ArrBitOnesIterator<'a> {
    #[inline] fn len(&self) -> usize {
        self.current_segment.count_ones() as usize + self.segment_iter.as_slice().count_bit_ones()
    }
}


impl SeedSet for u64 {
    type Iterator<'s> = U64BitOnesIter;

    #[inline(always)]
    fn iter(&self) -> U64BitOnesIter {
        U64BitOnesIter(*self)
    }

    #[inline(always)]
    fn contain(&self, seed: u8) -> bool {
        (*self & (1<<seed)) != 0
    }

    #[inline(always)]
    fn add(&mut self, seed: u8) {
        *self |= 1 << seed;
    }

    #[inline(always)]
    fn remove(&mut self, seed: u8) {
        *self &= !(1 << seed);
    }
    
    #[inline(always)]
    fn clear(&mut self) {
        *self = 0;
    }
}

impl SeedSet for [u64] {
    type Iterator<'s> = U64ArrBitOnesIterator<'s>;

    #[inline(always)]
    fn iter<'s>(&'s self) -> Self::Iterator<'s> {
        Self::Iterator::new(&self)
    }

    #[inline(always)]
    fn contain(&self, seed: u8) -> bool {
        self.get_bit(seed as usize)
    }

    #[inline(always)]
    fn add(&mut self, seed: u8) {
        self.set_bit(seed as usize);
    }

    #[inline(always)]
    fn remove(&mut self, seed: u8) {
        self.clear_bit(seed as usize);
    }
    
    #[inline(always)]
    fn clear(&mut self) {
        self.fill(0);
    }
}


