mod conf;
use binout::{AsIs, Serializer, VByte};
pub use conf::MapConf;

use std::hash::Hash;
use bitm::{BitAccess, Rank};

use super::{common::concatenate_values, kvset::{KVSet, SlicesMutSource}};
pub use super::level_sizer::LevelSizer;
use ph::{stats, utils::{read_bits, ArrayWithRank}, BuildDefaultSeededHasher, BuildSeededHasher};
use std::collections::HashMap;
use std::io;

use crate::fp::collision_solver::{CollisionSolver, CollisionSolverBuilder};
use dyn_size_of::GetSize;

/// Finger-printing based static function (immutable map) that maps hashable keys to unsigned integer values of given bit-size.
/// 
/// It usually takes somewhat more than *nb* bits to represent a function from an *n*-element set into a set of *b*-bit values.
/// (Smaller sizes are achieved when the set of values is small and the same values are assigned to multiple keys.)
/// The expected time complexities of its construction and evaluation are *O(n)* and *O(1)*, respectively.
pub struct Map<S = BuildDefaultSeededHasher> {
    array: ArrayWithRank,
    values: Box<[u64]>,    // BitVec
    bits_per_value: u8,
    level_sizes: Box<[usize]>,  // in 64-bit segments
    hash: S
}

impl<S: BuildSeededHasher> GetSize for Map<S> {
    fn size_bytes_dyn(&self) -> usize {
        self.array.size_bytes_dyn()
            + self.values.size_bytes_dyn()
            + self.level_sizes.size_bytes_dyn()
    }

    const USES_DYN_MEM: bool = true;
}

#[inline]
fn index<H: BuildSeededHasher, K: Hash + ?Sized>(hash: &H, k: &K, level_nr: u32, level_size: usize) -> usize {
    ph::utils::map64_to_64(hash.hash_one(k, level_nr), level_size as u64) as usize
}

#[derive(Default)]
struct Arrays {
    level_sizes: Vec::<usize>,
    arrays: Vec::<Box<[u64]>>,
    values_lens: Vec::<usize>,
    values: Vec::<Box<[u64]>>
}

impl Arrays {
    fn into_map<S>(self, hash: S, bits_per_value: u8) -> Map<S> {
        let (array, _)  = ArrayWithRank::build(self.arrays.concat().into_boxed_slice());
        Map::<S> {
            array,
            values: concatenate_values(&self.values, &self.values_lens, bits_per_value),
            bits_per_value,
            level_sizes: self.level_sizes.into_boxed_slice(),
            hash
        }
    }

    fn truncate(&mut self, len: usize) {
        self.arrays.truncate(len);
        self.level_sizes.truncate(len);
        self.values.truncate(len);
        self.values_lens.truncate(len);
    }
}

impl<S: BuildSeededHasher> Map<S> {

    /// Gets the value associated with the given `key` and reports statistics to `access_stats`.
    /// 
    /// If the `key` was not in the input key-value collection given during construction,
    /// either [`None`] or a value assigned to other key is returned.
    pub fn get_stats<K: Hash + ?Sized, A: stats::AccessStatsCollector>(&self, key: &K, access_stats: &mut A) -> Option<u64> {
        let mut array_begin_index = 0usize;
        let mut level = 0u32;
        loop {
            let level_size = *self.level_sizes.get(level as usize)? << 6usize;
            let i = array_begin_index + index(&self.hash, key, level, level_size);
            if self.array.content.get_bit(i) {
                access_stats.found_on_level(level);
                return Some(self.values.get_fragment(self.array.rank(i), self.bits_per_value));
            }
            array_begin_index += level_size;
            level += 1;
        }
    }

    /// Gets the value associated with the given `key`.
    /// 
    /// If the `key` was not in the input key-value collection given during construction,
    /// either [`None`] or a value assigned to other key is returned.
    pub fn get<K: Hash + ?Sized>(&self, key: &K) -> Option<u64> {
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

    /// Pre-builds [`Map`] for given key-value pairs `kv`, using the build configuration `conf` and reporting statistics with `stats`.
    /// After return `kv` contains the pairs which could not be added to the map.
    /// It is empty when construction is completed successfully.
    /// If the construction fails, the result is ready to transform into [`Map`] only if `construct_partial` is `true`.
    /// 
    /// `kv.kv_len()` must be passed as `input_size` and `kv.bits_per_value()` as `bits_per_value`.
    /// 
    /// When the construction fails, it is almost certain that the input contains either duplicate keys
    /// or keys indistinguishable by any hash function from the family used.
    fn _with_conf_stats<K, KV, LSC, CSB, BS>(
        kv: &mut KV,
        conf: &MapConf<LSC, CSB, S>,
        stats: &mut BS,
        bits_per_value: u8,
        construct_partial: bool
    ) -> Arrays
        where K: Hash, KV: KVSet<K>, LSC: LevelSizer, CSB: CollisionSolverBuilder, BS: stats::BuildStatsCollector
    {
        let mut res = Arrays::default();
        let mut input_size = kv.kv_len();
        let mut level_nr = 0u32;
        let mut levels_without_reduction = 0;   // number of levels without any reduction in number of the keys
        while input_size != 0 {
            let level_size_segments = conf.level_sizer.size_segments(kv);
            let level_size = level_size_segments * 64;
            stats.level(input_size, level_size);
            let mut collision_solver: <CSB as CollisionSolverBuilder>::CollisionSolver = conf.collision_solver.new(level_size_segments, bits_per_value);
            kv.process_all_values(|k| index(&conf.hash, k, level_nr, level_size), &mut collision_solver);
            let (current_array, current_values, current_values_len) = collision_solver.to_collision_and_values(bits_per_value);
            kv.retain_keys(|k| !current_array.get_bit(index(&conf.hash, k, level_nr, level_size)));

            let prev_input_size = input_size;
            input_size = kv.kv_len();
            if input_size == prev_input_size {
                if levels_without_reduction == 9 /*+1*/ {
                    if construct_partial { res.truncate(res.arrays.len()-levels_without_reduction); }
                    break;
                }
                levels_without_reduction += 1;
            } else {
                levels_without_reduction = 0;
            }

            res.arrays.push(current_array);
            res.level_sizes.push(level_size_segments);
            res.values.push(current_values);
            res.values_lens.push(current_values_len);
            level_nr += 1;
        }
        stats.end(input_size);
        res
    }

    /// Constructs [`Map`] for given key-value pairs `kv`,
    /// using the build configuration `conf` and reporting statistics with `stats`.
    /// 
    /// Panics if the construction fails.
    /// Then it is almost certain that the input contains either duplicate keys
    /// or keys indistinguishable by any hash function from the family used.
    #[inline]
    pub fn with_conf_stats<K, KV, LSC, CSB, BS>(kv: KV, conf: MapConf<LSC, CSB, S>, stats: &mut BS) -> Self
        where K: Hash, KV: KVSet<K>, LSC: LevelSizer, CSB: CollisionSolverBuilder, BS: stats::BuildStatsCollector
    {
        Self::try_with_conf_stats(kv, conf, stats).expect("Constructing fp::Map failed. Probably the input contains duplicate keys.")
    }

    /// Constructs [`Map`] for given key-value pairs `kv`,
    /// using the build configuration `conf` and reporting statistics with `stats`.
    /// 
    /// [`None`] is returned if the construction fails.
    /// Then it is almost certain that the input contains either duplicate keys
    /// or keys indistinguishable by any hash function from the family used.
    pub fn try_with_conf_stats<K, KV, LSC, CSB, BS>(mut kv: KV, conf: MapConf<LSC, CSB, S>, stats: &mut BS) -> Option<Self>
        where K: Hash, KV: KVSet<K>, LSC: LevelSizer, CSB: CollisionSolverBuilder, BS: stats::BuildStatsCollector
    {
        let bits_per_value = kv.bits_per_value();
        let res = Self::_with_conf_stats(&mut kv, &conf, stats, bits_per_value, false);
        if kv.kv_len() != 0 { return None; }
        drop(kv);
        Some(res.into_map(conf.hash, bits_per_value))
    }

    /// Constructs [`Map`] for given key-value pairs `kv`,
    /// using the build configuration `conf` and reporting statistics with `stats`.
    /// 
    /// If the construction fails, it returns `Err` with a triple *(f, k)*, where:
    /// - *m* is a [`Map`] handling only part of the key-value pairs,
    /// - *k* is a set of the remaining key-value pairs.
    /// If needed, the pairs from *k* can be placed in another data structure to handle all the keys.
    /// 
    /// If the construction fails, it is almost certain that the input contains either duplicate keys
    /// or keys indistinguishable by any hash function from the family used.
    /// The pairs with duplicate keys will be included in the *k* set.
    pub fn try_with_conf_stats_or_partial<K, KV, LSC, CSB, BS>(mut kv: KV, conf: MapConf<LSC, CSB, S>, stats: &mut BS) -> Result<Self, (Self, KV)>
        where K: Hash, KV: KVSet<K>, LSC: LevelSizer, CSB: CollisionSolverBuilder, BS: stats::BuildStatsCollector
    {
        let bits_per_value = kv.bits_per_value();
        let res = Self::_with_conf_stats(&mut kv, &conf, stats, bits_per_value, true);
        if kv.kv_len() != 0 { return Err((res.into_map(conf.hash, kv.bits_per_value()), kv)); }
        drop(kv);
        Ok(res.into_map(conf.hash, bits_per_value))
    }

    /// Build `Map` for given keys -> values map, where:
    /// - keys are given directly,
    /// - values are given as bit vector with bit_per_value.
    /// These arrays must be of the same length.
    fn with_slices_conf_stats<K, LSC, CSB, BS>(
        keys: &mut [K], values: &mut [u8],
        /*&mut [u64],*/ conf: MapConf<LSC, CSB, S>,
        stats: &mut BS
    ) -> Self
        where K: Hash, LSC: LevelSizer, CSB: CollisionSolverBuilder, BS: stats::BuildStatsCollector
    {
        Self::with_conf_stats(SlicesMutSource::new(keys, values, 0), conf, stats)
    }

    #[inline]
    pub fn with_slices_conf<K: Hash, LSC: LevelSizer, CSB: CollisionSolverBuilder>(
        keys: &mut [K], values: &mut [u8], /*&mut [u64],*/ conf: MapConf<LSC, CSB, S>) -> Self
    {
        Self::with_slices_conf_stats(keys, values, conf, &mut ())
    }

    /// Returns number of bytes which write will write.
    pub fn write_bytes(&self) -> usize {
        AsIs::size(self.bits_per_value) +
        VByte::array_size(&self.level_sizes) +
        AsIs::array_content_size(&self.array.content) +
        AsIs::array_content_size(&self.values)
    }

    /// Write `self` to the output.
    pub fn write(&self, output: &mut dyn io::Write) -> io::Result<()>
    {
        AsIs::write(output, self.bits_per_value)?;
        VByte::write_array(output, &self.level_sizes)?;
        AsIs::write_all(output, self.array.content.iter())?;
        AsIs::write_all(output, self.values.iter())
    }

    /// Read `self` from the `input` (`hasher` must be the same as used by written [`Map`]).
    pub fn read_with_hasher(input: &mut dyn io::Read, hasher: S) -> io::Result<Self>
    {
        let bits_per_value = AsIs::read(input)?;
        let level_sizes = VByte::read_array(input)?;
        let array_content = AsIs::read_n(input, level_sizes.iter().map(|v|*v as usize).sum::<usize>())?;
        let (array_with_rank, number_of_ones) = ArrayWithRank::build(array_content);
        let values = read_bits(input, number_of_ones as usize * bits_per_value as usize)?;
        Ok(Self {
            array: array_with_rank,
            values,
            bits_per_value,
            level_sizes,
            hash: hasher
        })
    }
}

impl Map {
    /// Read `self` from the `input`. Only `FPMap`s that use default hasher can be read by this method.
    pub fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Self::read_with_hasher(input, Default::default())
    }
}

impl<S: BuildSeededHasher> Map<S> {

    pub fn with_map_conf<K: Hash + Clone, H, LSC: LevelSizer, CSB: CollisionSolverBuilder, BS: stats::BuildStatsCollector>(
        map: &HashMap<K, u8, H>,
        conf: MapConf<LSC, CSB, S>,
        stats: &mut BS
    ) -> Self
    {
        let mut keys = Vec::<K>::with_capacity(map.len());
        let mut values = Vec::<u8>::with_capacity(map.len());
        for (k, v) in map {
            keys.push(k.clone());
            values.push(*v);
        }
        Self::with_slices_conf_stats(&mut keys, &mut values, conf, stats)
    }
}

impl Map {
    pub fn with_map<K: Hash + Clone, H, BS: stats::BuildStatsCollector>(map: &HashMap<K, u8, H>, stats: &mut BS) -> Self {
        Self::with_map_conf(map, Default::default(), stats)
    }
}

impl<K: Hash + Clone, H> From<&HashMap<K, u8, H>> for Map {
    fn from(map: &HashMap<K, u8, H>) -> Self {
        Self::with_map(map, &mut ())
    }
}

impl<K: Hash + Clone, H> From<HashMap<K, u8, H>> for Map {
    fn from(map: HashMap<K, u8, H>) -> Self {
        Self::with_map(&map, &mut ())
    }
}


#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use bitm::ceiling_div;
    use maplit::hashmap;

    fn test_read_write(fpmap: &Map) {
        let mut buff = Vec::new();
        fpmap.write(&mut buff).unwrap();
        assert_eq!(buff.len(), fpmap.write_bytes());
        let read = Map::read(&mut &buff[..]).unwrap();
        assert_eq!(fpmap.level_sizes, read.level_sizes);
    }

    fn test_fpmap_invariants(fpmap: &Map) {
        assert_eq!(fpmap.level_sizes.iter().map(|v| *v as usize).sum::<usize>(), fpmap.array.content.len());
        assert_eq!(
            ceiling_div(fpmap.array.content.iter().map(|v|v.count_ones()).sum::<u32>() as usize * fpmap.bits_per_value as usize, 64),
            fpmap.values.len()
        );
    }

    fn test_4pairs(conf: MapConf) {
        let fpmap = Map::with_map_conf(&hashmap!('a'=>1u8, 'b'=>2u8, 'c'=>1u8, 'd'=>3u8), conf, &mut ());
        assert_eq!(fpmap.get(&'a'), Some(1));
        assert_eq!(fpmap.get(&'b'), Some(2));
        assert_eq!(fpmap.get(&'c'), Some(1));
        assert_eq!(fpmap.get(&'d'), Some(3));
        test_fpmap_invariants(&fpmap);
        test_read_write(&fpmap);
    }

    #[test]
    fn with_hashmap_4pairs() {
        test_4pairs(MapConf::default());
    }

    fn test_8pairs<LSC: LevelSizer>(conf: MapConf<LSC>) {
        let fpmap = Map::with_map_conf(&hashmap!(
            'a' => 1, 'b' => 2, 'c' => 1, 'd' => 3,
            'e' => 4, 'f' => 1, 'g' => 5, 'h' => 6), conf, &mut ());
        assert_eq!(fpmap.get(&'a'), Some(1));
        assert_eq!(fpmap.get(&'b'), Some(2));
        assert_eq!(fpmap.get(&'c'), Some(1));
        assert_eq!(fpmap.get(&'d'), Some(3));
        assert_eq!(fpmap.get(&'e'), Some(4));
        assert_eq!(fpmap.get(&'f'), Some(1));
        assert_eq!(fpmap.get(&'g'), Some(5));
        assert_eq!(fpmap.get(&'h'), Some(6));
        test_fpmap_invariants(&fpmap);
        test_read_write(&fpmap);
    }

    #[test]
    fn with_hashmap_8pairs() {
        test_8pairs(MapConf::default());
    }

    #[test]
    fn test_fail_partial() {
        let mut k = ['a', 'b', 'a', 'c'];
        let mut v = [1, 2, 2, 3];
        let r = Map::try_with_conf_stats_or_partial(
            SlicesMutSource::new(&mut k, &mut v, 0), MapConf::default(), &mut ());
        assert!(r.is_err());
        if let Err((fpmap, kv)) = r {
            assert_eq!(kv.kv_len(), 2);
            assert_eq!(fpmap.get(&'b'), Some(2));
            assert_eq!(fpmap.get(&'c'), Some(3));
            test_fpmap_invariants(&fpmap);
            test_read_write(&fpmap);
        }
        assert!(Map::try_with_conf_stats(SlicesMutSource::new(&mut k, &mut v, 0), MapConf::default(), &mut ()).is_none());
    }
}