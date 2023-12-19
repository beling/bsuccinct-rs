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
    }
}