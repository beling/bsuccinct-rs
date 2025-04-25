use bitm::{bits_to_store, ceiling_div, get_bits57, init_bits57, n_lowest_bits, BitAccess, BitVec};
use dyn_size_of::GetSize;
#[cfg(feature = "sux")] use sux::traits::IndexedSeq;

/// Compressed array of usize integers that can be used by `PHast`.
pub trait CompressedArray {
    /// Construct `Self`.
    fn new(values: Vec<usize>, last: usize) -> Self;

    /// Get `index`-th item from the array.
    fn get(&self, index: usize) -> usize;
}

/// Builder used to construct `CompressedArray`.
pub trait CompressedBuilder: Sized {
    fn new(num_of_values: usize, max_value: usize) -> Self;
    fn push(&mut self, value: usize);

    #[inline]
    fn with_all(values: Vec<usize>, last: usize) -> Self {
        let mut builder = Self::new(values.len(), last);
        for value in values { builder.push(value); }
        builder
    }
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

    fn new(values: Vec<usize>, last: usize) -> Self {
        cseq::elias_fano::Builder::with_all(values, last).finish_s()
    }

    #[inline]
    fn get(&self, index: usize) -> usize {
        //self.get_or_panic(index) as usize
        unsafe { self.get_unchecked(index) as usize }
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
    fn new(values: Vec<usize>, last: usize) -> Self {
        CompactBuilder::with_all(values, last).compact
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
}

impl CompressedBuilder for CompactFastBuilder {
    #[inline] fn new(num_of_values: usize, max_value: usize) -> Self {
        let item_size = bits_to_store(max_value as u64);
        Self {
            compact: CompactFast { items: vec![0; ceiling_div(item_size as usize * num_of_values, 8) + 7].into_boxed_slice(), item_size },
            first_bit: 0,
        }
    }

    #[inline] fn push(&mut self, value: usize) {
        unsafe{init_bits57(self.compact.items.as_mut_ptr(), self.first_bit, value as u64)};
        self.first_bit += self.compact.item_size as usize;
    }
}

impl GetSize for CompactFast {
    fn size_bytes_dyn(&self) -> usize { self.items.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.items.size_bytes_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl CompressedArray for CompactFast {
    fn new(values: Vec<usize>, last: usize) -> Self {
        CompactFastBuilder::with_all(values, last).compact
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
    fn new(values: Vec<usize>, last: usize) -> Self {
        SuxEliasFano(sux::dict::EliasFanoBuilder::with_all(values, last).build_with_seq())
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
impl CompressedArray for CachelineEF {
    fn new(values: Vec<usize>, _last: usize) -> Self {
        let v: Vec<_> = values.iter().map(|v| *v as u64).collect();
        CachelineEF(cacheline_ef::CachelineEfVec::new(&v))
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