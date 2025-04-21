use bitm::{bits_to_store, ceiling_div, get_bits57, n_lowest_bits, set_bits57, BitAccess, BitVec};
use dyn_size_of::GetSize;
#[cfg(feature = "sux")] use sux::traits::IndexedSeq;

/// Builder used to construct `CompressedArray`.
pub trait CompressedBuilder {
    fn new(num_of_values: usize, max_value: usize) -> Self;
    fn push(&mut self, value: usize);

    #[inline]
    fn push_all(&mut self, values: impl IntoIterator<Item = usize>) {
        for value in values { self.push(value); }
    }
}

/// Compressed array of usize integers that can be used by `PHast`.
pub trait CompressedArray {
    type Builder: CompressedBuilder;

    fn finish(builder: Self::Builder) -> Self;

    #[inline] fn empty() -> Self where Self: Sized {
        Self::finish(Self::Builder::new(0, 0))
    }

    /// Construct array from the `bitmap` with given length and number of bit ones.
    #[inline] fn new(bitmap: &[u64], bitmap_len_bits: usize, number_of_ones: usize) -> Self where Self: Sized {
        if number_of_ones == 0 {
            Self::empty()
        } else {    // number_of_ones > 0
            let mut b = Self::Builder::new(number_of_ones, bitmap_largest(bitmap, bitmap_len_bits));
            for value in bitmap.bit_ones() { b.push(value); }
            Self::finish(b)
        }
    }

    /// Get `index`-th item from the array.
    fn get(&self, index: usize) -> usize;
}

/// Returns index of the last bit one in `bitmap` of given length.
#[inline]
pub fn bitmap_largest(bitmap: &[u64], bitmap_len_bits: usize) -> usize {
    let mut largest = bitmap_len_bits - 1;
    while !bitmap.get_bit(largest) { largest -= 1; }
    largest
}

/// CompressedArray implementation by Elias-Fano from `cseq` crate.
#[cfg(feature = "cseq")] pub type CSeqEliasFano = cseq::elias_fano::Sequence<bitm::CombinedSampling<bitm::ConstCombinedSamplingDensity<11>>, bitm::BinaryRankSearch>;

#[cfg(feature = "cseq")]
impl CompressedBuilder for cseq::elias_fano::Builder {
    #[inline] fn new(num_of_values: usize, max_value: usize) -> Self {
        cseq::elias_fano::Builder::new(num_of_values, max_value as u64+1)
    }

    #[inline] fn push(&mut self, value: usize) {
        unsafe { self.push_unchecked(value as u64); }
    }
}

#[cfg(feature = "cseq")]
impl CompressedArray for CSeqEliasFano {
    type Builder = cseq::elias_fano::Builder;
    
    #[inline] fn finish(builder: Self::Builder) -> Self {
        builder.finish_s()
    }

    #[inline]
    fn get(&self, index: usize) -> usize {
        //self.get_or_panic(index) as usize
        unsafe { self.get_unchecked(index) as usize }
    }
}

/// Implementation of `CompressedArray` that stores zig-zag encoded differences of values and linear functions
/// with the same number of bits required to store the largest difference.
pub struct ZigZagCompressedArray {
    pub items: Box<[u64]>,
    pub item_size: u8,
    //pub lowest: usize,    //TODO
    pub num_of_values: usize,
    pub max_value: usize
}

pub struct ZigZagBuilder {
    items: Vec<isize>,
    //lowest: usize,
    pub num_of_values: usize,
    pub max_value: usize
}

impl CompressedBuilder for ZigZagBuilder {
    fn new(num_of_values: usize, max_value: usize) -> Self {
        Self { items: Vec::with_capacity(num_of_values), num_of_values, max_value }
    }

    fn push(&mut self, value: usize) {
        todo!()
    }
}


/// Implementation of `CompressedArray` that stores each value with the same number of bits required to store the largest one.
pub struct Compact {
    pub items: Box<[u64]>,
    pub item_size: u8,
}

pub struct CompactBuilder {
    compact: Compact,
    index: usize
}

impl CompressedBuilder for CompactBuilder {
    fn new(num_of_values: usize, max_value: usize) -> Self {
        let item_size = bits_to_store(max_value as u64);
        Self {
            compact: Compact { items: Box::with_zeroed_bits(item_size as usize * num_of_values), item_size },
            index: 0
        }
    }

    #[inline] fn push(&mut self, value: usize) {
        self.compact.items.init_successive_bits(&mut self.index, value as u64, self.compact.item_size);
    }
}

impl GetSize for Compact {
    fn size_bytes_dyn(&self) -> usize { self.items.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.items.size_bytes_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl CompressedArray for Compact {
    type Builder = CompactBuilder;

    #[inline] fn finish(builder: Self::Builder) -> Self {
        builder.compact
    }

    #[inline] fn empty() -> Self where Self: Sized {
        Self { items: Box::new([]), item_size: 0 }
    }

    #[inline]
    fn get(&self, index: usize) -> usize {
        unsafe { self.items.get_fragment_unchecked(index, self.item_size) as usize }
    }
}


/// Implementation of `CompressedArray` that stores each value with the same number of bits required to store the largest one.
/// It uses unaligned memory reading and writing.
pub struct CompactFast {
    pub items: Box<[u8]>,
    pub item_size: u8,
}

pub struct CompactFastBuilder {
    compact: CompactFast,
    first_bit: usize,
    mask: u64
}

impl CompressedBuilder for CompactFastBuilder {
    #[inline] fn new(num_of_values: usize, max_value: usize) -> Self {
        let item_size = bits_to_store(max_value as u64);
        Self {
            compact: CompactFast { items: vec![0; ceiling_div(item_size as usize * num_of_values, 8) + 7].into_boxed_slice(), item_size },
            first_bit: 0,
            mask: n_lowest_bits(item_size)
        }
    }

    #[inline] fn push(&mut self, value: usize) {
        unsafe{set_bits57(self.compact.items.as_mut_ptr(), self.first_bit, value as u64, self.mask)};
        self.first_bit += self.compact.item_size as usize;
    }
}

impl GetSize for CompactFast {
    fn size_bytes_dyn(&self) -> usize { self.items.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.items.size_bytes_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl CompactFast {
    pub fn from_slice_max(slice: &[usize], max_item: usize) -> Self {
        let item_size = bits_to_store(max_item as u64);
        let mut items = vec![0; ceiling_div(item_size as usize * slice.len(), 8) + 7].into_boxed_slice();
        let mask = n_lowest_bits(item_size);
        for (index, value) in slice.into_iter().enumerate() {
            unsafe{set_bits57(items.as_mut_ptr(), index*item_size as usize, *value as u64, mask)};
        }
        Self { items, item_size }
    }
}

impl CompressedArray for CompactFast {
    type Builder = CompactFastBuilder;
    
    #[inline]
    fn finish(builder: Self::Builder) -> Self {
        builder.compact
    }

    #[inline] fn empty() -> Self {
        Self { items: Box::new([]), item_size: 0 }
    }

    #[inline]
    fn get(&self, index: usize) -> usize {
        (unsafe { get_bits57(self.items.as_ptr(), index * self.item_size as usize) & n_lowest_bits(self.item_size) }) as usize
    }
}


/// CompressedArray implementation by Elias-Fano from `sux` crate.
#[cfg(feature = "sux")] pub struct SuxEliasFano(sux::dict::elias_fano::EfSeq);

#[cfg(feature = "sux")] impl CompressedBuilder for sux::dict::EliasFanoBuilder {
    #[inline] fn new(num_of_values: usize, max_value: usize) -> Self {
        sux::dict::EliasFanoBuilder::new(num_of_values, max_value)
    }

    #[inline] fn push(&mut self, value: usize) {
        unsafe{ self.push_unchecked(value); }
    }
}

#[cfg(feature = "sux")]
impl CompressedArray for SuxEliasFano {
    type Builder = sux::dict::EliasFanoBuilder;
    
    #[inline] fn finish(builder: Self::Builder) -> Self {
        SuxEliasFano(builder.build_with_seq())
    }

    #[inline]
    fn get(&self, index: usize) -> usize {
        unsafe { self.0.get_unchecked(index) }
    }
}

#[cfg(feature = "sux")]
impl GetSize for SuxEliasFano {
    fn size_bytes_dyn(&self) -> usize {
        mem_dbg::MemSize::mem_size(&self.0, mem_dbg::SizeFlags::default()) - std::mem::size_of_val(self)
    }

    fn size_bytes_content_dyn(&self) -> usize { 
        mem_dbg::MemSize::mem_size(&self.0, mem_dbg::SizeFlags::default() | mem_dbg::SizeFlags::CAPACITY) - std::mem::size_of_val(self)
    }

    fn size_bytes(&self) -> usize {
        mem_dbg::MemSize::mem_size(&self.0, mem_dbg::SizeFlags::default())
    }

    const USES_DYN_MEM: bool = true;
}

#[cfg(feature = "cacheline-ef")]
/// CompressedArray implementation by Elias-Fano from `cacheline_ef` crate.
pub struct CachelineEF(cacheline_ef::CachelineEfVec);

#[cfg(feature = "cacheline-ef")]
impl CompressedBuilder for Vec<u64> {
    #[inline] fn new(num_of_values: usize, _max_value: usize) -> Self {
        Vec::with_capacity(num_of_values)
    }

    #[inline] fn push(&mut self, value: usize) {
        self.push(value as u64);
    }
}

#[cfg(feature = "cacheline-ef")]
impl CompressedArray for CachelineEF {
    type Builder = Vec<u64>;

    fn finish(builder: Self::Builder) -> Self {
        CachelineEF(cacheline_ef::CachelineEfVec::new(&builder))
    }

    fn empty() -> Self {
        CachelineEF(cacheline_ef::CachelineEfVec::default())
    }

    //#[inline]
    fn get(&self, index: usize) -> usize {
        unsafe { self.0.index_unchecked(index) as usize }
        //self.0.index(index) as usize
    }
}

#[cfg(feature = "cacheline-ef")]
impl GetSize for CachelineEF {
    fn size_bytes_dyn(&self) -> usize {
        mem_dbg::MemSize::mem_size(&self.0, mem_dbg::SizeFlags::default()) - std::mem::size_of_val(self)
    }

    fn size_bytes_content_dyn(&self) -> usize { 
        mem_dbg::MemSize::mem_size(&self.0, mem_dbg::SizeFlags::default() | mem_dbg::SizeFlags::CAPACITY) - std::mem::size_of_val(self)
    }

    fn size_bytes(&self) -> usize {
        mem_dbg::MemSize::mem_size(&self.0, mem_dbg::SizeFlags::default())
    }

    const USES_DYN_MEM: bool = true;
}

#[cfg(feature = "sux")] pub type DefaultCompressedArray = SuxEliasFano;
#[cfg(all(feature = "cacheline-ef", not(feature = "sux")))] pub type DefaultCompressedArray = CachelineEF;
#[cfg(all(feature = "cseq", not(feature = "sux"), not(feature="cacheline-ef")))] pub type DefaultCompressedArray = CSeqEliasFano;
#[cfg(all(not(feature="cseq"), not(feature = "sux"), not(feature="cacheline-ef")))] pub type DefaultCompressedArray = Compact;