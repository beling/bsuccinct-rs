use binout::{write_int, read_int};
use std::hash::Hash;
use bitm::{BitAccess, BitArrayWithRank, ceiling_div};

use crate::utils::ArrayWithRank;
use crate::{BuildDefaultSeededHasher, BuildSeededHasher, stats};

use crate::read_array;
use super::indexing2::{GroupSize, SeedSize, TwoToPowerBits, TwoToPowerBitsStatic};
use std::io;
use std::ops::RangeInclusive;
use std::sync::atomic::AtomicU64;
use dyn_size_of::GetSize;
use crate::fp::hash::{fphash_add_bit, fphash_remove_collided, fphash_sync_add_bit};
use crate::fp::indexing2::group_nr;

use rayon::prelude::*;
use crate::fp::keyset::{KeySet, SliceMutSource, SliceSourceWithRefs};

/// Configuration that is accepted by `FPHash2` constructors.
#[derive(Clone)]
pub struct FPHash2Conf<GS: GroupSize = TwoToPowerBits, SS: SeedSize = TwoToPowerBitsStatic<2>, S = BuildDefaultSeededHasher> {
    hash_builder: S,
    bits_per_seed: SS,
    bits_per_group: GS
}

impl Default for FPHash2Conf {
    fn default() -> Self {
        Self {
            hash_builder: Default::default(),
            bits_per_seed: Default::default(),
            bits_per_group: TwoToPowerBits::new(4)
        }
    }
}

impl<SS: SeedSize> FPHash2Conf<TwoToPowerBits, SS> {
    /// Returns configuration that uses seeds of size given in bits.
    pub fn bps(bits_per_seed: SS) -> Self {
        bits_per_seed.validate().unwrap();
        Self {
            hash_builder: Default::default(),
            bits_per_seed,
            bits_per_group: TwoToPowerBits::new(4),
        }
    }
}

impl<GS: GroupSize> FPHash2Conf<GS> {
    /// Returns configuration that uses groups of size given in bits.
    pub fn bpg(bits_per_group: GS) -> Self {
        bits_per_group.validate().unwrap();
        Self {
            hash_builder: Default::default(),
            bits_per_seed: Default::default(),
            bits_per_group,
        }
    }
}

impl<GS: GroupSize, SS: SeedSize> FPHash2Conf<GS, SS> {
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

impl<GS: GroupSize, S> FPHash2Conf<GS, TwoToPowerBitsStatic<2>, S> {
    pub fn hash_bpg(hash: S, bits_per_group: GS) -> Self {
        bits_per_group.validate().unwrap();
        Self {
            hash_builder: hash,
            bits_per_seed: Default::default(),
            bits_per_group,
        }
    }
}

impl<GS: GroupSize, SS: SeedSize, S: BuildSeededHasher> FPHash2Conf<GS, SS, S> {
    pub fn hash_bps_bpg(hash: S, bits_per_seed: SS, bits_per_group: GS) -> Self {
        bits_per_seed.validate().unwrap();
        bits_per_group.validate().unwrap();
        Self { hash_builder: hash, bits_per_seed, bits_per_group }  // 1<<6=64
    }

    /// Returns array index for given `hash` of key, size of level in groups, and group seed provided by `group_seed`.
    #[inline(always)] pub fn hash_index<GetGroupSeed>(&self, hash: u64, level_size_groups: u32, group_seed: GetGroupSeed) -> usize
        where GetGroupSeed: FnOnce(u32) -> u16  // returns group seed for group with given index
    {
        let group = group_nr(hash, level_size_groups);
        self.bits_per_group.bit_index_for_seed(hash, group_seed(group), group)
    }

    /// Returns array index for given `key`, seed and size (in groups) of level, and group seed provided by `group_seed`.
    #[inline(always)] pub fn key_index<GetGroupSeed, K>(&self, key: &K, level_seed: u32, level_size_groups: u32, group_seed: GetGroupSeed) -> usize
        where GetGroupSeed: FnOnce(u32) -> u16, K: Hash
    {
        self.hash_index(self.hash_builder.hash_one(key, level_seed), level_size_groups, group_seed)
    }

    /// Returns fingerprint array for given hashes of keys, level size, and group seeds (given as a function that returns seeds for provided group indices).
    fn build_array_for_hashes<GetGroupSeed>(&self, key_hashes: &[u64], level_size_segments: usize, level_size_groups: u32, group_seed: GetGroupSeed) -> Box<[u64]>
        where GetGroupSeed: Fn(u32) -> u16 + Copy  // returns group seed for group with given index
    {
        let mut result = vec![0u64; level_size_segments].into_boxed_slice();
        let mut collision = vec![0u64; level_size_segments].into_boxed_slice();
        for hash in key_hashes {
            fphash_add_bit(&mut result, &mut collision, self.hash_index(*hash, level_size_groups, group_seed));
        };
        fphash_remove_collided(&mut result, &collision);
        result
    }
}

impl<GS: GroupSize + Sync, SS: SeedSize, S: BuildSeededHasher + Sync> FPHash2Conf<GS, SS, S> {
    /// Returns fingerprint array for given hashes of keys, level size, and group seeds (given as a function that returns seeds for provided group indices).
    fn build_array_for_hashes_mt<GetGroupSeed>(&self, key_hashes: &[u64], level_size_segments: usize, level_size_groups: u32, group_seed: GetGroupSeed) -> Box<[u64]>
        where GetGroupSeed: Fn(u32) -> u16 + Sync + Copy  // returns group seed for group with given index
    {
        let mut result = vec![0u64; level_size_segments].into_boxed_slice();
        let result_atom = AtomicU64::from_mut_slice(&mut result);
        let mut collision: Box<[AtomicU64]> = (0..level_size_segments).map(|_| AtomicU64::default()).collect();
        key_hashes.par_iter().for_each(
            |hash| fphash_sync_add_bit(&result_atom, &collision, self.hash_index(*hash, level_size_groups, group_seed))
        );
        fphash_remove_collided(&mut result, AtomicU64::get_mut_slice(&mut collision));
        result
    }

    #[inline] fn build_array_for_hashes_maybe_mt<GetGroupSeed>(&self, key_hashes: &[u64], level_size_segments: usize, level_size_groups: u32, group_seed: GetGroupSeed, use_multiple_threads: bool) -> Box<[u64]>
        where GetGroupSeed: Fn(u32) -> u16 + Sync + Copy  // returns group seed for group with given index
    {
        if use_multiple_threads {
            self.build_array_for_hashes_mt(key_hashes, level_size_segments, level_size_groups, group_seed)
        } else {
            self.build_array_for_hashes(key_hashes, level_size_segments, level_size_groups, group_seed)
        }
    }
}

enum Seeds<ArrayValue, SSVecElement> {
    None,
    Single(Box<[ArrayValue]>, u16),
    PerGroup(Box<[ArrayValue]>, Box<[SSVecElement]>)
}

/// Helper structure for building fingerprinting-based minimal perfect hash function with group optimization (FMPHGO).
#[derive(Clone)]
pub struct FPHash2Builder<GS: GroupSize = TwoToPowerBits, SS: SeedSize = TwoToPowerBitsStatic<2>, S = BuildDefaultSeededHasher> {
    level_sizes: Vec::<u32>,
    arrays: Vec::<Box<[u64]>>,
    group_seeds: Vec::<Box<[SS::VecElement]>>,
    prehash_threshold: usize,   // maximum keys size to pre-hash
    relative_level_size: u16,
    use_multiple_threads: bool,
    conf: FPHash2Conf<GS, SS, S>,
}   // TODO introduce trait to make other builders possible

impl Default for FPHash2Builder {
    fn default() -> Self { Self::new(Default::default()) }
}

impl<GS: GroupSize + Sync, SS: SeedSize, S: BuildSeededHasher + Sync> FPHash2Builder<GS, SS, S>
{
    const DEFAULT_RELATIVE_LEVEL_SIZE: u16 = 100;
    const DEFAULT_PREHASH_THRESHOLD: usize = 1024*1024*128; // *8 bytes = max 1GB for pre-hashing

    fn with_lsize_pht_mt(conf: FPHash2Conf<GS, SS, S>, relative_level_size: u16, prehash_threshold: usize, use_multiple_threads: bool) -> Self {
        Self {
            level_sizes: Vec::<u32>::new(),
            arrays: Vec::<Box<[u64]>>::new(),
            group_seeds: Vec::<Box<[SS::VecElement]>>::new(),
            prehash_threshold,
            relative_level_size,
            use_multiple_threads: use_multiple_threads && rayon::current_num_threads() > 1,
            conf
        }
    }

    fn new(conf: FPHash2Conf<GS, SS, S>) -> Self {
        Self::with_lsize_pht_mt(conf, Self::DEFAULT_RELATIVE_LEVEL_SIZE, Self::DEFAULT_PREHASH_THRESHOLD, true)
    }

    /// Returns builder that uses at each level a bit-array of size `relative_level_size`
    /// given as a percent of number of input keys for the level.
    pub fn with_lsize(conf: FPHash2Conf<GS, SS, S>, relative_level_size: u16) -> Self {
        Self::with_lsize_pht_mt(conf, relative_level_size, Self::DEFAULT_PREHASH_THRESHOLD, true)
    }

    /// Returns builder that potentially uses multiple threads to build levels,
    /// and at each level a bit-array of size `relative_level_size`
    /// given as a percent of number of input keys for the level.
    pub fn with_lsize_mt(conf: FPHash2Conf<GS, SS, S>, relative_level_size: u16, use_multiple_threads: bool) -> Self {
        Self::with_lsize_pht_mt(conf, relative_level_size, Self::DEFAULT_PREHASH_THRESHOLD, use_multiple_threads)
    }

    fn push(&mut self, array: Box<[u64]>, seeds: Box<[SS::VecElement]>, size_groups: u32) {
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
    fn build_array<GetGroupSeed, KS, K>(&self, keys: &KS, level_size_segments: usize, level_size_groups: u32, group_seed: GetGroupSeed) -> Box<[u64]>
        where GetGroupSeed: Fn(u32) -> u16 + Copy,  // returns group seed for group with given index
            K: Hash, KS: KeySet<K>
    {
        let mut result = vec![0u64; level_size_segments].into_boxed_slice();
        let mut collision = vec![0u64; level_size_segments].into_boxed_slice();
        let level_seed = self.level_nr();
        keys.for_each_key(|key| fphash_add_bit(&mut result, &mut collision, self.conf.key_index(key, level_seed, level_size_groups, group_seed)),
                          |key| self.retained(key));
        fphash_remove_collided(&mut result, &collision);
        result
    }

    /// Returns fingerprint array for given hashes of keys, level size, and group seeds (given as a function that returns seeds for provided group indices).
    fn build_array_mt<GetGroupSeed, KS, K>(&self, keys: &KS, level_size_segments: usize, level_size_groups: u32, group_seed: GetGroupSeed) -> Box<[u64]>
        where GetGroupSeed: Fn(u32) -> u16 + Sync + Copy, K: Hash, KS: KeySet<K>  // returns group seed for group with given index
    {
        if !self.use_multiple_threads || !keys.has_par_for_each_key() {
            return self.build_array(keys, level_size_segments, level_size_groups, group_seed);
        }
        let mut result = vec![0u64; level_size_segments].into_boxed_slice();
        let result_atom = AtomicU64::from_mut_slice(&mut result);
        let mut collision: Box<[AtomicU64]> = (0..level_size_segments).map(|_| AtomicU64::default()).collect();
        let level_seed = self.level_nr();
        keys.par_for_each_key(
            |key| fphash_sync_add_bit(&result_atom, &collision, self.conf.key_index(key, level_seed, level_size_groups, group_seed)),
            |key| self.retained(key)
        );
        fphash_remove_collided(&mut result, AtomicU64::get_mut_slice(&mut collision));
        result
    }

    fn update_best<GetGroupSeed>(&self, level_size_groups: u32, best_array: &mut [u64], best_seeds: &mut [SS::VecElement], array: &[u64], array_seed: GetGroupSeed)
        where GetGroupSeed: Fn(u32) -> u16
    {
        for group_index in 0..level_size_groups {
            self.conf.bits_per_group.conditionally_copy_group(best_array, array, group_index as usize,
            |best, new|
                if best.count_ones() < new.count_ones() {
                    self.conf.bits_per_seed.set_seed(best_seeds, group_index as usize, array_seed(group_index));
                    true
                } else { false }
            )
        }
    }

    fn updated_best(&self, level_size_groups: u32, best: Seeds<u64, SS::VecElement>, array2: Box<[u64]>, seed2: u16) -> Seeds<u64, SS::VecElement>
    {
        match best {
            Seeds::None => Seeds::Single(array2, seed2),
            Seeds::PerGroup(mut array1, mut seeds1) => {
                self.update_best(level_size_groups, &mut array1, &mut seeds1, &array2, |_| seed2);
                Seeds::PerGroup(array1, seeds1)
            },
            Seeds::Single(mut array1, seed1) => {
                let mut seeds1 = self.conf.bits_per_seed.new_seed_vec(seed1, level_size_groups as usize);
                self.update_best(level_size_groups, &mut array1, &mut seeds1, &array2, |_| seed2);
                Seeds::PerGroup(array1, seeds1)
            }
        }
    }

    fn select_best_seeds(&self, level_size_groups: u32, s1: Seeds<u64, SS::VecElement>, s2: Seeds<u64, SS::VecElement>)
        -> Seeds<u64, SS::VecElement>
    {
        match (s1, s2) {
            (Seeds::PerGroup(mut array1, mut seeds1), Seeds::PerGroup(array2, seeds2)) => {
                self.update_best(level_size_groups, &mut array1, &mut seeds1, &array2,
                                 |g| self.conf.bits_per_seed.get_seed(&seeds2, g as usize));
                Seeds::PerGroup(array1, seeds1)
            },
            (s1, Seeds::Single(array2, seed2)) => self.updated_best(level_size_groups, s1, array2, seed2),
            (Seeds::Single(array1, seed1), s2) => self.updated_best(level_size_groups, s2, array1, seed1),
            (s1, Seeds::None) => s1,
            (Seeds::None, s2) => s2
        }
    }

    /// Update `best_seeds` and their numbers of collisions `best_counts`.
    fn update_best_seeds(best_seeds: &mut [SS::VecElement], best_counts: &mut [u8], array: &[u64], array_seed: u16, conf: &FPHash2Conf<GS, SS, S>)
    {
        for group_index in 0..best_counts.len() {
            let new = conf.bits_per_group.ones_in_group(&array, group_index);
            let best = &mut best_counts[group_index];
            if new > *best {
                *best = new;
                conf.bits_per_seed.set_seed(best_seeds, group_index, array_seed);
                //best_seeds.set_fragment(group_index, array_seed as u64, conf.bits_per_group_seed);
            }
        }
    }

    fn update_best_seeds_counts(&self, level_size_groups: usize, best_seeds: &mut [SS::VecElement], best_counts: &mut [u8], new_seeds: &[SS::VecElement], new_counts: &[u8]) {
        for group_index in 0..level_size_groups {
            let best_count = &mut best_counts[group_index];
            let new_count = new_counts[group_index];
            if new_count > *best_count {
                *best_count = new_count;
                self.conf.bits_per_seed.set_seed(best_seeds, group_index,
                                                 self.conf.bits_per_seed.get_seed(&new_seeds, group_index))
            }
        }
    }

    /// Select optimal group seeds for the given `keys` and level size.
    fn select_seeds_prehashed_counts_st(&self, key_hashes: &[u64], level_size_groups: usize, level_size_segments: usize) -> Box<[SS::VecElement]>
    {
        let last_seed = self.last_seed();
        let mut best_counts: Box<[u8]> = {
            let arr = self.conf.build_array_for_hashes(&key_hashes, level_size_segments, level_size_groups as u32, |_| 0);
            (0..level_size_groups).into_iter().map(|group_index| self.conf.bits_per_group.ones_in_group(&arr, group_index)).collect()
        };
        let mut best_seeds = self.conf.bits_per_seed.new_zeroed_seed_vec(level_size_groups);
        for group_seed in 1..=last_seed {
            let with_new_seed = self.conf.build_array_for_hashes(&key_hashes, level_size_segments, level_size_groups as u32, |_| group_seed);
            Self::update_best_seeds(&mut best_seeds, &mut best_counts, &with_new_seed, group_seed, &self.conf);
        }
        best_seeds
    }

    /// Select optimal group seeds for the given `keys` and level size.
    fn select_seeds_prehashed_counts_mt(&self, key_hashes: &[u64], level_size_groups: usize, level_size_segments: usize) -> Box<[SS::VecElement]> {
        let last_seed = self.last_seed();
        (0..=last_seed).into_par_iter().fold(|| None, |mut best: Option<(Box<[SS::VecElement]>, Box<[u8]>)>, seed| {
            let array = self.conf.build_array_for_hashes(&key_hashes, level_size_segments, level_size_groups as u32, |_| seed);
            if let Some((ref mut best_seeds, ref mut best_counts)) = best {
                Self::update_best_seeds(best_seeds, best_counts, &array, seed, &self.conf);
                best
            } else {
                Some((
                    self.conf.bits_per_seed.new_seed_vec(seed, level_size_groups),
                    (0..level_size_groups).into_iter().map(|group_index| self.conf.bits_per_group.ones_in_group(&array, group_index)).collect()
                ))
            }
        }).reduce_with(|mut best, new| {
            if let Some((ref mut best_seeds, ref mut best_counts)) = best {
                if let Some((new_seeds, new_counts)) = new {
                    self.update_best_seeds_counts(level_size_groups, &mut best_seeds[..], &mut best_counts[..], &new_seeds[..], &new_counts[..])
                }
                best
            } else { new }
        }).unwrap().unwrap().0
    }

    /// Select optimal group seeds for the given `keys` and level size.
    fn select_seeds_prehashed_counts_atomic_mt(&self, key_hashes: &[u64], level_size_groups: usize, level_size_segments: usize) -> Box<[SS::VecElement]> {
        let last_seed = self.last_seed();
        let mut best_counts: Box<[u8]> = {
            let arr = self.conf.build_array_for_hashes_mt(&key_hashes, level_size_segments, level_size_groups as u32, |_| 0);
            (0..level_size_groups).into_iter().map(|group_index| self.conf.bits_per_group.ones_in_group(&arr, group_index)).collect()
        };
        let mut best_seeds = self.conf.bits_per_seed.new_zeroed_seed_vec(level_size_groups);
        for group_seed in 1..=last_seed {
            let with_new_seed = self.conf.build_array_for_hashes_mt(&key_hashes, level_size_segments, level_size_groups as u32, |_| group_seed);
            Self::update_best_seeds(&mut best_seeds, &mut best_counts, &with_new_seed, group_seed, &self.conf);
        }
        best_seeds
    }

    /// Select optimal group seeds for the given `keys` and level size.
    fn select_seeds_counts<KS, K>(&self, keys: &KS, level_size_groups: usize, level_size_segments: usize) -> Box<[SS::VecElement]>
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        let last_seed = self.last_seed();
        if self.use_multiple_threads {
            (0..=last_seed).into_par_iter().fold(|| None, |mut best: Option<(Box<[SS::VecElement]>, Box<[u8]>)>, seed| {
                let array = self.build_array(keys, level_size_segments, level_size_groups as u32, |_| seed);
                if let Some((ref mut best_seeds, ref mut best_counts)) = best {
                    Self::update_best_seeds(best_seeds, best_counts, &array, seed, &self.conf);
                    best
                } else {
                    Some((
                        self.conf.bits_per_seed.new_seed_vec(seed, level_size_groups),
                        (0..level_size_groups).into_iter().map(|group_index| self.conf.bits_per_group.ones_in_group(&array, group_index)).collect()
                    ))
                }
            }).reduce_with(|mut best, new| {
                if let Some((ref mut best_seeds, ref mut best_counts)) = best {
                    if let Some((new_seeds, new_counts)) = new {
                        self.update_best_seeds_counts(level_size_groups, &mut best_seeds[..], &mut best_counts[..], &new_seeds[..], &new_counts[..])
                    }
                    best
                } else { new }
            }).unwrap().unwrap().0
        } else {
            let mut best_counts: Box<[u8]> = {
                let arr = self.build_array(keys, level_size_segments, level_size_groups as u32, |_| 0);
                (0..level_size_groups).into_iter().map(|group_index| self.conf.bits_per_group.ones_in_group(&arr, group_index)).collect()
            };
            let mut best_seeds = self.conf.bits_per_seed.new_zeroed_seed_vec(level_size_groups);
            for group_seed in 1..=last_seed {
                let with_new_seed = self.build_array(keys, level_size_segments, level_size_groups as u32, |_| group_seed);
                Self::update_best_seeds(&mut best_seeds, &mut best_counts, &with_new_seed, group_seed, &self.conf);
            }
            best_seeds
        }
    }

    fn build_next_level_prehash_counts<KS, K>(&mut self, keys: &mut KS, level_size_groups: usize, level_size_segments: usize)
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        let level_seed = self.level_nr();
        let key_hashes = keys.maybe_par_map_each_key(
            |k| self.conf.hash_builder.hash_one(k, level_seed),
            |key| self.retained(key),
            self.use_multiple_threads
        );
        let current_seeds = if !self.use_multiple_threads {
            self.select_seeds_prehashed_counts_st(&key_hashes, level_size_groups, level_size_segments)
        } else if level_size_segments >= (1<<17) { // 2^23 / 64
            self.select_seeds_prehashed_counts_atomic_mt(&key_hashes, level_size_groups, level_size_segments)
        } else {
            self.select_seeds_prehashed_counts_mt(&key_hashes, level_size_groups, level_size_segments)
        };
        let current_array = self.conf.build_array_for_hashes_maybe_mt(
            &key_hashes,
            level_size_segments, level_size_groups as u32,
            |group_index| self.conf.bits_per_seed.get_seed(&current_seeds, group_index as usize),
            self.use_multiple_threads
        );
        keys.maybe_par_retain_keys_with_indices(
            |i| !current_array.get_bit(
                self.conf.hash_index(key_hashes[i], level_size_groups as u32,
                                     |group| self.conf.bits_per_seed.get_seed(&current_seeds, group as usize))
            ),
            |key| !current_array.get_bit(
                self.conf.key_index(key, level_seed, level_size_groups as u32,
                                    |group| self.conf.bits_per_seed.get_seed(&current_seeds, group as usize))
            ),
            |key| self.retained(key),
            || current_array.iter().map(|v| v.count_ones() as usize).sum::<usize>(),
            self.use_multiple_threads
        );
        self.push(current_array, current_seeds, level_size_groups as u32);
    }

    fn build_next_level_counts<KS, K>(&mut self, keys: &mut KS, level_size_groups: usize, level_size_segments: usize)
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        let current_seeds = self.select_seeds_counts(keys, level_size_groups, level_size_segments);
        //let current_seeds = self.select_seeds_fewatonce_atomic_counts(keys, level_size_groups, level_size_segments);
        let current_array = self.build_array_mt(
            keys,
            level_size_segments, level_size_groups as u32,
            |group_index| self.conf.bits_per_seed.get_seed(&current_seeds, group_index as usize)
            //current_seeds.get_fragment(group_index as usize, conf.bits_per_group_seed) as u16
        );
        self.finish_level_building(keys, level_size_groups, current_array, current_seeds);
    }

    fn finish_level_building<KS, K>(&mut self, keys: &mut KS, level_size_groups: usize, current_array: Box<[u64]>, current_seeds: Box<[<SS as SeedSize>::VecElement]>)
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        let level_nr = self.level_nr();
        keys.maybe_par_retain_keys(
            |key| {
                let hash = self.conf.hash_builder.hash_one(key, level_nr);
                let group = group_nr(hash, level_size_groups as u32);
                let bit_index = self.conf.bits_per_group.bit_index_for_seed(
                    hash,
                    //current_seeds.get_fragment(group as usize, conf.bits_per_group_seed) as u16,
                    self.conf.bits_per_seed.get_seed(&current_seeds, group as usize),
                    group);
                !current_array.get_bit(bit_index)
            },
            |key| self.retained(key),
            || current_array.iter().map(|v| v.count_ones() as usize).sum::<usize>(),
            self.use_multiple_threads
        );
        self.push(current_array, current_seeds, level_size_groups as u32);
    }


    /// Builds levels for all `group_seeds` given using single thread.
    /// Returns the concatenation of the levels built.
    fn build_levels_st<K>(&self, keys: &impl KeySet<K>, level_size_segments: usize, level_size_groups: u32, group_seeds: RangeInclusive<u16>) -> Box<[u64]>
        where K: Hash + Sync
    {
        let total_array_size = level_size_segments * group_seeds.len();
        let mut result = vec![0u64; total_array_size].into_boxed_slice();
        let mut collision = vec![0u64; total_array_size].into_boxed_slice();
        let level_size = level_size_segments * 64;
        let seed = self.level_nr();
        keys.for_each_key(
            |key| {
                let hash = self.conf.hash_builder.hash_one(key, seed);
                let group = group_nr(hash, level_size_groups);
                let mut delta = 0;
                for group_seed in group_seeds.clone() {
                    let bit = self.conf.bits_per_group.bit_index_for_seed(hash, group_seed, group);
                    fphash_add_bit(&mut result, &mut collision, delta + bit);
                    delta += level_size;
                }
            },
            |key| self.retained(key)
        );
        fphash_remove_collided(&mut result, &mut collision);
        result
    }

    /// Builds levels for all `group_seeds` given using multiple threads.
    /// Returns the concatenation of the levels built.
    fn build_levels_atomic_mt<K>(&self, keys: &impl KeySet<K>, level_size_segments: usize, level_size_groups: u32, group_seeds: RangeInclusive<u16>) -> Box<[u64]>
        where K: Hash + Sync
    {
        let total_array_size = level_size_segments * group_seeds.len();
        let mut result = vec![0u64; total_array_size].into_boxed_slice();
        let result_atom = AtomicU64::from_mut_slice(&mut result);
        let mut collision: Box<[AtomicU64]> = (0..total_array_size).map(|_| AtomicU64::default()).collect();
        let level_size = level_size_segments * 64;
        let seed = self.level_nr();
        keys.par_for_each_key(
            |key| {
                let hash = self.conf.hash_builder.hash_one(key, seed);
                let group = group_nr(hash, level_size_groups);
                let mut delta = 0;
                for group_seed in group_seeds.clone() {
                    let bit = self.conf.bits_per_group.bit_index_for_seed(hash, group_seed, group);
                    fphash_sync_add_bit(&result_atom, &collision, delta + bit);
                    delta += level_size;
                }
            },
            |key| self.retained(key)
        );
        fphash_remove_collided(&mut result, AtomicU64::get_mut_slice(&mut collision));
        result
    }

    /// Select optimal group seeds for the given `keys` and level size.
    fn select_seeds_fewatonce_atomic_counts<KS, K>(&self, keys: &KS, level_size_groups: usize, level_size_segments: usize) -> Box<[SS::VecElement]>
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        let seeds_slice_size = 4;
        let last_seed = self.last_seed();
        if self.use_multiple_threads {
            let mut result = None;
            let mut seed_first = 0;
            loop {
                let seed_last = ((seed_first as u32) + seeds_slice_size - 1).min(last_seed as u32) as u16;
                let levels = self.build_levels_atomic_mt(keys, level_size_segments, level_size_groups as u32, seed_first..=seed_last);
                result = {
                    levels.par_chunks(level_size_segments).enumerate().fold(|| None, |mut best: Option<(Box<[SS::VecElement]>, Box<[u8]>)>, (in_seed, array)| {
                        let seed = seed_first + in_seed as u16;
                        if let Some((ref mut best_seeds, ref mut best_counts)) = best {
                            Self::update_best_seeds(best_seeds, best_counts, &array, seed, &self.conf);
                            best
                        } else {
                            Some((
                                self.conf.bits_per_seed.new_seed_vec(seed, level_size_groups),
                                (0..level_size_groups).into_iter().map(|group_index| self.conf.bits_per_group.ones_in_group(&array, group_index)).collect()
                            ))
                        }
                    }).chain([result]).reduce_with(|mut best, new| {
                        if let Some((ref mut best_seeds, ref mut best_counts)) = best {
                            if let Some((new_seeds, new_counts)) = new {
                                self.update_best_seeds_counts(level_size_groups, &mut best_seeds[..], &mut best_counts[..], &new_seeds[..], &new_counts[..])
                            }
                            best
                        } else { new }
                    }).unwrap()
                };
                if seed_last == last_seed { break }
                seed_first = seed_last + 1;
            }
            result.unwrap().0
        } else {
            let mut seed_first = 0;
            let mut best_seeds = self.conf.bits_per_seed.new_zeroed_seed_vec(level_size_groups);
            let mut best_counts: Option<Box<[u8]>> = None;
            loop {
                let seed_last = ((seed_first as u32) + seeds_slice_size - 1).min(last_seed as u32) as u16;
                let levels = self.build_levels_st(keys, level_size_segments, level_size_groups as u32, seed_first..=seed_last);
                let mut delta_beg = 0;
                for group_seed in seed_first..=seed_last {
                    let delta_end = delta_beg + level_size_segments;
                    if let Some(ref mut best_counts) = best_counts {
                        Self::update_best_seeds(&mut best_seeds, best_counts, &levels[delta_beg..delta_end], group_seed, &self.conf);
                    } else {
                        best_counts = Some(
                            (0..level_size_groups).into_iter().map(|group_index| self.conf.bits_per_group.ones_in_group(&levels, group_index)).collect()
                        );
                    }
                    delta_beg = delta_end;
                }
                if seed_last == last_seed { break }
                seed_first = seed_last + 1;
            }
            best_seeds
        }
    }

    /// Builds levels for all `group_seeds` given and merge them, using a single thread.
    fn best_level_fewatonce<K>(&self, prev_best: Option<(Box<[u64]>, Box<[SS::VecElement]>)>, keys: &impl KeySet<K>, level_size_segments: usize, level_size_groups: u32, group_seeds: RangeInclusive<u16>) -> (Box<[u64]>, Box<[SS::VecElement]>)
        where K: Hash + Sync
    {
        let levels_count = group_seeds.len();
        let mut results = vec![vec![0u64; level_size_segments].into_boxed_slice(); levels_count];
        let mut collision = vec![0u64; level_size_segments * levels_count].into_boxed_slice();
        keys.for_each_key(
            |key| {
                let hash = self.conf.hash_builder.hash_one(key, self.level_nr());
                let group = group_nr(hash, level_size_groups);
                let mut delta = 0;
                for (result, group_seed) in results.iter_mut().zip(group_seeds.clone()) {
                    let bit = self.conf.bits_per_group.bit_index_for_seed(hash, group_seed, group);
                    fphash_add_bit(result, &mut collision[delta..], bit);
                    delta += level_size_segments;
                }
            },
            |key| self.retained(key)
        );
        let mut delta = 0;
        for mut result in results.iter_mut() {
            fphash_remove_collided(&mut result, &mut collision[delta..]);
            delta += level_size_segments;
        }
        drop(collision);
        // TODO optimization to marge first candidate with best at the beginning, and construct best_seeds
        if let Some((mut best, mut best_seeds)) = prev_best {
            for (array, array_seed) in results.iter().zip(group_seeds) {
                self.update_best(level_size_groups, &mut best, &mut best_seeds, array, |_| array_seed);
            }
            (best, best_seeds)
        } else {
            let mut best_seeds = self.conf.bits_per_seed.new_seed_vec(*group_seeds.end(), level_size_groups as usize);
            let (mut best, candidates) = results.split_last_mut().unwrap();
            for (array, array_seed) in candidates.iter().zip(group_seeds) {
                self.update_best(level_size_groups, &mut best, &mut best_seeds, array, |_| array_seed);
            }
            (results.pop().unwrap(), best_seeds)
        }
    }

    fn build_next_level_fewatonce<KS, K>(&mut self, keys: &mut KS, level_size_groups: usize, level_size_segments: usize)
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        let last_seed = (1u32 << self.conf.bits_per_seed.into())-1;
        let seeds_slice_size = 2;
        let last_seed_slice = last_seed / seeds_slice_size;
        let (array, seeds) = if self.use_multiple_threads {
            (0..=last_seed_slice).into_par_iter().fold(|| None, |best: Option<(Box<[u64]>, Box<[SS::VecElement]>)>, seeds_slice| {
                let first_seed = seeds_slice * seeds_slice_size;
                Some(self.best_level_fewatonce(best, keys, level_size_segments, level_size_groups as u32,
                                               first_seed as u16..=(first_seed+seeds_slice_size-1).min(last_seed) as u16))
            }).reduce_with(|mut best, new| {
                if let Some((ref mut best_arr, ref mut best_seeds)) = best {
                    if let Some((new, new_seeds)) = new {
                        self.update_best(level_size_groups as u32, &mut best_arr[..], &mut best_seeds[..], &new[..],
                                         |g| self.conf.bits_per_seed.get_seed(&new_seeds, g as usize))
                    }
                    best
                } else { new }
            }).unwrap().unwrap()
        } else {
            let mut best = None;
            for first_seed in (0..=last_seed).step_by(seeds_slice_size as usize) {
                best = Some(self.best_level_fewatonce(best, keys, level_size_segments, level_size_groups as u32,
                                                      first_seed as u16..=(first_seed+seeds_slice_size-1).min(last_seed) as u16))
            }
            best.unwrap()
        };
        self.finish_level_building(keys, level_size_groups, array, seeds);
    }

    fn best_array_st<KS, K>(&mut self, keys: &mut KS, level_size_groups: usize, level_size_segments: usize) -> (Box<[u64]>, Box<[SS::VecElement]>)
        where K: Hash, KS: KeySet<K>
    {
        let mut best_array = self.build_array(keys, level_size_segments, level_size_groups as u32, |_| 0);
        let mut best_seeds = self.conf.bits_per_seed.new_zeroed_seed_vec(level_size_groups);
        for group_seed in 1..=self.last_seed() {
            let with_new_seed = self.build_array(keys, level_size_segments, level_size_groups as u32, |_| group_seed);
            self.update_best(level_size_groups as u32, &mut best_array, &mut best_seeds, &with_new_seed, |_| group_seed);
        }
        (best_array, best_seeds)
    }

    fn best_array_mt<KS, K>(&mut self, keys: &mut KS, level_size_groups: usize, level_size_segments: usize) -> (Box<[u64]>, Box<[SS::VecElement]>)
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        let s = (0..=self.last_seed()).into_par_iter().fold(|| Seeds::None::<u64, SS::VecElement>, |best, seed| {
            let new_arr = self.build_array(keys, level_size_segments, level_size_groups as u32, |_| seed);
            self.updated_best(level_size_groups as u32, best, new_arr, seed)
        }).reduce_with(|best, new| {
            self.select_best_seeds(level_size_groups as u32, best, new)
        }).unwrap();
        match s {
            Seeds::Single(a, seed) => (a, self.conf.bits_per_seed.new_seed_vec(seed, level_size_groups)),
            Seeds::PerGroup(a, seeds) => (a, seeds),
            Seeds::None => unreachable!()
        }
    }

    fn best_array_mt_atomic<KS, K>(&mut self, keys: &mut KS, level_size_groups: usize, level_size_segments: usize) -> (Box<[u64]>, Box<[SS::VecElement]>)
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        let mut best_array = self.build_array_mt(keys, level_size_segments, level_size_groups as u32, |_| 0);
        let mut best_seeds = self.conf.bits_per_seed.new_zeroed_seed_vec(level_size_groups);
        for group_seed in 1..=self.last_seed() {
            let with_new_seed = self.build_array_mt(keys, level_size_segments, level_size_groups as u32, |_| group_seed);
            self.update_best(level_size_groups as u32, &mut best_array, &mut best_seeds, &with_new_seed, |_| group_seed);
        }
        (best_array, best_seeds)
    }

    fn build_next_level<KS, K>(&mut self, keys: &mut KS, level_size_groups: usize, level_size_segments: usize)
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        //if keys.keys_len() < self.prehash_threshold {
         //   return self.build_next_level_prehash_counts(keys, level_size_groups, level_size_segments);
        //}
        let (array, seeds) = if !self.use_multiple_threads {
            self.best_array_st(keys, level_size_groups, level_size_segments)
        } else if level_size_segments >= (1<<17) {
            self.best_array_mt_atomic(keys, level_size_groups, level_size_segments)
        } else {
            self.best_array_mt(keys, level_size_groups, level_size_segments)
        };
        self.finish_level_building(keys, level_size_groups, array, seeds);
    }
}

/// Fingerprinting-based minimal perfect hash function with group optimization (FMPHGO).
///
/// See:
/// - P. Beling, *Fingerprinting-based minimal perfect hashing revisited*
pub struct FPHash2<GS: GroupSize = TwoToPowerBits, SS: SeedSize = TwoToPowerBitsStatic<2>, S = BuildDefaultSeededHasher> {
    array: ArrayWithRank,
    group_seeds: Box<[SS::VecElement]>,   //  Box<[u8]>,
    level_size: Box<[u32]>, // number of groups
    conf: FPHash2Conf<GS, SS, S>
    // 0..01..1 mask with number of ones = group size (in bits)
    //group_size_mask: u8,
}

impl<GS: GroupSize, SS: SeedSize, S: BuildSeededHasher> GetSize for FPHash2<GS, SS, S> {
    fn size_bytes_dyn(&self) -> usize {
        self.array.size_bytes_dyn()
            //+ self.seeds.len() * std::mem::size_of::<u8>()
            + self.group_seeds.size_bytes_dyn()
            + self.level_size.size_bytes_dyn()
    }

    const USES_DYN_MEM: bool = true;
}

impl<GS: GroupSize + Sync, SS: SeedSize, S: BuildSeededHasher + Sync> FPHash2<GS, SS, S> {

    /// Gets the value associated with the given `key` and reports statistics to `access_stats`.
    pub fn get_stats<K: Hash, A: stats::AccessStatsCollector>(&self, key: &K, access_stats: &mut A) -> Option<u64> {
        let mut groups_before = 0u32;
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
    #[inline] pub fn get<K: Hash>(&self, key: &K) -> Option<u64> {
        self.get_stats(key, &mut ())
    }

    /// Builds `FPHash2` for given `keys`, using the configuration `conf` and reporting statistics to `stats`.
    pub fn with_builder_stats<K, KS, BS>(mut keys: KS, mut levels: FPHash2Builder<GS, SS, S>, stats: &mut BS) -> Self
        where K: Hash + Sync, KS: KeySet<K> + Sync, BS: stats::BuildStatsCollector
    {
        while keys.keys_len() != 0 {
            let input_size = keys.keys_len();
            let (level_size_groups, level_size_segments) = levels.conf.bits_per_group.level_size_groups_segments(
                ceiling_div(input_size * levels.relative_level_size as usize, 100));
            //let seed = level_nr;
            stats.level(input_size, level_size_segments * 64);
            levels.build_next_level(&mut keys, level_size_groups, level_size_segments);
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

    pub fn with_builder<K, KS>(keys: KS, levels: FPHash2Builder<GS, SS, S>) -> Self
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        Self::with_builder_stats(keys, levels, &mut ())
    }

    pub fn with_conf_stats<K, KS, BS>(keys: KS, conf: FPHash2Conf<GS, SS, S>, stats: &mut BS) -> Self
        where K: Hash + Sync, KS: KeySet<K> + Sync, BS: stats::BuildStatsCollector
    {
        Self::with_builder_stats(keys, FPHash2Builder::new(conf), stats)
    }

    pub fn with_conf<K, KS>(keys: KS, conf: FPHash2Conf<GS, SS, S>) -> Self
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        Self::with_conf_stats(keys, conf, &mut ())
    }

    /// Builds `FPHash2` for given `keys`, using the configuration `conf`.
    #[inline] pub fn from_slice_with_conf_stats<K, BS>(keys: &[K], conf: FPHash2Conf<GS, SS, S>, stats: &mut BS) -> Self
        where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        Self::with_conf_stats(SliceSourceWithRefs::new(keys), conf, stats)
    }

    /// Builds `FPHash2` for given `keys`, using the configuration `conf`.
    #[inline] pub fn from_slice_with_conf<K>(keys: &[K], conf: FPHash2Conf<GS, SS, S>) -> Self
        where K: Hash + Sync
    {
        Self::with_conf_stats(SliceSourceWithRefs::new(keys), conf, &mut ())
    }

    /// Builds `FPHash2` for given `keys`, using the configuration `conf`.
    #[inline] pub fn from_slice_mut_with_conf_stats<K, BS>(keys: &mut [K], conf: FPHash2Conf<GS, SS, S>, stats: &mut BS) -> Self
        where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        Self::with_conf_stats(SliceMutSource::new(keys), conf, stats)
    }

    /// Builds `FPHash2` for given `keys`, using the configuration `conf`.
    #[inline] pub fn from_slice_mut_with_conf<K>(keys: &mut [K], conf: FPHash2Conf<GS, SS, S>) -> Self
        where K: Hash + Sync
    {
        Self::with_conf_stats(SliceMutSource::new(keys), conf, &mut ())
    }

    /// Returns number of bytes which `write` will write.
    pub fn write_bytes(&self) -> usize {
        std::mem::size_of::<u8>()   // bits_per_group_seed
            + self.conf.bits_per_group.write_size_bytes()
            + std::mem::size_of::<u32>()    // self.level_size.len()
            + self.level_size.size_bytes_dyn()
            + self.array.content.size_bytes_dyn()
            + self.group_seeds.size_bytes_dyn()
    }

    /// Writes `self` to the `output`.
    pub fn write(&self, output: &mut dyn io::Write) -> io::Result<()>
    {
        self.conf.bits_per_group.write(output)?;
        write_int!(output, self.level_size.len() as u32)?;
        self.level_size.iter().try_for_each(|l| { write_int!(output, l) })?;
        self.array.content.iter().try_for_each(|v| write_int!(output, v))?;
        self.conf.bits_per_seed.write_seed_vec(output, &self.group_seeds)
    }

    /// Reads `Self` from the `input`. Hasher must be the same as the one used to write.
    pub fn read_with_hasher(input: &mut dyn io::Read, hasher: S) -> io::Result<Self>
    {
        let bits_per_group = GS::read(input)?;
        let level_size = read_array!([u32; read u32] from input).into_boxed_slice();
        let number_of_groups = level_size.iter().map(|v|*v as usize).sum::<usize>();

        let array_content = read_array!(bits_per_group * number_of_groups; bits from input).into_boxed_slice();
        let (array_with_rank, _) = ArrayWithRank::build(array_content);

        let (bits_per_group_seed, group_seeds) = SS::read_seed_vec(input, number_of_groups)?;

        Ok(Self {
            array: array_with_rank,
            group_seeds,
            level_size,
            conf: FPHash2Conf {
                bits_per_seed: bits_per_group_seed,
                bits_per_group,
                hash_builder: hasher
            },
        })
    }

    pub fn level_sizes(&self) -> &[u32] {
        &self.level_size
    }
}

impl<GS: GroupSize + Sync, SS: SeedSize> FPHash2<GS, SS> {
    /// Reads `Self` from the `input`.
    /// Only `FPHash2`s that use default hasher can be read by this method.
    pub fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Self::read_with_hasher(input, Default::default())
    }
}

impl FPHash2 {
    /// Builds `FPHash2` for given `keys`, reporting statistics to `stats`.
    pub fn from_slice_with_stats<K, BS>(keys: &[K], stats: &mut BS) -> Self
        where K: Hash + Sync, BS: stats::BuildStatsCollector
    {
        Self::from_slice_with_conf_stats(keys, Default::default(), stats)
    }

    /// Builds `FPHash2` for given `keys`.
    pub fn from_slice<K: Hash + Sync>(keys: &[K]) -> Self {
        Self::from_slice_with_conf_stats(keys, Default::default(), &mut ())
    }
}

impl<K: Hash + Clone + Sync> From<&[K]> for FPHash2 {
    fn from(keys: &[K]) -> Self {
        Self::from_slice(&mut keys.to_owned())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_mphf;
    use std::fmt::{Debug, Display};
    use crate::fp::Bits;

    fn test_read_write<GS: GroupSize + Sync, SS: SeedSize>(h: &FPHash2<GS, SS>)
        where SS::VecElement: std::cmp::PartialEq + Debug
    {
        let mut buff = Vec::new();
        h.write(&mut buff).unwrap();
        assert_eq!(buff.len(), h.write_bytes());
        let read = FPHash2::<GS, SS>::read(&mut &buff[..]).unwrap();
        assert_eq!(h.level_size, read.level_size);
        assert_eq!(h.array.content, read.array.content);
        assert_eq!(h.group_seeds, read.group_seeds);
    }

    fn test_hash2_invariants<GS: GroupSize, SS: SeedSize>(h: &FPHash2<GS, SS>) {
        let number_of_groups = h.level_size.iter().map(|v| *v as usize).sum::<usize>();
        assert_eq!(h.bits_per_group * number_of_groups, h.array.content.len() * 64);
        assert_eq!(ceiling_div(number_of_groups * h.bits_per_seed.into() as usize, 64), h.group_seeds.len());
    }

    fn test_with_input<K: Hash + Clone + Display + Sync>(to_hash: &[K], bits_per_group: impl GroupSize + Sync) {
        let conf = FPHash2Conf::bps_bpg(Bits(3), bits_per_group);
        let h = FPHash2::from_slice_with_conf(&mut to_hash.to_vec(), conf);
        //dbg!(h.size_bytes() as f64 * 8.0/to_hash.len() as f64);
        test_mphf(to_hash, |key| h.get(key).map(|i| i as usize));
        test_hash2_invariants(&h);
        test_read_write(&h);
    }

    #[test]
    fn test_small_powers_of_two() {
        test_with_input(&[1, 2, 5], TwoToPowerBits::new(7));
        test_with_input(&[1, 2, 5], TwoToPowerBits::new(6));
        test_with_input(&[1, 2, 5], TwoToPowerBits::new(5));
        test_with_input(&[1, 2, 5], TwoToPowerBits::new(4));
        test_with_input(&[1, 2, 5], TwoToPowerBits::new(3));
        test_with_input(&[1, 2, 5], TwoToPowerBits::new(2));
        test_with_input(&[1, 2, 5], TwoToPowerBits::new(1));
        test_with_input(&[1, 2, 5], TwoToPowerBits::new(0));
        test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(7));
        test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(6));
        test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(5));
        test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(4));
        test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(3));
        test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(2));
        test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(1));
        test_with_input(&(-50..150).collect::<Vec<_>>(), TwoToPowerBits::new(0));
        test_with_input(&['a', 'b', 'c', 'd'], TwoToPowerBits::new(7));
        test_with_input(&['a', 'b', 'c', 'd'], TwoToPowerBits::new(6));
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
        test_with_input(&keys, TwoToPowerBits::new(7));
        test_with_input(&keys, TwoToPowerBits::new(6));
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