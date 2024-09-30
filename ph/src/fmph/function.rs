use std::hash::Hash;
use binout::{AsIs, Serializer, VByte};
use bitm::{BitAccess, Rank, ceiling_div};

use crate::utils::ArrayWithRank;
use crate::{BuildDefaultSeededHasher, BuildSeededHasher, stats, utils};

use std::io;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;
use rayon::prelude::*;
use dyn_size_of::GetSize;

use crate::fmph::keyset::{KeySet, SliceMutSource, SliceSourceWithRefs};

/// Build configuration that is accepted by [`Function`] constructors.
/// 
/// See field descriptions for details.
#[derive(Clone)]
pub struct BuildConf<S = BuildDefaultSeededHasher> {
    /// The family of hash functions used by the constructed FMPH. (default: [`BuildDefaultSeededHasher`])
    pub hash_builder: S,

    /// The threshold for the number of keys below which their hashes will be cached during level construction.
    /// (default: [`BuildConf::DEFAULT_CACHE_THRESHOLD`])
    /// 
    /// Caching speeds up level construction at the expense of memory consumption during construction
    /// (caching a single key requires 8 bytes of memory).
    /// Caching is particularly recommended for keys with complex types whose hashing is slow.
    /// It is possible to use a value of `0` to disable caching completely, or [`usize::MAX`] to use it on all levels.
    pub cache_threshold: usize,

    /// Size of each level given as a percentage of the number of level input keys. (default: `100`)
    /// 
    /// A value of 100 minimizes the size of the constructed minimum perfect hash function.
    /// Larger values speed up evaluation at the expense of increased size.
    /// For example, the values 100 and 200 lead to the sizes of approximately 2.8 and 3.4 bits per input key, respectively.
    /// It does not make sense to use values below 100.
    pub relative_level_size: u16,

    /// Whether to use multiple threads during construction. (default: `true`)
    /// 
    /// If `true`, the construction will be performed using the default [rayon] thread pool.
    pub use_multiple_threads: bool
}

impl Default for BuildConf {
    fn default() -> Self {
        Self {
            hash_builder: Default::default(),
            cache_threshold: Self::DEFAULT_CACHE_THRESHOLD,
            relative_level_size: 100,
            use_multiple_threads: true
        }
    }
}

impl BuildConf {
    /// Returns configuration that potentially uses [multiple threads](BuildConf::use_multiple_threads) to build [Function].
    pub fn mt(use_multiple_threads: bool) -> Self {
        Self { use_multiple_threads, ..Default::default() }
    }

    /// Returns configuration that uses custom [`cache_threshold`](BuildConf::cache_threshold) to build [Function].
    pub fn ct(cache_threshold: usize) -> Self {
        Self { cache_threshold, ..Default::default() }
    }

    /// Returns configuration that uses custom [`cache_threshold`](BuildConf::cache_threshold) and
    /// potentially uses [multiple threads](BuildConf::use_multiple_threads) to build [Function].
    pub fn ct_mt(cache_threshold: usize, use_multiple_threads: bool) -> Self {
        Self { use_multiple_threads, cache_threshold, ..Default::default() }
    }

    /// Returns configuration that uses at each level a bit-array
    /// of size [`relative_level_size`](BuildConf::relative_level_size)
    /// given as a percent of number of level input keys.
    pub fn lsize(relative_level_size: u16) -> Self {
        Self { relative_level_size, ..Default::default() }
    }

    /// Returns configuration that uses custom [`relative_level_size`](BuildConf::relative_level_size)
    /// and [`cache_threshold`](BuildConf::cache_threshold) to build [Function].
    pub fn lsize_ct(relative_level_size: u16, cache_threshold: usize) -> Self {
        Self { relative_level_size, cache_threshold, ..Default::default() }
    }

    /// Returns configuration that potentially uses [multiple threads](BuildConf::use_multiple_threads) and
    /// at each level a bit-array of size [`relative_level_size`](BuildConf::relative_level_size)
    /// given as a percent of number of level input keys.
    pub fn lsize_mt(relative_level_size: u16, use_multiple_threads: bool) -> Self {
        Self { relative_level_size, use_multiple_threads, ..Default::default() }
    }
}

impl<S> BuildConf<S> {
    /// The default value for [`relative_level_size`](BuildConf::relative_level_size),
    /// which results in building the cache with a maximum size of 1GB.
    pub const DEFAULT_CACHE_THRESHOLD: usize = 1024*1024*128; // *8 bytes = 1GB

    /// Returns configuration that uses custom [`hash_builder`](BuildConf::hash_builder).
    pub fn hash(hash_builder: S) -> Self {
        Self { hash_builder, cache_threshold: Self::DEFAULT_CACHE_THRESHOLD, relative_level_size: 100, use_multiple_threads: true }
    }

    /// Returns configuration that uses custom [`hash_builder`](BuildConf::hash_builder) and [`relative_level_size`](BuildConf::relative_level_size).
    pub fn hash_lsize(hash_builder: S, relative_level_size: u16) -> Self {
        Self { relative_level_size, ..Self::hash(hash_builder) }
    }

    /// Returns configuration that uses custom [`hash_builder`](BuildConf::hash_builder), [`relative_level_size`](BuildConf::relative_level_size)
    /// and potentially uses [multiple threads](BuildConf::use_multiple_threads) to build [Function].
    pub fn hash_lsize_mt(hash_builder: S, relative_level_size: u16, use_multiple_threads: bool) -> Self {
        Self { relative_level_size, hash_builder, use_multiple_threads, cache_threshold: Self::DEFAULT_CACHE_THRESHOLD }
    }

    /// Returns configuration that uses custom [`hash_builder`](BuildConf::hash_builder),
    /// [`relative_level_size`](BuildConf::relative_level_size) and [`cache_threshold`](BuildConf::cache_threshold)
    /// to build [Function].
    pub fn hash_lsize_ct(hash_builder: S, relative_level_size: u16, cache_threshold: usize) -> Self {
        Self { relative_level_size, hash_builder, use_multiple_threads: true, cache_threshold }
    }

    /// Returns configuration that uses custom [`hash_builder`](BuildConf::hash_builder),
    /// [`relative_level_size`](BuildConf::relative_level_size), [`cache_threshold`](BuildConf::cache_threshold)
    /// and potentially uses [multiple threads](BuildConf::use_multiple_threads) to build [Function].
    pub fn hash_lsize_ct_mt(hash_builder: S, relative_level_size: u16, cache_threshold: usize, use_multiple_threads: bool) -> Self {
        Self { relative_level_size, hash_builder, use_multiple_threads, cache_threshold }
    }
}

/// Set `bit_index` bit in `result`. If it already was set, then set it in `collision`.
#[cfg(not(feature = "no_branchless_bb"))]
#[inline]
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
#[inline]
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

pub(crate) fn concat(arrays: &mut [Box<[AtomicU64]>]) -> Vec<u64> {
    let result_len = arrays.iter().map(|a| a.len()).sum();
    let mut result = Vec::with_capacity(result_len);
    for a in arrays { result.extend_from_slice(get_mut_slice(a)) }
    result
}

/// Cast `v` to slice of `AtomicU64`.
#[inline]
pub(crate) fn from_mut_slice(v: &mut [u64]) -> &mut [AtomicU64] {
    use core::mem::align_of;
    let [] = [(); align_of::<AtomicU64>() - align_of::<u64>()];
    unsafe { &mut *(v as *mut [u64] as *mut [AtomicU64]) }
}   // copied from unstable rust, from_mut_slice, commit #94816, issue #76314

/// Cast `v` to slice of `u64`.
#[inline]
pub(crate) fn get_mut_slice(v: &mut [AtomicU64]) -> &mut [u64] {
    // SAFETY: the mutable reference guarantees unique ownership.
    unsafe { &mut *(v as *mut [AtomicU64] as *mut [u64]) }
}   // copied from unstable rust, get_mut_slice, commit #94816, issue #76314

// Remove from bit-array `result` all bits that are set in `collision`. Uses multiple threads.
/*pub(crate) fn fphash_par_remove_collided(result: &mut Box<[u64]>, collision: &[u64]) {
    result.par_iter_mut().zip(collision.par_iter()).for_each(|(r, c)| {
        *r &= !c;
    });
}*/ // works, bot difference is negligible

/// Returns the index of `key` at level with given `seed` and size (`level_size`), using given (seeded) `hash` method.
#[inline(always)] fn index(key: &impl Hash, hash: &impl BuildSeededHasher, seed: u32, level_size: usize) -> usize {
    utils::map64_to_64(hash.hash_one(key, seed), level_size as u64) as usize
}

/// Helper structure for building fingerprinting-based minimal perfect hash function (FMPH).
struct Builder<S> {
    arrays: Vec::<Box<[u64]>>,
    input_size: usize,
    conf: BuildConf<S>
}

impl<S: BuildSeededHasher + Sync> Builder<S> {
    pub fn new<K>(mut conf: BuildConf<S>, keys: &impl KeySet<K>) -> Self {
        if conf.use_multiple_threads { conf.use_multiple_threads = rayon::current_num_threads() > 1; }
        Self {
            arrays: Vec::<Box<[u64]>>::new(),
            input_size: keys.keys_len(),
            conf
        }
    }

    /// Returns whether `key` is retained (`false` if it is already hashed at the levels built so far).
    pub fn retained<K>(&self, key: &K) -> bool where K: Hash {
        self.arrays.iter().enumerate()
            .all(|(seed, a)| !a.get_bit(index(key, &self.conf.hash_builder, seed as u32, a.len() << 6)))
    }

    /// Build level for given sequence of key indices using a single thread
    fn build_array_for_indices_st(&self, bit_indices: &[usize], level_size_segments: usize) -> Box<[u64]>
    {
        let mut result = vec![0u64; level_size_segments].into_boxed_slice();
        let mut collision = vec![0u64; level_size_segments].into_boxed_slice();
        for bit_index in bit_indices {
            fphash_add_bit(&mut result, &mut collision, *bit_index);
        };
        fphash_remove_collided(&mut result, &collision);
        result
    }

    /// Build level for given sequence of key indices possibly using multiple threads.
    fn build_array_for_indices(&self, bit_indices: &[usize], level_size_segments: usize) -> Box<[u64]>
    {
        if !self.conf.use_multiple_threads {
            return self.build_array_for_indices_st(bit_indices, level_size_segments)
        }
        let mut result = vec![0u64; level_size_segments].into_boxed_slice();
        let result_atom = from_mut_slice(&mut result);
        let mut collision: Box<[AtomicU64]> = (0..level_size_segments).map(|_| AtomicU64::default()).collect();
        bit_indices.par_iter().for_each(
            |bit_index| fphash_sync_add_bit(&result_atom, &collision, *bit_index)
        );
        fphash_remove_collided(&mut result, get_mut_slice(&mut collision));
        result
    }

    /// Builds level using a single thread.
    fn build_level_st<K>(&self, keys: &impl KeySet<K>, level_size_segments: usize, seed: u32) -> Box<[u64]>
        where K: Hash
    {
        let mut result = vec![0u64; level_size_segments].into_boxed_slice();
        let mut collision = vec![0u64; level_size_segments].into_boxed_slice();
        let level_size = level_size_segments * 64;
        keys.for_each_key(
            |key| fphash_add_bit(&mut result, &mut collision, index(key, &self.conf.hash_builder, seed, level_size)),
            |key| self.retained(key)
        );
        fphash_remove_collided(&mut result, &collision);
        result
    }

    /// Builds level possibly (if `keys` can be iterated in parallel) using multiple threads
    fn build_level<K>(&self, keys: &impl KeySet<K>, level_size_segments: usize, seed: u32) -> Box<[u64]>
        where K: Hash + Sync
    {
        if !(self.conf.use_multiple_threads && keys.has_par_for_each_key()) {
            return self.build_level_st(keys, level_size_segments, seed);
        }
        let mut result = vec![0u64; level_size_segments].into_boxed_slice();
        let result_atom = from_mut_slice(&mut result);
        let mut collision: Box<[AtomicU64]> = (0..level_size_segments).map(|_| AtomicU64::default()).collect();
        let level_size = level_size_segments * 64;
        keys.par_for_each_key(
            |key| fphash_sync_add_bit(&&result_atom, &collision, index(key, &self.conf.hash_builder, seed, level_size)),
            |key| self.retained(key)
        );
        fphash_remove_collided(&mut result, get_mut_slice(&mut collision));
        result
    }

    /// Returns number of the level about to build (number of levels built so far).
    #[inline(always)] fn level_nr(&self) -> u32 { self.arrays.len() as u32 }

    fn build_levels<K, BS>(&mut self, keys: &mut impl KeySet<K>, stats: &mut BS)
        where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        let mut levels_without_reduction = 0;   // number of levels without any reduction in number of the keys
        while self.input_size != 0 {
            let level_size_segments = ceiling_div(self.input_size * self.conf.relative_level_size as usize, 64*100);
            let level_size = level_size_segments * 64;
            stats.level(self.input_size, level_size);
            let seed = self.level_nr();
            let array = if self.input_size < self.conf.cache_threshold {
                // build level with hash caching:
                let bit_indices = keys.maybe_par_map_each_key(
                    |key| index(key, &self.conf.hash_builder, seed, level_size),
                    |key| self.retained(key),
                    self.conf.use_multiple_threads
                );
                let array = self.build_array_for_indices(&bit_indices, level_size_segments);
                keys.maybe_par_retain_keys_with_indices(
                    |i| !array.get_bit(bit_indices[i]),
                    |key| !array.get_bit(index(key, &self.conf.hash_builder, seed, level_size)),
                    |key| self.retained(key),
                    || array.count_bit_ones(),
                    self.conf.use_multiple_threads
                );
                array
            } else {
                // build level without hash caching:
                let current_array = self.build_level(keys, level_size_segments, seed);
                keys.maybe_par_retain_keys(
                    |key| !current_array.get_bit(index(key, &self.conf.hash_builder, seed, level_size)),
                    |key| self.retained(key),
                    || current_array.count_bit_ones(),
                    self.conf.use_multiple_threads               
                );
                current_array
            };
            self.arrays.push(array);
            let prev_input_size = self.input_size;
            self.input_size = keys.keys_len();
            if self.input_size == prev_input_size {
                levels_without_reduction += 1;
                if levels_without_reduction == 10 {
                    self.arrays.truncate(self.arrays.len()-levels_without_reduction);
                    stats.end(self.input_size);
                    break;
                }
            } else {
                levels_without_reduction = 0;
            }
        }
        stats.end(0)
    }

    pub fn finish(self) -> Function<S> {
        let level_sizes = self.arrays.iter().map(|l| l.len() as u64).collect();
        let (array, _)  = ArrayWithRank::build(self.arrays.concat().into_boxed_slice());
        Function::<S> {
            array,
            level_sizes,
            hash_builder: self.conf.hash_builder
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
/// - P. Beling, *Fingerprinting-based minimal perfect hashing revisited*, ACM Journal of Experimental Algorithmics, 2023, <https://doi.org/10.1145/3596453>
/// - A. Limasset, G. Rizk, R. Chikhi, P. Peterlongo, *Fast and Scalable Minimal Perfect Hashing for Massive Key Sets*, SEA 2017
#[derive(Clone)]
pub struct Function<S = BuildDefaultSeededHasher> {
    array: ArrayWithRank,
    level_sizes: Box<[u64]>,
    hash_builder: S,
}

impl<S: BuildSeededHasher> GetSize for Function<S> {
    fn size_bytes_dyn(&self) -> usize { self.array.size_bytes_dyn() + self.level_sizes.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.array.size_bytes_content_dyn() + self.level_sizes.size_bytes_content_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl<S: BuildSeededHasher> Function<S> {

    /// Returns index of the key `k` at the level of the given number (`level_nr`) and `size`.
    #[inline(always)] fn index<K: Hash + ?Sized>(&self, k: &K, level_nr: u32, size: usize) -> usize {
        //utils::map64_to_32(self.hash_builder.hash_one(k, level_nr), size as u32) as usize
        utils::map64_to_64(self.hash_builder.hash_one(k, level_nr), size as u64) as usize
    }

    /// Gets the value associated with the given `key` and reports statistics to `access_stats`.
    /// 
    /// The returned value is in the range from `0` (inclusive) to the number of elements in the input key collection (exclusive).
    /// If the `key` was not in the input key collection given during construction,
    /// either [`None`] or an undetermined value from the specified range is returned.
    pub fn get_stats<K: Hash + ?Sized, A: stats::AccessStatsCollector>(&self, key: &K, access_stats: &mut A) -> Option<u64> {
        let mut array_begin_index = 0usize;
        let mut level_nr = 0u32;
        loop {
            let level_size = (*self.level_sizes.get(level_nr as usize)? as usize) << 6;
            let i = array_begin_index + self.index(key, level_nr, level_size);
            if self.array.content.get_bit(i) {
                access_stats.found_on_level(level_nr);
                return Some(unsafe{self.array.rank_unchecked(i)} as u64);
            }
            array_begin_index += level_size;
            level_nr += 1;
        }
    }

    /// Gets the value associated with the given `key`.
    /// 
    /// The returned value is in the range: `0` (inclusive), the number of elements in the input key collection (exclusive).
    /// If the `key` was not in the input key collection given during construction,
    /// either [`None`] or an undetermined value from the specified range is returned.
    #[inline] pub fn get<K: Hash + ?Sized>(&self, key: &K) -> Option<u64> {
        self.get_stats(key, &mut ())
    }

    /// Gets the value associated with the given `key` and reports statistics to `access_stats`.
    /// 
    /// The returned value is in the range: `0` (inclusive), the number of elements in the input key collection (exclusive).
    /// If the `key` was not in the input key collection given during construction,
    /// it either panics or returns an undetermined value from the specified range.
    #[inline] pub fn get_stats_or_panic<K: Hash + ?Sized, A: stats::AccessStatsCollector>(&self, key: &K, access_stats: &mut A) -> u64 {
        self.get_stats(key, access_stats).expect("Invalid access to an item outside the set given during construction.")
    }

    /// Gets the value associated with the given `key` and reports statistics to `access_stats`.
    /// 
    /// The returned value is in the range: `0` (inclusive), the number of elements in the input key collection (exclusive).
    /// If the `key` was not in the input key collection given during construction,
    /// it either panics or returns an undetermined value from the specified range.
    #[inline] pub fn get_or_panic<K: Hash + ?Sized>(&self, key: &K) -> u64 {
        self.get_stats_or_panic(key, &mut ())
    }

    /// Returns number of bytes which `write` will write.
    pub fn write_bytes(&self) -> usize {
        VByte::array_size(&self.level_sizes) + AsIs::array_content_size(&self.array.content)
    }

    /// Writes `self` to the `output`.
    pub fn write(&self, output: &mut dyn io::Write) -> io::Result<()>
    {
        VByte::write_array(output, &self.level_sizes)?;
        AsIs::write_all(output, self.array.content.iter())
    }

    /// Reads `Self` from the `input`. Hasher must be the same as the one used to write.
    pub fn read_with_hasher(input: &mut dyn io::Read, hasher: S) -> io::Result<Self>
    {
        let level_sizes = VByte::read_array(input)?;
        let array_content_len = level_sizes.iter().map(|v|*v as usize).sum::<usize>();
        let array_content = AsIs::read_n(input, array_content_len)?;
        let (array_with_rank, _) = ArrayWithRank::build(array_content);
        Ok(Self { array: array_with_rank, level_sizes, hash_builder: hasher })
    }

    /// Returns sizes of the successive levels.
    pub fn level_sizes(&self) -> &[u64] {
        &self.level_sizes
    }
}

impl<S: BuildSeededHasher + Sync> Function<S> {
    /// Constructs [`Function`] for given input `keys`,
    /// using the build configuration `conf` and reporting statistics with `stats`.
    /// 
    /// If the construction fails, it returns `Err` with a triple *(f, k, s)*, where:
    /// - *f* is a [`Function`] handling only part of the keys
    ///   (that returns numbers in the interval *[0, s-k.keys_len())*);
    /// - *k* is a set of the remaining keys,
    /// - *s* is the initial number of keys.
    /// If needed, the keys from *k* can be placed in another data structure to handle all the keys.
    /// 
    /// If the construction fails, it is almost certain that the input contains either duplicate keys
    /// or keys indistinguishable by any hash function from the family used.
    /// The duplicate keys will be included in the *k* set.
    pub fn try_with_conf_stats_or_partial<K, BS, KS>(mut keys: KS, conf: BuildConf<S>, stats: &mut BS) -> Result<Self, (Self, KS, usize)>
        where K: Hash + Sync, KS: KeySet<K>, BS: stats::BuildStatsCollector
    {
        let mut builder = Builder::new(conf, &keys);
        let initial_size = builder.input_size;
        builder.build_levels(&mut keys, stats);
        if builder.input_size == 0 {
            drop(keys);
            Ok(builder.finish())
        } else {
            Err((builder.finish(), keys, initial_size))
        }
    }

    /// Constructs [`Function`] for given `keys`, using the build configuration `conf` and reporting statistics with `stats`.
    /// 
    /// [`None`] is returned if the construction fails.
    /// Then it is almost certain that the input contains either duplicate keys
    /// or keys indistinguishable by any hash function from the family used.
    pub fn try_with_conf_stats<K, BS, KS>(mut keys: KS, conf: BuildConf<S>, stats: &mut BS) -> Option<Self>
        where K: Hash + Sync, KS: KeySet<K>, BS: stats::BuildStatsCollector
    {
        let mut builder = Builder::new(conf, &keys);
        builder.build_levels(&mut keys, stats);
        drop(keys);
        (builder.input_size == 0).then(|| builder.finish())
    }

    /// Constructs [`Function`] for given `keys`, using the build configuration `conf` and reporting statistics with `stats`.
    /// 
    /// Panics if the construction fails.
    /// Then it is almost certain that the input contains either duplicate keys
    /// or keys indistinguishable by any hash function from the family used.
    pub fn with_conf_stats<K, BS>(keys: impl KeySet<K>, conf: BuildConf<S>, stats: &mut BS) -> Self
    where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        Self::try_with_conf_stats(keys, conf, stats).expect("Constructing fmph::Function failed. Probably the input contains duplicate keys.")
    }

    /// Constructs [Function] for given `keys`, using the build configuration `conf`.
    /// 
    /// Panics if the construction fails.
    /// Then it is almost certain that the input contains either duplicate keys
    /// or keys indistinguishable by any hash function from the family used.
    #[inline] pub fn with_conf<K>(keys: impl KeySet<K>, conf: BuildConf<S>) -> Self
        where K: Hash + Sync
    {
        Self::with_conf_stats(keys, conf, &mut ())
    }

    /// Constructs [`Function`] for given `keys`, using the build configuration `conf` and reporting statistics with `stats`.
    /// 
    /// Panics if the construction fails.
    /// Then it is almost certain that the input contains either duplicate keys
    /// or keys indistinguishable by any hash function from the family used.
    #[inline] pub fn from_slice_with_conf_stats<K, BS>(keys: &[K], conf: BuildConf<S>, stats: &mut BS) -> Self
        where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        Self::with_conf_stats(SliceSourceWithRefs::<_, u8>::new(keys), conf, stats)
    }

    /// Constructs [`Function`] for given `keys`, using the build configuration `conf`.
    /// 
    /// Panics if the construction fails.
    /// Then it is almost certain that the input contains either duplicate keys
    /// or keys indistinguishable by any hash function from the family used.
    #[inline] pub fn from_slice_with_conf<K>(keys: &[K], conf: BuildConf<S>) -> Self
        where K: Hash + Sync
    {
        Self::with_conf_stats(SliceSourceWithRefs::<_, u8>::new(keys), conf, &mut ())
    }

    /// Constructs [`Function`] for given `keys`, using the build configuration `conf` and reporting statistics with `stats`.
    /// Note that `keys` can be reordered during construction.
    /// 
    /// Panics if the construction fails.
    /// Then it is almost certain that the input contains either duplicate keys
    /// or keys indistinguishable by any hash function from the family used.

    #[inline] pub fn from_slice_mut_with_conf_stats<K, BS>(keys: &mut [K], conf: BuildConf<S>, stats: &mut BS) -> Self
        where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        Self::with_conf_stats(SliceMutSource::new(keys), conf, stats)
    }

    /// Constructs [`Function`] for given `keys`, using the build configuration `conf`.
    /// Note that `keys` can be reordered during construction.
    /// 
    /// Panics if the construction fails.
    /// Then it is almost certain that the input contains either duplicate keys
    /// or keys indistinguishable by any hash function from the family used.
    #[inline] pub fn from_slice_mut_with_conf<K>(keys: &mut [K], conf: BuildConf<S>) -> Self
        where K: Hash + Sync
    {
        Self::with_conf_stats(SliceMutSource::new(keys), conf, &mut ())
    }
}

impl Function {
    /// Reads `Self` from the `input`.
    /// Only [Function]s that use default hasher can be read by this method.
    pub fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Self::read_with_hasher(input, Default::default())
    }

    /// Builds [`Function`] for given `keys`, reporting statistics with `stats`.
    /// 
    /// Panics if constructing [`Function`] fails.
    /// Then it is almost certain that the input contains either duplicate keys
    /// or keys indistinguishable by any hash function from the family used.
    pub fn with_stats<K, BS>(keys: impl KeySet<K>, stats: &mut BS) -> Self
        where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        Self::with_conf_stats(keys, Default::default(), stats)
    }

    /// Builds [`Function`] for given `keys`.
    /// 
    /// Panics if constructing [`Function`] fails.
    /// Then it is almost certain that the input contains either duplicate keys
    /// or keys indistinguishable by any hash function from the family used.
    pub fn new<K: Hash + Sync>(keys: impl KeySet<K>) -> Self {
        Self::with_conf_stats(keys, Default::default(), &mut ())
    }
}

impl<S> Function<S> {
    /// Returns the number of keys in the input collection given during construction.
    /// 
    /// The time complexity is proportional to the number returned.
    pub fn len(&self) -> usize {
        self.array.content.count_bit_ones()
    }
}

impl<K: Hash + Sync> From<&[K]> for Function {
    fn from(keys: &[K]) -> Self {
        Self::new(SliceSourceWithRefs::<_, u8>::new(keys))
    }
}

impl<K: Hash + Sync + Send> From<Vec<K>> for Function {
    fn from(keys: Vec<K>) -> Self {
        Self::new(keys)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fmt::Display;

    pub fn test_mphf_iter<K: std::fmt::Display, G: Fn(&K)->Option<u64>>(len: usize, keys: impl IntoIterator<Item=K>, mphf: G) {
        use bitm::BitVec;
        let mut seen = Box::<[u64]>::with_zeroed_bits(len);
        for key in keys {
            let index = mphf(&key);
            assert!(index.is_some(), "MPHF does not assign the value for the key {} which is in the input", key);
            let index = index.unwrap() as usize;
            assert!(index < len, "MPHF assigns too large value for the key {}: {}>{}.", key, index, len);
            assert!(!seen.get_bit(index), "MPHF assigns {} to {} and some other key included in the input", index, key);
            seen.set_bit(index);
        }
    }

    pub fn test_mphf<K: std::fmt::Display+Clone, G: Fn(&K)->Option<u64>>(mphf_keys: &[K], mphf: G) {
        test_mphf_iter(mphf_keys.len(), mphf_keys.iter().cloned(), mphf);
    }

    fn test_read_write(h: &Function) {
        let mut buff = Vec::new();
        h.write(&mut buff).unwrap();
        assert_eq!(buff.len(), h.write_bytes());
        let read = Function::read(&mut &buff[..]).unwrap();
        assert_eq!(h.level_sizes.len(), read.level_sizes.len());
        assert_eq!(h.array.content, read.array.content);
    }

    fn test_with_input<K: Hash + Clone + Display + Sync>(to_hash: &[K]) {
        let h = Function::from_slice_with_conf(to_hash, BuildConf::mt(false));
        test_mphf(to_hash, |key| h.get(key));
        test_read_write(&h);
        assert_eq!(h.len(), to_hash.len());
    }

    #[test]
    fn test_small() {
        test_with_input(&[1, 2, 5]);
        test_with_input(&(-50..150).collect::<Vec<_>>());
        test_with_input(&['a', 'b', 'c', 'd']);
    }

    #[test]
    fn test_large_size() {
        let keys = (-20000..20000).collect::<Vec<_>>();
        assert!(Function::from(&keys[..]).size_bytes() as f64 * (8.0/40000.0) < 2.9);
        assert!(Function::from_slice_with_conf(&keys[..], BuildConf::lsize(200)).size_bytes() as f64 * (8.0/40000.0) < 3.5);
    }

    #[test]
    fn test_dynamic() {
        const LEN: u64 = 50_000;
        let f = Function::new(
            crate::fmph::keyset::CachedKeySet::dynamic(|| 0..LEN, 10_000));
        test_mphf_iter(LEN as usize, 0..LEN, |key| f.get(key));
        assert!(f.size_bytes() as f64 * (8.0/LEN as f64) < 2.9);
    }

    #[test]
    fn test_dynamic_par() {
        const LEN: u64 = 50_000;
        let f = Function::new(
            crate::fmph::keyset::CachedKeySet::dynamic((|| 0..LEN, || (0..LEN).into_par_iter()), 10_000));
        test_mphf_iter(LEN as usize, 0..LEN, |key| f.get(key));
        assert!(f.size_bytes() as f64 * (8.0/LEN as f64) < 2.9);
    }

    #[test]
    #[ignore = "uses much memory and time"]
    fn test_fmph_for_over_2to32_keys() {
        const LEN: u64 = 5_000_000_000;
        let f = Function::with_stats(
            crate::fmph::keyset::CachedKeySet::dynamic(|| 0..LEN, usize::MAX/*1_000_000_000*/),
            &mut crate::stats::BuildStatsPrinter::stdout());
        test_mphf_iter(LEN as usize, 0..LEN, |key| f.get(key));
        assert!(f.size_bytes() as f64 * (8.0/LEN as f64) < 2.9);
    }

    #[test]
    fn test_duplicates() {
        assert!(Function::try_with_conf_stats(vec![1, 1], Default::default(), &mut ()).is_none());
        assert!(Function::try_with_conf_stats(vec![1, 2, 3, 1, 4], Default::default(), &mut ()).is_none());
    }

    #[test]
    fn test_duplicates_partial() {
        let keys = vec![1, 2, 3, 1, 4];
        let expected_initial_len = keys.len();
        let r = Function::try_with_conf_stats_or_partial(keys, Default::default(), &mut ());
        if let Err((mphf, mut remaining, initial_len)) = r {
            assert_eq!(initial_len, expected_initial_len);
            remaining.sort();
            assert_eq!(remaining, vec![1, 1]);
            test_mphf(&[2, 3, 4], |key| mphf.get(key));
        } else {
            assert!(false)
        }
    }
}