use std::iter::FusedIterator;

use bitm::{BitAccess, BitVec, ArrayWithRankSelect101111, CombinedSampling, Rank, Select, Select0, SelectForRank101111, Select0ForRank101111};
use dyn_size_of::GetSize;

/// Constructs bit vectors for the (current) level of velvet matrix.
/// Stores bits of `lower_bits` values from previous level in to vectors:
/// - `upper_bit` stores the most significant bits (msb; shown by `upper_bit_mask`) of the subsequent values,
///     in the order from previous level,
/// - `lower_bits` stores all (`bits_per_value`) less significant bits (shown by `upper_bit_mask-1`)
///     of the subsequent values, stable sorted by most significant bits (msb).
/// The following values show the bit indices to insert the next value:
/// - `upper_index` is index of `upper_bit`,
/// - `lower_zero_index` is index of `lower_bits` to insert next value with 0 msb,
/// - `lower_one_index` is index of `lower_bits` to insert next value with 1 msb,
struct LevelBuilder {
    upper_bit: Box<[u64]>,
    upper_index: usize,
    lower_bits: Box<[u64]>,
    lower_zero_index: usize,
    lower_one_index: usize,
    upper_bit_mask: u64,
    bits_per_value: u8
}

impl LevelBuilder {
    /// Construct level builder for given level `total_len` in bits, `number_of_zeros` among the most significant bits
    /// and index of most significant bit (`index_of_bit_to_extract`).
    fn new(number_of_zeros: usize, total_len: usize, index_of_bit_to_extract: u8) -> Self {
        Self {
            upper_bit: Box::with_zeroed_bits(total_len + 1),    // we add one bit to ensure that rank(len) will work
            upper_index: 0,
            lower_bits: Box::with_zeroed_bits(total_len * index_of_bit_to_extract as usize + 1), // we add one bit to ensure that rank(len) will work
            lower_zero_index: 0,
            lower_one_index: number_of_zeros * index_of_bit_to_extract as usize,
            upper_bit_mask: 1<<index_of_bit_to_extract,
            bits_per_value: index_of_bit_to_extract
        }
    }

    /// Adds subsequent `value` from previous level to `self`.
    fn push(&mut self, value: u64) {
        let is_one = value & self.upper_bit_mask != 0;
        self.upper_bit.init_successive_bit(&mut self.upper_index, is_one);
        self.lower_bits.init_successive_bits(
            if is_one { &mut self.lower_one_index } else { &mut self.lower_zero_index },
            value & (self.upper_bit_mask-1), self.bits_per_value);
    }
}

/// Level of the we wavelet matrix.
struct Level<S = CombinedSampling> {
    /// Level content as bit vector with support for rank and select queries.
    content: ArrayWithRankSelect101111::<S, S>,

    /// Number of zero bits in content.
    number_of_zeros: usize
}

impl<S> GetSize for Level<S> where ArrayWithRankSelect101111<S, S>: GetSize {
    fn size_bytes_dyn(&self) -> usize { self.content.size_bytes_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl<S> Level<S> where ArrayWithRankSelect101111<S, S>: From<Box<[u64]>> {
    /// Constructs level with given `content` that contain given number of zero bits.
    fn new(content: Box::<[u64]>, number_of_zeros: usize) -> Self {
        //let (bits, number_of_ones) = ArrayWithRank::build(level);
        //Self { bits, zeros: level_len - number_of_ones }
        Self { content: content.into(), number_of_zeros }
    }
}

impl<S> Level<S> where S: SelectForRank101111 {
    fn try_select(&self, rank: usize, len: usize) -> Option<usize> {
        self.content.try_select(rank).filter(|i| *i < len)
    }
}

impl<S> Level<S> where S: Select0ForRank101111 {
    fn try_select0(&self, rank: usize, len: usize) -> Option<usize> {
        self.content.try_select0(rank).filter(|i| *i < len)
    }
}

/// [`Sequence`] stores a sequence of `len` `bits_per_value`-bit values
/// using just over (about 4%) `len * bits_per_value` bits and
/// quickly (mostly in *O(bits_per_value)* time) executes many useful queries, such as:
/// - *access* a value with a given index - see [`WaveletMatrix::get`],
/// - *select* - see [`WaveletMatrix::select`],
/// - *rank* - see [`WaveletMatrix::rank`].
/// 
/// By default [`bitm::CombinedSampling`] is used as a select strategy for internal bit vectors,
/// but this can be changed to [`bitm::BinaryRankSearch`] to save a bit
/// of space (about 0.78%) at the cost of slower *select* queries.
/// 
/// Our implementation is based on the following paper which proposed the method:
/// - Claude, F., Navarro, G. "The Wavelet Matrix", 2012,
///   In: Calderón-Benavides, L., González-Caro, C., Chávez, E., Ziviani, N. (eds)
///   "String Processing and Information Retrieval", SPIRE 2012,
///   Lecture Notes in Computer Science, vol 7608, Springer, Berlin, Heidelberg,
///   <https://doi.org/10.1007/978-3-642-34109-0_18>
/// 
/// Additionally, our implementation draws some ideas from the Go implementation by Daisuke Okanohara,
/// available at <https://github.com/hillbig/waveletTree/>.
pub struct Sequence<S = CombinedSampling> {
    levels: Box<[Level<S>]>,
    len: usize
}

impl<S> Sequence<S> {
    /// Returns number of stored values.
    #[inline] pub fn len(&self) -> usize { self.len }

    /// Returns whether the sequence is empty.
    #[inline] pub fn is_empty(&self) -> bool { self.len == 0 }

    /// Returns the size of each value in bits.
    #[inline] pub fn bits_per_value(&self) -> u8 { self.levels.len() as u8 }
}

impl Sequence<CombinedSampling> {
    /// Constructs [`Sequence`] with `content_len` `bits_per_value`-bit
    /// values exposed by iterator returned by `content` function.
    pub fn from_fn<I, F>(content: F, content_len: usize, bits_per_value: u8) -> Self
        where I: IntoIterator<Item = u64>, F: FnMut() -> I {
        Self::from_fn_s(content, content_len, bits_per_value)
    }

    /// Constructs [`Sequence`] with `content` consisted of `content_len` `bits_per_value`-bit
    /// values contained in the bit vector.
    pub fn from_bits(content: &[u64], content_len: usize, bits_per_value: u8) -> Self {
        Self::from_bits_s(content, content_len, bits_per_value)
    }
}

impl<S> Sequence<S> where S: SelectForRank101111+Select0ForRank101111 {

    /// Constructs [`Sequence`] with `content_len` `bits_per_value`-bit
    /// values exposed by iterator returned by `content` function,
    /// and custom select strategy.
    pub fn from_fn_s<I, F>(mut content: F, content_len: usize, bits_per_value: u8) -> Self
        where I: IntoIterator<Item = u64>, F: FnMut() -> I
    {
        assert!(bits_per_value > 0 && bits_per_value <= 63);
        let mut levels = Vec::with_capacity(bits_per_value as usize);
        if bits_per_value == 1 {
            let mut level = Box::with_zeroed_bits(content_len);
            for (i, e) in content().into_iter().enumerate() {
                level.init_bit(i, e != 0);
            }
            levels.push(Level::new(level, content_len));
            return Self { levels: levels.into_boxed_slice(), len: content_len };
        }
        let mut number_of_zeros = [0; 64];
        for mut e in content() {
            e = !e;
            for zeros in &mut number_of_zeros[0..bits_per_value as usize] {
                *zeros += (e & 1) as usize;
                e >>= 1;
            }
        }
        let mut current_bit = bits_per_value - 1;
        let mut rest = {
            let mut level = LevelBuilder::new(
                number_of_zeros[current_bit as usize], content_len, current_bit);
            for e in content() {
                level.push(e);
            }
            levels.push(Level::new(level.upper_bit, number_of_zeros[current_bit as usize]));
            level.lower_bits
        };
        while current_bit >= 2 {
            let rest_bits_per_value = current_bit;
            current_bit -= 1;
            let mut level = LevelBuilder::new(
                number_of_zeros[current_bit as usize], content_len, current_bit);
            for index in (0..content_len*rest_bits_per_value as usize).step_by(rest_bits_per_value as usize) {
                level.push(rest.get_bits(index, rest_bits_per_value));
            }
            rest = level.lower_bits;
            levels.push(Level::new(level.upper_bit, number_of_zeros[current_bit as usize]));
        }
        levels.push(Level::new(rest, number_of_zeros[0]));
        Self { levels: levels.into_boxed_slice(), len: content_len }
    }

    /// Constructs [`WaveletMatrix`] with `content` consisted of `content_len` `bits_per_value`-bit
    /// values contained in the bit vector, and custom select strategy.
    pub fn from_bits_s(content: &[u64], content_len: usize, bits_per_value: u8) -> Self {
        Self::from_fn_s(
            || { (0..content_len).map(|index| content.get_fragment(index, bits_per_value)) },
             content_len, bits_per_value)
    }

    /// Returns a value with given `index`. The result is undefined if `index` is out of bound.
    pub unsafe fn get_unchecked(&self, mut index: usize) -> u64 {
        let mut result = 0;
        for level in self.levels.iter() {
            result <<= 1;
            if level.content.content.get_bit(index) {
                result |= 1;
                index = level.content.rank_unchecked(index) + level.number_of_zeros;
            } else {
                index = level.content.rank0_unchecked(index);
            }
        }
        result
    }

    /// Returns a value with given `index` or [`None`] if `index` is out of bound.
    #[inline] pub fn get(&self, index: usize) -> Option<u64> {
        (index < self.len()).then(|| unsafe {self.get_unchecked(index)})
    }

    /// Returns a value with given `index` or panics if `index` is out of bound.
    #[inline] pub fn get_or_panic(&self, index: usize) -> u64 {
        self.get(index).expect("wavelet_matrix::Sequence::get index out of bound")
    }

    /// Returns the number of `value` occurrences in the given `range`, or [`None`] if `range` is out of bound.
    pub fn try_count_in_range(&self, mut range: std::ops::Range<usize>, value: u64) -> Option<usize> {
        if self.len() < range.end { return None; }
        let mut level_bit_mask = 1 << self.bits_per_value();
        for level in self.levels.iter() {
            level_bit_mask >>= 1;
            if value & level_bit_mask == 0 {
                range.start = level.content.rank0(range.start);
                range.end = level.content.rank0(range.end);
            } else {
                range.start = level.content.rank(range.start) + level.number_of_zeros;
                range.end = level.content.rank(range.end) + level.number_of_zeros;
            }
        }
        Some(range.len())
    }

    /// Returns the number of `value` occurrences in the given `range`, or panics if `range` is out of bound.
    pub fn count_in_range(&self, range: std::ops::Range<usize>, value: u64) -> usize {
        self.try_count_in_range(range, value).expect("wavelet_matrix::Sequence::count_in_range range out of bound")
    }

    /// Returns the number of `value` occurrences before given `index`, or [`None`] if `index` is out of bound.
    #[inline] pub fn try_rank(&self, index: usize, value: u64) -> Option<usize> {
        self.try_count_in_range(0..index, value)
    }

    /// Returns the number of `value` occurrences before the given `index`, or panics if `index` is out of bound.
    #[inline] pub fn rank(&self, index: usize, value: u64) -> usize {
        self.try_rank(index, value).expect("wavelet_matrix::Sequence::rank index out of bound")
    }

    /// The method from Claude-Navarro paper, used by select methods.
    #[inline] fn sel(&self, rank: usize, value: u64, index: usize, level_nr: usize) -> Option<usize> {
        let level = match self.levels.get(level_nr) {
            Some(level) => level,
            None => return Some(index + rank)
        };
        if value & (1<<(self.levels.len()-level_nr-1)) == 0 {
            level.try_select0(
                self.sel(rank, value, level.content.rank0(index), level_nr + 1)?,
                self.len
            )
        } else {
            level.try_select(
                self.sel(rank, value, level.content.rank(index) + level.number_of_zeros, level_nr + 1)?
                - level.number_of_zeros,
                self.len
            )
        }
    }

    /// Returns the index of the `rank`-th (counting from 0) occurrence of `value`
    /// or [`None`] if there are not so many occurrences.
    pub fn try_select(&self, rank: usize, value: u64) -> Option<usize> {
        self.sel(rank, value, 0, 0)
    }

    /// Returns the index of the `rank`-th (counting from 0) occurrence of `value`
    /// or panics if there are not so many occurrences.
    #[inline] pub fn select(&self, rank: usize, value: u64) -> usize {
        self.try_select(rank, value).expect("wavelet_matrix::Sequence::select: there are no rank occurrences of the value")
    }

    /// Returns iterator over all values.
    pub fn iter(&self) -> impl Iterator<Item = u64> + DoubleEndedIterator + FusedIterator + '_ {
        (0..self.len()).map(|i| unsafe { self.get_unchecked(i) })
    }
}

impl<S> GetSize for Sequence<S> where ArrayWithRankSelect101111<S, S>: GetSize {
    fn size_bytes_dyn(&self) -> usize { self.levels.size_bytes_dyn() }
    const USES_DYN_MEM: bool = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let wm = Sequence::from_bits(&[], 0, 2);
        assert_eq!(wm.len(), 0);
        assert_eq!(wm.bits_per_value(), 2);
        assert_eq!(wm.get(0), None);
        assert_eq!(wm.rank(0, 0), 0);
        assert_eq!(wm.iter().next(), None);
    }

    #[test]
    fn test_1_level() {
        let wm = Sequence::from_bits(&[0b1101], 4, 1);
        assert_eq!(wm.len(), 4);
        assert_eq!(wm.bits_per_value(), 1);
        assert_eq!(wm.get(0), Some(1));
        assert_eq!(wm.get(1), Some(0));
        assert_eq!(wm.get(2), Some(1));
        assert_eq!(wm.get(3), Some(1));
        assert_eq!(wm.get(4), None);
        assert_eq!(wm.try_rank(3, 1), Some(2));
        assert_eq!(wm.try_rank(3, 0), Some(1));
        assert_eq!(wm.try_select(0, 0), Some(1));
        assert_eq!(wm.try_select(1, 0), None);
        assert_eq!(wm.try_select(2, 1), Some(3));
        assert_eq!(wm.try_select(3, 1), None);
        assert_eq!(wm.iter().collect::<Vec<_>>(), [1, 0, 1, 1]);
    }

    #[test]
    fn test_2_levels() {
        let wm = Sequence::from_bits(&[0b01_01_10_11], 4, 2);
        assert_eq!(wm.len(), 4);
        assert_eq!(wm.bits_per_value(), 2);
        assert_eq!(wm.get(0), Some(0b11));
        assert_eq!(wm.get(1), Some(0b10));
        assert_eq!(wm.get(2), Some(0b01));
        assert_eq!(wm.get(3), Some(0b01));
        assert_eq!(wm.get(4), None);
        assert_eq!(wm.try_rank(2, 0b10), Some(1));
        assert_eq!(wm.try_rank(2, 0b11), Some(1));
        assert_eq!(wm.try_rank(2, 0b01), Some(0));
        assert_eq!(wm.try_select(0, 0b10), Some(1));
        assert_eq!(wm.try_select(0, 0b01), Some(2));
        assert_eq!(wm.iter().collect::<Vec<_>>(), [0b11, 0b10, 0b01, 0b01]);
    }

    #[test]
    fn test_3_levels() {
        let wm = Sequence::from_bits(&[0b000_110], 2, 3);
        assert_eq!(wm.len(), 2);
        assert_eq!(wm.bits_per_value(), 3);
        assert_eq!(wm.get(0), Some(0b110));
        assert_eq!(wm.get(1), Some(0b000));
        assert_eq!(wm.get(2), None);
    }

    #[test]
    fn test_4_levels() {
        let wm = Sequence::from_bits(&[0b1101_1010_0001_0001_1011], 5, 4);
        assert_eq!(wm.len(), 5);
        assert_eq!(wm.bits_per_value(), 4);
        assert_eq!(wm.get(0), Some(0b1011));
        assert_eq!(wm.get(1), Some(0b0001));
        assert_eq!(wm.get(2), Some(0b0001));
        assert_eq!(wm.get(3), Some(0b1010));
        assert_eq!(wm.get(4), Some(0b1101));
        assert_eq!(wm.get(5), None);
        assert_eq!(wm.try_rank(0, 0b1011), Some(0));
        assert_eq!(wm.try_rank(1, 0b0001), Some(0));
        assert_eq!(wm.try_rank(2, 0b0001), Some(1));
        assert_eq!(wm.try_rank(3, 0b0001), Some(2));
        assert_eq!(wm.try_rank(4, 0b0001), Some(2));
        assert_eq!(wm.try_rank(5, 0b0001), Some(2));
        assert_eq!(wm.try_rank(6, 0b0001), None);
        assert_eq!(wm.try_select(0, 0b0001), Some(1));
        assert_eq!(wm.try_select(1, 0b0001), Some(2));
        assert_eq!(wm.try_select(2, 0b0001), None);
    }
}