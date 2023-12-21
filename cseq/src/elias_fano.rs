use std::iter::FusedIterator;

use bitm::{Select, ArrayWithRankSelect101111, CombinedSampling, SelectForRank101111, BitAccess, BitVec, n_lowest_bits};
use dyn_size_of::GetSize;

pub struct EliasFanoBuilder {
    hi: Box<[u64]>, // most significant bits of each element, unary coded
    lo: Box<[u64]>, // least significant bits of each element, vector of `bits_per_lo_entry` bit elements
    bits_per_lo: u8,  // bit size of each entry in lo
    len: usize,  // number of already added elements
    final_len: usize,   // total number of elements to add
    last_added: u64, // recently added value
    universe: u64
}

impl EliasFanoBuilder {
    /// Constructs [`EliasFanoBuilder`] to build [`EliasFano`] with `final_len` values in range [`0`, `universe`).
    /// After adding values in non-decreasing order by [`Self::push`] method,
    /// [`Self::finish`] can be called to construct [`EliasFano`].
    pub fn new(final_len: usize, universe: u64) -> Self {
        if final_len == 0 || universe == 0 {
            return Self { hi: Default::default(), lo: Default::default(), bits_per_lo: 0, len: 0, final_len: 0, last_added: 0, universe };
        }
        let bits_per_lo = (universe / final_len as u64).checked_ilog2().unwrap_or(0) as u8;
        Self {
            // adding the last (i.e. (final_len-1)-th) element with value universe-1 sets bit (final_len-1) + ((universe-1) >> bits_per_lo)
            hi: Box::with_zeroed_bits(final_len + ((universe-1) >> bits_per_lo) as usize),
            lo: Box::with_zeroed_bits(1.max(final_len * bits_per_lo as usize)),
            bits_per_lo,
            len: 0,
            final_len,
            last_added: 0,
            universe,
        }
    }

    pub unsafe fn push_unchecked(&mut self, value: u64) {
        self.hi.set_bit((value>>self.bits_per_lo) as usize + self.len);
        self.lo.init_successive_fragment(&mut self.len, value & n_lowest_bits(self.bits_per_lo), self.bits_per_lo);
        self.last_added = value;
    }

    pub fn push(&mut self, value: u64) {
        assert!(value < self.universe, "EliasFanoBuilder: cannot push value {value} outside the universe (<{})", self.universe);
        assert!(self.len < self.final_len, "EliasFanoBuilder: push exceeds the declared length of {} values", self.final_len);
        assert!(self.last_added <= value, "EliasFanoBuilder: values must be pushed in non-decreasing order, but received {value} after {}", self.last_added);
        unsafe { self.push_unchecked(value) }
    }

    pub fn push_all<I: IntoIterator<Item = u64>>(&mut self, values: I) {
        for value in values { self.push(value) }
    }

    pub fn finish_unchecked<S: SelectForRank101111>(self) -> EliasFano<S> {
        EliasFano::<S> {
            hi: self.hi.into(),
            lo: self.lo,
            bits_per_lo: self.bits_per_lo,
            len: self.len,
        }
    }

    pub fn finish<S: SelectForRank101111>(self) -> EliasFano<S> {
        assert_eq!(self.len, self.final_len, "EliasFanoBuilder finish: actual length ({}) differs from the declared ({})", self.len, self.final_len);
        self.finish_unchecked::<S>()
    }
}

pub struct EliasFano<S = CombinedSampling> {
    hi: ArrayWithRankSelect101111<S>,   // most significant bits of each element, unary coded
    lo: Box<[u64]>, // least significant bits of each element, vector of `bits_per_lo_entry` bit elements
    bits_per_lo: u8, // bit size of each entry in lo
    len: usize  // number of elements
}

impl<S> EliasFano<S> {
    /// Returns number of stored values.
    #[inline] pub fn len(&self) -> usize { self.len }

    /// Returns whether the sequence is empty.
    #[inline] pub fn is_empty(&self) -> bool { self.len == 0 }

    #[inline] pub unsafe fn advance_position_unchecked(&self, position: &mut EliasFanoPosition) {
        position.lo += 1;
        position.hi = if position.lo != self.len {
            self.hi.content.find_bit_one_unchecked(position.hi+1)
        } else {
            self.len * 64
        }
    }

    #[inline] pub unsafe fn advance_position_back_unchecked(&self, position: &mut EliasFanoPosition) {
        position.lo -= 1;
        position.hi = self.hi.content.rfind_bit_one_unchecked(position.hi-1);
    }

    #[inline] pub unsafe fn value_at_position_unchecked(&self, position: EliasFanoPosition) -> u64 {
        position.hi_bits() << self.bits_per_lo | self.lo.get_fragment(position.lo, self.bits_per_lo)
    }

    #[inline] pub fn value_at_position(&self, position: EliasFanoPosition) -> Option<u64> {
        (position.lo < self.len).then(|| unsafe { self.value_at_position_unchecked(position) })
    }

    #[inline] pub fn begin_position(&self) -> EliasFanoPosition {
        EliasFanoPosition { hi: self.hi.content.trailing_zero_bits(), lo: 0 }
    }

    #[inline] pub fn end_position(&self) -> EliasFanoPosition {
        EliasFanoPosition { hi: self.hi.content.len() * 64, lo: self.len }
    }

    #[inline] pub fn iter(&self) -> EliasFanoIterator<S> {
        EliasFanoIterator { collection: self, begin: self.begin_position(), end: self.end_position() } 
    }
}

impl<S: SelectForRank101111> EliasFano<S> {
    #[inline] pub fn get(&self, index: usize) -> Option<u64> {
        // TODO (index < len).then ...
        Some(
            (((self.hi.try_select(index)? - index) as u64) << self.bits_per_lo) |
            self.lo.get_fragment(index, self.bits_per_lo)
        )
    }

    pub fn get_or_panic(&self, index: usize) -> u64 {
        self.get(index).expect("EliasFano: get index out of bound")
    }
}

impl<S: SelectForRank101111> Select for EliasFano<S> {
    #[inline(always)] fn try_select(&self, rank: usize) -> Option<usize> {
        self.get(rank).map(|v| v as usize)
    }
}

impl<S> GetSize for EliasFano<S> where ArrayWithRankSelect101111<S>: GetSize {
    fn size_bytes_dyn(&self) -> usize { self.lo.size_bytes_dyn() + self.hi.size_bytes_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl<'ef, S> IntoIterator for &'ef EliasFano<S> {
    type Item = u64;
    type IntoIter = EliasFanoIterator<'ef, S>;
    #[inline] fn into_iter(self) -> Self::IntoIter { self.iter() }
}

#[derive(Clone, Copy)]
pub struct EliasFanoPosition {
    hi: usize,
    lo: usize
}

impl EliasFanoPosition {
    #[inline(always)] fn hi_bits(&self) -> u64 { (self.hi - self.lo) as u64 }
}

pub struct EliasFanoIterator<'ef, S> {
    collection: &'ef EliasFano<S>,
    begin: EliasFanoPosition,
    end: EliasFanoPosition
}

impl<S> Iterator for EliasFanoIterator<'_, S> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.begin.lo == self.end.lo { return None; }
        let result = unsafe { self.collection.value_at_position_unchecked(self.begin) };
        unsafe { self.collection.advance_position_unchecked(&mut self.begin) }
        Some(result)
    }
}

impl<S> DoubleEndedIterator for EliasFanoIterator<'_, S> {
    fn next_back(&mut self) -> Option<Self::Item> {
        (self.begin.lo != self.end.lo).then(|| unsafe {
            self.collection.advance_position_back_unchecked(&mut self.end);
            self.collection.value_at_position_unchecked(self.end)
        })
    }
}

impl<S> FusedIterator for EliasFanoIterator<'_, S> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_sparse() {
        let mut ef = EliasFanoBuilder::new(5, 1000);
        ef.push(0);
        ef.push(1);
        ef.push(801);
        ef.push(920);
        ef.push(999);
        let ef: EliasFano = ef.finish();
        assert_eq!(ef.get(0), Some(0));
        assert_eq!(ef.get(1), Some(1));
        assert_eq!(ef.get(2), Some(801));
        assert_eq!(ef.get(3), Some(920));
        assert_eq!(ef.get(4), Some(999));
        assert_eq!(ef.get(5), None);
        assert_eq!(ef.iter().collect::<Vec<_>>(), [0, 1, 801, 920, 999]);
        assert_eq!(ef.iter().rev().collect::<Vec<_>>(), [999, 920, 801, 1, 0]);
    }

    #[test]
    fn test_small_dense() {
        let mut ef = EliasFanoBuilder::new(5, 6);
        ef.push(0);
        ef.push(1);
        ef.push(3);
        ef.push(4);
        ef.push(5);
        let ef: EliasFano = ef.finish();
        assert_eq!(ef.get(0), Some(0));
        assert_eq!(ef.get(1), Some(1));
        assert_eq!(ef.get(2), Some(3));
        assert_eq!(ef.get(3), Some(4));
        assert_eq!(ef.get(4), Some(5));
        assert_eq!(ef.get(5), None);
        assert_eq!(ef.iter().collect::<Vec<_>>(), [0, 1, 3, 4, 5]);
        assert_eq!(ef.iter().rev().collect::<Vec<_>>(), [5, 4, 3, 1, 0]);
    }
}