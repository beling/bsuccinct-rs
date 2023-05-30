use bitm::{BitAccess, BitVec};
use ph::{BuildDefaultSeededHasher, BuildSeededHasher};

pub trait BufferManager {
    /// Construct array of n*bits_per_value bits.
    fn create(&mut self, n: usize, bits_per_value: u8) -> Box<[u64]>;

    /// Init cell with given index to given value in array constructed by create.
    #[inline(always)] fn init(&self, array: &mut [u64], index: usize, value: u64, bits_per_value: u8) {
        array.set_fragment(index, value, bits_per_value);
    }
}

impl BufferManager for () {
    fn create(&mut self, n: usize, bits_per_value: u8) -> Box<[u64]> {
        Box::<[u64]>::with_zeroed_bits(n * bits_per_value as usize)
    }

    #[inline(always)] fn init(&self, array: &mut [u64], index: usize, value: u64, bits_per_value: u8) {
        array.init_fragment(index, value, bits_per_value);
    }
}

#[derive(Copy, Clone)]
pub struct FillWithPattern{pattern: u64}

impl BufferManager for FillWithPattern {
    fn create(&mut self, n: usize, bits_per_value: u8) -> Box<[u64]> {
        Box::<[u64]>::with_64bit_segments(self.pattern, bitm::ceiling_div(n * bits_per_value as usize, 64), )
    }
}

#[derive(Copy, Clone)]
pub struct FillRandomly{seed: u64}

impl BufferManager for FillRandomly {
    fn create(&mut self, n: usize, bits_per_value: u8) -> Box<[u64]> {
        (0..bitm::ceiling_div(n * bits_per_value as usize, 64)).map(|_| {
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 7;
            self.seed ^= self.seed << 17;
            self.seed
        }).collect()
    }
}

#[derive(Default, Copy, Clone)]
pub struct MapConf<BM = (), S = BuildDefaultSeededHasher> {
    pub hash_builder: S,
    pub buffer_manager: BM
}

/*impl<S: Default> Default for BDZConf<(), S> {
    fn default() -> Self {
        Self { hash_builder: Default::default(), buffer_manager: Default::default() }
    }
}*/

impl MapConf {
    pub fn new() -> Self { Default::default() }
}

impl<S: BuildSeededHasher> MapConf<(), S> {
    pub fn hash(hash_builder: S) -> Self {
        Self { hash_builder, buffer_manager: Default::default() }
    }
}

impl<BM: BufferManager> MapConf<BM> {
    pub fn bm(buffer_manager: BM) -> Self {
        Self { hash_builder: Default::default(), buffer_manager }
    }
}

impl MapConf<FillWithPattern> {
    pub fn pattern(pattern: u64) -> Self {
        Self { hash_builder: Default::default(), buffer_manager: FillWithPattern{pattern} }
    }
}

impl MapConf<FillRandomly> {
    pub fn randomly(seed: u64) -> Self {
        Self { hash_builder: Default::default(), buffer_manager: FillRandomly{seed} }
    }
}

impl<BM: BufferManager, S: BuildSeededHasher> MapConf<BM, S> {
    pub fn bm_hash(buffer_manager: BM, hash_builder: S) -> Self {
        Self { hash_builder, buffer_manager }
    }
}