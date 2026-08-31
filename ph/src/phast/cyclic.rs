
//! Cyclic sets and arrays of values used during the construction of PHast functions.
//!
//! During the construction of PHast functions, values assigned by already-processed buckets
//! must be recorded so that later buckets avoid collisions. Since a window of buckets
//! covers only a cyclic range of values, these sets are cyclic,
//! which avoids wasting space for unused value ranges.

use bitm::BitAccess;
use std::ops::{Index, IndexMut};

use crate::phast::MAX_VALUES;

use super::MAX_WINDOW_SIZE;

/// A bit-set of values from a cyclic range of `SIZE_64*64` consecutive integers
/// (value `v` is stored at the bit position `v mod (SIZE_64*64)`).
/// `SIZE_64` is the size (in 64-bit words) of the underlying storage; it must be a power of two.
pub struct CyclicSet<const SIZE_64: usize>([u64; SIZE_64]);  // filled in pseudo-code

impl<const SIZE_64: usize> CyclicSet<SIZE_64> {
    /// Mask with the lowest `SIZE_64*64` bits, used to wrap values (bit index) into the cyclic range.
    const MASK: usize = SIZE_64*64 - 1;
    /// Mask used to wrap word index into the cyclic range.
    const CHUNK_MASK: usize = SIZE_64 - 1;

    /// Adds `value` to the set (if it is not already present).
    #[inline] pub fn add(&mut self, value: usize) {
        unsafe{ self.0.set_bit_unchecked(value & Self::MASK) }
    }

    /// Removes `value` from the set (if it is present).
    #[inline] pub fn remove(&mut self, value: usize) {
        unsafe{ self.0.clear_bit_unchecked(value & Self::MASK) }
    }

    /// Returns `true` if the set contains `value`.
    #[inline]
    pub fn contain(&self, value: usize) -> bool {
        unsafe{ self.0.get_bit_unchecked(value & Self::MASK) }
    }

    /// Returns bits representing `first_value` and the 63 consecutive values after it
    /// (i.e. bit `i` of the result represents value `first_value+i`).
    #[inline]
    pub fn get64(&self, first_value: usize) -> u64 {
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

/// [`CyclicSet`] able to store values used by slices of up to [`MAX_VALUES`] values.
pub type UsedValueSet = CyclicSet<{MAX_VALUES/64}>; // support slices up to 4096
/// [`CyclicSet`] able to store values used by slices of up to `2*`[`MAX_VALUES`] values.
pub type UsedValueSetLarge = CyclicSet<{MAX_VALUES*2/64}>;  // support slices up to 8192

/// A cyclic array: element at index `i` is stored at the position `i mod SIZE`
/// (`SIZE` must be a power of two). Used (e.g. as [`FreeValueMultiSetU16`]) to count
/// values that are still free in the currently processed window of buckets.
pub struct CyclicArray<T, const SIZE: usize = MAX_WINDOW_SIZE>(pub [T; SIZE]);

impl<T: Default, const SIZE: usize> Default for CyclicArray<T, SIZE> {
    /// Constructs the array filled with default values.
    #[inline(always)]
    fn default() -> Self {
        Self(std::array::from_fn(|_| Default::default()))
    }
}

impl<T: Clone, const SIZE: usize> CyclicArray<T, SIZE> {
    /// Constructs the array filled with `value`.
    #[inline(always)]
    pub fn filled_with(value: T) -> Self {
        Self(std::array::repeat(value))
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

    /// Returns the element at the cyclic index `index` (i.e. at the position `index % SIZE`).
    #[inline(always)] fn index(&self, index: usize) -> &Self::Output {
        unsafe { self.0.get_unchecked(index & (SIZE-1)) }
    }
}

impl<T, const SIZE: usize> IndexMut<usize> for CyclicArray<T, SIZE> {
    /// Returns the mutable reference to the element at the cyclic index `index`
    /// (i.e. at the position `index % SIZE`).
    #[inline(always)] fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe { self.0.get_unchecked_mut(index & (SIZE-1)) }
    }
}

/*impl<const SIZE_64: usize> GenericUsedValue for CyclicArray<u8, SIZE_64> {
    #[inline] fn add(&mut self, value: usize) {
        self[value] += 1;
    }

    #[inline] fn remove(&mut self, value: usize) {
        self[value] = 0;
    }
}*/

/// A cyclic array of `u16` counters over `MAX_VALUES` values,
/// used to count, for each value, how many keys (from the not-yet-assigned buckets)
/// are willing to take it (0 means that the value is free).
pub type FreeValueMultiSetU16 = CyclicArray<u16, MAX_VALUES>;