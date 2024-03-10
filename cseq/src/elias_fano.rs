//! Elias-Fano representation of a non-decreasing sequence of integers.

use std::{io, iter::FusedIterator, ops::{Deref, DerefMut}};

use binout::{AsIs, Serializer};
use bitm::{ceiling_div, n_lowest_bits, RankSelect101111, BitAccess, BitVec, CombinedSampling, ConstCombinedSamplingDensity, Rank, Select, Select0, Select0ForRank101111, SelectForRank101111};
use dyn_size_of::GetSize;

/// Builds [`Sequence`] of values added by push methods.
/// After adding values in non-decreasing order by [`Self::push`] method,
/// [`Self::finish`] can be called to construct [`Sequence`].
pub struct Builder<BV = Box<[u64]>> {
    hi: BV, // most significant bits of each item, unary coded
    lo: Box<[u64]>, // least significant bits of each item, vector of `bits_per_lo_entry` bit items
    bits_per_lo: u8,  // bit size of each entry in lo
    current_len: usize,  // number of already pushed items
    target_len: usize,   // total number of items to push
    last_added: u64, // value of recently pushed item
    universe: u64   // all pushed items must be in range [`0`, `universe`)
}

impl<BV> Builder<BV> {
    /// Returns declared *universe*. All pushed items must be in range [0, *universe*).
    #[inline] pub fn universe(&self) -> u64 { self.universe }

    /// Returns number of already pushed items.
    #[inline] pub fn current_len(&self) -> usize { self.current_len }

    /// Returns total number of items to push.
    #[inline] pub fn target_len(&self) -> usize { self.target_len }

    /// Returns value of recently pushed item (if any) or 0.
    #[inline] pub fn last_pushed(&self) -> u64 { self.last_added }
}

impl<BV: BitVec> Builder<BV> {
    /// Constructs [`Builder`] to build [`Sequence`] with custom bit vector type and
    /// `final_len` values in range [`0`, `universe`).
    /// After adding values in non-decreasing order by [`Self::push`] method,
    /// [`Self::finish`] can be called to construct [`Sequence`].
    pub fn new_b(final_len: usize, universe: u64) -> Self {
        if final_len == 0 || universe == 0 {
            return Self { hi: BV::with_64bit_segments(0, 0), lo: Default::default(), bits_per_lo: 0, current_len: 0, target_len: 0, last_added: 0, universe };
        }
        let bits_per_lo = (universe / final_len as u64).checked_ilog2().unwrap_or(0) as u8;
        Self {
            // adding the last (i.e. (final_len-1)-th) item with value universe-1 sets bit (final_len-1) + ((universe-1) >> bits_per_lo)
            hi: BV::with_zeroed_bits(final_len + ((universe-1) >> bits_per_lo) as usize),
            lo: Box::with_zeroed_bits(1.max(final_len * bits_per_lo as usize)),
            bits_per_lo,
            current_len: 0,
            target_len: final_len,
            last_added: 0,
            universe,
        }
    }
}

impl Builder {
    /// Constructs [`Builder`] to build [`Sequence`] with `final_len` values in range [`0`, `universe`).
    /// After adding values in non-decreasing order by [`Self::push`] method,
    /// [`Self::finish`] can be called to construct [`Sequence`].
    #[inline] pub fn new(final_len: usize, universe: u64) -> Self {
        Self::new_b(final_len, universe)
    }
}

impl<BV: DerefMut<Target = [u64]>> Builder<BV> {
    /// A version of [`Self::push`] without any checks and panic.
    pub unsafe fn push_unchecked(&mut self, value: u64) {
        self.hi.set_bit((value>>self.bits_per_lo) as usize + self.current_len);
        self.lo.init_successive_fragment(&mut self.current_len, value & n_lowest_bits(self.bits_per_lo), self.bits_per_lo);
        self.last_added = value;
    }

    /// Pushes a value that is `diff` greater than the previous one, or from 0 if pushing the first value.
    /// A version of [`Self::push_diff`] without any checks and panic.
    pub unsafe fn push_diff_unchecked(&mut self, diff: u64) {
        self.push_unchecked(self.last_added+diff)
    }

    /// Pushes a `value`. It must be greater than or equal to previous one, and less than universe.
    /// Otherwise, or in case of an attempt to push too many items, panics.
    pub fn push(&mut self, value: u64) {
        assert!(value < self.universe, "Elias-Fano Builder: cannot push value {value} outside the universe (<{})", self.universe);
        assert!(self.current_len < self.target_len, "Elias-Fano Builder: push exceeds the declared length of {} values", self.target_len);
        assert!(self.last_added <= value, "Elias-Fano Builder: values must be pushed in non-decreasing order, but received {value} after {}", self.last_added);
        unsafe { self.push_unchecked(value) }
    }

    /// Pushes a value that is `diff` greater than the previous one, or from 0 if pushing the first value.
    /// Panics if the pushed item is not less than universe or all declared items has been already pushed.
    pub fn push_diff(&mut self, diff: u64) {
        self.push(self.last_added.saturating_add(diff))
    }

    /// Pushes all `values`. Calls [`Self::push`] for all `values` items.
    pub fn push_all<I: IntoIterator<Item = u64>>(&mut self, values: I) {
        for value in values { self.push(value) }
    }

    /// Calls [`Self::push_diff`] for all `diffs` items.
    pub fn push_diffs<I: IntoIterator<Item = u64>>(&mut self, diffs: I) {
        for diff in diffs { self.push_diff(diff) }
    }
}

impl<BV: Deref<Target = [u64]>> Builder<BV> {
    /// Finishes building and returns [`Sequence`] containing the pushed items and custom select strategy.
    /// The resulted [`Sequence`] is invalid if not all declared items have been pushed.
    pub fn finish_unchecked_s<S: SelectForRank101111, S0: Select0ForRank101111>(self) -> Sequence<S, S0, BV> {
        Sequence::<S, S0, BV> {
            hi: self.hi.into(),
            lo: self.lo,
            bits_per_lo: self.bits_per_lo,
            len: self.current_len,
        }
    }

    /// Finishes building and returns [`Sequence`] containing the pushed items and custom select strategy.
    /// Panics if not all declared items have been pushed. 
    pub fn finish_s<S: SelectForRank101111, S0: Select0ForRank101111>(self) -> Sequence<S, S0, BV> {
        assert_eq!(self.current_len, self.target_len, "Cannot finish building Elias-Fano Sequence as the current length ({}) differs from the target ({})", self.current_len, self.target_len);
        self.finish_unchecked_s::<S, S0>()
    }

    /// Finishes building and returns [`Sequence`] containing the pushed items.
    /// The resulted [`Sequence`] is invalid if not all declared items have been pushed.
    #[inline] pub fn finish_unchecked(self) -> Sequence<DefaultSelectStrategy, DefaultSelectStrategy, BV> {
        self.finish_unchecked_s()
    }

    /// Finishes building and returns [`Sequence`] containing the pushed items.
    /// Panics if not all declared items have been pushed. 
    #[inline] pub fn finish(self) -> Sequence<DefaultSelectStrategy, DefaultSelectStrategy, BV> {
        self.finish_s()
    }
}

/// Default select strategy for Elias-Fano [`Sequence`].
pub type DefaultSelectStrategy = CombinedSampling<ConstCombinedSamplingDensity>;

/// Elias-Fano representation of a non-decreasing sequence of integers.
/// 
/// By default [`bitm::CombinedSampling`] is used used as both a select and select0 strategy
/// for internal bit vector (see [`bitm::RankSelect101111`]).
/// However, either of these two strategies can be changed to [`bitm::BinaryRankSearch`]
/// to save a bit of space (maximum about 0.39% per strategy) at the cost of slower:
/// - (for select) getting an item at the given index,
/// - (for select0) finding the index of an item with a value (exactly or at least) equal to the given.
/// 
/// The structure was invented by Peter Elias and, independently, Robert Fano:
/// - Peter Elias "Efficient storage and retrieval by content and address of static files",
///   J. ACM 21 (2) (1974) 246–260. doi:10.1145/321812.321820.
/// - Robert Mario Fano "On the number of bits required to implement an associative memory",
///   Memorandum 61, Computer Structures Group, Project MAC, MIT, Cambridge, Mass., nd (1971) 27.
/// 
/// Our implementation draws ideas from:
/// - Sebastiano Vigna "Quasi-succinct indices", 2013,
///   In Proceedings of the sixth ACM international conference on Web search and data mining (WSDM '13),
///   Association for Computing Machinery, New York, NY, USA, 83–92. <https://doi.org/10.1145/2433396.2433409>
/// - Daisuke Okanohara, Kunihiko Sadakane, "Practical entropy-compressed rank/select dictionary",
///   Proceedings of the Meeting on Algorithm Engineering & Expermiments, January 2007, pages 60–70,
///   <https://dl.acm.org/doi/10.5555/2791188.2791194> (Section 6 "SDarrays")
pub struct Sequence<S = DefaultSelectStrategy, S0 = DefaultSelectStrategy, BV=Box<[u64]>> {
    hi: RankSelect101111<S, S0, BV>,   // most significant bits of each item, unary coded
    lo: Box<[u64]>, // least significant bits of each item, vector of `bits_per_lo` bit items
    bits_per_lo: u8, // bit size of each entry in lo
    len: usize  // number of items
}

impl<S, S0, BV> Sequence<S, S0, BV> {
    /// Returns number of stored values.
    #[inline] pub fn len(&self) -> usize { self.len }

    /// Returns whether the sequence is empty.
    #[inline] pub fn is_empty(&self) -> bool { self.len == 0 }

    #[inline] pub fn bits_per_lo(&self) -> u8 { self.bits_per_lo }
}

impl<S, S0, BV: Deref<Target = [u64]>> Sequence<S, S0, BV> {
    /// Advance `position` by 1 forward. The result is undefined if `position` is invalid.
    #[inline] unsafe fn advance_position_unchecked(&self, position: &mut Position) {
        position.lo += 1;
        position.hi = if position.lo != self.len {
            self.hi.content.find_bit_one_unchecked(position.hi+1)
        } else {
            self.hi.content.len() * 64
        }
    }

    /// Advance `position` by 1 backward. The result is undefined if `position` points to index 0.
    #[inline] unsafe fn advance_position_back_unchecked(&self, position: &mut Position) {
        position.lo -= 1;
        position.hi = self.hi.content.rfind_bit_one_unchecked(position.hi-1);
    }

    /// Returns value at `position` and next advance `position`. The result is undefined if `position` is invalid.
    #[inline] unsafe fn position_next_unchecked(&self, position: &mut Position) -> u64 {
        let result = self.value_at_position_unchecked(*position);
        self.advance_position_unchecked(position);
        result
    }

    /// If the `position` is valid, returns its value and next advances it. Otherwise returns [`None`].
    #[inline] fn position_next(&self, position: &mut Position) -> Option<u64> {
        (position.lo != self.len).then(|| unsafe { self.position_next_unchecked(position) })
    }

    #[inline] unsafe fn value_at_position_unchecked(&self, position: Position) -> u64 {
        position.hi_bits() << self.bits_per_lo | self.lo.get_fragment(position.lo, self.bits_per_lo)
    }

    /// Returns difference between the value of given and the previous positions.
    /// The result is undefined if the `position` is invalid.
    #[inline] unsafe fn diff_at_position_unchecked(&self, mut position: Position) -> u64 {
        let current = self.value_at_position_unchecked(position);
        if position.lo == 0 { return current; }
        self.advance_position_back_unchecked(&mut position);
        current - self.value_at_position_unchecked(position)
    }

    /// Returns difference between the value of given and the previous positions.
    /// Returns [`None`] if the `position` is invalid.
    #[inline] fn diff_at_position(&self, position: Position) -> Option<u64> {
        (position.lo != self.len).then(|| unsafe { self.diff_at_position_unchecked(position) })
    }

    /// Returns value at given `position` or [`None`] if the `position` is invalid.
    #[inline] fn value_at_position(&self, position: Position) -> Option<u64> {
        (position.lo != self.len).then(|| unsafe { self.value_at_position_unchecked(position) })
    }

    /// Returns position that points to the first item of `self`.
    #[inline] fn begin_position(&self) -> Position {
        Position { hi: self.hi.content.trailing_zero_bits(), lo: 0 }
    }

    /// Returns position that points past the end.
    #[inline] fn end_position(&self) -> Position {
        Position { hi: self.hi.content.len() * 64, lo: self.len }
    }

    /// Converts `position` to [`Cursor`].
    #[inline] fn cursor(&self, position: Position) -> Cursor<S, S0, BV> {
        Cursor { sequence: &self, position }
    }

    /// Returns iterator over `self` values.
    #[inline] pub fn iter(&self) -> Iterator<S, S0, BV> {
        Iterator { sequence: self, begin: self.begin_position(), end: self.end_position() } 
    }

    /// Returns an iterator that gives the value of the first item followed by
    /// the differences between the values of subsequent items.
    #[inline] pub fn diffs(&self) -> DiffIterator<S, S0, BV> {
        DiffIterator { sequence: self, position: self.begin_position(), prev_value: 0 } 
    }

    /// Returns cursor that points to the first item of `self`.
    #[inline] pub fn begin(&self) -> Cursor<S, S0, BV> {
        self.cursor(self.begin_position())
    }
    
    /// Returns cursor that points past the end.
    #[inline] pub fn end(&self) -> Cursor<S, S0, BV> {
        self.cursor(self.end_position())
    }

    /// Returns number of bytes which `write` will write.
    pub fn write_bytes(&self) -> usize {
        AsIs::size(self.bits_per_lo) +
        AsIs::array_size(&self.hi.content) +
        if self.bits_per_lo != 0 && self.len != 0 { AsIs::array_content_size(&self.lo) } else { 0 }
    }

    /// Writes `self` to the `output`.
    pub fn write(&self, output: &mut dyn io::Write) -> io::Result<()>
    {
        AsIs::write(output, self.bits_per_lo)?;
        AsIs::write_array(output, &self.hi.content)?;
        if self.bits_per_lo != 0 && self.len != 0 { AsIs::write_all(output, self.lo.iter())? };
        Ok(())
    }
}

impl Sequence {
    /// Constructs [`Sequence`] filled with elements from the `items` slice, which must be in non-decreasing order.
    #[inline] pub fn with_items_from_slice<I: Into<u64> + Clone>(items: &[I]) -> Self {
        Self::with_items_from_slice_s(items)
    }

    /// Reads `self` from the `input`.
    pub fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Self::read_s(input)
    }
}

impl<S: SelectForRank101111, S0: Select0ForRank101111, BV: Deref<Target = [u64]>+FromIterator<u64>> Sequence<S, S0, BV> {

    /// Reads `self` from the `input`.
    /// 
    /// Custom select strategies do not have to be the same as the ones used by the written sequence.
    pub fn read_s(input: &mut dyn io::Read) -> io::Result<Self> {
        let bits_per_lo: u8 = AsIs::read(input)?;
        let hi: BV = <AsIs as Serializer<u64>>::read_array_iter(input)?.collect::<io::Result<BV>>()?;
        let (hi, len) = RankSelect101111::build(hi);
        let lo = if bits_per_lo != 0 && len != 0 {
            AsIs::read_n(input, ceiling_div(len * bits_per_lo as usize, 64))?
        } else {
            (if len == 0 { vec![] } else { vec![0] }).into_boxed_slice()
        };
        Ok(Self { hi, lo, bits_per_lo, len })
    }
}

impl<S: SelectForRank101111, S0: Select0ForRank101111, BV: BitVec+DerefMut<Target = [u64]>> Sequence<S, S0, BV> {
    /// Constructs [`Sequence`] with custom select strategy and
    /// filled with elements from the `items` slice, which must be in non-decreasing order.
    pub fn with_items_from_slice_s<I: Into<u64> + Clone>(items: &[I]) -> Self {
        let mut b = Builder::<BV>::new_b(items.len(), items.last().map_or(0, |v| v.clone().into()+1));
        b.push_all(items.iter().map(|v| v.clone().into()));
        b.finish_unchecked_s()
    }
}

impl<S: SelectForRank101111, S0, BV: Deref<Target = [u64]>> Sequence<S, S0, BV> {
    /// Returns value at given `index`. The result is undefined if `index` is out of bounds.
    #[inline] pub unsafe fn get_unchecked(&self, index: usize) -> u64 {
        (((unsafe{self.hi.select_unchecked(index)} - index) as u64) << self.bits_per_lo) |
            self.lo.get_fragment(index, self.bits_per_lo)
    }

    /// Returns value at given `index` or [`None`] if `index` is out of bounds.
    #[inline] pub fn get(&self, index: usize) -> Option<u64> {
        (index < self.len).then(|| unsafe{self.get_unchecked(index)} )
    }

    /// Returns value at given `index` or panics if `index` is out of bounds.
    pub fn get_or_panic(&self, index: usize) -> u64 {
        self.get(index).expect("attempt to retrieve value for an index out of bounds of the Elias-Fano Sequence")
    }

    /// Returns difference between the value at given `index` and the previous value.
    /// If `index` is 0, returns value at index 0,just like [`Self::get_unchecked`].
    /// The result is undefined if `index` is out of bounds.
    #[inline] pub unsafe fn diff_unchecked(&self, index: usize) -> u64 {
        self.diff_at_position_unchecked(self.position_at_unchecked(index))
    }

    /// Returns difference between the value at given `index` and the previous value.
    /// If `index` is 0, returns value at index 0, just like [`Self::get`].
    /// Returns [`None`] if `index` is out of bounds.
    #[inline] pub fn diff(&self, index: usize) -> Option<u64> {
        (index < self.len).then(|| unsafe{self.diff_unchecked(index)})
    }

    /// Returns difference between the value at given `index` and the previous value.
    /// If `index` is 0, returns value at index 0, just like [`Self::get_or_panic`].
    /// Panics if `index` is out of bounds.
    #[inline] pub fn diff_or_panic(&self, index: usize) -> u64 {
        self.diff(index).expect("attempt to retrieve diff for an index out of bounds of the Elias-Fano Sequence")
    }

    #[inline] unsafe fn position_at_unchecked(&self, index: usize) -> Position {
        Position { hi: self.hi.select_unchecked(index), lo: index }
    }

    /*#[inline] fn position_at(&self, index: usize) -> Option<Position> {
        (index < self.len).then(|| unsafe { self.position_at_unchecked(index) })
    }*/

    /// Returns valid cursor that points to given `index` of `self`.
    /// Result is undefined if `index` is out of bounds.
    #[inline] pub unsafe fn cursor_at_unchecked(&self, index: usize) -> Cursor<S, S0, BV> {
        self.cursor(self.position_at_unchecked(index))
    }

    /// Returns valid cursor that points to given `index` of `self`,
    /// or [`None`] if `index` is out of bounds.
    #[inline] pub unsafe fn cursor_at(&self, index: usize) -> Option<Cursor<S, S0, BV>> {
        (index < self.len).then(|| unsafe { self.cursor_at_unchecked(index) })
    }
}

impl<S, S0: Select0ForRank101111, BV: Deref<Target = [u64]>> Sequence<S, S0, BV> {
    /// Returns the uncorrected position of first `self` item with value greater than or equal to given `value`.
    /// The `hi` of result may need correction (moving forward to first 1 bit) if it is not an index of 1 bit.
    /// `lo` is already correct.
    fn geq_position_uncorrected(&self, value: u64) -> Position {
        let value_hi = (value >> self.bits_per_lo) as usize;
        let mut hi_index = self.hi.try_select0(value_hi).unwrap_or_else(|| self.hi.content.len() * 64);  // index of 0 just after our ones
        // TODO do we always have such 0? maybe it is better to select0(value_hi-1) and next scan forward?
        let mut lo_index = hi_index - value_hi;

        let value_lo = value as u64 & n_lowest_bits(self.bits_per_lo);
        // skiping values that has the same most significant bits but greater or equal lower bits, stop at value with lower less significant bits:
        while lo_index > 0 && self.hi.content.get_bit(hi_index - 1) &&
             value_lo <= self.lo.get_fragment(lo_index-1, self.bits_per_lo)
        {
            lo_index -= 1;
            hi_index -= 1;
        }
        Position { hi: hi_index, lo: lo_index }
    }

    /// Returns the position of first `self` item with value greater than or equal to given `value`.
    fn geq_position(&self, value: u64) -> Position {
        let mut result = self.geq_position_uncorrected(value);
        result.hi = self.hi.content.find_bit_one(result.hi).unwrap_or_else(|| self.hi.content.len() * 64);
        result
    }

    fn position_of(&self, value: u64) -> Option<Position> {
        let geq_position = self.geq_position(value);
        self.value_at_position(geq_position).and_then(|geq_value| (geq_value==value).then_some(geq_position))
    }

    /// Returns the cursor pointed to the first `self` item with value greater than or equal to given `value`.
    #[inline] pub fn geq_cursor(&self, value: u64) -> Cursor<S, S0, BV> {
        self.cursor(self.geq_position(value))
    }

    /// Returns the index of the first `self` item with value greater than or equal to given `value`.
    #[inline] pub fn geq_index(&self, value: u64) -> usize {
        self.geq_position_uncorrected(value).lo
    }

    /// Returns the cursor pointing to the first occurrence of `value` or [`None`] if `self` does not contain `value`.
    #[inline] pub fn cursor_of(&self, value: u64) -> Option<Cursor<S, S0, BV>> {
        self.position_of(value).map(|position| self.cursor(position))
    }

    /// Returns the index of the first occurrence of `value` or [`None`] if `self` does not contain `value`.
    #[inline] pub fn index_of(&self, value: u64) -> Option<usize> {
        self.position_of(value).map(|p| p.lo)
    }
}

impl<S: SelectForRank101111, S0, BV: Deref<Target = [u64]>> Select for Sequence<S, S0, BV> {
    #[inline] unsafe fn select_unchecked(&self, rank: usize) -> usize {
        self.get_unchecked(rank) as usize
    }

    #[inline] fn try_select(&self, rank: usize) -> Option<usize> {
        self.get(rank).map(|v| v as usize)
    }
}

impl<S, S0: Select0ForRank101111, BV: Deref<Target = [u64]>> Rank for Sequence<S, S0, BV> {
    /// Returns the number of `self` items with values less than given `value`.
    #[inline] unsafe fn rank_unchecked(&self, value: usize) -> usize {
        self.geq_index(value as u64)
    }
    
    /// Returns the number of `self` items with values less than given `value`.
    #[inline] fn try_rank(&self, value: usize) -> Option<usize> {
        Some(self.geq_index(value as u64))
    }
}

impl<S, S0, BV> GetSize for Sequence<S, S0, BV> where RankSelect101111<S, S0, BV>: GetSize {
    fn size_bytes_dyn(&self) -> usize { self.lo.size_bytes_dyn() + self.hi.size_bytes_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl<'ef, S, S0, BV: Deref<Target = [u64]>> IntoIterator for &'ef Sequence<S, S0, BV> {
    type Item = u64;
    type IntoIter = Iterator<'ef, S, S0, BV>;
    #[inline] fn into_iter(self) -> Self::IntoIter { self.iter() }
}

/// Position in Elias-Fano [`Sequence`].
/// Used internally by [`Iterator`] and [`Cursor`].
#[derive(Clone, Copy)]
struct Position {
    hi: usize,
    lo: usize
}

impl Position {
    #[inline(always)] fn hi_bits(&self) -> u64 { (self.hi - self.lo) as u64 }
}

/// Iterator over [`Sequence`] values, returned by [`Sequence::iter`] .
pub struct Iterator<'ef, S, S0, BV> {
    sequence: &'ef Sequence<S, S0, BV>,
    begin: Position,
    end: Position
}

impl<S, S0, BV> Iterator<'_, S, S0, BV> {
    /// Returns the [`Sequence`] over which `self` iterates.
    pub fn sequence(&self) -> &Sequence<S, S0, BV> { self.sequence }

    /// Returns index of the value about to return by `next`.
    pub fn index(&self) -> usize { self.begin.lo }

    /// Returns index of the value returned by last `next_back`.
    pub fn back_index(&self) -> usize { self.begin.lo }
}

impl<S, S0, BV: Deref<Target = [u64]>> std::iter::Iterator for Iterator<'_, S, S0, BV> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        (self.begin.lo != self.end.lo).then(|| unsafe { self.sequence.position_next_unchecked(&mut self.begin) })
    }
}

impl<S, S0, BV: Deref<Target = [u64]>> DoubleEndedIterator for Iterator<'_, S, S0, BV> {
    fn next_back(&mut self) -> Option<Self::Item> {
        (self.begin.lo != self.end.lo).then(|| unsafe {
            self.sequence.advance_position_back_unchecked(&mut self.end);
            self.sequence.value_at_position_unchecked(self.end)
        })
    }
}

impl<S, S0, BV: Deref<Target = [u64]>> FusedIterator for Iterator<'_, S, S0, BV> {}

/// Iterator that yields the value of the first item followed by the differences
/// between the values of subsequent items of [`Sequence`].
pub struct DiffIterator<'ef, S, S0, BV> {
    sequence: &'ef Sequence<S, S0, BV>,
    position: Position,
    prev_value: u64
}

impl<S, S0, BV> DiffIterator<'_, S, S0, BV> {
    /// Returns the [`Sequence`] over which `self` iterates.
    pub fn sequence(&self) -> &Sequence<S, S0, BV> { self.sequence }

    /// Returns index of the value about to return by `next`.
    pub fn index(&self) -> usize { self.position.lo }
}

impl<S, S0, BV: Deref<Target = [u64]>> std::iter::Iterator for DiffIterator<'_, S, S0, BV> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        let current_value = self.sequence.position_next(&mut self.position)?;
        let result = current_value - self.prev_value;
        self.prev_value = current_value;
        Some(result)
    }
}

impl<S, S0, BV: Deref<Target = [u64]>> FusedIterator for DiffIterator<'_, S, S0, BV> {}

/// Points either a position or past the end in Elias-Fano [`Sequence`].
/// It is a kind of iterator over the [`Sequence`].
#[derive(Clone, Copy)]
pub struct Cursor<'ef, S, S0, BV> {
    sequence: &'ef Sequence<S, S0, BV>,
    position: Position,
}

impl<S, S0, BV> Cursor<'_, S, S0, BV> {
    /// Returns the [`Sequence`] in which `self` points the item.
    pub fn sequence(&self) -> &Sequence<S, S0, BV> { self.sequence }

    /// Returns whether `self` points is past the end (is invalid).
    #[inline] pub fn is_end(&self) -> bool { self.position.lo == self.sequence.len }

    /// Returns whether `self` is valid (i.e., not past the end) and thus its value can be obtained.
    #[inline] pub fn is_valid(&self) -> bool { self.position.lo != self.sequence.len }

    /// Returns [`Sequence`] index pointed by `self`, i.e. converts `self` to index.
    #[inline] pub fn index(&self) -> usize { self.position.lo }
}

impl<S, S0, BV: Deref<Target = [u64]>> Cursor<'_, S, S0, BV> {
    /// Returns value pointed by `self`. Result is undefined if `self` points past the end.
    #[inline] pub unsafe fn value_unchecked(&self) -> u64 {
        return self.sequence.value_at_position_unchecked(self.position)
    }

    /// Returns value pointed by `self` or [`None`] if it points past the end.
    #[inline] pub fn value(&self) -> Option<u64> {
        return self.sequence.value_at_position(self.position)
    }

    /// If possible, advances `self` one position forward and returns `true`. Otherwise returns `false`.
    #[inline] pub fn advance(&mut self) -> bool {
        if self.is_end() { return false; }
        unsafe { self.sequence.advance_position_unchecked(&mut self.position) };
        true
    }

    /// If possible, advances `self` one position backward and returns `true`. Otherwise returns `false`.
    #[inline] pub fn advance_back(&mut self) -> bool {
        if self.position.lo == 0 { return false; }
        unsafe { self.sequence.advance_position_back_unchecked(&mut self.position) };
        true
    }

    /// Advances `self` one position backward and next returns value pointed by `self`.
    pub fn next_back(&mut self) -> Option<u64> {
        (self.position.lo != 0).then(|| unsafe {
            self.sequence.advance_position_back_unchecked(&mut self.position);
            self.sequence.value_at_position_unchecked(self.position)
        })
    }

    /// Returns difference between the value of `self` and the previous positions.
    /// The result is undefined if `self` is invalid.
    #[inline] pub unsafe fn diff_unchecked(&self) -> u64 {
        self.sequence.diff_at_position_unchecked(self.position)
    }

    /// Returns difference between the value of `self` and the previous positions,
    /// or [`None`] if `self` is invalid.
    #[inline] pub fn diff(&self) -> Option<u64> {
        self.sequence.diff_at_position(self.position)
    }

    /// Returns an iterator that gives the the differences between the values of subsequent items,
    /// starting from `self`.
    #[inline] pub fn diffs(&self) -> DiffIterator<'_, S, S0, BV> {
        if self.position.lo == 0 { return self.sequence.diffs(); }
        let mut prev = self.position;
        unsafe{self.sequence.advance_position_back_unchecked(&mut prev)};
        DiffIterator { sequence: self.sequence, position: self.position, prev_value: unsafe{self.sequence.value_at_position_unchecked(prev)} }
    }
}

impl<S, S0, BV: Deref<Target = [u64]>> std::iter::Iterator for Cursor<'_, S, S0, BV> {
    type Item = u64;

    /// Returns value pointed by `self` and advances it one position forward.
    fn next(&mut self) -> Option<Self::Item> {
        self.sequence.position_next(&mut self.position)
    }
}

impl<S, S0, BV: Deref<Target = [u64]>> FusedIterator for Cursor<'_, S, S0, BV> {}


#[cfg(test)]
mod tests {
    use bitm::BinaryRankSearch;
    use super::*;

    fn test_read_write<S: SelectForRank101111, S0: Select0ForRank101111>(seq: Sequence<S, S0>) {
        let mut buff = Vec::new();
        seq.write(&mut buff).unwrap();
        assert_eq!(buff.len(), seq.write_bytes());
        let read = Sequence::<S, S0>::read_s(&mut &buff[..]).unwrap();
        assert_eq!(seq.len, read.len);
        assert_eq!(seq.hi.content, read.hi.content);
        assert_eq!(seq.lo, read.lo);
    }

    #[test]
    fn test_empty() {
        let ef = Builder::new(0, 0).finish();
        assert_eq!(ef.get(0), None);
        assert_eq!(ef.rank(0), 0);
        assert_eq!(ef.iter().collect::<Vec<_>>(), []);
        assert_eq!(ef.iter().rev().collect::<Vec<_>>(), []);
        test_read_write(ef);
    }

    #[test]
    fn test_small_sparse() {
        let mut ef = Builder::new(5, 1000);
        ef.push(0);
        ef.push(1);
        ef.push(801);
        ef.push(920);
        ef.push(999);
        let ef = ef.finish_s::<BinaryRankSearch, BinaryRankSearch>();
        assert_eq!(ef.get(0), Some(0));
        assert_eq!(ef.get(1), Some(1));
        assert_eq!(ef.get(2), Some(801));
        assert_eq!(ef.get(3), Some(920));
        assert_eq!(ef.get(4), Some(999));
        assert_eq!(ef.get(5), None);
        assert_eq!(ef.iter().collect::<Vec<_>>(), [0, 1, 801, 920, 999]);
        assert_eq!(ef.iter().rev().collect::<Vec<_>>(), [999, 920, 801, 1, 0]);
        assert_eq!(ef.geq_cursor(801).collect::<Vec<_>>(), [801, 920, 999]);
        assert_eq!(ef.geq_cursor(802).collect::<Vec<_>>(), [920, 999]);
        assert_eq!(ef.rank(0), 0);
        assert_eq!(ef.rank(1), 1);
        assert_eq!(ef.rank(2), 2);
        assert_eq!(ef.rank(800), 2);
        assert_eq!(ef.rank(801), 2);
        assert_eq!(ef.rank(802), 3);
        assert_eq!(ef.rank(920), 3);
        assert_eq!(ef.rank(921), 4);
        assert_eq!(ef.rank(999), 4);
        assert_eq!(ef.rank(1000), 5);
        let mut c = ef.cursor_of(920).unwrap();
        assert_eq!(c.index(), 3);
        assert_eq!(c.value(), Some(920));
        c.advance();
        assert_eq!(c.index(), 4);
        assert_eq!(c.value(), Some(999));
        c.advance();
        assert_eq!(c.index(), 5);
        assert_eq!(c.value(), None);
        test_read_write(ef);
    }

    #[test]
    fn test_small_dense() {
        let mut ef = Builder::new(5, 6);
        ef.push(0);
        ef.push(1);
        ef.push(3);
        ef.push(3);
        ef.push(5);
        let ef: Sequence = ef.finish();
        assert_eq!(ef.get(0), Some(0));
        assert_eq!(ef.get(1), Some(1));
        assert_eq!(ef.get(2), Some(3));
        assert_eq!(ef.get(3), Some(3));
        assert_eq!(ef.get(4), Some(5));
        assert_eq!(ef.get(5), None);
        assert_eq!(ef.iter().collect::<Vec<_>>(), [0, 1, 3, 3, 5]);
        assert_eq!(ef.geq_cursor(3).collect::<Vec<_>>(), [3, 3, 5]);
        assert_eq!(ef.geq_cursor(10).collect::<Vec<_>>(), []);
        assert_eq!(ef.iter().rev().collect::<Vec<_>>(), [5, 3, 3, 1, 0]);
        assert_eq!(ef.rank(0), 0);
        assert_eq!(ef.rank(1), 1);
        assert_eq!(ef.rank(2), 2);
        assert_eq!(ef.rank(3), 2);
        assert_eq!(ef.rank(4), 4);
        assert_eq!(ef.rank(5), 4);
        assert_eq!(ef.rank(6), 5);
        assert_eq!(ef.diff(0), Some(0));
        assert_eq!(ef.diff(1), Some(1));
        assert_eq!(ef.diff(2), Some(2));
        assert_eq!(ef.diff(3), Some(0));
        assert_eq!(ef.diff(4), Some(2));
        assert_eq!(ef.diff(5), None);
        assert_eq!(ef.diffs().collect::<Vec<_>>(), [0, 1, 2, 0, 2]);
        assert_eq!(ef.geq_cursor(3).diffs().collect::<Vec<_>>(), [2, 0, 2]);
        test_read_write(ef);
    }

    #[test]
    fn test_mid_1() {
        let mut ef = Builder::new(1<<16, 1<<16);
        ef.push_all(0..1<<16);
        let ef: Sequence = ef.finish();
        for i in (1..1<<16).step_by(33) {
            assert_eq!(ef.get(i), Some(i as u64));
            assert_eq!(ef.diff(i), Some(1));
            assert_eq!(ef.index_of(i as u64), Some(i));
            assert_eq!(ef.geq_index(i as u64), i);
        }
        test_read_write(ef);
    }

    #[test]
    fn test_mid_3() {
        let mut ef = Builder::new(1<<16, (1<<16)*3 + 1);
        ef.push_all((1..=1<<16).map(|v|v*3));   //1, 3, 6, ...
        let ef: Sequence = ef.finish();
        for i in (1usize..1<<16).step_by(33) {
            let value = (i as u64 + 1) * 3;
            assert_eq!(ef.get(i), Some(value), "get({}) should be {}", i, value);
            assert_eq!(ef.diff(i), Some(3));
            assert_eq!(ef.index_of(value), Some(i));
            assert_eq!(ef.geq_index(value), i);
        }
        test_read_write(ef);
    }

    #[test]
    #[ignore = "uses much memory and time"]
    fn test_huge() {
        let mut ef = Builder::new(1<<33, (1<<33)/2 + 1);
        ef.push_all((1..=1<<33).map(|v|v/2));
        let ef: Sequence = ef.finish();
        let mut prev_value = 0;
        for i in (1<<33)-2050..1<<33 {
            let value = (i as u64 + 1) / 2;
            assert_eq!(ef.get(i), Some(value));
            if prev_value != 0 {
                if value != prev_value {
                    assert_eq!(ef.index_of(value), Some(i));
                    assert_eq!(ef.geq_index(value), i);
                }
                assert_eq!(ef.diff(i), Some(value-prev_value));
            }
            prev_value = value;
        }
        test_read_write(ef);
    }
}