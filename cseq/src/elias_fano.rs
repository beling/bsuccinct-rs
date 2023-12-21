use std::iter::FusedIterator;

use bitm::{Select, ArrayWithRankSelect101111, CombinedSampling, SelectForRank101111, BitAccess, BitVec, n_lowest_bits};
use dyn_size_of::GetSize;

pub struct Builder {
    hi: Box<[u64]>, // most significant bits of each element, unary coded
    lo: Box<[u64]>, // least significant bits of each element, vector of `bits_per_lo_entry` bit elements
    bits_per_lo: u8,  // bit size of each entry in lo
    len: usize,  // number of already added elements
    final_len: usize,   // total number of elements to add
    last_added: u64, // recently added value
    universe: u64   // all values must be in range [`0`, `universe`)
}

impl Builder {
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

    pub fn finish_unchecked<S: SelectForRank101111>(self) -> Sequence<S> {
        Sequence::<S> {
            hi: self.hi.into(),
            lo: self.lo,
            bits_per_lo: self.bits_per_lo,
            len: self.len,
        }
    }

    pub fn finish<S: SelectForRank101111>(self) -> Sequence<S> {
        assert_eq!(self.len, self.final_len, "EliasFanoBuilder finish: actual length ({}) differs from the declared ({})", self.len, self.final_len);
        self.finish_unchecked::<S>()
    }
}

pub struct Sequence<S = CombinedSampling> {
    hi: ArrayWithRankSelect101111<S>,   // most significant bits of each element, unary coded
    lo: Box<[u64]>, // least significant bits of each element, vector of `bits_per_lo_entry` bit elements
    bits_per_lo: u8, // bit size of each entry in lo
    len: usize  // number of elements
}

impl<S> Sequence<S> {
    /// Returns number of stored values.
    #[inline] pub fn len(&self) -> usize { self.len }

    /// Returns whether the sequence is empty.
    #[inline] pub fn is_empty(&self) -> bool { self.len == 0 }

    #[inline] unsafe fn advance_position_unchecked(&self, position: &mut EliasFanoPosition) {
        position.lo += 1;
        position.hi = if position.lo != self.len {
            self.hi.content.find_bit_one_unchecked(position.hi+1)
        } else {
            self.len * 64
        }
    }

    #[inline] unsafe fn advance_position_back_unchecked(&self, position: &mut EliasFanoPosition) {
        position.lo -= 1;
        position.hi = self.hi.content.rfind_bit_one_unchecked(position.hi-1);
    }

    #[inline] unsafe fn value_at_position_unchecked(&self, position: EliasFanoPosition) -> u64 {
        position.hi_bits() << self.bits_per_lo | self.lo.get_fragment(position.lo, self.bits_per_lo)
    }

    #[inline] fn value_at_position(&self, position: EliasFanoPosition) -> Option<u64> {
        (position.lo < self.len).then(|| unsafe { self.value_at_position_unchecked(position) })
    }

    #[inline] fn begin_position(&self) -> EliasFanoPosition {
        EliasFanoPosition { hi: self.hi.content.trailing_zero_bits(), lo: 0 }
    }

    #[inline] fn end_position(&self) -> EliasFanoPosition {
        EliasFanoPosition { hi: self.hi.content.len() * 64, lo: self.len }
    }

    #[inline] pub fn iter(&self) -> Iterator<S> {
        Iterator { sequence: self, begin: self.begin_position(), end: self.end_position() } 
    }
}

impl<S: SelectForRank101111> Sequence<S> {
    #[inline] pub fn get(&self, index: usize) -> Option<u64> {
        (index < self.len).then(|| 
            (((unsafe{self.hi.select_unchecked(index)} - index) as u64) << self.bits_per_lo) |
            self.lo.get_fragment(index, self.bits_per_lo)
        )
    }

    pub fn get_or_panic(&self, index: usize) -> u64 {
        self.get(index).expect("EliasFano: get index out of bound")
    }
}

impl<S: SelectForRank101111> Select for Sequence<S> {
    #[inline(always)] fn try_select(&self, rank: usize) -> Option<usize> {
        self.get(rank).map(|v| v as usize)
    }
}

impl<S> GetSize for Sequence<S> where ArrayWithRankSelect101111<S>: GetSize {
    fn size_bytes_dyn(&self) -> usize { self.lo.size_bytes_dyn() + self.hi.size_bytes_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl<'ef, S> IntoIterator for &'ef Sequence<S> {
    type Item = u64;
    type IntoIter = Iterator<'ef, S>;
    #[inline] fn into_iter(self) -> Self::IntoIter { self.iter() }
}

/// Position in Elias-Fano [`Sequence`].
/// Used internally by [`Iterator`] and [`Cursor`].
#[derive(Clone, Copy)]
struct EliasFanoPosition {
    hi: usize,
    lo: usize
}

impl EliasFanoPosition {
    #[inline(always)] fn hi_bits(&self) -> u64 { (self.hi - self.lo) as u64 }
}

pub struct Iterator<'ef, S> {
    sequence: &'ef Sequence<S>,
    begin: EliasFanoPosition,
    end: EliasFanoPosition
}

impl<S> std::iter::Iterator for Iterator<'_, S> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.begin.lo == self.end.lo { return None; }
        let result = unsafe { self.sequence.value_at_position_unchecked(self.begin) };
        unsafe { self.sequence.advance_position_unchecked(&mut self.begin) }
        Some(result)
    }
}

impl<S> DoubleEndedIterator for Iterator<'_, S> {
    fn next_back(&mut self) -> Option<Self::Item> {
        (self.begin.lo != self.end.lo).then(|| unsafe {
            self.sequence.advance_position_back_unchecked(&mut self.end);
            self.sequence.value_at_position_unchecked(self.end)
        })
    }
}

impl<S> FusedIterator for Iterator<'_, S> {}

/// Shows position in Elias-Fano [`Sequence`].
pub struct Cursor<'ef, S> {
    sequence: &'ef Sequence<S>,
    position: EliasFanoPosition,
}

impl<S> Cursor<'_, S> {
    /// Returns whether the cursor is past the end (invalid).
    #[inline] pub fn is_end(&self) -> bool { self.position.lo != self.sequence.len }

    /// Returns whether the cursor is valid (i.e., not past the end) and thus its value can be obtained.
    #[inline] pub fn is_valid(&self) -> bool { self.position.lo != self.sequence.len }

    /// Returns value pointed by the cursor. The result is undefined if cursors points past the end.
    #[inline] pub unsafe fn value_unchecked(&self) -> u64 {
        return self.sequence.value_at_position_unchecked(self.position)
    }

    /// Returns value pointed by the cursor or [`None`] if it points past the end.
    #[inline] pub fn value(&self) -> Option<u64> {
        return self.sequence.value_at_position(self.position)
    }

    /// If possible, advances `self` by 1 position and returns `true`. Otherwise returns `false`.
    #[inline] pub fn advance(&mut self) -> bool {
        if self.is_end() { return false; }
        unsafe { self.sequence.advance_position_unchecked(&mut self.position) };
        true
    }

    /// If possible, advances `self` by minus 1 position and returns `true`. Otherwise returns `false`.
    #[inline] pub fn advance_back(&mut self) -> bool {
        if self.position.lo == 0 { return false; }
        unsafe { self.sequence.advance_position_back_unchecked(&mut self.position) };
        true
    }
}

impl<S> std::iter::Iterator for Cursor<'_, S> {
    type Item = u64;

    /// Advance cursor by one position forward.
    fn next(&mut self) -> Option<Self::Item> {
        if self.is_end() { return None; }
        let result = unsafe { self.value_unchecked() };
        unsafe { self.sequence.advance_position_unchecked(&mut self.position) }
        Some(result)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_sparse() {
        let mut ef = Builder::new(5, 1000);
        ef.push(0);
        ef.push(1);
        ef.push(801);
        ef.push(920);
        ef.push(999);
        let ef: Sequence = ef.finish();
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
        let mut ef = Builder::new(5, 6);
        ef.push(0);
        ef.push(1);
        ef.push(3);
        ef.push(4);
        ef.push(5);
        let ef: Sequence = ef.finish();
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