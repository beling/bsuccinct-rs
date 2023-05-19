use std::hash::Hash;
use binout::{VByte, Serializer, AsIs};
use bitm::{BitAccess, BitArrayWithRank, ceiling_div};

use crate::utils::{ArrayWithRank, read_bits};
use crate::{BuildDefaultSeededHasher, BuildSeededHasher, stats};

use super::Bits8;
use super::function::{from_mut_slice, get_mut_slice};
use super::goindexing::{GroupSize, SeedSize, TwoToPowerBitsStatic};
use std::io;
use std::sync::atomic::AtomicU64;
use dyn_size_of::GetSize;
use crate::fmph::function::{fphash_add_bit, fphash_remove_collided, fphash_sync_add_bit};
use crate::fmph::goindexing::group_nr;

use rayon::prelude::*;
use crate::fmph::keyset::{KeySet, SliceMutSource, SliceSourceWithRefs};

/// Configuration that is accepted by [`GOFunction`] constructors.
/// 
/// Good configurations can be obtained by calling one of the following functions:
/// [default_biggest](GOConf::default_biggest), [default_bigger](GOConf::default_bigger),
/// [default](GOConf::default), [default_smallest](GOConf::default_smallest).
/// These functions are listed in order of increasing performance (in terms of size and evaluation speed)
/// and time to construct the minimum perfect hash function.
/// More details are included in their documentation and the paper:
/// P. Beling, *Fingerprinting-based minimal perfect hashing revisited*, ACM Journal of Experimental Algorithmics, 2023, <https://doi.org/10.1145/3596453>
#[derive(Clone)]
pub struct GOConf<GS: GroupSize = TwoToPowerBitsStatic::<4>, SS: SeedSize = TwoToPowerBitsStatic<2>, S = BuildDefaultSeededHasher> {
    /// The family of hash functions used by the constructed FMPHGO. (default: [`BuildDefaultSeededHasher`])
    hash_builder: S,
    /// Size of seeds (in bits). (default: 4)
    bits_per_seed: SS,
    /// Size of groups (in bits). (default: 16)
    bits_per_group: GS
}

impl GOConf<TwoToPowerBitsStatic::<3>, TwoToPowerBitsStatic::<0>, BuildDefaultSeededHasher> {
    /// Creates a configuration in which the seed and group sizes are 1 and 8 bits respectively,
    /// which (when relative level size is 100) leads to a minimum perfect hash function whose:
    /// - size is about 2.52 bits per input key,
    /// - the expected number of levels visited during the evaluation is about 2.18,
    /// - construction takes about 4 times less time compared to the [default](GOConf::default) configuration.
    pub fn default_biggest() -> Self {
        Self {
            hash_builder: Default::default(),
            bits_per_seed: Default::default(),
            bits_per_group: Default::default()
        }
    }
}

impl GOConf<TwoToPowerBitsStatic::<4>, TwoToPowerBitsStatic::<1>, BuildDefaultSeededHasher> {
    /// Creates a configuration in which the seed and group sizes are 2 and 16 bits respectively,
    /// which (when relative level size is 100) leads to a minimum perfect hash function whose:
    /// - size is about 2.36 bits per input key,
    /// - the expected number of levels visited during the evaluation is about 2.04,
    /// - construction takes about 3 times less time compared to the [default](GOConf::default) configuration.
    pub fn default_bigger() -> Self {
        Self {
            hash_builder: Default::default(),
            bits_per_seed: Default::default(),
            bits_per_group: Default::default()
        }
    }
}

impl Default for GOConf {
    /// Creates a configuration in which the seed and group sizes are 4 and 16 bits respectively,
    /// which (when relative level size is 100) leads to a minimum perfect hash function whose:
    /// - size is about 2.21 bits per input key,
    /// - the expected number of levels visited during the evaluation is about 1.73.
    fn default() -> Self {
        Self {
            hash_builder: Default::default(),
            bits_per_seed: Default::default(),
            bits_per_group: Default::default()
        }
    }
}

impl GOConf<TwoToPowerBitsStatic::<5>, Bits8, BuildDefaultSeededHasher> {
    /// Creates a configuration in which the seed and group sizes are 8 and 32 bits respectively,
    /// which (when relative level size is 100) leads to a minimum perfect hash function whose:
    /// - size is about 2.10 bits per input key,
    /// - the expected number of levels visited during the evaluation is about 1.64,
    /// - construction takes about 13 times longer compared to the [default](GOConf::default) configuration.
    pub fn default_smallest() -> Self {
        Self {
            hash_builder: Default::default(),
            bits_per_seed: Default::default(),
            bits_per_group: Default::default()
        }
    }
}

impl<GS: GroupSize, SS: SeedSize> GOConf<GS, SS> {
    /// Returns a configuration that uses seeds and groups of the sizes given in bits.
    pub fn bps_bpg(bits_per_seed: SS, bits_per_group: GS) -> Self {
        bits_per_seed.validate().unwrap();
        bits_per_group.validate().unwrap();
        Self {
            hash_builder: Default::default(),
            bits_per_seed,
            bits_per_group,
        }
    }
}

impl<GS: GroupSize, SS: SeedSize, S: BuildSeededHasher> GOConf<GS, SS, S> {
    /// Returns a configuration that uses given family of hash functions and seeds and groups of the sizes given in bits.
    pub fn hash_bps_bpg(hash_builder: S, bits_per_seed: SS, bits_per_group: GS) -> Self {
        bits_per_seed.validate().unwrap();
        bits_per_group.validate().unwrap();
        Self { hash_builder, bits_per_seed, bits_per_group }  // 1<<6=64
    }

    /// Returns array index for given `hash` of key, size of level in groups, and group seed provided by `group_seed`.
    #[inline(always)] pub fn hash_index<GetGroupSeed>(&self, hash: u64, level_size_groups: u64, group_seed: GetGroupSeed) -> usize
        where GetGroupSeed: FnOnce(u64) -> u16  // returns group seed for group with given index
    {
        let group = group_nr(hash, level_size_groups);
        self.bits_per_group.bit_index_for_seed(hash, group_seed(group), group)
    }

    /// Returns array index for given `key`, seed and size (in groups) of level, and group seed provided by `group_seed`.
    #[inline(always)] pub fn key_index<GetGroupSeed, K>(&self, key: &K, level_seed: u32, level_size_groups: u64, group_seed: GetGroupSeed) -> usize
        where GetGroupSeed: FnOnce(u64) -> u16, K: Hash
    {
        self.hash_index(self.hash_builder.hash_one(key, level_seed), level_size_groups, group_seed)
    }

    /// Returns fingerprint array for given hashes of keys, level size, and group seeds (given as a function that returns seeds for provided group indices).
    fn build_array_for_hashes(&self, key_hashes: &[u64], level_size_segments: usize, level_size_groups: u64, group_seed: u16) -> Box<[u64]>
    {
        let mut result = vec![0u64; level_size_segments].into_boxed_slice();
        let mut collision = vec![0u64; level_size_segments].into_boxed_slice();
        for hash in key_hashes {
            fphash_add_bit(&mut result, &mut collision, self.hash_index(*hash, level_size_groups, |_| group_seed));
        };
        fphash_remove_collided(&mut result, &collision);
        result
    }
}

impl<GS: GroupSize + Sync, SS: SeedSize, S: BuildSeededHasher + Sync> GOConf<GS, SS, S> {
    /// Returns fingerprint array for given hashes of keys, level size, and group seeds (given as a function that returns seeds for provided group indices).
    fn build_array_for_hashes_mt(&self, key_hashes: &[u64], level_size_segments: usize, level_size_groups: u64, group_seed: u16) -> Box<[u64]>
    {
        let mut result = vec![0u64; level_size_segments].into_boxed_slice();
        let result_atom = from_mut_slice(&mut result);
        let mut collision: Box<[AtomicU64]> = (0..level_size_segments).map(|_| AtomicU64::default()).collect();
        key_hashes.par_iter().for_each(
            |hash| fphash_sync_add_bit(&result_atom, &collision, self.hash_index(*hash, level_size_groups, |_| group_seed))
        );
        fphash_remove_collided(&mut result, get_mut_slice(&mut collision));
        result
    }
}

/// Helper structure for building fingerprinting-based minimal perfect hash function with group optimization (FMPHGO, [GOFunction]).
#[derive(Clone)]
pub struct GOBuilder<GS: GroupSize = TwoToPowerBitsStatic::<4>, SS: SeedSize = TwoToPowerBitsStatic<2>, S = BuildDefaultSeededHasher> {
    level_sizes: Vec::<u64>,
    arrays: Vec::<Box<[u64]>>,
    group_seeds: Vec::<Box<[SS::VecElement]>>,
    pub cache_threshold: usize,     // maximum size of the keys set to cache hashes
    pub relative_level_size: u16,
    use_multiple_threads: bool,
    pub conf: GOConf<GS, SS, S>,
}   // TODO introduce trait to make other builders possible

impl Default for GOBuilder {
    fn default() -> Self { Self::new(Default::default()) }
}

impl<GS: GroupSize + Sync, SS: SeedSize, S: BuildSeededHasher + Sync> GOBuilder<GS, SS, S>
{
    const DEFAULT_RELATIVE_LEVEL_SIZE: u16 = 100;
    const DEFAULT_CACHE_THRESHOLD: usize = 1024*1024*128; // *8 bytes = max 1GB for pre-hashing

    /// Enable / disable building levels with multiple threads.
    pub fn set_use_multiple_threads(&mut self, use_multiple_threads: bool) {
        self.use_multiple_threads = use_multiple_threads && rayon::current_num_threads() > 1;
    }

    #[inline(always)] pub fn get_use_multiple_threads(&self) -> bool { self.use_multiple_threads }

    pub fn with_lsize_ct_mt(conf: GOConf<GS, SS, S>, relative_level_size: u16, cache_threshold: usize, use_multiple_threads: bool) -> Self {
        Self {
            level_sizes: Vec::<u64>::new(),
            arrays: Vec::<Box<[u64]>>::new(),
            group_seeds: Vec::<Box<[SS::VecElement]>>::new(),
            cache_threshold,
            relative_level_size,
            use_multiple_threads: use_multiple_threads && rayon::current_num_threads() > 1,
            conf
        }
    }

    pub fn new(conf: GOConf<GS, SS, S>) -> Self {
        Self::with_lsize_ct_mt(conf, Self::DEFAULT_RELATIVE_LEVEL_SIZE, Self::DEFAULT_CACHE_THRESHOLD, true)
    }

    pub fn with_lsize_ct(conf: GOConf<GS, SS, S>, relative_level_size: u16, cache_threshold: usize) -> Self {
        Self::with_lsize_ct_mt(conf, relative_level_size, cache_threshold, true)
    }

    /// Returns builder that uses at each level a bit-array of size `relative_level_size`
    /// given as a percent of number of input keys for the level.
    pub fn with_lsize(conf: GOConf<GS, SS, S>, relative_level_size: u16) -> Self {
        Self::with_lsize_ct_mt(conf, relative_level_size, Self::DEFAULT_CACHE_THRESHOLD, true)
    }

    /// Returns builder that potentially uses multiple threads to build levels,
    /// and at each level a bit-array of size `relative_level_size`
    /// given as a percent of number of input keys for the level.
    pub fn with_lsize_mt(conf: GOConf<GS, SS, S>, relative_level_size: u16, use_multiple_threads: bool) -> Self {
        Self::with_lsize_ct_mt(conf, relative_level_size, Self::DEFAULT_CACHE_THRESHOLD, use_multiple_threads)
    }

    pub fn with_mt(conf: GOConf<GS, SS, S>, use_multiple_threads: bool) -> Self {
        Self::with_lsize_ct_mt(conf, Self::DEFAULT_RELATIVE_LEVEL_SIZE, Self::DEFAULT_CACHE_THRESHOLD, use_multiple_threads)
    }

    fn push(&mut self, array: Box<[u64]>, seeds: Box<[SS::VecElement]>, size_groups: u64) {
        self.arrays.push(array);
        self.group_seeds.push(seeds);
        self.level_sizes.push(size_groups);
    }

    /// Returns number the level about to build (number of levels built so far).
    #[inline(always)] fn level_nr(&self) -> u32 { self.level_sizes.len() as u32 }

    #[inline(always)] fn last_seed(&self) -> u16 { ((1u32 << self.conf.bits_per_seed.into())-1) as u16 }

    /// Returns whether `key` is retained (`false` if it is already hashed at the levels built so far).
    fn retained<K>(&self, key: &K) -> bool where K: Hash {
        self.arrays.iter().zip(self.group_seeds.iter()).zip(self.level_sizes.iter()).enumerate()
            .all(|(level_seed, ((a, seeds), level_size_groups))| {
                !a.get_bit(self.conf.key_index(key, level_seed as u32, *level_size_groups,
                |group| self.conf.bits_per_seed.get_seed(seeds, group as usize)))
            })
    }

    /// Returns fingerprint array for given keys, level size, and group seeds (given as a function that returns seeds for provided group indices).
    #[inline(always)]
    fn build_array<KS, K>(&self, keys: &KS, level_size_segments: usize, level_size_groups: u64, group_seed: u16) -> Box<[u64]>
        where   // returns group seed for group with given index
            K: Hash, KS: KeySet<K>
    {
        let mut result = vec![0u64; level_size_segments].into_boxed_slice();
        let mut collision = vec![0u64; level_size_segments].into_boxed_slice();
        let level_seed = self.level_nr();
        keys.for_each_key(|key| fphash_add_bit(&mut result, &mut collision, self.conf.key_index(key, level_seed, level_size_groups, |_| group_seed)),
                          |key| self.retained(key));
        fphash_remove_collided(&mut result, &collision);
        result
    }

    /// Returns fingerprint array for given hashes of keys, level size, and group seeds (given as a function that returns seeds for provided group indices).
    #[inline(always)]
    fn build_array_mt<KS, K>(&self, keys: &KS, level_size_segments: usize, level_size_groups: u64, group_seed: u16) -> Box<[u64]>
        where K: Hash, KS: KeySet<K>  // returns group seed for group with given index
    {
        if !keys.has_par_for_each_key() {
            return self.build_array(keys, level_size_segments, level_size_groups, group_seed);
        }
        let mut result = vec![0u64; level_size_segments as usize].into_boxed_slice();
        let result_atom = from_mut_slice(&mut result);
        let mut collision: Box<[AtomicU64]> = (0..level_size_segments).map(|_| AtomicU64::default()).collect();
        let level_seed = self.level_nr();
        keys.par_for_each_key(
            |key| fphash_sync_add_bit(&result_atom, &collision, self.conf.key_index(key, level_seed, level_size_groups, |_| group_seed)),
            |key| self.retained(key)
        );
        fphash_remove_collided(&mut result, get_mut_slice(&mut collision));
        result
    }

    /// Update `best_array` and `best_seeds` copying groups that are better (have more ones in `array`) from `array` and `array_seed`.
    fn update_best<GetGroupSeed>(&self, level_size_groups: u64, best_array: &mut [u64], best_seeds: &mut [SS::VecElement], array: &[u64], array_seed: GetGroupSeed)
        where GetGroupSeed: Fn(u64) -> u16
    {
        for group_index in 0..level_size_groups {
            self.conf.bits_per_group.copy_group_if_better(best_array, array, group_index as usize,
                || self.conf.bits_per_seed.set_seed(best_seeds, group_index as usize, array_seed(group_index))
            )
        }
    }

    /// Build (by calling `build_for_group`) arrays for all group seeds sequentially and select best groups and seeds (which are returned).
    /// `build_for_group` can use multiple threads internally to build each array.
    #[inline(always)]
    fn best_array<AB>(&self, build_for_group: AB, level_size_groups: u64) -> (Box<[u64]>, Box<[SS::VecElement]>)
        where AB: Fn(u16) -> Box<[u64]> // build array for given group nr
    {
        let mut best_array = build_for_group(0);
        let mut best_seeds = self.conf.bits_per_seed.new_zeroed_seed_vec(level_size_groups as usize);
        for group_seed in 1..=self.last_seed() {
            let with_new_seed = build_for_group(group_seed);
            self.update_best(level_size_groups, &mut best_array, &mut best_seeds, &with_new_seed, |_| group_seed);
        }
        (best_array, best_seeds)
    }

    fn build_next_level_with_cache<KS, K>(&mut self, keys: &mut KS, level_size_groups: u64, level_size_segments: usize)
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        let level_seed = self.level_nr();
        let key_hashes = keys.maybe_par_map_each_key(
            |k| self.conf.hash_builder.hash_one(k, level_seed),
            |key| self.retained(key),
            self.use_multiple_threads
        );
        let (array, seeds) = if self.use_multiple_threads {
            self.best_array(|g| self.conf.build_array_for_hashes_mt(&key_hashes, level_size_segments, level_size_groups, g), level_size_groups)
        } else {
            self.best_array(|g| self.conf.build_array_for_hashes(&key_hashes, level_size_segments, level_size_groups, g), level_size_groups)
        };
        keys.maybe_par_retain_keys_with_indices(
            |i| !array.get_bit(
                self.conf.hash_index(key_hashes[i], level_size_groups,
                                     |group| self.conf.bits_per_seed.get_seed(&seeds, group as usize))
            ),
            |key| !array.get_bit(
                self.conf.key_index(key, level_seed, level_size_groups,
                                    |group| self.conf.bits_per_seed.get_seed(&seeds, group as usize))
            ),
            |key| self.retained(key),
            || array.count_bit_ones(),
            self.use_multiple_threads
        );
        self.push(array, seeds, level_size_groups);
    }

    fn build_next_level<KS, K>(&mut self, keys: &mut KS, level_size_groups: u64, level_size_segments: usize)
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        if keys.keys_len() < self.cache_threshold {
            return self.build_next_level_with_cache(keys, level_size_groups, level_size_segments);
        }
        let (array, seeds) = if self.use_multiple_threads {
            self.best_array(|g| self.build_array_mt(keys, level_size_segments, level_size_groups, g), level_size_groups)
        } else {
            self.best_array(|g| self.build_array(keys, level_size_segments, level_size_groups, g), level_size_groups)
        };
        let level_nr = self.level_nr();
        keys.maybe_par_retain_keys(
            |key| {
                let hash = self.conf.hash_builder.hash_one(key, level_nr);
                let group = group_nr(hash, level_size_groups as u64);
                let bit_index = self.conf.bits_per_group.bit_index_for_seed(
                    hash,
                    //current_seeds.get_fragment(group as usize, conf.bits_per_group_seed) as u16,
                    self.conf.bits_per_seed.get_seed(&seeds, group as usize),
                    group);
                !array.get_bit(bit_index)
            },
            |key| self.retained(key),
            || array.count_bit_ones(),
            self.use_multiple_threads
        );
        self.push(array, seeds, level_size_groups as u64);
    }
}

/// Fingerprinting-based minimal perfect hash function with group optimization (FMPHGO).
///
/// See:
/// - P. Beling, *Fingerprinting-based minimal perfect hashing revisited*, ACM Journal of Experimental Algorithmics, 2023, <https://doi.org/10.1145/3596453>
pub struct GOFunction<GS: GroupSize = TwoToPowerBitsStatic::<4>, SS: SeedSize = TwoToPowerBitsStatic<2>, S = BuildDefaultSeededHasher> {
    array: ArrayWithRank,
    group_seeds: Box<[SS::VecElement]>,   //  Box<[u8]>,
    level_size: Box<[u64]>, // number of groups
    conf: GOConf<GS, SS, S>
    // 0..01..1 mask with number of ones = group size (in bits)
    //group_size_mask: u8,
}

impl<GS: GroupSize, SS: SeedSize, S: BuildSeededHasher> GetSize for GOFunction<GS, SS, S> {
    fn size_bytes_dyn(&self) -> usize {
        self.array.size_bytes_dyn()
            //+ self.seeds.len() * std::mem::size_of::<u8>()
            + self.group_seeds.size_bytes_dyn()
            + self.level_size.size_bytes_dyn()
    }

    const USES_DYN_MEM: bool = true;
}

impl<GS: GroupSize, SS: SeedSize, S: BuildSeededHasher> GOFunction<GS, SS, S> {

    /// Gets the value associated with the given `key` and reports statistics to `access_stats`.
    /// 
    /// The returned value is in the range: `0` (inclusive), the number of elements in the input key collection (exclusive).
    /// If the `key` was not in the input key collection, either `None` or an undetermined value from the specified range is returned.
    pub fn get_stats<K: Hash, A: stats::AccessStatsCollector>(&self, key: &K, access_stats: &mut A) -> Option<u64> {
        let mut groups_before = 0u64;
        let mut level_nr = 0u32;
        loop {
            let level_size_groups = *self.level_size.get(level_nr as usize)?;
            /*let bit_index = self.conf.key_index(key, level_nr, level_size_groups,
                                                |g| self.conf.bits_per_seed.get_seed(&self.group_seeds, (groups_before + g) as usize)
            ); // wrong as bit_index_for_seed is called with wrong group */
            let hash = self.conf.hash_builder.hash_one(key, level_nr);
            let group = groups_before + group_nr(hash, level_size_groups);
            let seed = self.conf.bits_per_seed.get_seed(&self.group_seeds, group as usize);
            let bit_index = self.conf.bits_per_group.bit_index_for_seed(hash, seed, group);
            if self.array.content.get_bit(bit_index) {
                access_stats.found_on_level(level_nr);
                return Some(self.array.rank(bit_index) as u64);
            }
            groups_before += level_size_groups;
            level_nr += 1;
        }
    }

    /// Gets the value associated with the given `key`.
    /// 
    /// The returned value is in the range: `0` (inclusive), the number of elements in the input key collection (exclusive).
    /// If the `key` was not in the input key collection, either `None` or an undetermined value from the specified range is returned.
    #[inline] pub fn get<K: Hash>(&self, key: &K) -> Option<u64> {
        self.get_stats(key, &mut ())
    }

    /// Returns number of bytes which `write` will write.
    pub fn write_bytes(&self) -> usize {
        self.conf.bits_per_group.write_size_bytes()
            + VByte::array_size(&self.level_size)
            + AsIs::array_content_size(&self.array.content)
            + std::mem::size_of::<u8>() + self.group_seeds.size_bytes_content_dyn()
    }

    /// Writes `self` to the `output`.
    pub fn write(&self, output: &mut dyn io::Write) -> io::Result<()>
    {
        self.conf.bits_per_group.write(output)?;
        VByte::write_array(output, &self.level_size)?;
        AsIs::write_all(output, self.array.content.iter())?;
        self.conf.bits_per_seed.write_seed_vec(output, &self.group_seeds)
    }

    /// Reads `Self` from the `input`. Hash builder must be the same as the one used to write.
    pub fn read_with_hasher(input: &mut dyn io::Read, hash_builder: S) -> io::Result<Self>
    {
        let bits_per_group = GS::read(input)?;
        let level_size = VByte::read_array(input)?;
        let number_of_groups = level_size.iter().map(|v|*v as usize).sum::<usize>();

        let array_content = read_bits(input, bits_per_group * number_of_groups)?;
        let (array_with_rank, _) = ArrayWithRank::build(array_content);

        let (bits_per_group_seed, group_seeds) = SS::read_seed_vec(input, number_of_groups)?;

        Ok(Self {
            array: array_with_rank,
            group_seeds,
            level_size,
            conf: GOConf {
                bits_per_seed: bits_per_group_seed,
                bits_per_group,
                hash_builder
            },
        })
    }

    pub fn level_sizes(&self) -> &[u64] {
        &self.level_size
    }
}

impl<GS: GroupSize + Sync, SS: SeedSize, S: BuildSeededHasher + Sync> GOFunction<GS, SS, S> {
    /// Builds [GOFunction] for given `keys`, using the configuration `conf` and reporting statistics to `stats`.
    pub fn with_builder_stats<K, KS, BS>(mut keys: KS, mut levels: GOBuilder<GS, SS, S>, stats: &mut BS) -> Self
        where K: Hash + Sync, KS: KeySet<K> + Sync, BS: stats::BuildStatsCollector
    {
        while keys.keys_len() != 0 {
            let input_size = keys.keys_len();
            let (level_size_groups, level_size_segments) = levels.conf.bits_per_group.level_size_groups_segments(
                ceiling_div(input_size * levels.relative_level_size as usize, 100));
            //let seed = level_nr;
            stats.level(input_size, level_size_segments * 64);
            levels.build_next_level(&mut keys, level_size_groups as u64, level_size_segments);
        }
        drop(keys);
        stats.end();
        let (array, _)  = ArrayWithRank::build(levels.arrays.concat().into_boxed_slice());
        let group_seeds_concatenated = levels.conf.bits_per_seed.concatenate_seed_vecs(&levels.level_sizes, levels.group_seeds);
        Self {
            array,
            group_seeds: group_seeds_concatenated,
            conf: levels.conf,
            level_size: levels.level_sizes.into_boxed_slice(),
        }
    }

    pub fn with_builder<K, KS>(keys: KS, levels: GOBuilder<GS, SS, S>) -> Self
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        Self::with_builder_stats(keys, levels, &mut ())
    }

    pub fn with_conf_stats<K, KS, BS>(keys: KS, conf: GOConf<GS, SS, S>, stats: &mut BS) -> Self
        where K: Hash + Sync, KS: KeySet<K> + Sync, BS: stats::BuildStatsCollector
    {
        Self::with_builder_stats(keys, GOBuilder::new(conf), stats)
    }

    pub fn with_conf_mt_stats<K, KS, BS>(keys: KS, conf: GOConf<GS, SS, S>, use_multiple_threads: bool, stats: &mut BS) -> Self
        where K: Hash + Sync, KS: KeySet<K> + Sync, BS: stats::BuildStatsCollector
    {
        Self::with_builder_stats(keys, GOBuilder::with_mt(conf, use_multiple_threads), stats)
    }

    #[inline] pub fn with_conf<K, KS>(keys: KS, conf: GOConf<GS, SS, S>) -> Self
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        Self::with_conf_stats(keys, conf, &mut ())
    }

    #[inline] pub fn with_conf_mt<K, KS>(keys: KS, conf: GOConf<GS, SS, S>, use_multiple_threads: bool) -> Self
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        Self::with_conf_mt_stats(keys, conf, use_multiple_threads, &mut ())
    }

    /// Builds [GOFunction] for given `keys`, using the configuration `conf`.
    #[inline] pub fn from_slice_with_conf_stats<K, BS>(keys: &[K], conf: GOConf<GS, SS, S>, stats: &mut BS) -> Self
        where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        Self::with_conf_stats(SliceSourceWithRefs::<_, u8>::new(keys), conf, stats)
    }

    #[inline] pub fn from_slice_with_conf_mt_stats<K, BS>(keys: &[K], conf: GOConf<GS, SS, S>, use_multiple_threads: bool, stats: &mut BS) -> Self
        where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        Self::with_conf_mt_stats(SliceSourceWithRefs::<_, u8>::new(keys), conf, use_multiple_threads, stats)
    }

    /// Builds [GOFunction] for given `keys`, using the configuration `conf`.
    #[inline] pub fn from_slice_with_conf<K>(keys: &[K], conf: GOConf<GS, SS, S>) -> Self
        where K: Hash + Sync
    {
        Self::with_conf_stats(SliceSourceWithRefs::<_, u8>::new(keys), conf, &mut ())
    }

    #[inline] pub fn from_slice_with_conf_mt<K>(keys: &[K], conf: GOConf<GS, SS, S>, use_multiple_threads: bool) -> Self
        where K: Hash + Sync { Self::with_conf_mt_stats(SliceSourceWithRefs::<_, u8>::new(keys), conf, use_multiple_threads, &mut ()) }

    /// Builds [GOFunction] for given `keys`, using the configuration `conf`.
    #[inline] pub fn from_slice_mut_with_conf_stats<K, BS>(keys: &mut [K], conf: GOConf<GS, SS, S>, stats: &mut BS) -> Self
        where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        Self::with_conf_stats(SliceMutSource::new(keys), conf, stats)
    }

    /// Builds [GOFunction] for given `keys`, using the configuration `conf`.
    #[inline] pub fn from_slice_mut_with_conf<K>(keys: &mut [K], conf: GOConf<GS, SS, S>) -> Self
        where K: Hash + Sync
    {
        Self::with_conf_stats(SliceMutSource::new(keys), conf, &mut ())
    }
}

impl<GS: GroupSize + Sync, SS: SeedSize> GOFunction<GS, SS> {
    /// Reads `Self` from the `input`.
    /// Only [GOFunction]s that use default hasher can be read by this method.
    pub fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Self::read_with_hasher(input, Default::default())
    }
}

impl GOFunction {
    /// Builds [GOFunction] for given `keys`, reporting statistics to `stats`.
    pub fn from_slice_with_stats<K, BS>(keys: &[K], stats: &mut BS) -> Self
        where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        Self::from_slice_with_conf_stats(keys, Default::default(), stats)
    }

    /// Builds [GOFunction] for given `keys`.
    pub fn from_slice<K: Hash + Sync>(keys: &[K]) -> Self {
        Self::from_slice_with_conf_stats(keys, Default::default(), &mut ())
    }
}

impl<K: Hash + Clone + Sync> From<&[K]> for GOFunction {
    fn from(keys: &[K]) -> Self {
        Self::from_slice(&mut keys.to_owned())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::{utils::test_mphf, fmph::TwoToPowerBits};
    use std::fmt::{Debug, Display};
    use crate::fmph::Bits;

    fn test_read_write<GS: GroupSize + Sync, SS: SeedSize>(h: &GOFunction<GS, SS>)
        where SS::VecElement: std::cmp::PartialEq + Debug
    {
        let mut buff = Vec::new();
        h.write(&mut buff).unwrap();
        assert_eq!(buff.len(), h.write_bytes());
        let read = GOFunction::<GS, SS>::read(&mut &buff[..]).unwrap();
        assert_eq!(h.level_size, read.level_size);
        assert_eq!(h.array.content, read.array.content);
        assert_eq!(h.group_seeds, read.group_seeds);
    }

    fn test_hash2_invariants<GS: GroupSize, SS: SeedSize>(h: &GOFunction<GS, SS>) {
        let number_of_groups = h.level_size.iter().map(|v| *v as usize).sum::<usize>();
        assert_eq!(h.conf.bits_per_group * number_of_groups, h.array.content.len() * 64);
        assert_eq!(ceiling_div(number_of_groups * h.conf.bits_per_seed.into() as usize, 64), h.group_seeds.len());
    }

    fn test_with_input<K: Hash + Clone + Display + Sync>(to_hash: &[K], bits_per_group: impl GroupSize + Sync) {
        let conf = GOConf::bps_bpg(Bits(3), bits_per_group);
        let h = GOFunction::from_slice_with_conf_mt(&mut to_hash.to_vec(), conf, false);
        //dbg!(h.size_bytes() as f64 * 8.0/to_hash.len() as f64);
        test_mphf(to_hash, |key| h.get(key).map(|i| i as usize));
        test_hash2_invariants(&h);
        test_read_write(&h);
    }

    #[test]
    fn test_small_powers_of_two() {
        //test_with_input(&[1, 2, 5], TwoToPowerBits::new(7));     // not supported for now, upto 63 bit / group
        //test_with_input(&[1, 2, 5], TwoToPowerBits::new(6));     // not supported for now, upto 63 bit / group
        test_with_input(&[1, 2, 5], TwoToPowerBits::new(5));
        test_with_input(&[1, 2, 5], TwoToPowerBits::new(4));
        test_with_input(&[1, 2, 5], TwoToPowerBits::new(3));
        test_with_input(&[1, 2, 5], TwoToPowerBits::new(2));
        test_with_input(&[1, 2, 5], TwoToPowerBits::new(1));
        test_with_input(&[1, 2, 5], TwoToPowerBits::new(0));
        //test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(7)); // not supported for now, upto 63 bit / group
        //test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(6)); // not supported for now, upto 63 bit / group
        test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(5));
        test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(4));
        test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(3));
        test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(2));
        test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(1));
        test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(0));
        //test_with_input(&['a', 'b', 'c', 'd'], TwoToPowerBits::new(7)); // not supported for now, upto 63 bit / group
        //test_with_input(&['a', 'b', 'c', 'd'], TwoToPowerBits::new(6)); // not supported for now, upto 63 bit / group
        test_with_input(&['a', 'b', 'c', 'd'], TwoToPowerBits::new(5));
        test_with_input(&['a', 'b', 'c', 'd'], TwoToPowerBits::new(4));
        test_with_input(&['a', 'b', 'c', 'd'], TwoToPowerBits::new(3));
        test_with_input(&['a', 'b', 'c', 'd'], TwoToPowerBits::new(2));
        test_with_input(&['a', 'b', 'c', 'd'], TwoToPowerBits::new(1));
        test_with_input(&['a', 'b', 'c', 'd'], TwoToPowerBits::new(0));
    }

    #[test]
    fn test_small_bits() {
        test_with_input(&[1, 2, 5], Bits(3));
        test_with_input(&[1, 2, 5], Bits(5));
        test_with_input(&[1, 2, 5], Bits(20));
        test_with_input(&[1, 2, 5], Bits(60));
        test_with_input(&[1, 2, 5], Bits(63));
        test_with_input(&(-50..150).collect::<Vec<_>>(), Bits(3));
        test_with_input(&(-50..150).collect::<Vec<_>>(), Bits(5));
        test_with_input(&(-50..150).collect::<Vec<_>>(), Bits(20));
        test_with_input(&(-50..150).collect::<Vec<_>>(), Bits(60));
        test_with_input(&(-50..150).collect::<Vec<_>>(), Bits(63));
        test_with_input(&['a', 'b', 'c', 'd'], Bits(3));
        test_with_input(&['a', 'b', 'c', 'd'], Bits(5));
        test_with_input(&['a', 'b', 'c', 'd'], Bits(20));
        test_with_input(&['a', 'b', 'c', 'd'], Bits(60));
        test_with_input(&['a', 'b', 'c', 'd'], Bits(63));
    }

    #[test]
    fn test_medium() {
        let keys: Vec<_> = (-2000..2000).map(|v| 3*v).collect();
        //test_with_input(&keys, TwoToPowerBits::new(7));   // not supported for now, upto 63 bit / group
        //test_with_input(&keys, TwoToPowerBits::new(6));   // not supported for now, upto 63 bit / group
        test_with_input(&keys, TwoToPowerBits::new(5));
        test_with_input(&keys, TwoToPowerBits::new(4));
        test_with_input(&keys, TwoToPowerBits::new(3));
        test_with_input(&keys, TwoToPowerBits::new(2));
        test_with_input(&keys, TwoToPowerBits::new(1));
        test_with_input(&keys, TwoToPowerBits::new(0));
        test_with_input(&keys, Bits(3));
        test_with_input(&keys, Bits(5));
        test_with_input(&keys, Bits(10));
        test_with_input(&keys, Bits(13));
        test_with_input(&keys, Bits(20));
        test_with_input(&keys, Bits(27));
        test_with_input(&keys, Bits(33));
        test_with_input(&keys, Bits(45));
        test_with_input(&keys, Bits(50));
        test_with_input(&keys, Bits(55));
        test_with_input(&keys, Bits(60));
        test_with_input(&keys, Bits(63));
    }
}