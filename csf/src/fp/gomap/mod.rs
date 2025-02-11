use bitm::{BitAccess, Rank};
use ph::{BuildDefaultSeededHasher, BuildSeededHasher, stats, utils::ArrayWithRank};
use ph::fmph::{goindexing::group_nr, GroupSize, SeedSize, TwoToPowerBitsStatic};
pub use ph::fmph::GOConf;
use dyn_size_of::GetSize;
use std::hash::Hash;

mod conf;
pub use conf::GOMapConf;

/*use crate::fp::CollisionSolver;

use super::kvset::KVSet;
use super::{CollisionSolverBuilder, LevelSizer};*/

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
    pub fn get_stats<K: Hash + ?Sized, A: stats::AccessStatsCollector>(&self, key: &K, access_stats: &mut A) -> Option<u64> {
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
                return Some(self.values.get_fragment(self.array.rank(bit_index), self.bits_per_value));
            }
            groups_before += level_size_groups;
            level_nr += 1;
        }
    }

    /// Gets the value associated with the given `key`.
    /// 
    /// If the `key` was not in the input key-value collection given during construction,
    /// either [`None`] or a value assigned to other key is returned.
    #[inline] pub fn get<K: Hash + ?Sized>(&self, key: &K) -> Option<u64> {
        self.get_stats(key, &mut ())
    }

    /// Gets the value associated with the given `key` and reports statistics to `access_stats`.
    /// 
    /// If the `key` was not in the input key-value collection given during construction,
    /// it either panics or returns a value assigned to other key is returned.
    #[inline] pub fn get_stats_or_panic<K: Hash + ?Sized, A: stats::AccessStatsCollector>(&self, key: &K, access_stats: &mut A) -> u64 {
        self.get_stats(key, access_stats).expect("Invalid access to a key outside the set given during construction.")
    }

    /// Gets the value associated with the given `key`.
    /// 
    /// If the `key` was not in the input key-value collection given during construction,
    /// it either panics or returns a value assigned to other key is returned.
    #[inline] pub fn get_or_panic<K: Hash + ?Sized>(&self, key: &K) -> u64 {
        self.get_stats_or_panic(key, &mut ())
    }

    

    /*TODO pub fn with_conf_stats<K, KV, LSC, CSB, BS>(kv: KV, conf: GOMapConf<LSC, CSB, GS, SS, S>, stats: &mut BS) -> Self
        where K: Hash, KV: KVSet<K>, LSC: LevelSizer, CSB: CollisionSolverBuilder, BS: stats::BuildStatsCollector
    {
        let bits_per_value = kv.bits_per_value();
        let level_sizes = Vec::<usize>::new();
        let arrays = Vec::<Box<[u64]>>::new();
        let values_lens = Vec::<usize>::new();
        let values = Vec::<Box<[u64]>>::new();
        let groups = Vec::<Box<[u64]>>::new();
        let mut input_size = kv.kv_len();
        let mut level_nr = 0;
        while input_size != 0 {           
            let (level_size_groups, level_size_segments) = conf.goconf.bits_per_group
                .level_size_groups_segments(conf.level_sizer.size_segments(&kv) * 64);
            stats.level(input_size, level_size_segments * 64);
            
            let mut collision_solver: <CSB as CollisionSolverBuilder>::CollisionSolver = conf.collision_solver.new(level_size_segments, bits_per_value);
            kv.process_all_values(|key| conf.goconf.key_index(key, level_nr, level_size_groups as u64,
                |_| 0), &mut collision_solver);
            let collisions = collision_solver.to_collision_array();
            let mut best_counts = vec![0u32; level_size_groups].into_boxed_slice();
            kv.for_each_key(|key| {
                let hash = conf.goconf.hash_builder.hash_one(key, level_nr);
                let group = group_nr(hash, level_size_groups as u64);
                let bit_nr = conf.goconf.bits_per_group.bit_index_for_seed(hash, 0, group);
                if collisions.get_bit(bit_nr) { best_counts[group as usize] += 1; }
            });
            let mut best_seeds = conf.goconf.bits_per_seed.new_zeroed_seed_vec(level_size_groups);
            
        }
        todo!()
    }*/
}
