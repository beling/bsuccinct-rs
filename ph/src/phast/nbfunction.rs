use std::hash::Hash;

use dyn_size_of::GetSize;
use seedable_hash::{BuildDefaultSeededHasher, BuildSeededHasher};
use voracious_radix_sort::RadixSort;
use rayon::prelude::*;

use crate::{fmph::Bits8, phast::{Conf, Core, CoreConf, GenericCore, ProdOfValues, RandomPlacement, SeedChooser, SeedChooserCore, SeedEvaluator, builder::{bucket_begin_mt, bucket_begin_st, try_nobump_build_st}, function::{SeedEx, hash_all_par}, seed_chooser::{SeedNoBumpCore, SeedOnlyNoBump}}, seeds::SeedSize};

/// NBFunction (No Bump Function) is a variant of PHast (Perfect Hashing made fast)
/// that do not use bumping.
/// 
/// In practice, this variant can only be built for certain configurations,
/// specifically those with a loading factor below 100% (i.e. not minimal).
/// However, it provides very fast evaluation, with only 1 cache miss.
/// 
/// See:
/// Piotr Beling, Peter Sanders, *PHast - Perfect Hashing made fast*, 2025, <https://arxiv.org/abs/2504.17918>
pub struct NBFunction<C, SS, S = BuildDefaultSeededHasher>
    where C: Core, SS: SeedSize
{
    seeds: SeedEx<SS::VecElement, C>,
    seed: u64,
    hasher: S,
    seed_chooser: SeedNoBumpCore,
    seed_size: SS,  // seed size, K=2**bits_per_seed
}

impl<C: Core, SS: SeedSize, S> GetSize for NBFunction<C, SS, S> {
    fn size_bytes_dyn(&self) -> usize { self.seeds.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.seeds.size_bytes_content_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl<C: Core, SS: SeedSize, S: BuildSeededHasher> NBFunction<C, SS, S> {
    /// Returns value assigned to the given `key`.
    #[inline(always)]
    pub fn get<K>(&self, key: &K) -> usize where K: Hash + ?Sized {
        let key_hash = self.hasher.hash_one(key, self.seed);
        let seed = unsafe { self.seeds.seed_for(self.seed_size, key_hash) };
        self.seed_chooser.f(key_hash, seed, &self.seeds.core)
    }

    /// Constructs [`NBFunction`] for given `keys`, using a single thread and given configuration.
    /// `keys` cannot contain duplicates.
    pub fn with_slice_conf_se<K, SE, CC>(keys: &[K], conf: Conf<SS, CC, S>, seed_evaluator: SE, tries: u64) -> Option<Self>
        where K: Hash, CC: CoreConf<Core = C>, SE: SeedEvaluator
    {
        Self::new(keys.len(), conf, seed_evaluator, tries, 1, |hasher, seed|
            keys.iter().map(|k| hasher.hash_one(k, seed)).collect()
        )
    }

    /// Constructs [`NBFunction`] for given `keys`, using multiple (given number of) threads and given configuration.
    /// 
    /// Multithreading is used only for key hashing, sorting, and determining bucket sizes.
    /// Therefore, using this ('threads') version is recommended only when the expected number of build attempts is very small.
    /// Otherwise, as long as the key set is small enough to fit its hashes for different seeds into memory, it is better to use the regular 'mt' version.
    /// 
    /// `keys` should not contain duplicates.
    pub fn with_slice_conf_threads_se<K, SE, CC>(keys: &[K], conf: Conf<SS, CC, S>, tries: u64, threads_num: usize, seed_evaluator: SE) -> Option<Self>
        where K: Hash, CC: CoreConf<Core = C>, SE: SeedEvaluator, K: Hash+Sync+Send, S: Sync
    {
        Self::new(keys.len(), conf, seed_evaluator, tries, threads_num, |hasher, seed|
            hash_all_par(&keys, hasher, seed)
        )
    }

    /// Constructs [`NBFunction`] for given `keys`, using multiple threads and given configuration.
    /// 
    /// Multithreading is used to perform parallel construction attempts with different hash function seeds.
    /// 
    /// `keys` should not contain duplicates.
    pub fn with_slice_conf_mt_se<K, SE, CC>(keys: &[K], conf: Conf<SS, CC, S>, tries: u64, seed_evaluator: SE) -> Option<Self>
        where K: Hash, CC: CoreConf<Core = C>, SE: SeedEvaluator, K: Hash+Sync+Send, S: Send+Sync+Clone
    {
        Self::new_mt(keys.len(), conf, seed_evaluator, tries, |hasher, seed|
            keys.iter().map(|k| hasher.hash_one(k, seed)).collect())
    }

    /// Constructs [`NBFunction`] for given `keys`, using a single thread and given configuration.
    /// `keys` cannot contain duplicates.
    #[inline] pub fn with_slice_conf<K, CC>(keys: &[K], conf: Conf<SS, CC, S>, tries: u64) -> Option<Self>
        where K: Hash, CC: CoreConf<Core = C>
    {
        Self::with_slice_conf_se(keys, conf, ProdOfValues, tries)
    }

    /// Constructs [`NBFunction`] for given `keys`, using multiple (given number of) threads and given configuration.
    /// 
    /// Multithreading is used only for key hashing, sorting, and determining bucket sizes.
    /// Therefore, using this ('threads') version is recommended only when the expected number of build attempts is very small.
    /// Otherwise, as long as the key set is small enough to fit its hashes for different seeds into memory, it is better to use the regular 'mt' version.
    /// 
    /// `keys` should not contain duplicates.
    #[inline] pub fn with_slice_conf_threads<K, CC>(keys: &[K], conf: Conf<SS, CC, S>, tries: u64, threads_num: usize) -> Option<Self>
        where K: Hash, CC: CoreConf<Core = C>, K: Hash+Sync+Send, S: Sync
    {
        Self::with_slice_conf_threads_se(keys, conf, tries, threads_num, ProdOfValues)
    }

    /// Constructs [`NBFunction`] for given `keys`, using multiple (given number of) threads and given configuration.
    /// 
    /// Multithreading is used to perform parallel construction attempts with different hash function seeds.
    /// 
    /// `keys` should not contain duplicates.
    #[inline] pub fn with_slice_conf_mt<K, CC>(keys: &[K], conf: Conf<SS, CC, S>, tries: u64) -> Option<Self>
        where K: Hash, CC: CoreConf<Core = C>, K: Hash+Sync+Send, S: Send+Sync+Clone
    {
        Self::with_slice_conf_mt_se(keys, conf, tries, ProdOfValues)
    }

    pub fn build_st<SE, CC>(hashes: &mut [u64], conf: &Conf<SS, CC, S>, seed_chooser: &SeedOnlyNoBump<SE>, core: &C) -> Option<Box<[SS::VecElement]>>
    where SE: SeedEvaluator, CC: CoreConf<Core = C>
    {
        hashes.voracious_sort();
        let evaluator = seed_chooser.bucket_evaluator(conf.bits_per_seed(), core.slice_len());
        try_nobump_build_st(hashes, *core, conf.seed_size, evaluator, *seed_chooser, bucket_begin_st(&hashes, core)).map(|(seeds, _)| seeds)
        /*.map(|(seeds, _)| {
            Self { seeds: SeedEx{ seeds, core: *core }, seed, hasher: conf.hasher.clone(), seed_chooser: SeedNoBumpCore, seed_size: conf.seed_size }
        })*/
    }

    /// Constructs [`NBFunction`] for given number of keys and configuration.
    /// `hashes(hasher, seed)` must return `num_of_keys` hashes.
    pub fn new<H, SE, CC>(num_of_keys: usize, conf: Conf<SS, CC, S>, seed_evaluator: SE, tries: u64, threads_num: usize, hashes: H) -> Option<Self>
        where H: Fn(&S, u64) -> Box<[u64]>, CC: CoreConf<Core = C>, SE: SeedEvaluator
    {
        let seed_chooser = SeedOnlyNoBump(seed_evaluator);
        let core = SeedNoBumpCore.f_core_lf(num_of_keys, conf.loading_factor_1000, &conf.core_conf, conf.seed_size.into());
        if threads_num > 1 {
            for seed in 0..tries {
                let mut hashes = hashes(&conf.hasher, seed);
                hashes.voracious_mt_sort(threads_num);
                let evaluator = seed_chooser.bucket_evaluator(conf.bits_per_seed(), core.slice_len());
                if let Some((seeds, _)) = try_nobump_build_st(&hashes, core, conf.seed_size, evaluator, seed_chooser, bucket_begin_mt(&hashes, &core, threads_num)) {
                    return Some(Self { seeds: SeedEx{ seeds, core }, seed, hasher: conf.hasher, seed_chooser: SeedNoBumpCore, seed_size: conf.seed_size });
                }
            }
        } else {
            for seed in 0..tries {
                if let Some(seeds) = Self::build_st(&mut hashes(&conf.hasher, seed), &conf, &seed_chooser, &core) { 
                    return Some(Self { seeds: SeedEx{ seeds, core }, seed, hasher: conf.hasher, seed_chooser: SeedNoBumpCore, seed_size: conf.seed_size })
                }
            }
        }
        None
    }

    pub fn new_mt<H, SE, CC>(num_of_keys: usize, conf: Conf<SS, CC, S>, seed_evaluator: SE, tries: u64, hashes: H) -> Option<Self>
        where H: Fn(&S, u64) -> Box<[u64]>, CC: CoreConf<Core = C>, SE: SeedEvaluator, S: Send+Sync+Clone, H: Sync
    {
        let seed_chooser = SeedOnlyNoBump(seed_evaluator);
        let core = SeedNoBumpCore.f_core_lf(num_of_keys, conf.loading_factor_1000, &conf.core_conf, conf.seed_size.into());
        (0..tries).into_par_iter().find_map_any(|seed|
            Self::build_st(&mut hashes(&conf.hasher, seed), &conf, &seed_chooser, &core).map(|seeds|
                Self { seeds: SeedEx{ seeds, core }, seed, hasher: conf.hasher.clone(), seed_chooser: SeedNoBumpCore, seed_size: conf.seed_size }
            ))
    }
}

impl<C: Core, SS: SeedSize, S> NBFunction<C, SS, S> {
    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys,
    /// i.e. 1 + maximum value that minimal function can return.
    #[inline(always)] pub fn minimal_output_range(&self, num_of_keys: usize) -> usize { self.seed_chooser.minimal_output_range(num_of_keys) }

    /// Returns output range of `self`, i.e. 1 + maximum value that `self` can return.
    pub fn output_range(&self) -> usize {
        self.seeds.core.output_range(self.seed_chooser, self.seed_size.into())
    }

    /// Seed used by the function to hash keys.
    #[inline] pub fn seed(&self) -> u64 { self.seed }
}

impl NBFunction<GenericCore, Bits8, BuildDefaultSeededHasher> {
    /// Constructs [`NBFunction`] for given `keys`, using a single thread and given loading factor.
    /// In comparison to `from_slice_st`, the function constructed by `from_slice_st_fast` is faster to evaluate
    /// but requires smaller `loading_factor_1000`.
    /// Recommended `loading_factor_1000` is from `970` (for fast building) to `990` (for small range).
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_st_fast<K>(keys: &[K], loading_factor_1000: u16, tries: u64) -> Option<Self> where K: Hash {
        Self::with_slice_conf(keys, Conf::generic8_nobump_fast(loading_factor_1000), tries)
    }

    /// Constructs [`NBFunction`] for given `keys`, using multiple threads and given loading factor.
    /// In comparison to `from_slice_st`, the function constructed by `from_slice_st_fast` is faster to evaluate
    /// but requires smaller `loading_factor_1000`.
    /// Recommended `loading_factor_1000` is from `970` (for fast building) to `990` (for small range).
    /// 
    /// Multithreading is used only for key hashing, sorting, and determining bucket sizes.
    /// Therefore, using this ('smallmt') version is recommended only when the expected number of build attempts is very small.
    /// Otherwise, as long as the key set is small enough to fit its hashes for different seeds into memory, it is better to use the regular 'mt' version.
    /// 
    /// `keys` should not contain duplicates.
    pub fn from_slice_smallmt_fast<K>(keys: &[K], loading_factor_1000: u16, tries: u64) -> Option<Self> where K: Hash+Send+Sync {
        Self::with_slice_conf_threads(keys, Conf::generic8_nobump_fast(loading_factor_1000), tries,
        std::thread::available_parallelism().map_or(1, |v| v.into()))
    }

    /// Constructs [`NBFunction`] for given `keys`, using multiple threads and given loading factor.
    /// In comparison to `from_slice_st`, the function constructed by `from_slice_st_fast` is faster to evaluate
    /// but requires smaller `loading_factor_1000`.
    /// Recommended `loading_factor_1000` is from `970` (for fast building) to `990` (for small range).
    /// 
    /// Multithreading is used to perform parallel construction attempts with different hash function seeds.
    /// 
    /// `keys` should not contain duplicates.
    pub fn from_slice_mt_fast<K>(keys: &[K], loading_factor_1000: u16, tries: u64) -> Option<Self> where K: Hash+Send+Sync {
        Self::with_slice_conf_mt(keys, Conf::generic8_nobump_fast(loading_factor_1000), tries)
    }
}

impl NBFunction<GenericCore<RandomPlacement>, Bits8, BuildDefaultSeededHasher> {
    /// Constructs [`NBFunction`] for given `keys`, using a single thread and given loading factor.
    /// Recommended `loading_factor_1000` is from `970` (for fast building) to `995` (for small range).
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_st<K>(keys: &[K], loading_factor_1000: u16, tries: u64) -> Option<Self> where K: Hash {
        Self::with_slice_conf(keys, Conf::generic8_nobump(loading_factor_1000), tries)
    }

    /// Constructs [`NBFunction`] for given `keys`, using multiple threads and given loading factor.
    /// Recommended `loading_factor_1000` is from `970` (for fast building) to `995` (for small range).
    /// 
    /// Multithreading is used only for key hashing, sorting, and determining bucket sizes.
    /// Therefore, using this ('smallmt') version is recommended only when the expected number of build attempts is very small.
    /// Otherwise, as long as the key set is small enough to fit its hashes for different seeds into memory, it is better to use the regular 'mt' version.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_smallmt<K>(keys: &[K], loading_factor_1000: u16, tries: u64) -> Option<Self> where K: Hash+Send+Sync {
        Self::with_slice_conf_threads(keys, Conf::generic8_nobump(loading_factor_1000), tries,
        std::thread::available_parallelism().map_or(1, |v| v.into()))
    }

    /// Constructs [`NBFunction`] for given `keys`, using multiple threads and given loading factor.
    /// Recommended `loading_factor_1000` is from `970` (for fast building) to `995` (for small range).
    /// 
    /// Multithreading is used to perform parallel construction attempts with different hash function seeds.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_mt<K>(keys: &[K], loading_factor_1000: u16, tries: u64) -> Option<Self> where K: Hash+Send+Sync {
        Self::with_slice_conf_mt(keys, Conf::generic8_nobump(loading_factor_1000), tries)
    }
}