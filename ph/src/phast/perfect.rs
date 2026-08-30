//! [`Perfect`] – (k-)perfect (not necessarily minimal) hash function based on PHast.

use dyn_size_of::GetSize;
use seedable_hash::{BuildDefaultSeededHasher, BuildSeededHasher};
use voracious_radix_sort::RadixSort;
use std::hash::Hash;
use rayon::prelude::*;

use crate::{phast::{Conf, CoreConf, GenericCore, KSeedEvaluatorConf, ProdOfValues, SeedChooserCore, SeedOnly, SeedOnlyCore, SeedOnlyK, SeedOnlyKCore, SumOfValues, builder::{build_mt, build_st}, conf::Core, function::{Level, SeedEx, hash_all_par}, seed_chooser::SeedChooserConf}, seeds::{Bits8, SeedSize}};

/// PHast (Perfect Hashing made fast) - (K-)Perfect (not necessary minimal) Hash Function
/// with very fast evaluation developed by Piotr Beling and Peter Sanders.
/// Experimental.
/// 
/// Can be used with the following seed choosers (which specify a particular PHast variant):
/// [`ShiftOnlyWrapped`](crate::phast::ShiftOnlyWrapped), [`ShiftSeedWrapped`](crate::phast::ShiftSeedWrapped), [`SeedOnly`](crate::phast::SeedOnly), [`SeedOnlyK`](crate::phast::SeedOnlyK).
/// 
/// See:
/// Piotr Beling, Peter Sanders, *PHast - Perfect Hashing made fast*, 2025, <https://arxiv.org/abs/2504.17918>
pub struct Perfect<C: Core, SS: SeedSize, SCC = SeedOnlyCore, S = BuildDefaultSeededHasher>
{
    /// Seeds and core of the first level.
    level0: SeedEx<SS::VecElement, C>,
    /// Levels mapping the keys bumped from `level0` to disjoint ranges of values
    /// (each level adds the total output range of the previous levels as its `shift`).
    levels: Box<[Level<SS::VecElement, C>]>,
    /// Hasher used to hash keys.
    pub(crate) hasher: S,
    /// Core of the seed chooser used at evaluation time.
    seed_chooser: SCC,
    /// Seed size (number of bits per seed).
    seed_size: SS
}

impl<C: Core, SCC, SS: SeedSize, S> GetSize for Perfect<C, SS, SCC, S> {
    fn size_bytes_dyn(&self) -> usize {
        self.level0.size_bytes_dyn() + self.levels.size_bytes_dyn()
    }
    fn size_bytes_content_dyn(&self) -> usize {
        self.level0.size_bytes_content_dyn() + self.levels.size_bytes_content_dyn()
    }
    const USES_DYN_MEM: bool = true;
}

impl<C: Core, SS: SeedSize, SCC, S> Perfect<C, SS, SCC, S> {
    /// Returns the number of levels of `self` (including the first one).
    #[inline] pub fn levels(&self) -> usize { self.levels.len()+1 }
}

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore, S: BuildSeededHasher> Perfect<C, SS, SCC, S> {
    
    /// Returns value assigned to the given `key`.
    /// 
    /// The returned value is in the range from `0` (inclusive) to the number of elements in the input key `self.output_range()` (exclusive).
    /// `key` must come from the input key collection given during construction.
    #[inline(always)]   //inline(always) is important here
    pub fn get<K>(&self, key: &K) -> usize where K: Hash + ?Sized {
        let key_hash = self.hasher.hash_one(key, 0);
        let seed = unsafe { self.level0.seed_for(self.seed_size, key_hash) };
        if seed != 0 { return self.seed_chooser.f(key_hash, seed, &self.level0.core); }

        for level_nr in 0..self.levels.len() {
            let l = &self.levels[level_nr];
            let key_hash = self.hasher.hash_one(key, level_nr as u64 + 1);
            let seed = unsafe { l.seeds.seed_for(self.seed_size, key_hash) };
            if seed != 0 {
                return self.seed_chooser.f(key_hash, seed, &l.seeds.core) + l.shift
            }
        }

        unreachable!("phast::Perfect::get called for key not included in the input set")
    }

    /// Constructs [`Perfect`] function for given `keys`, using a single thread and given configuration.
    /// `keys` cannot contain duplicates.
    pub fn with_vec_conf_sc<K, CC, SC>(mut keys: Vec::<K>, conf: Conf<SS, CC, S>, seed_chooser: SC) -> Self
        where K: Hash, SC: SeedChooserConf<Core=SCC>, CC: CoreConf<Core = C>
    {
        Self::_new(|conf| {
            let level0 = Self::build_level_st(&mut keys, conf, seed_chooser.clone(), 0);
            (keys, level0)
        }, |keys, level_nr, conf| {
            Self::build_level_st(keys, conf, seed_chooser.clone(), level_nr)
        }, conf, seed_chooser.core())
    }

    /// Constructs [`Perfect`] function for given `keys`, using multiple (given number of) threads and given configuration
    /// 
    /// `keys` cannot contain duplicates.
    pub fn with_vec_conf_threads_sc<K, CC, SC>(mut keys: Vec::<K>, conf: Conf<SS, CC, S>, threads_num: usize, seed_chooser: SC) -> Self
        where K: Hash+Sync+Send, S: Sync, SC: SeedChooserConf<Core=SCC>, CC: CoreConf<Core = C> {
        if threads_num == 1 { return Self::with_vec_conf_sc(keys, conf, seed_chooser); }
        Self::_new(|conf| {
            let level0 = Self::build_level_mt(&mut keys, conf, threads_num, seed_chooser.clone(), 0);
            (keys, level0)
        }, |keys, level_nr, conf| {
            Self::build_level_mt(keys, conf, threads_num, seed_chooser.clone(), level_nr)
        }, conf, seed_chooser.core())
    }


    /// Constructs [`Perfect`] function for given `keys`, using a single thread and given configuration:
    /// `keys` cannot contain duplicates.
    pub fn with_slice_conf_sc<K, CC, SC>(keys: &[K], conf: Conf<SS, CC, S>, seed_chooser: SC) -> Self
        where K: Hash+Clone, SC: SeedChooserConf<Core=SCC>, CC: CoreConf<Core = C>
    {
        Self::_new(|conf| {
            Self::build_level_from_slice_st(keys, conf, seed_chooser.clone(), 0)
        }, |keys, level_nr, conf| {
            Self::build_level_st(keys, conf, seed_chooser.clone(), level_nr)
        }, conf, seed_chooser.core())
    }


    /// Constructs [`Perfect`] function for given `keys`, using multiple (given number of) threads and given configuration.
    ///
    /// `keys` cannot contain duplicates.
    pub fn with_slice_conf_threads_sc<K, CC, SC>(keys: &[K], conf: Conf<SS, CC, S>, threads_num: usize, seed_chooser: SC) -> Self
        where K: Hash+Sync+Send+Clone, S: Sync, SC: SeedChooserConf<Core=SCC>, CC: CoreConf<Core = C>
    {
        if threads_num == 1 { return Self::with_slice_conf_sc(keys, conf, seed_chooser); }
        Self::_new(|conf| {
            Self::build_level_from_slice_mt(keys, conf, threads_num, seed_chooser.clone(), 0)
        }, |keys, level_nr, conf| {
            Self::build_level_mt(keys, conf, threads_num, seed_chooser.clone(), level_nr)
        }, conf, seed_chooser.core())
    }

    /// Common construction: builds the first level with `build_first`,
    /// then repeatedly builds the next (disjoint) levels with `build_level` until no keys are left.
    #[inline]
    fn _new<K, BF, BL, CC>(build_first: BF, build_level: BL, conf: Conf<SS, CC, S>, seed_chooser: SCC) -> Self
        where BF: FnOnce(&Conf<SS, CC, S>) -> (Vec::<K>, SeedEx<SS::VecElement, C>),
            BL: Fn(&mut Vec::<K>, u64, &Conf<SS, CC, S>) -> SeedEx<SS::VecElement, C>,
            K: Hash, CC: CoreConf<Core = C>
        {
        let (mut keys, level0) = build_first(&conf);
        let mut shift = level0.core.output_range(seed_chooser, conf.seed_size.into());
        let mut levels = Vec::with_capacity(16);
        while !keys.is_empty() {
            let seeds = build_level(&mut keys, levels.len() as u64+1, &conf);
            let out_range = seeds.core.output_range(seed_chooser, conf.seed_size.into());
            levels.push(Level { seeds, shift });
            shift += out_range;
        }
        Self {
            level0,
            levels: levels.into_boxed_slice(),
            hasher: conf.hasher,
            seed_chooser,
            seed_size: conf.seed_size,
        }
    }

    /// Builds a level (with given `level_nr`, used to seed the hasher) for `keys` given as a slice;
    /// returns the keys bumped to the next level and the level. Uses a single thread.
    #[inline]
    fn build_level_from_slice_st<K, CC, SC>(keys: &[K], conf: &Conf<SS, CC, S>, seed_chooser: SC, level_nr: u64)
        -> (Vec<K>, SeedEx<SS::VecElement, C>)
        where K: Hash+Clone, SC: SeedChooserConf<Core=SCC>, CC: CoreConf<Core = C>
    {
        let mut hashes: Box<[_]> = keys.iter().map(|k| conf.hasher.hash_one(k, level_nr)).collect();
        //radsort::unopt::sort(&mut hashes);
        hashes.voracious_sort();
        let core = seed_chooser.f_core_lf(hashes.len(), conf.loading_factor_1000, &conf.core_conf, conf.bits_per_seed());
        let (bucket_evaluator, seed_chooser) = seed_chooser.evaluators(conf.bits_per_seed(), core.slice_len());
        let (seeds, builder) = build_st(&hashes, core, conf.seed_size, bucket_evaluator, seed_chooser);
        let mut keys_vec = Vec::with_capacity(builder.bumped_len(&seeds));
        drop(builder);
        keys_vec.extend(keys.iter().filter(|key| {
            unsafe { conf.seed_size.get_seed(&seeds, core.bucket_for(conf.hasher.hash_one(key, level_nr))) == 0 }
        }).cloned());
        (keys_vec, SeedEx{ seeds, core })
    }

    /// Builds a level (with given `level_nr`, used to seed the hasher) for `keys` given as a slice;
    /// returns the keys bumped to the next level and the level. Uses up to `threads_num` threads.
    #[inline]
    fn build_level_from_slice_mt<K, CC, SC>(keys: &[K], conf: &Conf<SS, CC, S>, threads_num: usize, seed_chooser: SC, level_nr: u64)
        -> (Vec<K>, SeedEx<SS::VecElement, C>)
        where K: Hash+Sync+Send+Clone, S: Sync, SC: SeedChooserConf<Core=SCC>, CC: CoreConf<Core = C>
    {
        let mut hashes: Box<[_]> = hash_all_par(keys, &conf.hasher, level_nr);
        //radsort::unopt::sort(&mut hashes);
        hashes.voracious_mt_sort(threads_num);
        let core = seed_chooser.f_core_lf(hashes.len(), conf.loading_factor_1000, &conf.core_conf, conf.bits_per_seed());
        let (bucket_evaluator, seed_chooser) = seed_chooser.evaluators(conf.bits_per_seed(), core.slice_len());
        let (seeds, builder) = build_mt(&hashes, core, conf.seed_size, bucket_evaluator, seed_chooser, threads_num);
        let mut keys_vec = Vec::with_capacity(builder.bumped_len(&seeds));
        drop(builder);
        keys_vec.par_extend(keys.into_par_iter().filter(|key| {
            unsafe { conf.seed_size.get_seed(&seeds, core.bucket_for(conf.hasher.hash_one(key, level_nr))) == 0 }
        }).cloned());
        (keys_vec, SeedEx{ seeds, core })
    }

    /// Builds a level (with given `level_nr`, used to seed the hasher) for `keys`;
    /// leaves the keys bumped to the next level in `keys` and returns the level. Uses a single thread.
    #[inline(always)]
    fn build_level_st<K, CC, SC>(keys: &mut Vec::<K>, conf: &Conf<SS, CC, S>, seed_chooser: SC, level_nr: u64) -> SeedEx<SS::VecElement, C>
        where K: Hash, SC: SeedChooserConf<Core=SCC>, CC: CoreConf<Core = C>
    {
        let mut hashes: Box<[_]> = keys.iter().map(|k| conf.hasher.hash_one(k, level_nr)).collect();
        hashes.voracious_sort();
        let core = seed_chooser.f_core_lf(hashes.len(), conf.loading_factor_1000, &conf.core_conf, conf.bits_per_seed());
        let (bucket_evaluator, seed_chooser) = seed_chooser.evaluators(conf.bits_per_seed(), core.slice_len());
        let (seeds, _) = build_st(&hashes, core, conf.seed_size, bucket_evaluator, seed_chooser);
        keys.retain(|key| {
            unsafe { conf.seed_size.get_seed(&seeds, core.bucket_for(conf.hasher.hash_one(key, level_nr))) == 0 }
        });
        SeedEx{ seeds, core }
    }

    /// Builds a level (with given `level_nr`, used to seed the hasher) for `keys`;
    /// leaves the keys bumped to the next level in `keys` and returns the level. Uses up to `threads_num` threads.
    #[inline]
    fn build_level_mt<K, CC, SC>(keys: &mut Vec::<K>, conf: &Conf<SS, CC, S>, threads_num: usize, seed_chooser: SC, level_nr: u64)
        -> SeedEx<SS::VecElement, C>
        where K: Hash+Sync+Send, S: Sync, SC: SeedChooserConf<Core=SCC>, CC: CoreConf<Core = C>
    {
        let mut hashes: Box<[_]> = hash_all_par(keys, &conf.hasher, level_nr);
        //radsort::unopt::sort(&mut hashes);
        hashes.voracious_mt_sort(threads_num);
        let core = seed_chooser.f_core_lf(hashes.len(), conf.loading_factor_1000, &conf.core_conf, conf.bits_per_seed());
        let (bucket_evaluator, seed_chooser) = seed_chooser.evaluators(conf.bits_per_seed(), core.slice_len());
        let (seeds, builder) = build_mt(&hashes, core, conf.seed_size, bucket_evaluator, seed_chooser, threads_num);
        let mut result = Vec::with_capacity(builder.bumped_len(&seeds));
        drop(builder);
        std::mem::swap(keys, &mut result);
        keys.par_extend(result.into_par_iter().filter(|key| {
            unsafe { conf.seed_size.get_seed(&seeds, core.bucket_for(conf.hasher.hash_one(key, level_nr))) == 0 }
        }));
        SeedEx{ seeds, core }
    }

    /// Returns maximum number of keys which can be mapped to the same value by `k`-[`Perfect`] function `self`.
    #[inline(always)] pub fn k(&self) -> u16 { self.seed_chooser.k() }

    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys,
    /// i.e. 1 + maximum value that minimal function can return.
    #[inline(always)] pub fn minimal_output_range(&self, num_of_keys: usize) -> usize { self.seed_chooser.minimal_output_range(num_of_keys) }

    /// Returns output range of `self`, i.e. 1 + maximum value that `self` can return.
    pub fn output_range(&self) -> usize {
        if let Some(last_level) = self.levels.last() {
            last_level.shift + last_level.seeds.core.output_range(self.seed_chooser, self.seed_size.into())
        } else {
            self.level0.core.output_range(self.seed_chooser, self.seed_size.into())
        }
    }
}

impl Perfect<GenericCore, Bits8, SeedOnlyCore, BuildDefaultSeededHasher> {
    /// Constructs [`Perfect`] function for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_st<K>(keys: Vec::<K>) -> Self where K: Hash {
        Self::with_vec_conf_sc(keys, Conf::default_generic8(), SeedOnly(ProdOfValues))
    }

    /// Constructs [`Perfect`] function for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_mt<K>(keys: Vec::<K>) -> Self where K: Hash+Send+Sync {
        Self::with_vec_conf_threads_sc(keys, Conf::default_generic8(),
        std::thread::available_parallelism().map_or(1, |v| v.into()), SeedOnly(ProdOfValues))
    }

    /// Constructs [`Perfect`] function for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_st<K>(keys: &[K]) -> Self where K: Hash+Clone {
        Self::with_slice_conf_sc(keys, Conf::default_generic8(), SeedOnly(ProdOfValues))
    }

    /// Constructs [`Perfect`] function for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_mt<K>(keys: &[K]) -> Self where K: Hash+Clone+Send+Sync {
        Self::with_slice_conf_threads_sc(keys, Conf::default_generic8(),
        std::thread::available_parallelism().map_or(1, |v| v.into()), SeedOnly(ProdOfValues))
    }
}

impl Perfect<GenericCore, Bits8, SeedOnlyKCore, BuildDefaultSeededHasher> {
    /// Constructs `k`-[`Perfect`] function for given `keys`, using a single thread.
    /// `k`-[`Perfect`] function maps `k` or less different keys to each value.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn k_from_vec_se_st<K, SEC>(k: u16, keys: Vec::<K>, seed_evaluator: SEC) -> Self
        where K: Hash, SEC: KSeedEvaluatorConf
    {
        Self::with_vec_conf_sc(keys, Conf::default_generic8(), SeedOnlyK::with_evaluator(k, seed_evaluator))
    }

    /// Constructs `k`-[`Perfect`] function for given `keys`, using multiple threads.
    /// `k`-[`Perfect`] function maps `k` or less different keys to each value.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn k_from_vec_se_mt<K, SEC>(k: u16, keys: Vec::<K>, seed_evaluator: SEC) -> Self
        where K: Hash+Send+Sync, SEC: KSeedEvaluatorConf
    {
        Self::with_vec_conf_threads_sc(keys, Conf::default_generic8(),
        std::thread::available_parallelism().map_or(1, |v| v.into()), SeedOnlyK::with_evaluator(k, seed_evaluator))
    }

    /// Constructs `k`-[`Perfect`] function for given `keys`, using a single thread.
    /// `k`-[`Perfect`] function maps `k` or less different keys to each value.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn k_from_slice_se_st<K, SEC>(k: u16, keys: &[K], seed_evaluator: SEC) -> Self 
        where K: Hash+Clone, SEC: KSeedEvaluatorConf
    {
        Self::with_slice_conf_sc(keys, Conf::default_generic8(), SeedOnlyK::with_evaluator(k, seed_evaluator))
    }

    /// Constructs `k`-[`Perfect`] function for given `keys`, using multiple threads.
    /// `k`-[`Perfect`] function maps `k` or less different keys to each value.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn k_from_slice_se_mt<K, SEC>(k: u16, keys: &[K], seed_evaluator: SEC) -> Self 
        where K: Hash+Clone+Send+Sync, SEC: KSeedEvaluatorConf {
        Self::with_slice_conf_threads_sc(keys, Conf::default_generic8(),
        std::thread::available_parallelism().map_or(1, |v| v.into()), SeedOnlyK::with_evaluator(k, seed_evaluator))
    }
}

impl Perfect<GenericCore, Bits8, SeedOnlyKCore, BuildDefaultSeededHasher> {
    /// Constructs `k`-[`Perfect`] function for given `keys`, using a single thread.
    /// `k`-[`Perfect`] function maps `k` or less different keys to each value.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn k_from_vec_st<K>(k: u16, keys: Vec::<K>) -> Self where K: Hash {
        Self::k_from_vec_se_st(k, keys, SumOfValues)
    }

    /// Constructs `k`-[`Perfect`] function for given `keys`, using multiple threads.
    /// `k`-[`Perfect`] function maps `k` or less different keys to each value.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn k_from_vec_mt<K>(k: u16, keys: Vec::<K>) -> Self where K: Hash+Send+Sync {
        Self::k_from_vec_se_mt(k, keys, SumOfValues)
    }

    /// Constructs `k`-[`Perfect`] function for given `keys`, using a single thread.
    /// `k`-[`Perfect`] function maps `k` or less different keys to each value.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn k_from_slice_st<K>(k: u16, keys: &[K]) -> Self where K: Hash+Clone {
        Self::k_from_slice_se_st(k, keys, SumOfValues)
    }

    /// Constructs `k`-[`Perfect`] function for given `keys`, using multiple threads.
    /// `k`-[`Perfect`] function maps `k` or less different keys to each value.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn k_from_slice_mt<K>(k: u16, keys: &[K]) -> Self where K: Hash+Clone+Send+Sync {
        Self::k_from_slice_se_mt(k, keys, SumOfValues)
    }
}


#[cfg(test)]
pub(crate) mod tests {
    use std::fmt::Display;

    use crate::utils::{verify_partial_kphf, verify_partial_phf};

    use super::*;

    fn phf_test<SCC, K, SS, S, C>(f: &Perfect<C, SS, SCC, S>, keys: &[K])
        where K: Display+Hash, SCC: SeedChooserCore, SS: SeedSize, S: BuildSeededHasher, C: Core
    {
        verify_partial_phf(f.output_range(), keys, |key| Some(f.get(key)));
    }

    fn kphf_test<K: Display+Hash, SS: SeedSize, S: BuildSeededHasher, C: Core>(f: &Perfect<C, SS, SeedOnlyKCore, S>, keys: &[K]) {
        verify_partial_kphf(f.k(), f.output_range(), keys, |key| Some(f.get(key)));
    }
    
    #[test]
    fn test_small() {
        let input = [1, 2, 3, 4, 5];
        let f = Perfect::from_slice_st(&input);
        phf_test(&f, &input);
    }

    #[test]
    fn test_medium() {
        let input: Box<[u16]> = (0..1000).collect();
        let f = Perfect::from_slice_st(&input);
        phf_test(&f, &input);
    }

    #[test]
    fn test_small_k() {
        let input = [1, 2, 3, 4, 5];
        let f = Perfect::k_from_slice_st(3, &input);
        kphf_test(&f, &input);
    }

    #[test]
    fn test_medium_k() {
        let input: Box<[u16]> = (0..1000).collect();
        let f = Perfect::k_from_slice_st(3, &input);
        kphf_test(&f, &input);
    }
}