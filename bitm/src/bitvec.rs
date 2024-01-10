use std::iter::FusedIterator;
use crate::n_lowest_bits_0_64;

use super::{ceiling_div, n_lowest_bits, n_lowest_bits_1_64};

pub trait Mask {
    fn masked(v: u64, len: u8) -> u64;
    #[inline(always)] fn check(len: u8) { if len > 64 { panic!("length {len} is invalid as it exceeds 64") } }
}

#[derive(Clone, Copy)]
pub struct Unmasked;

impl Mask for Unmasked {
    #[inline(always)] fn masked(v: u64, _len: u8) -> u64 { v }
}

#[derive(Clone, Copy)]
pub struct M0_63;

impl Mask for M0_63 {
    #[inline(always)] fn masked(v: u64, len: u8) -> u64 { v & n_lowest_bits(len) }
    #[inline(always)] fn check(len: u8) { if len > 63 { panic!("length {len} is invalid as it exceeds 63") } }
}

#[derive(Clone, Copy)]
pub struct M1_64;

impl Mask for M1_64 {
    #[inline(always)] fn masked(v: u64, len: u8) -> u64 { v & n_lowest_bits_1_64(len) }
    #[inline(always)] fn check(len: u8) { if len > 63 { panic!("length {len} is invalid as it should be in range [1, 64]") } }
}

#[derive(Clone, Copy)]
pub struct M0_64;

impl Mask for M0_64 {
    #[inline(always)] fn masked(v: u64, len: u8) -> u64 { v & n_lowest_bits_0_64(len) }
}

#[derive(Clone, Copy)]
pub struct BitRange<M: Mask = M0_63> {
    pub begin: usize,
    pub len: u8,
    m: std::marker::PhantomData<M>
}

impl<M: Mask> BitRange<M> {
    #[inline(always)] pub fn begin_len(begin: usize, len: u8) -> Self { Self { begin, len, m: Default::default() } }
    #[inline(always)] pub fn successive_begin_len(begin: &mut usize, len: u8) -> Self {
         let r = Self::begin_len(*begin, len); *begin += len as usize; r
    }
    #[inline(always)] pub fn fragment(index: usize, len: u8) -> Self { Self { begin: index * len as usize, len, m: Default::default() } }
    #[inline(always)] pub fn successive_fragment(index: &mut usize, len: u8) -> Self {
        let r = Self::fragment(*index, len); *index += 1; r
    }

    #[inline(always)] pub fn begin_len_checked(begin: usize, len: u8) -> Self { M::check(len); Self::begin_len(begin, len) }
    #[inline(always)] pub fn successive_begin_len_checked(begin: &mut usize, len: u8) -> Self {
        M::check(len); Self::successive_begin_len_checked(begin, len)
    }
    #[inline(always)] pub fn fragment_checked(index: usize, len: u8) -> Self { M::check(len); Self::fragment(index, len) }
    #[inline(always)] pub fn successive_fragment_checked(index: &mut usize, len: u8) -> Self {
        M::check(len); Self::successive_fragment_checked(index, len)
    }
}

/// Iterator over bits set to 1 (if `B` is `true`) or 0 (if `B` is `false`) in slice of `u64`.
pub struct BitBIterator<'a, const B: bool> {
    segment_iter: std::slice::Iter<'a, u64>,
    first_segment_bit: usize,
    current_segment: u64
}

impl<'a, const B: bool> BitBIterator<'a, B> {
    /// Constructs iterator over bits set in the given `slice`.
    pub fn new(slice: &'a [u64]) -> Self {
        let mut segment_iter = slice.into_iter();
        let current_segment = if B {
            segment_iter.next().copied().unwrap_or(0)
        } else {
            !segment_iter.next().copied().unwrap_or(0)
        };
        Self {
            segment_iter,
            first_segment_bit: 0,
            current_segment
        }
    }
}

impl<'a, const B: bool> Iterator for BitBIterator<'a, B> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current_segment == 0 {
            self.current_segment = if B { *self.segment_iter.next()? } else { !*self.segment_iter.next()? };
            self.first_segment_bit += 64;
        }
        let result = self.current_segment.trailing_zeros();
        self.current_segment ^= 1<<result;
        Some(self.first_segment_bit + (result as usize))
    }

    #[inline] fn size_hint(&self) -> (usize, Option<usize>) {
        let result = self.len();
        (result, Some(result))
    }
}

impl<'a, const B: bool> ExactSizeIterator for BitBIterator<'a, B> {
    #[inline] fn len(&self) -> usize {
        if B {
            self.current_segment.count_ones() as usize + self.segment_iter.as_slice().count_bit_ones()
        } else {
            self.current_segment.count_zeros() as usize + self.segment_iter.as_slice().count_bit_zeros()
        }
    }
}

impl<'a, const B: bool> FusedIterator for BitBIterator<'a, B> where std::slice::Iter<'a, u64>: FusedIterator {}

/// Iterator over bits set to 1 in slice of `u64`.
pub type BitOnesIterator<'a> = BitBIterator<'a, true>;

/// Iterator over bits set to 0 in slice of `u64`.
pub type BitZerosIterator<'a> = BitBIterator<'a, false>;

/// The trait that is implemented for the array of `u64` and extends it with methods for
/// accessing and modifying single bits or arbitrary fragments consisted of few (up to 63) bits.
pub trait BitAccess {
    /// Gets bit with given index `bit_nr`. Panics if `bit_nr` is out of bounds.
    fn get_bit(&self, bit_nr: usize) -> bool;

    /// Gets bit with given index `bit_nr`, without bounds checking.
    unsafe fn get_bit_unchecked(&self, bit_nr: usize) -> bool;

    /// Gets bit with given index `bit_nr` and increase `bit_nr` by 1. Panics if `bit_nr` is out of bounds.
    #[inline] fn get_successive_bit(&self, bit_nr: &mut usize) -> bool {
        let result = self.get_bit(*bit_nr);
        *bit_nr += 1;
        result
    }

    /// Set bit with given index `bit_nr` to `value` (`1` if `true`, `0` otherwise). Panics if `bit_nr` is out of bounds.
    fn set_bit_to(&mut self, bit_nr: usize, value: bool);

    /// Set bit with given index `bit_nr` to `value` (`1` if `true`, `0` otherwise), without bounds checking.
    unsafe fn set_bit_to_unchecked(&mut self, bit_nr: usize, value: bool);

    /// Set bit with given index `bit_nr` to `value` (`1` if `true`, `0` otherwise) and increase `bit_nr` by 1.
    /// Panics if `bit_nr` is out of bound.
    #[inline] fn set_successive_bit_to(&mut self, bit_nr: &mut usize, value: bool) {
        self.set_bit_to(*bit_nr, value); *bit_nr += 1;
    }

    /// Set bit with given index `bit_nr` to `value` (`1` if `true`, `0` otherwise) and increase `bit_nr` by 1.
    /// The result is undefined if `bit_nr` is out of bound.
    #[inline] unsafe fn set_successive_bit_to_unchecked(&mut self, bit_nr: &mut usize, value: bool) {
        self.set_bit_to_unchecked(*bit_nr, value); *bit_nr += 1;
    }

    /// Initialize bit with given index `bit_nr` to `value` (`1` if `true`, `0` otherwise).
    /// Before initialization, the bit is assumed to be cleared or already set to `value`.
    /// Panics if `bit_nr` is out of bounds.
    fn init_bit(&mut self, bit_nr: usize, value: bool);

    /// Initialize bit with given index `bit_nr` to `value` (`1` if `true`, `0` otherwise).
    /// Before initialization, the bit is assumed to be cleared or already set to `value`.
    /// The result is undefined if `bit_nr` is out of bounds.
    unsafe fn init_bit_unchecked(&mut self, bit_nr: usize, value: bool);

    /// Initialize bit with given index `bit_nr` to `value` (`1` if `true`, `0` otherwise)
    /// and increase `bit_nr` by 1.
    /// Before initialization, the bit is assumed to be cleared or already set to `value`.
    /// Panics if `bit_nr` is out of bounds.
    #[inline] fn init_successive_bit(&mut self, bit_nr: &mut usize, value: bool) {
        self.init_bit(*bit_nr, value); *bit_nr += 1;
    }

    /// Initialize bit with given index `bit_nr` to `value` (`1` if `true`, `0` otherwise)
    /// and increase `bit_nr` by 1.
    /// Before initialization, the bit is assumed to be cleared or already set to `value`.
    /// The result is undefined if `bit_nr` is out of bounds.
    #[inline] unsafe fn init_successive_bit_unchecked(&mut self, bit_nr: &mut usize, value: bool) {
        self.init_bit_unchecked(*bit_nr, value); *bit_nr += 1;
    }

    /// Sets bit with given index `bit_nr` to `1`. Panics if `bit_nr` is out of bounds.
    fn set_bit(&mut self, bit_nr: usize);

    /// Sets bit with given index `bit_nr` to `1`, without bounds checking.
    unsafe fn set_bit_unchecked(&mut self, bit_nr: usize);

    /// Sets bit with given index `bit_nr` to `0`. Panics if `bit_nr` is out of bounds.
    fn clear_bit(&mut self, bit_nr: usize);

    /// Sets bit with given index `bit_nr` to `0`, without bounds checking.
    unsafe fn clear_bit_unchecked(&mut self, bit_nr: usize);

    /// Gets at least `len` bits beginning from the bit index `begin`. Panics if the range is out of bounds.
    #[inline] fn get_bits_unmasked<M: Mask>(&self, addr: BitRange<M>) -> u64 {
        self.try_get_bits_unmasked(addr).expect("bit range out of bounds")
    }

    /// Gets bits `[begin, begin+len)`. Panics if the range is out of bounds.
    #[inline] fn get_bits<M: Mask>(&self, addr: BitRange<M>) -> u64 {
        //if len == 0 { return 0; }
        self.get_bits_unmasked(addr) & n_lowest_bits(addr.len)
    }

    /// Gets at least `len` bits beginning from the bit index `begin`.
    /// Returns [`None`] if the range is out of bounds.
    fn try_get_bits_unmasked<M: Mask>(&self, addr: BitRange<M>) -> Option<u64>;

    /// Gets bits `[begin, begin+len)`. Returns [`None`] if the range is out of bounds.
    #[inline(always)] fn try_get_bits<M: Mask>(&self, addr: BitRange<M>) -> Option<u64> {
        self.try_get_bits_unmasked(addr).map(|result| result & n_lowest_bits(addr.len))
    }

    /// Gets at least `len` bits beginning from the bit index `begin` without bounds checking.
    unsafe fn get_bits_unmasked_unchecked<M: Mask>(&self, addr: BitRange<M>) -> u64;

    /// Gets bits `[begin, begin+len)` without bounds checking.
    #[inline(always)] unsafe fn get_bits_unchecked<M: Mask>(&self, addr: BitRange<M>) -> u64 {
        self.get_bits_unmasked_unchecked(addr) & n_lowest_bits(addr.len)
    }

    /// Gets bits `[begin, begin+len)` and increase `bit_nr` by `len`.
    #[inline] fn get_successive_bits(&self, begin: &mut usize, len: u8) -> u64 {
        let result = self.get_bits::<M0_63>(BitRange::begin_len(*begin, len));
        *begin += len as usize;
        result
    }

    /// Initialize bits `[begin, begin+len)` to `v`.
    /// Before initialization, the bits are assumed to be cleared or already set to `v`.
    fn init_bits(&mut self, begin: usize, v: u64, len: u8);

    /// Initialize bits `[begin, begin+len)` to `v` and increase `begin` by `len`.
    /// Before initialization, the bits are assumed to be cleared or already set to `v`.
    #[inline] fn init_successive_bits(&mut self, begin: &mut usize, v: u64, len: u8) {
        self.init_bits(*begin, v, len); *begin += len as usize;
    }

    /// Sets bits `[begin, begin+len)` to the content of `v`. Panics if the range is out of bounds.
    fn set_bits(&mut self, begin: usize, v: u64, len: u8);

    /// Sets bits `[begin, begin+len)` to the content of `v` and increase `begin` by `len`. Panics if the range is out of bounds.
    #[inline] fn set_successive_bits(&mut self, begin: &mut usize, v: u64, len: u8) {
        self.set_bits(*begin, v, len);  *begin += len as usize;
    }

    /// Sets bits `[begin, begin+len)` to the content of `v`, without bounds checking.
    unsafe fn set_bits_unchecked(&mut self, begin: usize, v: u64, len: u8);

    /// Xor at least `len` bits of `self`, staring from index `begin`, with `v`. Panics if the range is out of bounds.
    fn xor_bits(&mut self, begin: usize, v: u64, len: u8);

    /// Xor at least `len` bits of `self`, staring from index `begin`, with `v` and increase `begin` by `len`.
    /// Panics if the range is out of bounds.
    fn xor_successive_bits(&mut self, begin: &mut usize, v: u64, len: u8) {
        self.xor_bits(*begin, v, len);  *begin += len as usize;
    }

    /// Returns the number of zeros (cleared bits).
    fn count_bit_zeros(&self) -> usize;

    /// Returns the number of ones (set bits).
    fn count_bit_ones(&self) -> usize;

    /// Returns iterator over indices of ones (set bits).
    fn bit_ones(&self) -> BitOnesIterator;

    /// Returns iterator over indices of ones (set bits).
    fn bit_zeros(&self) -> BitZerosIterator;

    /// Gets `index`-th fragment of `v_size` bits, i.e. bits with indices in range [`index*v_size`, `index*v_size+v_size`).
    /// Panics if the range is out of bounds.
    #[inline(always)] fn get_fragment(&self, index: usize, v_size: u8) -> u64 {
        self.get_bits(BitRange::begin_len(index * v_size as usize, v_size))
    }

    /// Gets `index`-th fragment of `v_size` bits, i.e. bits with indices in range [`index*v_size`, `index*v_size+v_size`).
    /// Returns [`None`] if the range is out of bounds.
    #[inline(always)] fn try_get_fragment(&self, index: usize, v_size: u8) -> Option<u64> {
        self.try_get_bits(BitRange::begin_len(index * v_size as usize, v_size))
    }

    /// Gets `index`-th fragment of `v_size` bits, i.e. bits with indices in range [`index*v_size`, `index*v_size+v_size`), without bounds checking.
    #[inline(always)] unsafe fn get_fragment_unchecked(&self, index: usize, v_size: u8) -> u64 {
        self.get_bits_unchecked(BitRange::begin_len(index * v_size as usize, v_size))
    }

    /// Gets `index`-th fragment of `v_size` bits and increases `index` by 1.
    #[inline(always)] fn get_successive_fragment(&self, index: &mut usize, v_size: u8) -> u64 {
        let result = self.get_fragment(*index, v_size);
        *index += 1;
        result
    }

    /// Initializes `index`-th fragment of `v_size` bits, i.e. bits with indices in range [`index*v_size`, `index*v_size+v_size`), to `v`.
    /// Panics if the range is out of bounds. Before initialization, the bits are assumed to be cleared or already set to `v`.
    #[inline(always)] fn init_fragment(&mut self, index: usize, v: u64, v_size: u8) {
        self.init_bits(index * v_size as usize, v, v_size)
    }

    /// Initializes `index`-th fragment of `v_size` bits, i.e. bits with indices in range [`index*v_size`, `index*v_size+v_size`), to `v`
    /// Next, increases `index` by 1. Panics if the range is out of bounds.
    /// Before initialization, the bits are assumed to be cleared or already set to `v`.
    #[inline(always)] fn init_successive_fragment(&mut self, index: &mut usize, v: u64, v_size: u8) {
        self.init_fragment(*index, v, v_size);  *index += 1;
    }

    /// Sets index`-th fragment of `v_size` bits, i.e. bits with indices in range [`index*v_size`, `index*v_size+v_size`), to `v`.
    /// Panics if the range is out of bounds.
    #[inline(always)] fn set_fragment(&mut self, index: usize, v: u64, v_size: u8) {
        self.set_bits(index * v_size as usize, v, v_size)
    }

    /// Sets `index`-th fragment of `v_size` bits, i.e. bits with indices in range [`index*v_size`, `index*v_size+v_size`), to `v`.
    /// The result is undefined if the range is out of bounds.
    #[inline(always)] unsafe fn set_fragment_unchecked(&mut self, index: usize, v: u64, v_size: u8) {
        self.set_bits_unchecked(index * v_size as usize, v, v_size)
    }

    /// Sets index`-th fragment of `v_size` bits, i.e. bits with indices in range [`index*v_size`, `index*v_size+v_size`), to `v`.
    /// Next, increases `index` by 1. Panics if the range is out of bounds.
    #[inline(always)] fn set_successive_fragment(&mut self, index: &mut usize, v: u64, v_size: u8) {
        self.set_fragment(*index, v, v_size);   *index += 1;
    }

    /// Xor at least `v_size` bits of `self` begging from `index*v_size` with `v`. Panics if the range is out of bounds.
    #[inline(always)] fn xor_fragment(&mut self, index: usize, v: u64, v_size: u8) {
        self.xor_bits(index * v_size as usize, v, v_size)
    }

    /// Xor at least `v_size` bits of `self` begging from `index*v_size` with `v` and increase `index` by 1.
    /// Panics if the range is out of bounds.
    #[inline(always)] fn xor_successive_fragment(&mut self, index: &mut usize, v: u64, v_size: u8) {
        self.xor_fragment(*index, v, v_size);   *index += 1;
    }

    /// Swaps ranges of bits: [`index1*v_size`, `index1*v_size+v_size`) with [`index2*v_size`, `index2*v_size+v_size`).
    fn swap_fragments(&mut self, index1: usize, index2: usize, v_size: u8) {
        // TODO faster implementation
        let v1 = self.get_fragment(index1, v_size);
        unsafe{self.set_fragment_unchecked(index1, self.get_fragment(index2, v_size), v_size)};
        unsafe{self.set_fragment_unchecked(index2, v1, v_size);}
    }

    /// Conditionally (if `new_value` does not return [`None`]) changes
    /// the value `old` stored at bits `[begin, begin+v_size)`
    /// to the one returned by `new_value` (whose argument is `old`).
    /// Returns `old` (the value before change).
    fn conditionally_change_bits<NewValue>(&mut self, new_value: NewValue, begin: usize, v_size: u8) -> u64
        where NewValue: FnOnce(u64) -> Option<u64>
    {
        let old = self.get_bits(BitRange::begin_len(begin, v_size));
        if let Some(new) = new_value(old) { unsafe{self.set_bits_unchecked(begin, new, v_size)}; }
        old
    }

    /// Conditionally (if `new_value` does not return [`None`]) changes
    /// the value `old` stored at bits [`index*v_size`, `index*v_size+v_size`)
    /// to the one returned by `new_value` (whose argument is `old`).
    /// Returns `old` (the value before change).
    #[inline(always)] fn conditionally_change_fragment<NewValue>(&mut self, new_value: NewValue, index: usize, v_size: u8) -> u64
        where NewValue: FnOnce(u64) -> Option<u64>
    {
        self.conditionally_change_bits(new_value, index * v_size as usize, v_size)
    }

    /// Conditionally (if `predicate` return `true`) replaces the bits
    /// [`begin`, `begin+v_size`) of `self` by the bits [`begin`, `begin+v_size`) of `src`.
    /// Subsequent `predicate` arguments are the bits [`begin`, `begin+v_size`) of:
    /// `self` and `src`.
    #[inline(always)] fn conditionally_copy_bits<Pred>(&mut self, src: &Self, predicate: Pred, begin: usize, v_size: u8)
        where Pred: FnOnce(u64, u64) -> bool
    {
        let src_bits = src.get_bits(BitRange::begin_len(begin, v_size));
        self.conditionally_change_bits(|self_bits| predicate(self_bits, src_bits).then(|| src_bits), begin, v_size);
    }

    /// Conditionally (if `predicate` return `true`) replaces the bits
    /// [`index*v_size`, `index*v_size+v_size`) of `self`
    /// by the bits [`index*v_size`, `index*v_size+v_size`) of `src`.
    /// Subsequent `predicate` arguments are the bits [`index*v_size`, `index*v_size+v_size`) of:
    /// `self` and `src`.
    #[inline(always)] fn conditionally_copy_fragment<Pred>(&mut self, src: &Self, predicate: Pred, index: usize, v_size: u8)
        where Pred: FnOnce(u64, u64) -> bool
    {
        self.conditionally_copy_bits(src, predicate, index * v_size as usize, v_size)
    }

    /// Returns the number of trailing 0 bits.
    fn trailing_zero_bits(&self) -> usize;

    /// Returns the lowest index of 1-bit that is grater or equal to `start_index`.
    /// The result is undefined if there is no such index.
    unsafe fn find_bit_one_unchecked(&self, start_index: usize) -> usize;

    /// Returns the lowest index of 1-bit that is grater or equal to `start_index`.
    /// Retruns [`None`] if there is no such index.
    fn find_bit_one(&self, start_index: usize) -> Option<usize>;

    /// Returns the greatest index of 1-bit that is lower or equal to `start_index`.
    /// The result is undefined if there is no such index.
    unsafe fn rfind_bit_one_unchecked(&self, start_index: usize) -> usize;
}

/// The trait that is implemented for `Box<[u64]>` and extends it with bit-oriented constructors.
pub trait BitVec where Self: Sized {
    /// Returns vector of `segments_len` 64 bit segments, each segment initialized to `segments_value`.
    fn with_64bit_segments(segments_value: u64, segments_len: usize) -> Self;

    /// Returns vector of bits filled with `words_count` `word`s of length `word_len_bits` bits each.
    fn with_bitwords(word: u64, word_len_bits: u8, words_count: usize) -> Self;

    /// Returns vector of `segments_len` 64 bit segments, with all bits set to `0`.
    #[inline(always)] fn with_zeroed_64bit_segments(segments_len: usize) -> Self {
        Self::with_64bit_segments(0, segments_len)
    }

    /// Returns vector of `segments_len` 64 bit segments, with all bits set to `1`.
    #[inline(always)] fn with_filled_64bit_segments(segments_len: usize) -> Self {
        Self::with_64bit_segments(u64::MAX, segments_len)
    }

    /// Returns vector of `bit_len` bits, all set to `0`.
    #[inline(always)] fn with_zeroed_bits(bit_len: usize) -> Self {
        Self::with_zeroed_64bit_segments(ceiling_div(bit_len, 64))
    }

    /// Returns vector of `bit_len` bits, all set to `1`.
    #[inline(always)] fn with_filled_bits(bit_len: usize) -> Self {
        Self::with_filled_64bit_segments(ceiling_div(bit_len, 64))
    }

    //fn with_bit_fragments<V: Into<u64>, I: IntoIterator<Item=V>>(items: I, fragment_count: usize, bits_per_fragment: u8) -> Box<[u64]>
}

impl BitVec for Box<[u64]> {
    #[inline(always)] fn with_64bit_segments(segments_value: u64, segments_len: usize) -> Self {
        vec![segments_value; segments_len].into_boxed_slice()
    }

    fn with_bitwords(word: u64, word_len_bits: u8, words_count: usize) -> Self {
        let mut result = Self::with_zeroed_bits(words_count * word_len_bits as usize);
        for index in 0..words_count { result.init_fragment(index, word, word_len_bits); }
        result
    }
}

/*#[inline(always)] pub fn bitvec_len_for_bits(bits_len: usize) -> usize { ceiling_div(bits_len, 64) }

#[inline(always)] pub fn bitvec_with_segments_len_and_value(segments_len: usize, segments_value: u64) -> Box<[u64]> {
    vec![segments_value; segments_len].into_boxed_slice()
}
#[inline(always)] pub fn bitvec_with_segments_len_zeroed(segments_len: usize) -> Box<[u64]> {
    bitvec_with_segments_len_and_value(segments_len, 0)
}
#[inline(always)] pub fn bitvec_with_segments_len_filled(segments_len: usize) -> Box<[u64]> {
    bitvec_with_segments_len_and_value(segments_len, u64::MAX)
}
#[inline(always)] pub fn bitvec_with_bits_len_zeroed(bits_len: usize) -> Box<[u64]> {
    bitvec_with_segments_len_zeroed(bitvec_len_for_bits(bits_len))
}
#[inline(always)] pub fn bitvec_with_bits_len_filled(bits_len: usize) -> Box<[u64]> {
    bitvec_with_segments_len_filled(bitvec_len_for_bits(bits_len))
}

pub fn bitvec_with_items<V: Into<u64>, I: IntoIterator<Item=V>>(items: I, fragment_count: usize, bits_per_fragment: u8) -> Box<[u64]> {
    let mut result = bitvec_with_bits_len_zeroed(fragment_count * bits_per_fragment as usize);
    for (i, v) in items.into_iter().enumerate() {
        result.init_fragment(i, v.into(), bits_per_fragment);
    }
    result
}*/

/// Set `bit_nr` bit of `v` to given `value`.
#[inline(always)] fn set_bit_to(to_change: &mut u64, bit_nr: usize, value: bool) {
    *to_change &= !(1u64 << bit_nr);
    *to_change |= (value as u64) << bit_nr;
}

#[inline(always)] fn set_bits_to(to_change: &mut u64, shifted_v: u64, shifted_v_mask: u64) {
    *to_change &= !shifted_v_mask;
    *to_change |= shifted_v;
}

impl BitAccess for [u64] {
    #[inline(always)] fn get_bit(&self, bit_nr: usize) -> bool {
        self[bit_nr / 64] & (1u64 << (bit_nr % 64)) != 0
    }

    #[inline(always)] fn set_bit_to(&mut self, bit_nr: usize, value: bool) {
        set_bit_to(&mut self[bit_nr / 64], bit_nr % 64, value)
    }

    #[inline(always)] unsafe fn set_bit_to_unchecked(&mut self, bit_nr: usize, value: bool) {
        set_bit_to(self.get_unchecked_mut(bit_nr / 64), bit_nr % 64, value)
        //if value { self.set_bit_unchecked(bit_nr) } else { self.clear_bit_unchecked(bit_nr) }
    }

    #[inline(always)] unsafe fn get_bit_unchecked(&self, bit_nr: usize) -> bool {
        self.get_unchecked(bit_nr / 64) & (1u64 << (bit_nr % 64) as u64) != 0
    }

    #[inline(always)] fn init_bit(&mut self, bit_nr: usize, value: bool) {
        self[bit_nr / 64] |= (value as u64) << (bit_nr % 64);
    }

    #[inline] unsafe fn init_bit_unchecked(&mut self, bit_nr: usize, value: bool) {
        *self.get_unchecked_mut(bit_nr / 64) |= (value as u64) << (bit_nr % 64);
    }

    #[inline(always)] fn set_bit(&mut self, bit_nr: usize) {
        self[bit_nr / 64] |= 1u64 << (bit_nr % 64)
    }

    #[inline(always)] unsafe fn set_bit_unchecked(&mut self, bit_nr: usize) {
        *self.get_unchecked_mut(bit_nr / 64) |= 1u64 << (bit_nr % 64)
    }

    #[inline(always)] fn clear_bit(&mut self, bit_nr: usize) {
        self[bit_nr / 64] &= !((1u64) << (bit_nr % 64))
    }

    #[inline(always)] unsafe fn clear_bit_unchecked(&mut self, bit_nr: usize) {
        *self.get_unchecked_mut(bit_nr / 64) &= !((1u64) << (bit_nr % 64))
    }

    fn count_bit_zeros(&self) -> usize {
        self.into_iter().map(|s| s.count_zeros() as usize).sum()
    }

    fn count_bit_ones(&self) -> usize {
        self.into_iter().map(|s| s.count_ones() as usize).sum()
    }

    #[inline(always)] fn bit_ones(&self) -> BitOnesIterator {
        BitOnesIterator::new(self)
    }

    #[inline(always)] fn bit_zeros(&self) -> BitZerosIterator {
        BitZerosIterator::new(self)
    }

    #[inline] fn try_get_bits_unmasked<M: Mask>(&self, addr: BitRange<M>) -> Option<u64> {
        //((begin+(len as usize))/64 < self.len()).then(|| unsafe{self.get_bits_unmasked_unchecked(begin, len)})
        let (segment, offset) = (addr.begin / 64, (addr.begin % 64) as u8);
        let w1 = self.get(segment)? >> offset;
        //let bits_in_w1 = 64-offset;
        Some(if offset+addr.len > 64 /*len > bits_in_w1*/ { // do we need more bits (from next segment)? Does len > bits_in_w1?
            let bits_in_w1 = 64-offset; // w1 has bits_in_w1 lowest bit set (copied from index_segment)
            w1 | (self.get(segment+1)? << bits_in_w1)
        } else {
            w1
        })
    }

    #[inline] unsafe fn get_bits_unmasked_unchecked<M: Mask>(&self, addr: BitRange<M>) -> u64 {
        let (segment, offset) = (addr.begin / 64, (addr.begin % 64) as u8);
        let w1 = self.get_unchecked(segment) >> offset;
        if offset+addr.len > 64 /*len > bits_in_w1*/ { // do we need more bits (from next segment)?
            let bits_in_w1 = 64-offset; // w1 has bits_in_w1 lowest bit set (copied from index_segment)
            w1 | (self.get_unchecked(segment+1) << bits_in_w1)
        } else {
            w1
        }
    }

    fn init_bits(&mut self, begin: usize, v: u64, len: u8) {
        debug_assert!({let f = self.get_bits(BitRange::begin_len(begin, len)); f == 0 || f == v});
        let (segment, offset) = (begin / 64, (begin % 64) as u8);
        if offset + len > 64 {
            self[segment+1] |= v >> (64-offset);
        }
        self[segment] |= v << offset;
    }

    fn set_bits(&mut self, begin: usize, v: u64, len: u8) {
        let (segment, offset) = (begin / 64, (begin % 64) as u8);
        let v_mask = n_lowest_bits(len);
        //let lo_bit_len = 64-offset;
        if offset + len > 64 /*len > lo_bit_len*/ {
            let shift = 64-offset; //lo_bit_len
            set_bits_to(&mut self[segment+1], v>>shift, v_mask>>shift);
        }
        set_bits_to(&mut self[segment], v<<offset, v_mask<<offset);
    }

    unsafe fn set_bits_unchecked(&mut self, begin: usize, v: u64, len: u8) {
        let (segment, offset) = (begin / 64, (begin % 64) as u8);
        let v_mask = n_lowest_bits(len);
        if offset + len > 64 {
            let shift = 64-offset; //lo_bit_len
            set_bits_to(self.get_unchecked_mut(segment+1), v>>shift, v_mask>>shift);
        }
        set_bits_to(self.get_unchecked_mut(segment), v<<offset, v_mask<<offset);
    }

    fn xor_bits(&mut self, begin: usize, v: u64, len: u8) {
        let (segment, offset) = (begin / 64, (begin % 64) as u8);
        if offset + len > 64 {
            let shift = 64-offset;
            self[segment+1] ^= v >> shift;
        }
        self[segment] ^= v << offset;
    }

    fn conditionally_change_bits<NewValue>(&mut self, new_value: NewValue, begin: usize, v_size: u8) -> u64
        where NewValue: FnOnce(u64) -> Option<u64>
    {
        let (segment, offset) = (begin / 64, (begin % 64) as u64);
        let w1 = self[segment]>>offset;
        let bits_in_w1 = 64-offset;
        let v_mask = n_lowest_bits(v_size);
        let r = if v_size as u64 > bits_in_w1 {
            w1 | (self[segment+1] << bits_in_w1)
        } else {
            w1
        } & v_mask;
        if let Some(v) = new_value(r) {
            if v_size as u64 > bits_in_w1 {
                set_bits_to(&mut self[segment + 1], v >> bits_in_w1, v_mask >> bits_in_w1);
            }
            set_bits_to(&mut self[segment], v << offset, v_mask << offset);
        }
        r
    }

    fn conditionally_copy_bits<Pred>(&mut self, src: &Self, predicate: Pred, begin: usize, v_size: u8)
        where Pred: FnOnce(u64, u64) -> bool
    {
        let (segment, offset) = (begin / 64, (begin % 64) as u8);
        let self_w1 = self[segment]>>offset;
        let mut src_w1 = src[segment]>>offset;
        let bits_in_w1 = 64-offset;
        let v_mask = n_lowest_bits(v_size);
        if v_size > bits_in_w1 {
            let w2_mask = v_mask >> bits_in_w1;
            let self_bits = self_w1 | ((self[segment+1] & w2_mask) << bits_in_w1);
            let src_w2 = src[segment+1] & w2_mask;
            if predicate(self_bits, src_w1 | (src_w2 << bits_in_w1)) {
                set_bits_to(&mut self[segment+1], src_w2, w2_mask);
                set_bits_to(&mut self[segment], src_w1 << offset, v_mask << offset);
            }
        } else {
            src_w1 &= v_mask;
            if predicate(self_w1 & v_mask, src_w1) {
                set_bits_to(&mut self[segment], src_w1 << offset, v_mask << offset);
            }
        };
    }

    fn trailing_zero_bits(&self) -> usize {
        for (i, v) in self.iter().copied().enumerate() {
            if v != 0 { return i * 64 + v.trailing_zeros() as usize; }
        }
        self.len() * 64 // the vector contains only zeros
    }

    fn find_bit_one(&self, start_index: usize) -> Option<usize> {
        let mut word_index = start_index / 64;
        let mut bits = self.get(word_index)? & !n_lowest_bits((start_index % 64) as u8);
        while bits == 0 {
            word_index += 1;
            bits = *self.get(word_index)?;
        }
        Some(word_index * 64 + (bits.trailing_zeros() as usize))
    }

    unsafe fn find_bit_one_unchecked(&self, start_index: usize) -> usize {
        let mut word_index = start_index / 64;
        let mut bits = self.get_unchecked(word_index) & !n_lowest_bits((start_index % 64) as u8);
        while bits == 0 {
            word_index += 1;
            bits = *self.get_unchecked(word_index);
        }
        word_index * 64 + bits.trailing_zeros() as usize
    }

    unsafe fn rfind_bit_one_unchecked(&self, start_index: usize) -> usize {
        let mut word_index = start_index / 64;
        let mut bits = self.get_unchecked(word_index) & n_lowest_bits_1_64((start_index % 64) as u8 + 1);
        while bits == 0 {
            word_index -= 1;
            bits = *self.get_unchecked(word_index);
        }
        word_index * 64 + bits.ilog2() as usize
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn fragments_init_set_swap() {
        let mut b = Box::<[u64]>::with_zeroed_64bit_segments(2);
        assert_eq!(b.as_ref(), [0u64, 0u64]);
        b.init_fragment(1, 0b101, 3);
        assert_eq!(b.get_fragment(1, 3), 0b101);
        assert_eq!(unsafe{b.find_bit_one_unchecked(0)}, 3);
        assert_eq!(unsafe{b.find_bit_one_unchecked(3)}, 3);
        assert_eq!(unsafe{b.find_bit_one_unchecked(4)}, 5);
        assert_eq!(b.get_fragment(0, 3), 0);
        assert_eq!(b.get_fragment(2, 3), 0);
        b.init_fragment(2, 0b10110_10110_10110_10110_10110_10110, 30);
        assert_eq!(b.get_fragment(2, 30), 0b10110_10110_10110_10110_10110_10110);
        assert_eq!(b.get_fragment(1, 30), 0);
        assert_eq!(b.get_fragment(3, 30), 0);
        b.set_fragment(2, 0b11010_11010_11111_00000_11111_10110, 30);
        assert_eq!(b.get_fragment(2, 30), 0b11010_11010_11111_00000_11111_10110);
        assert_eq!(b.get_fragment(1, 30), 0);
        assert_eq!(b.get_fragment(3, 30), 0);
        b.swap_fragments(2, 3, 30);
        assert_eq!(b.get_fragment(3, 30), 0b11010_11010_11111_00000_11111_10110);
        assert_eq!(b.get_fragment(2, 30), 0);
        assert_eq!(b.get_fragment(1, 30), 0);
    }

    #[test]
    fn fragments_conditionally_change() {
        let mut b = Box::<[u64]>::with_zeroed_64bit_segments(2);
        let old = b.conditionally_change_fragment(|old| if 0b101>old {Some(0b101)} else {None}, 1, 3);
        assert_eq!(old, 0);
        assert_eq!(b.get_fragment(1, 3), 0b101);
        assert_eq!(b.get_fragment(0, 3), 0);
        assert_eq!(b.get_fragment(2, 3), 0);
        let bits = 0b10110_10110_10110_10110_10110_10110;
        let old = b.conditionally_change_fragment(|old| if old==bits {Some(bits)} else {None}, 2, 30);
        assert_eq!(old, 0);
        assert_eq!(b.get_fragment(2, 30), 0);
        assert_eq!(b.get_fragment(1, 30), 0);
        assert_eq!(b.get_fragment(3, 30), 0);
        let old = b.conditionally_change_fragment(|old| if old!=bits {Some(bits)} else {None}, 2, 30);
        assert_eq!(old, 0);
        assert_eq!(b.get_fragment(2, 30), bits);
        assert_eq!(b.get_fragment(1, 30), 0);
        assert_eq!(b.get_fragment(3, 30), 0);
        let bits2 = 0b1100_11111_00000_10110_00111_11100;
        let old = b.conditionally_change_fragment(|old| if old!=bits2 {Some(bits2)} else {None}, 2, 30);
        assert_eq!(old, bits);
        assert_eq!(b.get_fragment(2, 30), bits2);
        assert_eq!(b.get_fragment(1, 30), 0);
        assert_eq!(b.get_fragment(3, 30), 0);
    }

    #[test]
    fn fragments_conditionally_copy() {
        let src = Box::<[u64]>::with_filled_64bit_segments(2);
        let mut dst = Box::<[u64]>::with_zeroed_64bit_segments(2);

        dst.conditionally_copy_fragment(&src,
                                        |old, new| { assert_eq!(old, 0); assert_eq!(new, 0b111); old > new},
                                        11, 3);
        assert_eq!(dst.get_fragment(11, 3), 0);
        assert_eq!(dst.get_fragment(12, 3), 0);
        dst.conditionally_copy_fragment(&src,
                                        |old, new| { assert_eq!(old, 0); assert_eq!(new, 0b111); old < new},
                                        11, 3);
        assert_eq!(dst.get_fragment(11, 3), 0b111);
        assert_eq!(dst.get_fragment(12, 3), 0);

        dst.conditionally_copy_fragment(&src,
            |old, new| { assert_eq!(old, 0); assert_eq!(new, 0b111); old > new},
            21, 3);
        assert_eq!(dst.get_fragment(21, 3), 0);
        assert_eq!(dst.get_fragment(22, 3), 0);
        dst.conditionally_copy_fragment(&src,
                                        |old, new| { assert_eq!(old, 0); assert_eq!(new, 0b111); old < new},
                                        21, 3);
        assert_eq!(dst.get_fragment(21, 3), 0b111);
        assert_eq!(dst.get_fragment(22, 3), 0);
    }

    #[test]
    fn bits() {
        let mut b = Box::<[u64]>::with_filled_64bit_segments(2);
        assert_eq!(b.as_ref(), [u64::MAX, u64::MAX]);
        assert_eq!(b.count_bit_ones(), 128);
        assert_eq!(b.count_bit_zeros(), 0);
        assert!(b.get_bit(3));
        assert!(b.get_bit(73));
        b.clear_bit(73);
        assert_eq!(b.count_bit_ones(), 127);
        assert_eq!(b.count_bit_zeros(), 1);
        assert!(!b.get_bit(73));
        assert!(b.get_bit(72));
        assert!(b.get_bit(74));
        b.set_bit(73);
        assert!(b.get_bit(73));
        b.xor_bits(72, 0b011, 3);
        assert!(!b.get_bit(72));
        assert!(!b.get_bit(73));
        assert!(b.get_bit(74));
    }

    #[test]
    fn iterators() {
        let b = [0b101u64, 0b10u64];
        let mut ones = b.bit_ones();
        assert_eq!(ones.len(), 3);
        assert_eq!(ones.next(), Some(0));
        assert_eq!(ones.len(), 2);
        assert_eq!(ones.next(), Some(2));
        assert_eq!(ones.len(), 1);
        assert_eq!(ones.next(), Some(64+1));
        assert_eq!(ones.len(), 0);
        assert_eq!(ones.next(), None);
        assert_eq!(ones.len(), 0);
        assert_eq!(ones.next(), None);
        assert_eq!(ones.len(), 0);
    }
}