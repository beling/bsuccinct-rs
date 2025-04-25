use std::marker::PhantomData;

use bitm::{bits_to_store, ceiling_div, get_bits57, init_bits57, n_lowest_bits, BitAccess, BitVec};
use dyn_size_of::GetSize;
#[cfg(feature = "sux")] use sux::traits::IndexedSeq;

/// Compressed array of usize integers that can be used by `PHast`.
pub trait CompressedArray {

    /// Expect `values` to have `usize::MAX` for unused values; `false` if `values` must be sorted.
    const MAX_FOR_UNUSED: bool = false;

    /// Construct `Self`.
    fn new(values: Vec<usize>, last_in_value: usize, number_of_keys: usize) -> Self;

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

    fn new(values: Vec<usize>, last: usize, _num_of_keys: usize) -> Self {
        cseq::elias_fano::Builder::with_all(values, last).finish_s()
    }

    #[inline]
    fn get(&self, index: usize) -> usize {
        //self.get_or_panic(index) as usize
        unsafe { self.get_unchecked(index) as usize }
    }
}

/// Represents linear function f(i) = floor((multipler*i + offset) / divider).
pub struct LinearRegression {
    multipler: isize,   //usize
    offset: isize,      //usize
    divider: usize,
}

impl LinearRegression {
    /// Returns linear function f(i) = round((multipler*i + offset) / divider) + total_offset.
    #[inline] pub fn rounded(multipler: isize, offset: isize, divider: usize) -> Self {
        Self { multipler, offset: offset + divider as isize / 2, divider }
    }

    /// Add `total_offset` to each value returned by `get`.
    #[inline] pub fn add_total_offset(&mut self, total_offset: usize) {
        self.offset += (total_offset * self.divider) as isize;
    }

    /// Returns the value of function.
    #[inline(always)] pub fn get(&self, i: usize) -> usize {
        (self.multipler*i as isize + self.offset) as usize / self.divider
    }

    /// Get corrections for `self` and shift `self` to enable all corrections being non-negative.
    pub fn corrections(&mut self, values: Vec<usize>) -> CompactFast {
        let mut total_offset = 0;   // total offset for regression, max v - r difference
        let mut max_diff = 0;   // max r - v difference
        for (i, v) in values.iter().copied().enumerate() {
            if v == usize::MAX { continue; }
            let r = self.get(i);
            if r < v { 
                let diff = v - r;
                if diff > total_offset { total_offset = diff; }
            } else {
                let diff = r - v;
                if diff > max_diff { max_diff = diff; }
            }
        }
        self.add_total_offset(total_offset);  // new regression gives values >= included in values
        //let bits_per_correction = bits_to_store((total_offset + max_diff) as u64);
        let mut corrections = CompactFastBuilder::new(values.len(), total_offset + max_diff);
        //let mut real_max_diff = 0;
        for (i, v) in values.iter().copied().enumerate() {
            if v == usize::MAX {
                corrections.push(0);
            } else {
                let diff = self.get(i) - v;
                debug_assert!(diff <= total_offset + max_diff);
                corrections.push(diff);
                //if diff > real_max_diff { real_max_diff = diff; }
            }
        }
        corrections.compact
        //assert_eq!(real_max_diff, total_offset + max_diff);
    }
}

pub trait LinearRegressionConstructor {
    fn new(values: &[usize], num_of_keys: usize) -> LinearRegression;
}

pub struct Simple;

impl LinearRegressionConstructor for Simple {
    #[inline] fn new(values: &[usize], num_of_keys: usize) -> LinearRegression {
        LinearRegression::rounded(num_of_keys as isize, num_of_keys as isize, values.len()+1)
    }
}

pub struct LeastSquares;

impl LinearRegressionConstructor for LeastSquares {
    fn new(values: &[usize], _num_of_keys: usize) -> LinearRegression {        
        let mut n= 0;
        let mut x_sum = 0;
        let mut y_sum = 0;
        for (x, y) in values.iter().copied().enumerate() {
            if y == usize::MAX { continue; }
            n += 1;
            x_sum += x;
            y_sum += y;
        }
        if n == 0 { return LinearRegression::rounded(0, 0, 0); }
        let mut l = 0;
        let mut m = 0;
        for (x, y) in values.iter().copied().enumerate() {
            if y == usize::MAX { continue; }
            let x_diff = (n * x) as isize - x_sum as isize;
            m += (x_diff * x_diff) as usize;
            l += x_diff * ((n * y) as isize - y_sum as isize);
        }
        LinearRegression::rounded(n as isize * l, (m * y_sum) as isize - l * x_sum as isize, m * n)
    }
}

/// Implementation of `CompressedArray` that stores differences of values and linear regression
/// with the same number of bits required to store the largest difference.
pub struct LinearRegressionArray<C> {
    regression: LinearRegression,
    corrections: CompactFast,
    constructor: PhantomData<C>
}

impl<C: LinearRegressionConstructor> CompressedArray for LinearRegressionArray<C> {
    const MAX_FOR_UNUSED: bool = true;
    
    fn new(values: Vec<usize>, _last: usize, num_of_keys: usize) -> Self {
        let mut regression = C::new(&values, num_of_keys);
        let corrections = regression.corrections(values);
        Self { regression, corrections, constructor: PhantomData }
    }

    fn get(&self, index: usize) -> usize {
        self.regression.get(index) - self.corrections.get(index)
        //(unsafe { get_bits57(self.corrections.as_ptr(), index * self.bits_per_correction as usize) & n_lowest_bits(self.bits_per_correction) }) as usize
    }
}

impl<C> GetSize for LinearRegressionArray<C> {
    fn size_bytes_dyn(&self) -> usize { self.corrections.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.corrections.size_bytes_dyn() }
    const USES_DYN_MEM: bool = true;
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
    fn new(values: Vec<usize>, last: usize, _num_of_keys: usize) -> Self {
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
    fn new(values: Vec<usize>, last: usize, _num_of_keys: usize) -> Self {
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
    fn new(values: Vec<usize>, last: usize, _num_of_keys: usize) -> Self {
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