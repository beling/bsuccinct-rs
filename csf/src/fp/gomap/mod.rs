use bitm::{BitAccess, Rank};
use ph::{BuildDefaultSeededHasher, BuildSeededHasher, stats, utils::ArrayWithRank};
use ph::fmph::{goindexing::group_nr, GroupSize, SeedSize, TwoToPowerBitsStatic};
pub use ph::fmph::GOConf;
use dyn_size_of::GetSize;
use std::hash::Hash;

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
    level_sizes: Box<[u64]>, // number of groups
    goconf: GOConf<GS, SS, S>,
}

impl<GS: GroupSize, SS: SeedSize, S> GetSize for GOMap<GS, SS, S> {
    fn size_bytes_dyn(&self) -> usize {
        self.array.size_bytes_dyn()
            + self.values.size_bytes_dyn()
            + self.group_seeds.size_bytes_dyn()
            + self.level_sizes.size_bytes_dyn()
    }
    const USES_DYN_MEM: bool = true;
}

impl<GS: GroupSize, SS: SeedSize, S: BuildSeededHasher> GOMap<GS, SS, S> {

    /// Gets the value associated with the given `key` and reports statistics to `access_stats`.
    /// 
    /// If the `key` was not in the input key-value collection given during construction,
    /// either [`None`] or a value assigned to other key is returned.
    pub fn get_stats<K: Hash, A: stats::AccessStatsCollector>(&self, key: &K, access_stats: &mut A) -> Option<u8> {
        let mut groups_before = 0u64;
        let mut level_nr = 0u32;
        loop {
            let level_size_groups = *self.level_sizes.get(level_nr as usize)?;
            let hash = self.goconf.hash_builder.hash_one(key, level_nr);
            let group = groups_before + group_nr(hash, level_size_groups);
            let seed = self.goconf.bits_per_seed.get_seed(&self.group_seeds, group as usize);
            let bit_index = self.goconf.bits_per_group.bit_index_for_seed(hash, seed, group);
            if self.array.content.get_bit(bit_index) {
                access_stats.found_on_level(level_nr);
                //return Some(unsafe{self.array.rank_unchecked(bit_index)} as u64);
                return Some(self.values.get_fragment(self.array.rank(bit_index), self.bits_per_value) as u8);
            }
            groups_before += level_size_groups;
            level_nr += 1;
        }
    }

    /// Gets the value associated with the given `key`.
    /// 
    /// If the `key` was not in the input key-value collection given during construction,
    /// either [`None`] or a value assigned to other key is returned.
    #[inline] pub fn get<K: Hash>(&self, key: &K) -> Option<u8> {
        self.get_stats(key, &mut ())
    }

    
}
