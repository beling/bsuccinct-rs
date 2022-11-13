use binout::{read_int, write_int};
use std::hash::Hash;
use bitm::{BitAccess, BitArrayWithRank, ceiling_div};

use crate::utils::ArrayWithRank;
use crate::{BuildDefaultSeededHasher, BuildSeededHasher, stats, utils};

use crate::read_array;
use std::io;
use std::sync::atomic::{AtomicU64};
use std::sync::atomic::Ordering::Relaxed;
use dyn_size_of::GetSize;

use crate::fp::keyset::{KeySet, SliceMutSource, SliceSourceWithRefsEmptyCleaning};

/// Configuration that is accepted by `FPHash` constructors.
#[derive(Clone)]
pub struct FPHashConf<S = BuildDefaultSeededHasher> {
    hash: S,
    relative_level_size: u16,
    use_multiple_threads: bool
}

impl Default for FPHashConf {
    fn default() -> Self {
        Self {
            hash: Default::default(),
            relative_level_size: 100,
            use_multiple_threads: true
        }
    }
}

impl FPHashConf {
    /// Returns configuration that potentially uses multiple threads to build `FPHash`.
    pub fn threads(use_multiple_threads: bool) -> Self {
        Self { use_multiple_threads, ..Default::default() }
    }

    /// Returns configuration that uses at each level a bit-array of size `relative_level_size`
    /// given as a percent of number of input keys for the level.
    pub fn lsize(relative_level_size: u16) -> Self {
        Self { relative_level_size, ..Default::default() }
    }

    /// Returns configuration that potentially uses multiple threads and
    /// at each level a bit-array of size `relative_level_size`
    /// given as a percent of number of input keys for the level.
    pub fn lsize_threads(relative_level_size: u16, use_multiple_threads: bool) -> Self {
        Self { relative_level_size, use_multiple_threads, ..Default::default() }
    }
}

impl<S> FPHashConf<S> {
    pub fn hash(hash: S) -> Self {
        Self { hash, relative_level_size: 100, use_multiple_threads: true }
    }
    pub fn hash_lsize(hash: S, relative_level_size: u16) -> Self {
        Self { relative_level_size, ..Self::hash(hash) }
    }
    pub fn hash_lsize_threads(hash: S, relative_level_size: u16, use_multiple_threads: bool) -> Self {
        Self { relative_level_size, hash, use_multiple_threads }
    }
}

/// Set `bit_index` bit in `result`. If it already was set, then set it in `collision`.
#[cfg(not(feature = "no_branchless_bb"))]
pub(crate) fn fphash_add_bit(result: &mut [u64], collision: &mut [u64], bit_index: usize) {
    let index = bit_index / 64;
    let mask = 1u64 << (bit_index % 64) as u64;
    collision[index] |= result[index] & mask;
    result[index] |= mask;
    //result[index] |= (!collision[index] & mask);
}

/// Set `bit_index` bit in `result`. If it already was set, then set it in `collision`.
#[cfg(feature = "no_branchless_bb")]
pub(crate) fn fphash_add_bit(result: &mut Box<[u64]>, collision: &mut Box<[u64]>, bit_index: usize) {
    let index = bit_index / 64;
    let mask = 1u64 << (bit_index % 64) as u64;

    if collision[index] & mask == 0 {   // no collision
        if result[index] & mask == 0 {
            result[index] |= mask;
        } else {
            collision[index] |= mask;
            //result[index] &= !mask;
        }
    }
}

/// Set `bit_index` bit in `result`. If it already was set, then set it in `collision`.
pub(crate) fn fphash_sync_add_bit(result: &[AtomicU64], collision: &[AtomicU64], bit_index: usize) {
    let index = bit_index / 64;
    let mask = 1u64 << (bit_index % 64) as u64;
    #[cfg(feature = "no_branchless_bb")] if collision[index].load(Relaxed) & mask != 0 { return; } // TODO opłaca się? bezpieczne? benchmarki pokazują że się opłaca!!
    let old_result = result[index].fetch_or(mask, Relaxed);
    if old_result & mask != 0 { collision[index].fetch_or(mask, Relaxed); }
    //collision[index].fetch_or(old_result & mask, Relaxed);    // alternative to line above
}

/// Remove from bit-array `result` all bits that are set in `collision`.
pub(crate) fn fphash_remove_collided(result: &mut Box<[u64]>, collision: &[u64]) {
    for (r, c) in result.iter_mut().zip(collision.iter()) {
        *r &= !c;
    }
}

// Remove from bit-array `result` all bits that are set in `collision`. Uses multiple threads.
/*pub(crate) fn fphash_par_remove_collided(result: &mut Box<[u64]>, collision: &[u64]) {
    result.par_iter_mut().zip(collision.par_iter()).for_each(|(r, c)| {
        *r &= !c;
    });
}*/ // works, bot difference is negligible

/// Returns the index of `key` at level with given `seed` and size, using given (seeded) `hash` method.
#[inline(always)] fn index(key: &impl Hash, hash: &impl BuildSeededHasher, seed: u32, level_size: usize) -> usize {
    utils::map64_to_64(hash.hash_one(key, seed), level_size as u64) as usize
}

/// Helper structure for building fingerprinting-based minimal perfect hash function (FMPH).
struct FPHashBuilder<S> {
    arrays: Vec::<Box<[u64]>>,
    input_size: usize,
    use_multiple_threads: bool,
    conf: FPHashConf<S>

}

impl<S: BuildSeededHasher + Sync> FPHashBuilder<S> {
    pub fn new<K>(conf: FPHashConf<S>, keys: &impl KeySet<K>) -> Self {
        Self {
            arrays: Vec::<Box<[u64]>>::new(),
            input_size: keys.keys_len(),
            use_multiple_threads: conf.use_multiple_threads && (keys.has_par_for_each_key() || keys.has_par_retain_keys()) && rayon::current_num_threads() > 1,
            conf
        }
    }

    /// Returns whether `key` is retained (`false` if it is already hashed at the levels built so far).
    pub fn retained<K>(&self, key: &K) -> bool where K: Hash {
        self.arrays.iter().enumerate()
            .all(|(seed, a)| !a.get_bit(index(key, &self.conf.hash, seed as u32, a.len() << 6)))
    }

    /// Builds level using a single thread.
    fn build_level_st<K>(&self, keys: &impl KeySet<K>, level_size_segments: u32, seed: u32) -> Box<[u64]>
        where K: Hash
    {
        let level_size_segments = level_size_segments as usize;
        let mut result = vec![0u64; level_size_segments].into_boxed_slice();
        let mut collision = vec![0u64; level_size_segments].into_boxed_slice();
        let level_size = level_size_segments * 64;
        keys.for_each_key(
            |key| fphash_add_bit(&mut result, &mut collision, index(key, &self.conf.hash, seed, level_size)),
            |key| self.retained(key)
        );
        fphash_remove_collided(&mut result, &collision);
        result
    }

    /// Builds level using multiple threads.
    fn build_level_mt<K>(&self, keys: &impl KeySet<K>, level_size_segments: u32, seed: u32) -> Box<[u64]>
        where K: Hash + Sync
    {
        if !keys.has_par_for_each_key() {
            return self.build_level_st(keys, level_size_segments, seed);
        }
        let level_size_segments = level_size_segments as usize;
        let mut result = vec![0u64; level_size_segments].into_boxed_slice();
        let result_atom = AtomicU64::from_mut_slice(&mut result);
        let mut collision: Box<[AtomicU64]> = (0..level_size_segments).map(|_| AtomicU64::default()).collect();
        let level_size = level_size_segments * 64;
        keys.par_for_each_key(
            |key| fphash_sync_add_bit(&&result_atom, &collision, index(key, &self.conf.hash, seed, level_size)),
            |key| self.retained(key)
        );
        fphash_remove_collided(&mut result, AtomicU64::get_mut_slice(&mut collision));
        result
    }

    /// Returns number the level about to build (number of levels built so far).
    #[inline(always)] fn level_nr(&self) -> u32 { self.arrays.len() as u32 }

    fn build_levels<K, BS>(&mut self, keys: &mut impl KeySet<K>, stats: &mut BS)
        where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        while self.input_size != 0 {
            let level_size_segments = ceiling_div(self.input_size * self.conf.relative_level_size as usize, 64*100) as u32;
            let level_size = level_size_segments as usize * 64;
            stats.level(self.input_size, level_size);
            let seed = self.level_nr();
            self.arrays.push(if self.use_multiple_threads {
                let current_array = self.build_level_mt(keys, level_size_segments, seed);
                keys.par_retain_keys(
                    |k| !current_array.get_bit(utils::map64_to_64(self.conf.hash.hash_one(&k, seed), level_size as u64) as usize),
                    |k| self.retained(k),
                    || self.input_size - current_array.iter().map(|v| v.count_ones() as usize).sum::<usize>()
                );
                current_array
            } else {
                let current_array = self.build_level_st(keys, level_size_segments, seed);
                keys.retain_keys(
                    |k| !current_array.get_bit(utils::map64_to_64(self.conf.hash.hash_one(&k, seed), level_size as u64) as usize),
                    |k| self.retained(k),
                    || self.input_size - current_array.iter().map(|v| v.count_ones() as usize).sum::<usize>()
                );
                current_array
            });
            self.input_size = keys.keys_len();
        }
    }

    /*fn fphash_build_level_MT2<K, S>(hash: &S, keys: &[K], level_size_segments: u32, seed: u32, thread_pool: &Option<ThreadPool>) -> Box<[u64]>
        where S: BuildSeededHasher + Sync, K: Hash + Sync
    {
        if let Some(thread_pool) = thread_pool {
            let level_size_segments = level_size_segments as usize;
            let level_size = level_size_segments * 64;
            let (mut result, collisions) = thread_pool.install(|| {
                keys.into_par_iter()
                //keys.par_chunks(100)
                    .fold(|| (vec![0u64; level_size_segments].into_boxed_slice(),
                         vec![0u64; level_size_segments].into_boxed_slice()),
                    |(mut a, mut c), k| {
                        //for k in keys { // for par_chunks
                            let bit = utils::map64_to_64(hash.hash_one(k, seed), level_size as u64) as usize;
                            fphash_add_bit(&mut a, &mut c, bit);
                        //}   //  for par_chunks
                        (a, c)
                    }
                ).reduce_with(|(mut arr1, mut col1), (arr2, col2)| {
                    for (((a1, c1), a2), c2) in arr1.iter_mut().zip(col1.iter_mut()).zip(arr2.into_iter()).zip(col2.into_iter()) {
                        *c1 |= *c2 | (*a1 & a2);
                        *a1 |= a2;
                    };
                    (arr1, col1)
                }).unwrap()
            });
            fphash_remove_collided(&mut result, &collisions);
            result
        } else {
            fphash_build_level(hash, keys, level_size_segments, seed)
        }
    }*/
}


/// Fingerprinting-based minimal perfect hash function (FMPH).
///
/// See:
/// - A. Limasset, G. Rizk, R. Chikhi, P. Peterlongo, *Fast and Scalable Minimal Perfect Hashing for Massive Key Sets*, SEA 2017
/// - P. Beling, *Fingerprinting-based minimal perfect hashing revisited*
#[derive(Clone)]
pub struct FPHash<S = BuildDefaultSeededHasher> {
    array: ArrayWithRank,
    level_sizes: Box<[u32]>,
    hash_builder: S,
}

impl<S: BuildSeededHasher> GetSize for FPHash<S> {
    fn size_bytes_dyn(&self) -> usize { self.array.size_bytes_dyn() + self.level_sizes.size_bytes_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl<S: BuildSeededHasher + Sync> FPHash<S> {

    #[inline(always)] fn index<K: Hash>(&self, k: &K, level_nr: u32, size: usize) -> usize {
        //utils::map64_to_32(self.hash_builder.hash_one(k, level_nr), size as u32) as usize
        utils::map64_to_64(self.hash_builder.hash_one(k, level_nr), size as u64) as usize
    }

    /// Gets the value associated with the given `key` and reports statistics to `access_stats`.
    pub fn get_stats<K: Hash, A: stats::AccessStatsCollector>(&self, key: &K, access_stats: &mut A) -> Option<u64> {
        let mut array_begin_index = 0usize;
        let mut level_nr = 0u32;
        loop {
            let level_size = (*self.level_sizes.get(level_nr as usize)? as usize) << 6;
            let i = array_begin_index + self.index(key, level_nr, level_size);
            if self.array.content.get_bit(i) {
                access_stats.found_on_level(level_nr);
                return Some(self.array.rank(i) as u64);
            }
            array_begin_index += level_size;
            level_nr += 1;
        }
    }

    /// Gets the value associated with the given `key`.
    pub fn get<K: Hash>(&self, key: &K) -> Option<u64> {
        self.get_stats(key, &mut ())
    }

    /// Builds `FPHash` for given `keys`, using the configuration `conf` and reporting statistics to `stats`.
    pub fn with_conf_stats<K, BS>(mut keys: impl KeySet<K>, conf: FPHashConf<S>, stats: &mut BS) -> Self
        where K: Hash + Sync,
              BS: stats::BuildStatsCollector
    {
        let mut builder = FPHashBuilder::new(conf, &keys);
        builder.build_levels(&mut keys, stats);
        drop(keys);
        stats.end();
        let level_sizes = builder.arrays.iter().map(|l| l.len() as u32).collect();
        let (array, _)  = ArrayWithRank::build(builder.arrays.concat().into_boxed_slice());
        Self {
            array,
            level_sizes,
            hash_builder: builder.conf.hash
        }
    }

    /// Builds `FPHash` for given `keys`, using the configuration `conf`.
    #[inline] pub fn with_conf<K>(keys: impl KeySet<K>, conf: FPHashConf<S>) -> Self
        where K: Hash + Sync
    {
        Self::with_conf_stats(keys, conf, &mut ())
    }

    /// Builds `FPHash` for given `keys`, using the configuration `conf`.
    #[inline] pub fn from_slice_with_conf_stats<K, BS>(keys: &[K], conf: FPHashConf<S>, stats: &mut BS) -> Self
        where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        Self::with_conf_stats(SliceSourceWithRefsEmptyCleaning::<_, u16>::new(keys), conf, stats)
    }

    /// Builds `FPHash` for given `keys`, using the configuration `conf`.
    #[inline] pub fn from_slice_with_conf<K>(keys: &[K], conf: FPHashConf<S>) -> Self
        where K: Hash + Sync
    {
        Self::with_conf_stats(SliceSourceWithRefsEmptyCleaning::<_, u16>::new(keys), conf, &mut ())
    }

    /// Builds `FPHash` for given `keys`, using the configuration `conf`.
    #[inline] pub fn from_slice_mut_with_conf_stats<K, BS>(keys: &mut [K], conf: FPHashConf<S>, stats: &mut BS) -> Self
        where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        Self::with_conf_stats(SliceMutSource::new(keys), conf, stats)
    }

    /// Builds `FPHash` for given `keys`, using the configuration `conf`.
    #[inline] pub fn from_slice_mut_with_conf<K>(keys: &mut [K], conf: FPHashConf<S>) -> Self
        where K: Hash + Sync
    {
        Self::with_conf_stats(SliceMutSource::new(keys), conf, &mut ())
    }

    /// Returns number of bytes which `write` will write.
    pub fn write_bytes(&self) -> usize {
            std::mem::size_of::<u32>()
                + self.level_sizes.len() * std::mem::size_of::<u32>()
                + self.array.content.size_bytes_dyn()
    }

    /// Writes `self` to the `output`.
    pub fn write(&self, output: &mut dyn io::Write) -> io::Result<()>
    {
        write_int!(output, self.level_sizes.len() as u32)?;
        self.level_sizes.iter().try_for_each(|s| write_int!(output, *s))?;
        self.array.content.iter().try_for_each(|v| write_int!(output, v))
    }

    /// Reads `Self` from the `input`. Hasher must be the same as the one used to write.
    pub fn read_with_hasher(input: &mut dyn io::Read, hasher: S) -> io::Result<Self>
    {
        let levels = read_array!([u32; read u32] from input);
        let array_content_len = levels.iter().map(|v|*v as usize).sum::<usize>();
        let array_content = read_array!([u64; array_content_len] from input).into_boxed_slice();
        let (array_with_rank, _) = ArrayWithRank::build(array_content);
        Ok(Self { array: array_with_rank, level_sizes: levels.into_boxed_slice(), hash_builder: hasher })
    }
}

impl FPHash {
    /// Reads `Self` from the `input`.
    /// Only `FPHash`s that use default hasher can be read by this method.
    pub fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Self::read_with_hasher(input, Default::default())
    }

    /// Builds `FPHash` for given `keys`, reporting statistics to `stats`.
    pub fn with_stats<K, BS>(keys: impl KeySet<K>, stats: &mut BS) -> Self
        where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        Self::with_conf_stats(keys, Default::default(), stats)
    }

    /// Builds `FPHash` for given `keys`.
    pub fn new<K: Hash + Sync>(keys: impl KeySet<K>) -> Self {
        Self::with_conf_stats(keys, Default::default(), &mut ())
    }
}

impl<K: Hash + Clone + Sync> From<&[K]> for FPHash {
    fn from(keys: &[K]) -> Self {
        Self::new(SliceSourceWithRefsEmptyCleaning::<_, u16>::new(keys))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Display;
    use crate::utils::test_mphf;

    fn test_read_write(h: &FPHash) {
        let mut buff = Vec::new();
        h.write(&mut buff).unwrap();
        assert_eq!(buff.len(), h.write_bytes());
        let read = FPHash::read(&mut &buff[..]).unwrap();
        assert_eq!(h.level_sizes.len(), read.level_sizes.len());
        assert_eq!(h.array.content, read.array.content);
    }

    fn test_with_input<K: Hash + Clone + Display + Sync>(to_hash: &[K]) {
        let h = FPHash::from_slice_with_conf(to_hash, FPHashConf::threads(1));
        test_mphf(to_hash, |key| h.get(key).map(|i| i as usize));
        test_read_write(&h);
    }

    #[test]
    fn test_small() {
        test_with_input(&[1, 2, 5]);
        test_with_input(&(-50..150).collect::<Vec<_>>());
        test_with_input(&['a', 'b', 'c', 'd']);
    }
}