use binout::{write_int, read_int};
use std::hash::Hash;
use bitm::{BitAccess, BitArrayWithRank, ceiling_div};

use crate::utils::{ArrayWithRank, threads_count};
use crate::{BuildDefaultSeededHasher, BuildSeededHasher, stats};

use crate::read_array;
use super::indexing2::{GroupSize, SeedSize, TwoToPowerBits, TwoToPowerBitsStatic};
use std::io;
use std::num::NonZeroUsize;
use std::ops::{Range, RangeInclusive};
use std::sync::atomic::AtomicU64;
use dyn_size_of::GetSize;
use crate::fp::hash::{fphash_add_bit, fphash_remove_collided, fphash_sync_add_bit};
use crate::fp::indexing2::group_nr;

use rayon::prelude::*;
use rayon::ThreadPool;
use crate::fp::keyset::{KeySet, SliceMutSource, SliceSourceWithRefs};

/// Configuration that is accepted by `FPHash2` constructors.
#[derive(Clone)]
pub struct FPHash2Conf<GS: GroupSize = TwoToPowerBits, SS: SeedSize = TwoToPowerBitsStatic<2>, S = BuildDefaultSeededHasher> {
    hash_builder: S,
    bits_per_seed: SS,
    bits_per_group: GS,
    relative_level_size: u16,
    threads_count: NonZeroUsize
}

impl Default for FPHash2Conf {
    fn default() -> Self {
        Self { hash_builder: Default::default(), bits_per_seed: Default::default(), bits_per_group: TwoToPowerBits::new(4), relative_level_size: 100, threads_count: threads_count(0) }
    }
}

impl FPHash2Conf {
    /// Returns configuration that uses at each level a bit-array of size `relative_level_size`
    /// given as a percent of number of input keys for the level.
    pub fn lsize(relative_level_size: u16) -> Self {
        Self { relative_level_size, ..Default::default() }
    }
    /// Returns configuration that uses given number of `threads` and
    /// at each level a bit-array of size `relative_level_size`
    /// given as a percent of number of input keys for the level.
    pub fn lsize_threads(relative_level_size: u16, threads: isize) -> Self {
        Self { relative_level_size, threads_count: threads_count(threads), ..Default::default() }
    }
}

impl<SS: SeedSize> FPHash2Conf<TwoToPowerBits, SS> {
    /// Returns configuration that uses seeds of size given in bits.
    pub fn bps(bits_per_seed: SS) -> Self {
        bits_per_seed.validate().unwrap();
        Self { hash_builder: Default::default(), bits_per_seed, bits_per_group: TwoToPowerBits::new(4), relative_level_size: 100, threads_count: threads_count(0)  }
    }
    pub fn bps_lsize(bits_per_seed: SS, relative_level_size: u16) -> Self {
        Self { relative_level_size, ..Self::bps(bits_per_seed) }  // 1<<6=64
    }
    pub fn bps_threads(bits_per_seed: SS, threads: isize) -> Self {
        Self { threads_count: threads_count(threads), ..Self::bps(bits_per_seed) }
    }
    pub fn bps_lsize_threads(bits_per_seed: SS, relative_level_size: u16, threads: isize) -> Self {
        Self { relative_level_size, threads_count: threads_count(threads), ..Self::bps(bits_per_seed) }
    }
}

impl<GS: GroupSize> FPHash2Conf<GS> {
    /// Returns configuration that uses groups of size given in bits.
    pub fn bpg(bits_per_group: GS) -> Self {
        bits_per_group.validate().unwrap();
        Self { hash_builder: Default::default(), bits_per_seed: Default::default(), bits_per_group, relative_level_size: 100, threads_count: threads_count(0) }
    }
    pub fn bpg_lsize(bits_per_group: GS, relative_level_size: u16) -> Self {
        //assert!(bits_per_group_log2 <= 8);
        Self { relative_level_size, ..Self::bpg(bits_per_group) }  // 1<<6=64
    }
}

impl<GS: GroupSize, SS: SeedSize> FPHash2Conf<GS, SS> {
    pub fn bps_bpg(bits_per_seed: SS, bits_per_group: GS) -> Self {
        bits_per_seed.validate().unwrap();
        bits_per_group.validate().unwrap();
        Self { hash_builder: Default::default(), bits_per_seed, bits_per_group, relative_level_size: 100, threads_count: threads_count(0) }
    }
    pub fn bps_bpg_lsize(bits_per_seed: SS, bits_per_group: GS, relative_level_size: u16) -> Self {
        Self { relative_level_size, ..Self::bps_bpg(bits_per_seed, bits_per_group) }  // 1<<6=64
    }
    pub fn bps_bpg_lsize_threads(bits_per_seed: SS, bits_per_group: GS, relative_level_size: u16, threads: isize) -> Self {
        Self { relative_level_size, threads_count: threads_count(threads), ..Self::bps_bpg(bits_per_seed, bits_per_group) }  // 1<<6=64
    }
}

impl<GS: GroupSize, S> FPHash2Conf<GS, TwoToPowerBitsStatic<2>, S> {
    pub fn hash_bpg(hash: S, bits_per_group: GS) -> Self {
        bits_per_group.validate().unwrap();
        Self { hash_builder: hash, bits_per_seed: Default::default(), bits_per_group, relative_level_size: 100, threads_count: threads_count(0) }
    }
    pub fn hash_bpg_lsize(hash: S, bits_per_group: GS, relative_level_size: u16) -> Self {
        //assert!(bits_per_group_log2 <= 8);
        Self { relative_level_size, ..Self::hash_bpg(hash, bits_per_group) }  // 1<<6=64
    }
}

impl<GS: GroupSize, SS: SeedSize, S> FPHash2Conf<GS, SS, S> {
    pub fn hash_bps_bpg(hash: S, bits_per_seed: SS, bits_per_group: GS) -> Self {
        bits_per_seed.validate().unwrap();
        bits_per_group.validate().unwrap();
        Self { hash_builder: hash, bits_per_seed, bits_per_group, relative_level_size: 100, threads_count: threads_count(0) }  // 1<<6=64
    }
    pub fn hash_bps_bpg_lsize(hash: S, bits_per_seed: SS, bits_per_group: GS, relative_level_size: u16) -> Self {
        Self { relative_level_size, ..Self::hash_bps_bpg(hash, bits_per_seed, bits_per_group) }  // 1<<6=64
    }
    pub fn hash_bps_bpg_lsize_threads(hash: S, bits_per_seed: SS, bits_per_group: GS, relative_level_size: u16, threads: isize) -> Self {
        Self { relative_level_size, threads_count: threads_count(threads), ..Self::hash_bps_bpg(hash, bits_per_seed, bits_per_group) }  // 1<<6=64
    }
}

/// Helper structure for building fingerprinting-based minimal perfect hash function with group optimization (FMPHGO).
struct FPHash2Builder<GS: GroupSize, SS: SeedSize, S> {
    input_size: usize,
    level_sizes: Vec::<u32>,
    arrays: Vec::<Box<[u64]>>,
    group_seeds: Vec::<Box<[SS::VecElement]>>,
    thread_pool: Option<ThreadPool>,
    conf: FPHash2Conf<GS, SS, S>,
}

impl<GS: GroupSize + Sync, SS: SeedSize, S: BuildSeededHasher + Sync> FPHash2Builder<GS, SS, S>
{
    fn new(input_size: usize, conf: FPHash2Conf<GS, SS, S>) -> Self
    {
        Self {
            input_size,
            level_sizes: Vec::<u32>::new(),
            arrays: Vec::<Box<[u64]>>::new(),
            group_seeds: Vec::<Box<[SS::VecElement]>>::new(),
            thread_pool: (conf.threads_count.get() > 1).then(|| rayon::ThreadPoolBuilder::new().num_threads(conf.threads_count.get()).build().unwrap()),
            conf
        }
    }

    fn push(&mut self, array: Box<[u64]>, seeds: Box<[SS::VecElement]>, size_groups: u32) {
        self.arrays.push(array);
        self.group_seeds.push(seeds);
        self.level_sizes.push(size_groups);
    }

    /// Returns number the level about to build (number of levels built so far).
    #[inline(always)] fn level_nr(&self) -> u32 { self.level_sizes.len() as u32 }

    /// Returns whether `key` is retained (`false` if it is already hashed at the levels built so far).
    fn retained<K>(&self, key: &K) -> bool where K: Hash + Sync {
        self.arrays.iter().zip(self.group_seeds.iter()).zip(self.level_sizes.iter()).enumerate()
            .all(|(level_nr, ((a, seeds), level_size_groups))| {
                let hash = self.conf.hash_builder.hash_one(key, level_nr as u32);
                let group = group_nr(hash, *level_size_groups);
                let group_seed = self.conf.bits_per_seed.get_seed(seeds, group as usize);
                !a.get_bit(self.conf.bits_per_group.bit_index_for_seed(hash, group_seed, group))
            })
    }

    /// Returns fingerprint array for given hashes of keys, level size, and group seeds (given as a function that returns seeds for provided group indices).
    fn build_array_for_hashes<GetGroupSeed>(&self, key_hashes: &[u64], level_size_segments: usize, level_size_groups: u32, group_seed: GetGroupSeed) -> Box<[u64]>
        where GetGroupSeed: Fn(u32) -> u16  // returns group seed for group with given index
    {
        let mut result = vec![0u64; level_size_segments].into_boxed_slice();
        let mut collision = vec![0u64; level_size_segments].into_boxed_slice();
        key_hashes.iter().for_each(|hash| {
            let group = group_nr(*hash, level_size_groups);
            let bit = self.conf.bits_per_group.bit_index_for_seed(*hash, group_seed(group), group);
            fphash_add_bit(&mut result, &mut collision, bit);
        });
        fphash_remove_collided(&mut result, &collision);
        result
    }

    /// Returns fingerprint array for given keys, level size, and group seeds (given as a function that returns seeds for provided group indices).
    fn build_array<GetGroupSeed, KS, K>(&self, keys: &KS, level_size_segments: usize, level_size_groups: u32, group_seed: GetGroupSeed) -> Box<[u64]>
        where GetGroupSeed: Fn(u32) -> u16,  // returns group seed for group with given index
            K: Hash + Sync, KS: KeySet<K> + Sync
    {
        let mut result = vec![0u64; level_size_segments].into_boxed_slice();
        let mut collision = vec![0u64; level_size_segments].into_boxed_slice();
        keys.for_each_key(|key| {
            let hash = self.conf.hash_builder.hash_one(key, self.level_nr());
            let group = group_nr(hash, level_size_groups);
            let bit = self.conf.bits_per_group.bit_index_for_seed(hash, group_seed(group), group);
            fphash_add_bit(&mut result, &mut collision, bit);
        }, |key| self.retained(key));
        fphash_remove_collided(&mut result, &collision);
        result
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
    fn select_seeds_prehashed(&self, key_hashes: &[u64], level_size_groups: usize, level_size_segments: usize) -> Box<[SS::VecElement]>
    {
        let last_seed = ((1u32 << self.conf.bits_per_seed.into())-1) as u16;
        if let Some(thread_pool) = &self.thread_pool {
            thread_pool.install(|| {
                (0..=last_seed).into_par_iter().fold(|| None, |mut best: Option<(Box<[SS::VecElement]>, Box<[u8]>)>, seed| {
                    let array = self.build_array_for_hashes(&key_hashes, level_size_segments, level_size_groups as u32, |_| seed);
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
                        best
                    } else { new }
                }).unwrap().unwrap().0
            })
        } else {
            let mut best_counts: Box<[u8]> = {
                let arr = self.build_array_for_hashes(&key_hashes, level_size_segments, level_size_groups as u32, |_| 0);
                (0..level_size_groups).into_iter().map(|group_index| self.conf.bits_per_group.ones_in_group(&arr, group_index)).collect()
            };
            let mut best_seeds = self.conf.bits_per_seed.new_zeroed_seed_vec(level_size_groups);
            for group_seed in 1..=last_seed {
                let with_new_seed = self.build_array_for_hashes(&key_hashes, level_size_segments, level_size_groups as u32, |_| group_seed);
                Self::update_best_seeds(&mut best_seeds, &mut best_counts, &with_new_seed, group_seed, &self.conf);
            }
            best_seeds
        }
    }


    /// Select optimal group seeds for the given `keys` and level size.
    fn select_seeds<KS, K>(&self, keys: &KS, level_size_groups: usize, level_size_segments: usize) -> Box<[SS::VecElement]>
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        let last_seed = ((1u32 << self.conf.bits_per_seed.into())-1) as u16;
        if let Some(thread_pool) = &self.thread_pool {
            thread_pool.install(|| {
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
                        best
                    } else { new }
                }).unwrap().unwrap().0
            })
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

    /*fn select_seeds_old<K, KS>(conf: &Conf<GS, SS, S>, level_nr: u32, level_size_groups: usize, level_size_segments: usize, keys: &KS) -> Box<[SS::VecElement]>
    where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        let seeds_count = 1u32 << conf.bits_per_seed.into();
        let threads_count = conf.threads_count.min(NonZeroUsize::new(seeds_count as usize).unwrap());
        if threads_count.get() == 1 {  // single thread calculations
            let mut best_counts: Box<[u8]> = {
                let arr = Self::build_array(&conf, keys, level_size_segments, level_size_groups as u32, level_nr, |_| 0);
                (0..level_size_groups).into_iter().map(|group_index| conf.bits_per_group.ones_in_group(&arr, group_index)).collect()
            };
            let mut best_seeds = conf.bits_per_seed.new_zeroed_seed_vec(level_size_groups);
            for group_seed in 1u32..seeds_count {
                let with_new_seed = Self::build_array(&conf, keys, level_size_segments, level_size_groups as u32, level_nr, |_| group_seed as u16);
                Self::update_best_seeds(&mut best_seeds, &mut best_counts, &with_new_seed, group_seed as u16, &conf);
            }
            best_seeds
        } else {    // multiple thread calculations
            let next_group_seed = AtomicU32::new(0);    // next group seed to check
            let r = threads_run(threads_count, || {
                let group_seed = next_group_seed.fetch_add(1, Ordering::Relaxed);
                if group_seed >= seeds_count { return None; }
                let group_seed = group_seed as u16;
                let mut best_counts: Box<[u8]> = {
                    let arr = Self::build_array(&conf, keys, level_size_segments, level_size_groups as u32, level_nr, |_| group_seed);
                    (0..level_size_groups).into_iter().map(|group_index| conf.bits_per_group.ones_in_group(&arr, group_index)).collect()
                };
                //let mut best_seeds = Box::<[u64]>::with_bitwords(group_seed as _, conf.bits_per_group_seed, level_size_groups);
                let mut best_seeds = conf.bits_per_seed.new_seed_vec(group_seed, level_size_groups);
                loop {
                    let group_seed = next_group_seed.fetch_add(1, Ordering::Relaxed);
                    if group_seed >= seeds_count as u32 { return Some((best_seeds, best_counts)); }
                    let group_seed = group_seed as u16;
                    let with_new_seed = Self::build_array(&conf, keys, level_size_segments, level_size_groups as u32, level_nr, |_| group_seed);
                    Self::update_best_seeds(&mut best_seeds, &mut best_counts, &with_new_seed, group_seed, &conf);
                }
            }).unwrap();
            let mut best: Option<(Box<[SS::VecElement]>, Box<[u8]>)> = None;
            for thread_best in r {
                if let Some((new_seeds, new_counts)) = thread_best {
                    if let Some((best_seeds, best_counts)) = best.as_mut() {
                        for group_index in 0..level_size_groups {
                            let best_count = &mut best_counts[group_index];
                            let new_count = new_counts[group_index];
                            if new_count > *best_count {
                                *best_count = new_count;
                                conf.bits_per_seed.set_seed(best_seeds, group_index,
                                                            conf.bits_per_seed.get_seed(&new_seeds, group_index))
                                /*best_seeds.set_fragment(group_index,
                                                         new_seeds.get_fragment(group_index, conf.bits_per_group_seed),
                                                         conf.bits_per_group_seed);*/
                            }
                        }
                    } else {
                        best = Some((new_seeds, new_counts));
                    }
                }
            }
            best.unwrap().0
        }   // end of multiple thread calculations
    }*/

    fn build_next_level<KS, K>(&mut self, keys: &mut KS, level_size_groups: usize, level_size_segments: usize)
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        let current_seeds = self.select_seeds_new(keys, level_size_groups, level_size_segments);
        let current_array = self.build_array(
            keys,
            level_size_segments, level_size_groups as u32,
            |group_index| self.conf.bits_per_seed.get_seed(&current_seeds, group_index as usize)
            //current_seeds.get_fragment(group_index as usize, conf.bits_per_group_seed) as u16
        );
        let level_nr = self.level_nr();
        keys.retain_keys(
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
            || self.input_size - current_array.iter().map(|v| v.count_ones() as usize).sum::<usize>(),
            self.thread_pool.as_ref()
        );
        self.push(current_array, current_seeds, level_size_groups as u32);
        self.input_size = keys.keys_len();
    }

    /*fn build_next_level<KS, K>(&mut self, keys: &mut KS, level_size_groups: usize, level_size_segments: usize)
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        let level_seed = self.level_nr();
        let key_hashes = if let Some(thread_pool) = &self.thread_pool {
            keys.par_map_each_key(
                |k| self.conf.hash_builder.hash_one(k, level_seed),
                self.input_size,
                |key| self.retained(key),
                thread_pool
            )
        } else {
            keys.map_each_key(
                |k| self.conf.hash_builder.hash_one(k, level_seed),
                self.input_size,
                |key| self.retained(key)
            )
        };
        //let current_seeds = levels.select_seeds(level_size_groups, level_size_segments, &keys);
        let current_seeds = self.select_seeds_prehashed(&key_hashes, level_size_groups, level_size_segments);
        let current_array = self.build_array_for_hashes(
            &key_hashes,
            level_size_segments, level_size_groups as u32,
            |group_index| self.conf.bits_per_seed.get_seed(&current_seeds, group_index as usize)
            //current_seeds.get_fragment(group_index as usize, conf.bits_per_group_seed) as u16
        );
        let level_nr = self.level_nr();
        keys.retain_keys(
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
            || self.input_size - current_array.iter().map(|v| v.count_ones() as usize).sum::<usize>(),
            self.thread_pool.as_ref()
        );
        self.push(current_array, current_seeds, level_size_groups as u32);
        self.input_size = keys.keys_len();
    }*/

    /// Builds levels for all `group_seeds` given using single thread.
    /// Returns the concatenation of the levels built.
    fn build_levels_st<K>(&self, keys: &impl KeySet<K>, level_size_segments: usize, level_size_groups: u32, group_seeds: RangeInclusive<u16>) -> Box<[u64]>
        where K: Hash + Sync
    {
        let total_array_size = level_size_segments * group_seeds.len();
        let mut result = vec![0u64; total_array_size].into_boxed_slice();
        let mut collision = vec![0u64; total_array_size].into_boxed_slice();
        let level_size = level_size_segments * 64;
        keys.for_each_key(
            |key| {
                let hash = self.conf.hash_builder.hash_one(key, self.level_nr());
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
    fn build_levels_mt<K>(&self, keys: &impl KeySet<K>, level_size_segments: usize, level_size_groups: u32, group_seeds: RangeInclusive<u16>, thread_pool: &ThreadPool) -> Box<[u64]>
        where K: Hash + Sync
    {
        let total_array_size = level_size_segments * group_seeds.len();
        let mut result = vec![0u64; total_array_size].into_boxed_slice();
        let result_atom = AtomicU64::from_mut_slice(&mut result);
        let mut collision: Box<[AtomicU64]> = (0..total_array_size).map(|_| AtomicU64::default()).collect();
        let level_size = level_size_segments * 64;
        keys.par_for_each_key(
            |key| {
                let hash = self.conf.hash_builder.hash_one(key, self.level_nr());
                let group = group_nr(hash, level_size_groups);
                let mut delta = 0;
                for group_seed in group_seeds.clone() {
                    let bit = self.conf.bits_per_group.bit_index_for_seed(hash, group_seed, group);
                    fphash_sync_add_bit(&result_atom, &collision, delta + bit);
                    delta += level_size;
                }
            },
            |key| self.retained(key),
            thread_pool
        );
        fphash_remove_collided(&mut result, AtomicU64::get_mut_slice(&mut collision));
        result
    }

    /// Select optimal group seeds for the given `keys` and level size.
    fn select_seeds_new<KS, K>(&self, keys: &KS, level_size_groups: usize, level_size_segments: usize) -> Box<[SS::VecElement]>
        where K: Hash + Sync, KS: KeySet<K> + Sync
    {
        let last_seed = ((1u32 << self.conf.bits_per_seed.into())-1) as u16;
        if let Some(thread_pool) = &self.thread_pool {
            let mut result = None;
            let mut seed_first = 0;
            loop {
                let seed_last = ((seed_first as u32) + 7).min(last_seed as u32) as u16;
                let levels = self.build_levels_mt(keys, level_size_segments, level_size_groups as u32, seed_first..=seed_last, thread_pool);
                result = thread_pool.install(|| {
                    levels.par_chunks(level_size_segments).enumerate().fold(|| None, |mut best: Option<(Box<[SS::VecElement]>, Box<[u8]>)>, (seed, array)| {
                        let seed = seed as u16;
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
                });
                if seed_last == last_seed { break }
                seed_first = seed_last + 1;
            }
            result.unwrap().0
        } else {
            let mut seed_first = 0;
            let mut best_seeds = self.conf.bits_per_seed.new_zeroed_seed_vec(level_size_groups);
            let mut best_counts: Option<Box<[u8]>> = None;
            loop {
                let seed_last = ((seed_first as u32) + 7).min(last_seed as u32) as u16;
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
}

/// Fingerprinting-based minimal perfect hash function with group optimization (FMPHGO).
///
/// See:
/// - P. Beling, *Fingerprinting-based minimal perfect hashing revisited*
pub struct FPHash2<GS: GroupSize = TwoToPowerBits, SS: SeedSize = TwoToPowerBitsStatic<2>, S = BuildDefaultSeededHasher> {
    array: ArrayWithRank,
    group_seeds: Box<[SS::VecElement]>,   //  Box<[u8]>,
    level_size: Box<[u32]>, // number of groups
    hash_builder: S,
    bits_per_seed: SS,

    /// number of bits that each group occupies
    bits_per_group: GS,
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

    /*fn get_bit<K: Hash>(&self, groups_before: usize, key: &K, level_nr: u32, level_size_segments: usize) -> bool {
        let mut hasher = self.hash_builder.build_hasher();
        let group = groups_before + group_nr(hasher, key, level_nr, level_size_segments);
        self.array.content[group] & (1 << self.in_group_index(&self.seeds, group)) != 0
        //(group, (hasher.finish() % 64) as u8)
    }*/

    /// Gets the value associated with the given `key` and reports statistics to `access_stats`.
    pub fn get_stats<K: Hash, A: stats::AccessStatsCollector>(&self, key: &K, access_stats: &mut A) -> Option<u64> {
        let mut groups_before = 0u32;
        let mut level_nr = 0u32;
        loop {
            let level_size_groups = *self.level_size.get(level_nr as usize)?;
            let hash = self.hash_builder.hash_one(key, level_nr);
            let group = groups_before + group_nr(hash, level_size_groups);
            let seed = self.bits_per_seed.get_seed(&self.group_seeds, group as usize);
            let bit_index = self.bits_per_group.bit_index_for_seed(hash, seed, group);
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
    pub fn with_conf_stats<K, KS, BS>(mut keys: KS, conf: FPHash2Conf<GS, SS, S>, stats: &mut BS) -> Self
        where K: Hash + Sync, KS: KeySet<K> + Sync, BS: stats::BuildStatsCollector
    {
        let mut levels = FPHash2Builder::new(keys.keys_len(), conf);
        while levels.input_size != 0 {
            let (level_size_groups, level_size_segments) = levels.conf.bits_per_group.level_size_groups_segments(
                ceiling_div(levels.input_size * levels.conf.relative_level_size as usize, 100));
            //let seed = level_nr;
            stats.level(levels.input_size, level_size_segments * 64);
            levels.build_next_level(&mut keys, level_size_groups, level_size_segments);
        }
        drop(keys);
        drop(levels.thread_pool);
        stats.end();
        let (array, _)  = ArrayWithRank::build(levels.arrays.concat().into_boxed_slice());
        let group_seeds_concatenated = levels.conf.bits_per_seed.concatenate_seed_vecs(&levels.level_sizes, levels.group_seeds);
        Self {
            array,
            group_seeds: group_seeds_concatenated,
            hash_builder: levels.conf.hash_builder,
            level_size: levels.level_sizes.into_boxed_slice(),
            bits_per_seed: levels.conf.bits_per_seed,
            bits_per_group: levels.conf.bits_per_group
        }
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
            + self.bits_per_group.write_size_bytes()
            + std::mem::size_of::<u32>()    // self.level_size.len()
            + self.level_size.size_bytes_dyn()
            + self.array.content.size_bytes_dyn()
            + self.group_seeds.size_bytes_dyn()
    }

    /// Writes `self` to the `output`.
    pub fn write(&self, output: &mut dyn io::Write) -> io::Result<()>
    {
        self.bits_per_group.write(output)?;
        write_int!(output, self.level_size.len() as u32)?;
        self.level_size.iter().try_for_each(|l| { write_int!(output, l) })?;
        self.array.content.iter().try_for_each(|v| write_int!(output, v))?;
        self.bits_per_seed.write_seed_vec(output, &self.group_seeds)
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
            bits_per_seed: bits_per_group_seed,
            bits_per_group,
            hash_builder: hasher
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