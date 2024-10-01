//! Wavelet Matrix representation of symbol sequence.

use std::{io, iter::FusedIterator, ops::{Deref, DerefMut}};

use binout::{AsIs, Serializer};
use bitm::{BitAccess, BitVec, RankSelect101111, CombinedSampling, Rank, Select, Select0, SelectForRank101111, Select0ForRank101111, bits_to_store, ceiling_div};
use dyn_size_of::GetSize;

/// Constructs bit vectors for the (current) level of velvet matrix.
/// Stores bits of `lower_bits` items from previous level in to vectors:
/// - `upper_bit` stores the most significant bits (msb; shown by `upper_bit_mask`) of the subsequent items,
///     in the order from previous level,
/// - `lower_bits` stores all (`bits_per_item`) less significant bits (shown by `upper_bit_mask-1`)
///     of the subsequent items, stable sorted by most significant bits (msb).
/// The following items show the bit indices to insert the next item:
/// - `upper_index` is index of `upper_bit`,
/// - `lower_zero_index` is index of `lower_bits` to insert next item with 0 msb,
/// - `lower_one_index` is index of `lower_bits` to insert next item with 1 msb,
struct LevelBuilder<BV> {
    upper_bit: BV,
    upper_index: usize,
    lower_bits: BV,
    lower_zero_index: usize,
    lower_one_index: usize,
    upper_bit_mask: u64,
    bits_per_item: u8
}

impl<BV: DerefMut<Target = [u64]> + BitVec> LevelBuilder<BV> {
    /// Construct level builder for given level `total_len` in bits, `number_of_zeros` among the most significant bits
    /// and index of most significant bit (`index_of_bit_to_extract`).
    fn new(number_of_zeros: usize, total_len: usize, index_of_bit_to_extract: u8) -> Self {
        Self {
            upper_bit: BV::with_zeroed_bits(total_len + 1),    // we add one bit to ensure that rank(len) will work
            upper_index: 0,
            lower_bits: BV::with_zeroed_bits(total_len * index_of_bit_to_extract as usize + 1), // we add one bit to ensure that rank(len) will work
            lower_zero_index: 0,
            lower_one_index: number_of_zeros * index_of_bit_to_extract as usize,
            upper_bit_mask: 1<<index_of_bit_to_extract,
            bits_per_item: index_of_bit_to_extract
        }
    }

    /// Adds subsequent `item` from previous level to `self`.
    fn push(&mut self, item: u64) {
        let is_one = item & self.upper_bit_mask != 0;
        self.upper_bit.init_successive_bit(&mut self.upper_index, is_one);
        self.lower_bits.init_successive_bits(
            if is_one { &mut self.lower_one_index } else { &mut self.lower_zero_index },
            item & (self.upper_bit_mask-1), self.bits_per_item);
    }
}

/// Level of the we wavelet matrix.
struct Level<S = CombinedSampling, BV = Box<[u64]>> {
    /// Level content as bit vector with support for rank and select queries.
    content: RankSelect101111::<S, S, BV>,

    /// Number of zero bits in content.
    number_of_zeros: usize
}

impl<S, BV> GetSize for Level<S, BV> where RankSelect101111<S, S, BV>: GetSize {
    fn size_bytes_dyn(&self) -> usize { self.content.size_bytes_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl<S, BV> Level<S, BV> where RankSelect101111<S, S, BV>: From<BV> {
    /// Constructs level with given `content` that contain given number of zero bits.
    #[inline] fn new(content: BV, number_of_zeros: usize) -> Self {
        //let (bits, number_of_ones) = ArrayWithRank::build(level);
        //Self { bits, zeros: level_len - number_of_ones }
        Self { content: content.into(), number_of_zeros }
    }
}

impl<S, BV> Level<S, BV> where S: SelectForRank101111, BV: Deref<Target = [u64]> {
    #[inline] fn try_select(&self, rank: usize, len: usize) -> Option<usize> {
        self.content.try_select(rank).filter(|i| *i < len)
    }
}

impl<S, BV> Level<S, BV> where S: Select0ForRank101111, BV: Deref<Target = [u64]> {
    #[inline] fn try_select0(&self, rank: usize, len: usize) -> Option<usize> {
        self.content.try_select0(rank).filter(|i| *i < len)
    }
}

/// [`Sequence`] stores a sequence of `len` `bits_per_item`-bit items within the Wavelet Matrix,
/// using just over (about 4%) `len * bits_per_item` bits and
/// quickly (mostly in *O(bits_per_item)* time) executes many useful queries, such as:
/// - *access* a item with a given index - see [`Self::get`],
/// - *select* - see [`Self::select`],
/// - *rank* - see [`Self::rank`].
/// 
/// By default [`bitm::CombinedSampling`] is used as a select strategy `S` for internal bit vectors
/// (see [`bitm::RankSelect101111`]), but this can be changed to [`bitm::BinaryRankSearch`]
/// to save a bit of space (about 0.78%) at the cost of slower *select* queries.
/// 
/// Our implementation is based on the following paper which proposed the method:
/// - Claude, F., Navarro, G. "The Wavelet Matrix", 2012,
///   In: Calderón-Benavides, L., González-Caro, C., Chávez, E., Ziviani, N. (eds)
///   "String Processing and Information Retrieval", SPIRE 2012,
///   Lecture Notes in Computer Science, vol 7608, Springer, Berlin, Heidelberg,
///   <https://doi.org/10.1007/978-3-642-34109-0_18>
/// 
/// Additionally, our implementation draws some ideas (like elimination of recursion)
/// from the Go implementation by Daisuke Okanohara,
/// available at <https://github.com/hillbig/waveletTree/>.
pub struct Sequence<S = CombinedSampling, BV = Box<[u64]>> {
    levels: Box<[Level<S, BV>]>,
    len: usize
}

impl<S, BV> Sequence<S, BV> {
    /// Returns number of stored items.
    #[inline] pub fn len(&self) -> usize { self.len }

    /// Returns whether the sequence is empty.
    #[inline] pub fn is_empty(&self) -> bool { self.len == 0 }

    /// Returns the size of each item in bits.
    #[inline] pub fn bits_per_item(&self) -> u8 { self.levels.len() as u8 }
}

impl Sequence<CombinedSampling> {
    /// Constructs [`Sequence`] with `content_len` `bits_per_item`-bit
    /// items exposed by iterator returned by `content` function.
    pub fn from_fn_len<I, F>(content: F, content_len: usize, bits_per_item: u8) -> Self
        where I: IntoIterator<Item = u64>, F: FnMut() -> I
    {
        Self::from_fn_len_s(content, content_len, bits_per_item)
    }

    /// Constructs [`Sequence`] with items exposed by iterator returned by `content` function.
    pub fn from_fn<I, F>(content: F) -> Self
        where I: IntoIterator<Item = u64>, F: FnMut() -> I
    {
        Self::from_fn_s(content)
    }

    /// Constructs [`Sequence`] with `content` consisted of `content_len` `bits_per_item`-bit
    /// items contained in the bit vector.
    pub fn from_bits(content: &[u64], content_len: usize, bits_per_item: u8) -> Self {
        Self::from_bits_s(content, content_len, bits_per_item)
    }

    /// Reads `self` from the `input`.
    pub fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Self::read_s(input)
    }
}

impl<S, BV> Sequence<S, BV> where S: SelectForRank101111+Select0ForRank101111, BV: BitVec+DerefMut<Target = [u64]> {

    /// Constructs [`Sequence`] with `content_len` `bits_per_item`-bit
    /// items exposed by iterator returned by `content` function,
    /// and custom select strategy.
    pub fn from_fn_len_s<I, F>(mut content: F, content_len: usize, bits_per_item: u8) -> Self
        where I: IntoIterator<Item = u64>, F: FnMut() -> I
    {
        assert!(bits_per_item > 0 && bits_per_item <= 63);
        let mut levels = Vec::with_capacity(bits_per_item as usize);
        if bits_per_item == 1 {
            let mut level = BV::with_zeroed_bits(content_len+1);
            for (i, e) in content().into_iter().enumerate() {
                level.init_bit(i, e != 0);
            }
            levels.push(Level::new(level, content_len));
            return Self { levels: levels.into_boxed_slice(), len: content_len };
        }
        let mut number_of_zeros = [0; 64];
        for mut e in content() {
            e = !e;
            for zeros in &mut number_of_zeros[0..bits_per_item as usize] {
                *zeros += (e & 1) as usize;
                e >>= 1;
            }
        }
        let mut current_bit = bits_per_item - 1;
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
            let rest_bits_per_item = current_bit;
            current_bit -= 1;
            let mut level = LevelBuilder::new(
                number_of_zeros[current_bit as usize], content_len, current_bit);
            for index in (0..content_len*rest_bits_per_item as usize).step_by(rest_bits_per_item as usize) {
                level.push(rest.get_bits(index, rest_bits_per_item));
            }
            rest = level.lower_bits;
            levels.push(Level::new(level.upper_bit, number_of_zeros[current_bit as usize]));
        }
        levels.push(Level::new(rest, number_of_zeros[0]));
        Self { levels: levels.into_boxed_slice(), len: content_len }
    }

    /// Constructs [`Sequence`] with items exposed by iterator returned by `content` function,
    /// and custom select strategy.
    pub fn from_fn_s<I, F>(mut content: F) -> Self
        where I: IntoIterator<Item = u64>, F: FnMut() -> I
    {
        let mut content_len = 0;
        let mut max_value = 1;  // we use 1 or more bits/item
        for v in content() {
            if v > max_value { max_value = v }
            content_len += 1;
        }
        Self::from_fn_len_s(content, content_len, bits_to_store(max_value))
    }

    /// Constructs [`Sequence`] with `content` consisted of `content_len` `bits_per_item`-bit
    /// items contained in the bit vector, and custom select strategy.
    pub fn from_bits_s(content: &[u64], content_len: usize, bits_per_item: u8) -> Self {
        Self::from_fn_len_s(
            || { (0..content_len).map(|index| content.get_fragment(index, bits_per_item)) },
             content_len, bits_per_item)
    }
}

impl<S, BV> Sequence<S, BV> where S: SelectForRank101111+Select0ForRank101111, BV: BitVec+Deref<Target = [u64]> {

    /// Returns an item with given `index`. The result is undefined if `index` is out of bounds.
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

    /// Returns an item with given `index` or [`None`] if `index` is out of bounds.
    #[inline] pub fn get(&self, index: usize) -> Option<u64> {
        (index < self.len()).then(|| unsafe {self.get_unchecked(index)})
    }

    /// Returns an item with given `index` or panics if `index` is out of bounds.
    #[inline] pub fn get_or_panic(&self, index: usize) -> u64 {
        self.get(index).expect("wavelet_matrix::Sequence::get index out of bound")
    }

    /// Returns the number of `item` occurrences in the given `range`, or [`None`] if `range` is out of bounds.
    pub fn try_count_in_range(&self, mut range: std::ops::Range<usize>, item: u64) -> Option<usize> {
        if self.len() < range.end { return None; }
        if item >> self.bits_per_item() != 0 { return Some(0); }
        let mut level_bit_mask = 1 << self.bits_per_item();
        for level in self.levels.iter() {
            level_bit_mask >>= 1;
            if item & level_bit_mask == 0 {
                range.start = level.content.rank0(range.start);
                range.end = level.content.rank0(range.end);
            } else {
                range.start = level.content.rank(range.start) + level.number_of_zeros;
                range.end = level.content.rank(range.end) + level.number_of_zeros;
            }
        }
        Some(range.len())
    }

    /// Returns the number of `item` occurrences in the given `range`, or panics if `range` is out of bounds.
    pub fn count_in_range(&self, range: std::ops::Range<usize>, item: u64) -> usize {
        self.try_count_in_range(range, item).expect("wavelet_matrix::Sequence::count_in_range range out of bound")
    }

    /// Returns the number of `item` occurrences before given `index`, or [`None`] if `index` is out of bounds.
    #[inline] pub fn try_rank(&self, index: usize, item: u64) -> Option<usize> {
        self.try_count_in_range(0..index, item)
    }

    /// Returns the number of `item` occurrences before the given `index`, or panics if `index` is out of bounds.
    #[inline] pub fn rank(&self, index: usize, item: u64) -> usize {
        self.try_rank(index, item).expect("wavelet_matrix::Sequence::rank index out of bound")
    }

    /// The method from Claude-Navarro paper, used by select methods.
    #[inline] fn sel(&self, rank: usize, item: u64, index: usize, level_nr: usize) -> Option<usize> {
        let level = match self.levels.get(level_nr) {
            Some(level) => level,
            None => return Some(index + rank)
        };
        if item & (1<<(self.levels.len()-level_nr-1)) == 0 {
            level.try_select0(
                self.sel(rank, item, level.content.rank0(index), level_nr + 1)?,
                self.len
            )
        } else {
            level.try_select(
                self.sel(rank, item, level.content.rank(index) + level.number_of_zeros, level_nr + 1)?
                - level.number_of_zeros,
                self.len
            )
        }
    }

    /// Returns the index of the `rank`-th (counting from 0) occurrence of `item`
    /// or [`None`] if there are not so many occurrences.
    pub fn try_select(&self, rank: usize, item: u64) -> Option<usize> {
        if item >> self.bits_per_item() != 0 { return None; }
        self.sel(rank, item, 0, 0)
    }

    /// Returns the index of the `rank`-th (counting from 0) occurrence of `item`
    /// or panics if there are not so many occurrences.
    #[inline] pub fn select(&self, rank: usize, item: u64) -> usize {
        self.try_select(rank, item).expect("wavelet_matrix::Sequence::select: not enough occurrences of the item")
    }

    /// Returns iterator over all items.
    pub fn iter(&self) -> impl Iterator<Item = u64> + DoubleEndedIterator + FusedIterator + '_ {
        (0..self.len()).map(|i| unsafe { self.get_unchecked(i) })
    }
}

impl<S, BV> Sequence<S, BV> where S: SelectForRank101111+Select0ForRank101111, BV: BitVec+Deref<Target = [u64]>+FromIterator<u64> {

    /// Reads `self` from the `input`.
    /// 
    /// Custom select strategy does not have to be the same as the one used by the written sequence.
    pub fn read_s(input: &mut dyn io::Read) -> io::Result<Self> {
        let len = AsIs::read(input)?;
        let bits_per_item: u8 = AsIs::read(input)?;
        let mut levels = Vec::with_capacity(bits_per_item as usize);
        for _ in 0..bits_per_item {
            let number_of_zeros = AsIs::read(input)?;
            //let content = AsIs::read_n(input, ceiling_div(len+1, 64))?;
            let content = <AsIs as Serializer<u64>>::read_n_iter(input, ceiling_div(len+1, 64)).collect::<io::Result::<BV>>()?;
            levels.push(Level::<S, BV>::new(content, number_of_zeros))
        }
        Ok(Self { levels: levels.into_boxed_slice(), len })
    }
}

impl<S, BV> GetSize for Sequence<S, BV> where RankSelect101111<S, S, BV>: GetSize {
    fn size_bytes_dyn(&self) -> usize { self.levels.size_bytes_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl<S, BV> Sequence<S, BV> where BV: Deref<Target = [u64]> {
    /// Returns number of bytes which `write` will write.
    pub fn write_bytes(&self) -> usize {
        AsIs::size(self.len) +
        AsIs::size(self.bits_per_item()) +
        self.levels.iter()
            .map(|level| AsIs::size(level.number_of_zeros) + AsIs::array_content_size(&level.content.content))
            .sum::<usize>()
    }

    /// Writes `self` to the `output`.
    pub fn write(&self, output: &mut dyn io::Write) -> io::Result<()>
    {
        AsIs::write(output, self.len)?;
        AsIs::write(output, self.bits_per_item())?;
        self.levels.iter().try_for_each(|level| {
            AsIs::write(output, level.number_of_zeros)?;
            AsIs::write_all(output, level.content.content.iter())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_read_write<S: SelectForRank101111+Select0ForRank101111>(seq: Sequence<S>) {
        let mut buff = Vec::new();
        seq.write(&mut buff).unwrap();
        assert_eq!(buff.len(), seq.write_bytes());
        let read = Sequence::<S>::read_s(&mut &buff[..]).unwrap();
        assert_eq!(seq.len(), read.len());
        for level_index in 0..seq.levels.len() {
            assert_eq!(seq.levels[level_index].number_of_zeros, read.levels[level_index].number_of_zeros);
            assert_eq!(seq.levels[level_index].content.content, read.levels[level_index].content.content);
        }
    }

    #[test]
    fn test_empty() {
        let wm = Sequence::from_bits(&[], 0, 2);
        assert_eq!(wm.len(), 0);
        assert_eq!(wm.bits_per_item(), 2);
        assert_eq!(wm.get(0), None);
        assert_eq!(wm.rank(0, 0), 0);
        assert_eq!(wm.iter().next(), None);
        test_read_write(wm);
    }

    #[test]
    fn test_zeros() {
        let wm = Sequence::from_fn(|| [0, 0, 0]);
        assert_eq!(wm.len(), 3);
        assert_eq!(wm.bits_per_item(), 1);
        assert_eq!(wm.get(0), Some(0));
        assert_eq!(wm.get(2), Some(0));
        assert_eq!(wm.get(3), None);
        test_read_write(wm);
    }

    #[test]
    fn test_1_level() {
        let wm = Sequence::from_bits(&[0b1101], 4, 1);
        assert_eq!(wm.len(), 4);
        assert_eq!(wm.bits_per_item(), 1);
        assert_eq!(wm.get(0), Some(1));
        assert_eq!(wm.get(1), Some(0));
        assert_eq!(wm.get(2), Some(1));
        assert_eq!(wm.get(3), Some(1));
        assert_eq!(wm.get(4), None);
        assert_eq!(wm.try_rank(3, 1), Some(2));
        assert_eq!(wm.try_rank(3, 0), Some(1));
        assert_eq!(wm.try_rank(3, 2), Some(0));
        assert_eq!(wm.try_select(0, 0), Some(1));
        assert_eq!(wm.try_select(1, 0), None);
        assert_eq!(wm.try_select(2, 1), Some(3));
        assert_eq!(wm.try_select(3, 1), None);
        assert_eq!(wm.try_select(0, 2), None);
        assert_eq!(wm.iter().collect::<Vec<_>>(), [1, 0, 1, 1]);
        test_read_write(wm);
    }

    #[test]
    fn test_2_levels() {
        let wm = Sequence::from_bits(&[0b01_01_10_11], 4, 2);
        assert_eq!(wm.len(), 4);
        assert_eq!(wm.bits_per_item(), 2);
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
        test_read_write(wm);
    }

    #[test]
    fn test_3_levels() {
        let wm = Sequence::from_bits(&[0b000_110], 2, 3);
        assert_eq!(wm.len(), 2);
        assert_eq!(wm.bits_per_item(), 3);
        assert_eq!(wm.get(0), Some(0b110));
        assert_eq!(wm.get(1), Some(0b000));
        assert_eq!(wm.get(2), None);
        test_read_write(wm);
    }

    #[test]
    fn test_4_levels() {
        let wm = Sequence::from_bits(&[0b1101_1010_0001_0001_1011], 5, 4);
        assert_eq!(wm.len(), 5);
        assert_eq!(wm.bits_per_item(), 4);
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
        test_read_write(wm);
    }

    #[test]
    fn test_mid_8_levels() {
        let wm = Sequence::from_fn(|| (0..1<<16).map(|v| v % 256));
        assert_eq!(wm.len(), 1<<16);
        assert_eq!(wm.bits_per_item(), 8);
        for i in (0..1<<16).step_by(33) {
            assert_eq!(wm.get(i), Some(i as u64 % 256), "wrong value at index {i}");
            assert_eq!(wm.try_rank(i, 255), Some(i/256), "wrong 255 rank at index {i}");
        }
        test_read_write(wm);
    }

    #[test]
    fn test_mid_21_levels() {
        let wm = Sequence::from_fn(|| (0..1<<16).map(|v| v * 32));
        assert_eq!(wm.len(), 1<<16);
        assert_eq!(wm.bits_per_item(), 21);
        for i in (1..1<<16).step_by(33) {
            assert_eq!(wm.get(i), Some(i as u64 * 32), "wrong value at index {i}");
            assert_eq!(wm.try_rank(i, 0), Some(1), "wrong 0 rank at index {i}");
            assert_eq!(wm.try_rank(i, 31), Some(0), "wrong 31 rank at index {i}");
        }
        test_read_write(wm);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    #[ignore = "uses much memory and time"]
    fn test_huge_11_levels() {
        let wm = Sequence::from_fn(|| (0..1<<32).map(|v| v % 2048));
        assert_eq!(wm.len(), 1<<32);
        assert_eq!(wm.bits_per_item(), 11);
        for i in (1<<32)-2055..1<<32 {
            assert_eq!(wm.get(i), Some(i as u64 % 2048), "wrong value at index {i}");
            assert_eq!(wm.try_rank(i, 2047), Some(i/2048), "wrong 2047 rank at index {i}");
        }
        test_read_write(wm);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    #[ignore = "uses much memory and time"]
    fn test_huge_9_levels() {
        let wm = Sequence::from_fn(|| (0..1<<33).map(|v| v % 512));
        assert_eq!(wm.len(), 1<<33);
        assert_eq!(wm.bits_per_item(), 9);
        for i in (0..1<<33).step_by(33) {
            assert_eq!(wm.get(i), Some(i as u64 % 512), "wrong value at index {i}");
            assert_eq!(wm.try_rank(i, 511), Some(i/512), "wrong 511 rank at index {i}");
        }
        test_read_write(wm);
    }
}