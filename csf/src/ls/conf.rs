use bitm::{BitAccess, BitVec};
use ph::{BuildDefaultSeededHasher, BuildSeededHasher};

/// Trait for pre-filling vector of values in [`ls::Map`](crate::ls::Map).
/// 
/// Pre-filling affects the values returned for keys not contained in the map.
pub trait ValuesPreFiller {
    /// Construct array of `n*bits_per_value` bits.
    fn create(&mut self, n: usize, bits_per_value: u8) -> Box<[u64]>;

    /// Init cell with given `index` to given `value` in the `array` constructed by [`Self::create`].
    #[inline(always)] fn init(&self, array: &mut [u64], index: usize, value: u64, bits_per_value: u8) {
        array.set_fragment(index, value, bits_per_value);
    }
}

/// Pre-fills the value vector of [`ls::Map`](crate::ls::Map) with zeros.
impl ValuesPreFiller for () {
    fn create(&mut self, n: usize, bits_per_value: u8) -> Box<[u64]> {
        Box::<[u64]>::with_zeroed_bits(n * bits_per_value as usize)
    }

    #[inline(always)] fn init(&self, array: &mut [u64], index: usize, value: u64, bits_per_value: u8) {
        array.init_fragment(index, value, bits_per_value);
    }
}

/// Pre-fills each 64-bit fragment of the value vector of [`ls::Map`](crate::ls::Map) with given pattern.
#[derive(Copy, Clone)]
pub struct FillWithPattern(u64);

impl ValuesPreFiller for FillWithPattern {
    fn create(&mut self, n: usize, bits_per_value: u8) -> Box<[u64]> {
        Box::<[u64]>::with_64bit_segments(self.0, bitm::ceiling_div(n * bits_per_value as usize, 64), )
    }
}

/// Pre-fills the value vector of [`ls::Map`](crate::ls::Map) with random values.
#[derive(Copy, Clone)]
pub struct FillRandomly{seed: u64}

impl ValuesPreFiller for FillRandomly {
    fn create(&mut self, n: usize, bits_per_value: u8) -> Box<[u64]> {
        (0..bitm::ceiling_div(n * bits_per_value as usize, 64)).map(|_| {
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 7;
            self.seed ^= self.seed << 17;
            self.seed
        }).collect()
    }
}

/// Configuration accepted by [`ls::Map`](crate::ls::Map) constructors.
#[derive(Default, Copy, Clone)]
pub struct MapConf<VPF = (), S = BuildDefaultSeededHasher> {
    /// The family of hash functions used by the constructed [`ls::Map`](crate::ls::Map). (default: [`BuildDefaultSeededHasher`])
    pub hash_builder: S,

    /// Pre-filler for vector of values in [`ls::Map`](crate::ls::Map).
    /// It affects the values returned for keys not contained in the map.
    /// Default pre-filler initializes the value vector with zeros.
    pub value_prefiller: VPF
}

/*impl<S: Default> Default for BDZConf<(), S> {
    fn default() -> Self {
        Self { hash_builder: Default::default(), buffer_manager: Default::default() }
    }
}*/

impl MapConf {
    /// Constructs default configuration.
    #[inline] pub fn new() -> Self { Default::default() }
}

impl<S: BuildSeededHasher> MapConf<(), S> {
    /// Constructs configuration with custom `hash_builder`.
    #[inline] pub fn hash(hash_builder: S) -> Self {
        Self { hash_builder, value_prefiller: Default::default() }
    }
}

impl<BM: ValuesPreFiller> MapConf<BM> {
    /// Constructs configuration with custom `value_prefiller`.
    #[inline] pub fn prefiller(value_prefiller: BM) -> Self {
        Self { hash_builder: Default::default(), value_prefiller }
    }
}

impl MapConf<FillWithPattern> {
    /// Constructs configuration with [`FillWithPattern`] value pre-filler.
    #[inline] pub fn pattern(pattern: u64) -> Self {
        Self::prefiller(FillWithPattern(pattern))
    }
}

impl MapConf<FillRandomly> {
    /// Constructs configuration with [`FillRandomly`] value pre-filler.
    #[inline] pub fn randomly(seed: u64) -> Self {
        Self::prefiller(FillRandomly{seed})
    }
}

impl<VPF: ValuesPreFiller, S: BuildSeededHasher> MapConf<VPF, S> {
    /// Constructs configuration with custom `value_prefiller` and `hash_builder`.
    #[inline] pub fn prefiller_hash(value_prefiller: VPF, hash_builder: S) -> Self {
        Self { hash_builder, value_prefiller }
    }
}