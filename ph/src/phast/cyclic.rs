
use bitm::BitAccess;
use std::ops::{Index, IndexMut};

use crate::phast::MAX_VALUES;

use super::MAX_WINDOW_SIZE;

/// Set or multi-set of values.
pub trait GenericUsedValue: Default + Send {
    /// Adds value to the set (or multi-set) of used values (increases amount by `1` in the case of multi-set).
    fn add(&mut self, value: usize);

    /// Completely removes `value` from `self` (sets amount to `0` in the case of multi-set).
    fn remove(&mut self, value: usize);
}

/// SIZE in 64-bit segments, must be the power of two
pub struct CyclicSet<const SIZE_64: usize>([u64; SIZE_64]);  // filled in pseudo-code

impl<const SIZE_64: usize> GenericUsedValue for CyclicSet<SIZE_64> {
    #[inline] fn add(&mut self, value: usize) {
        unsafe{ self.0.set_bit_unchecked(value & Self::MASK) }
    }

    #[inline] fn remove(&mut self, value: usize) {
        unsafe{ self.0.clear_bit_unchecked(value & Self::MASK) }
    }
}

impl<const SIZE_64: usize> CyclicSet<SIZE_64> {
    const MASK: usize = SIZE_64*64 - 1;
    const CHUNK_MASK: usize = SIZE_64 - 1;

    #[inline]
    pub(crate) fn contain(&self, value: usize) -> bool {
        unsafe{ self.0.get_bit_unchecked(value & Self::MASK) }
    }

    /// Returns `first_value` and 63 consecutive values as a bitset.
    #[inline]
    pub(crate) fn get64(&self, first_value: usize) -> u64 {
        let chunk_index = first_value / 64;
        let bit_in_lo = first_value % 64;
        let lo = unsafe{ *self.0.get_unchecked(chunk_index & Self::CHUNK_MASK) };
        if bit_in_lo == 0 { return lo; }
        let hi = unsafe{ *self.0.get_unchecked((chunk_index+1) & Self::CHUNK_MASK) };
        (lo >> bit_in_lo) | (hi << (64-bit_in_lo))
    }

    /*#[inline]
    pub fn contain_add(&mut self, value: usize) -> bool {
        let cell = unsafe{ self.0.get_unchecked_mut((value & Self::MASK) / 64) };
        let bit = 1u64 << (value % 64);
        if *cell & bit != 0 { return true };
        *cell |= bit;
        return false;
    }*/



    /*
    #[inline] pub fn remove_fragment_64(&mut self, chunk_index: usize) {
        unsafe{ *self.0.get_unchecked_mut(chunk_index & Self::CHUNK_MASK) = 0 };
    }*/
}

impl<const SIZE_64: usize> Default for CyclicSet<SIZE_64> {
    #[inline] fn default() -> Self {
        Self(std::array::from_fn(|_| 0))
    }
}

pub type UsedValueSet = CyclicSet<{MAX_VALUES/64}>;

/// SIZE must be the power of 2
pub struct CyclicArray<T, const SIZE: usize = MAX_WINDOW_SIZE>(pub [T; SIZE]);

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

    #[inline(always)] fn index(&self, index: usize) -> &Self::Output {
        unsafe { self.0.get_unchecked(index & (SIZE-1)) }
    }
}

impl<T, const SIZE: usize> IndexMut<usize> for CyclicArray<T, SIZE> {
    #[inline(always)] fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe { self.0.get_unchecked_mut(index & (SIZE-1)) }
    }
}

impl<const SIZE_64: usize> GenericUsedValue for CyclicArray<u8, SIZE_64> {
    #[inline] fn add(&mut self, value: usize) {
        self[value] += 1;
    }

    #[inline] fn remove(&mut self, value: usize) {
        self[value] = 0;
    }
}

pub type UsedValueMultiSetU8 = CyclicArray<u8, {MAX_VALUES/64}>;