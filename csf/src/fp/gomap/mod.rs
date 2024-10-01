use ph::{BuildDefaultSeededHasher, BuildSeededHasher, stats, utils::ArrayWithRank};
use ph::fmph::{goindexing::group_nr, GroupSize, SeedSize, TwoToPowerBitsStatic};
pub use ph::fmph::GOConf;
use dyn_size_of::GetSize;

mod conf;
pub use conf::GOMapConf;

/// Finger-printing based compressed static function (immutable map)
/// that uses group optimization and maps hashable keys to unsigned integer values of given bit-size.
/// 
/// It usually takes somewhat more than *nb* bits to represent a function from an *n*-element set into a set of *b*-bit values.
/// (Smaller sizes are achieved when the set of values is small and the same values are assigned to multiple keys.)
/// The expected time complexities of its construction and evaluation are *O(n)* and *O(1)*, respectively.
pub struct GOMap<GS: GroupSize = TwoToPowerBitsStatic::<4>, SS: SeedSize = TwoToPowerBitsStatic<2>, S = BuildDefaultSeededHasher> {
    array: ArrayWithRank,
    values: Box<[u64]>,    // BitVec
    bits_per_value: u8,
    group_seeds: Box<[SS::VecElement]>,   //  Box<[u8]>,
    level_size: Box<[u64]>, // number of groups
    goconf: GOConf<GS, SS, S>,
}

impl<GS: GroupSize, SS: SeedSize, S> GetSize for GOMap<GS, SS, S> {
    fn size_bytes_dyn(&self) -> usize {
        self.array.size_bytes_dyn()
            + self.values.size_bytes_dyn()
            + self.group_seeds.size_bytes_dyn()
            + self.level_size.size_bytes_dyn()
    }
    const USES_DYN_MEM: bool = true;
}

