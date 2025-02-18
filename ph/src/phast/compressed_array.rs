use bitm::{bits_to_store, BitAccess, BitVec};
use dyn_size_of::GetSize;
#[cfg(feature = "sux")] use sux::traits::IndexedSeq;

/// Compressed array of usize integers that can be used by `PHast`.
pub trait CompressedArray {
    /// Construct array from the `bitmap` with given length and number of bit ones.
    fn new(bitmap: &[u64], bitmap_len_bits: usize, number_of_ones: usize) -> Self;

    /// Get `index`-th item from the array.
    fn get(&self, index: usize) -> usize;
}

/// Returns index of the last bit one in `bitmap` of given length.
#[inline]
fn bitmap_largest(bitmap: &[u64], bitmap_len_bits: usize) -> usize {
    let mut largest = bitmap_len_bits - 1;
    while !bitmap.get_bit(largest) { largest -= 1; }
    largest
}

/// CompressedArray implementation by Elias-Fano from `cseq` crate.
#[cfg(feature = "cseq")] pub type CSeqEliasFano = cseq::elias_fano::Sequence<bitm::CombinedSampling<bitm::ConstCombinedSamplingDensity<11>>, bitm::BinaryRankSearch>;

#[cfg(feature = "cseq")]
impl CompressedArray for CSeqEliasFano {
    #[inline]
    fn new(bitmap: &[u64], bitmap_len_bits: usize, number_of_ones: usize) -> Self {
        if number_of_ones == 0 {
            cseq::elias_fano::Builder::new(0, 0).finish_s()
        } else {    // number_of_ones > 0
            let mut b = cseq::elias_fano::Builder::new(number_of_ones, bitmap_largest(bitmap, bitmap_len_bits) as u64+1);
            for value in bitmap.bit_ones() { unsafe { b.push_unchecked(value as u64); } }
            b.finish_s()
        }
    }

    #[inline]
    fn get(&self, index: usize) -> usize {
        //self.get_or_panic(index) as usize
        unsafe { self.get_unchecked(index) as usize }
    }
}

/// Implementation of `CompressedArray` that stores each value with the same number of bits required to store the largest one.
pub struct Compact {
    items: Box<[u64]>,
    item_size: u8,
}

impl GetSize for Compact {
    fn size_bytes_dyn(&self) -> usize { self.items.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.items.size_bytes_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl CompressedArray for Compact {
    #[inline]
    fn new(bitmap: &[u64], bitmap_len_bits: usize, number_of_ones: usize) -> Self {
        if number_of_ones == 0 {
            Self { items: Box::new([]), item_size: 0 }
        } else {    // number_of_ones > 0
            let item_size = bits_to_store(bitmap_largest(bitmap, bitmap_len_bits) as u64);
            let mut items = Box::with_zeroed_bits(item_size as usize * number_of_ones);
            let mut index = 0;
            for value in bitmap.bit_ones() {
                items.init_successive_fragment(&mut index, value as u64, item_size);
            }
            Self { items, item_size }
        }
    }

    #[inline]
    fn get(&self, index: usize) -> usize {
        unsafe { self.items.get_fragment_unchecked(index, self.item_size) as usize }
        //self.items.get_fragment(index, self.item_size) as usize   
    }
}

/// CompressedArray implementation by Elias-Fano from `sux` crate.
#[cfg(feature = "sux")] pub struct SuxEliasFano(sux::dict::elias_fano::EfSeq);

#[cfg(feature = "sux")]
impl CompressedArray for SuxEliasFano {
    fn new(bitmap: &[u64], bitmap_len_bits: usize, number_of_ones: usize) -> Self {
        SuxEliasFano(if number_of_ones == 0 {
            sux::dict::EliasFanoBuilder::new(0, 0).build_with_seq()
        } else {    // number_of_ones > 0
            let mut b = sux::dict::EliasFanoBuilder::new(number_of_ones, bitmap_largest(bitmap, bitmap_len_bits));
            for value in bitmap.bit_ones() { unsafe{ b.push_unchecked(value); } }
            b.build_with_seq()
        })
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
    fn new(bitmap: &[u64], _bitmap_len_bits: usize, number_of_ones: usize) -> Self {
        CachelineEF(if number_of_ones == 0 {
            cacheline_ef::CachelineEfVec::default()
        } else {    // number_of_ones > 0
            let mut vals = Vec::with_capacity(number_of_ones);
            for value in bitmap.bit_ones() { vals.push(value as u64); }
            cacheline_ef::CachelineEfVec::new(&vals)
        })
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