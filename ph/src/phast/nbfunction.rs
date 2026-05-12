use std::hash::Hash;

use seedable_hash::{BuildDefaultSeededHasher, BuildSeededHasher};
use voracious_radix_sort::RadixSort;

use crate::{fmph::Bits8, phast::{Conf, Core, CoreConf, GenericCore, ProdOfValues, SeedChooser, SeedChooserCore, SeedEvaluator, builder::{bucket_begin_mt, bucket_begin_st, try_nobump_build_st}, function::{SeedEx, hash_all_par}, seed_chooser::{SeedNoBumpCore, SeedOnlyNoBump}}, seeds::SeedSize};

/// NBFunction (No Bump Function) is a variant of PHast (Perfect Hashing made fast)
/// that do not use bumping.
/// 
/// In practice, this variant can only be built for certain configurations,
/// specifically those with a loading factor below 100% (i.e. not minimal).
/// However, it provides very fast evaluation, with only 1 cache miss.
/// 
/// See:
/// Piotr Beling, Peter Sanders, *PHast - Perfect Hashing made fast*, 2025, <https://arxiv.org/abs/2504.17918>
pub struct NBFunction<C: Core, SS, S = BuildDefaultSeededHasher>
    where SS: SeedSize
{
    seeds: SeedEx<SS::VecElement, C>,
    seed: u64,
    hasher: S,
    seed_chooser: SeedNoBumpCore,
    seed_size: SS,  // seed size, K=2**bits_per_seed
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
    pub fn with_slice_conf_se<K, SE, CC>(keys: &[K], conf: Conf<SS, CC, S>, seed_evaluator: SE) -> Self
        where K: Hash, CC: CoreConf<Core = C>, SE: SeedEvaluator
    {
        Self::new(keys.len(), conf, seed_evaluator, 1, |hasher, seed|
            keys.iter().map(|k| hasher.hash_one(k, seed)).collect()
        )
    }

    /// Constructs [`NBFunction`] for given `keys`, using multiple (given number of) threads and given configuration.
    /// Multithreading is used only for key hashing, sorting, and determining bucket sizes.
    /// `keys` cannot contain duplicates.
    pub fn with_slice_conf_threads_se<K, SE, CC>(keys: &[K], conf: Conf<SS, CC, S>, threads_num: usize, seed_evaluator: SE) -> Self
        where K: Hash, CC: CoreConf<Core = C>, SE: SeedEvaluator, K: Hash+Sync+Send, S: Sync
    {
        Self::new(keys.len(), conf, seed_evaluator, threads_num, |hasher, seed|
            hash_all_par(&keys, hasher, seed)
        )
    }

    /// Constructs [`NBFunction`] for given number of keys and configuration.
    /// `hashes(hasher, seed)` must return `num_of_keys` hashes.
    pub fn new<H, SE, CC>(num_of_keys: usize, conf: Conf<SS, CC, S>, seed_evaluator: SE, threads_num: usize, hashes: H) -> Self 
        where H: Fn(&S, u64) -> Box<[u64]>, CC: CoreConf<Core = C>, SE: SeedEvaluator
    {
        let seed_chooser = SeedOnlyNoBump(seed_evaluator);
        let core = SeedNoBumpCore.f_core_lf(num_of_keys, conf.loading_factor_1000, &conf.core_conf, conf.seed_size.into());
        let mut seed = 0;
        if threads_num > 1 {
            loop {
                let mut hashes = hashes(&conf.hasher, seed);
                hashes.voracious_mt_sort(threads_num);
                let evaluator = seed_chooser.bucket_evaluator(conf.bits_per_seed(), core.slice_len());
                if let Some((seeds, _)) = try_nobump_build_st(&hashes, core, conf.seed_size, evaluator, seed_chooser, bucket_begin_mt(&hashes, &core, threads_num)) {
                    return Self { seeds: SeedEx{ seeds, core }, seed, hasher: conf.hasher, seed_chooser: SeedNoBumpCore, seed_size: conf.seed_size };
                }
                seed += 1;
            }
        } else {
            loop {
                let mut hashes = hashes(&conf.hasher, seed);
                hashes.voracious_sort();
                let evaluator = seed_chooser.bucket_evaluator(conf.bits_per_seed(), core.slice_len());
                if let Some((seeds, _)) = try_nobump_build_st(&hashes, core, conf.seed_size, evaluator, seed_chooser, bucket_begin_st(&hashes, &core)) {
                    return Self { seeds: SeedEx{ seeds, core }, seed, hasher: conf.hasher, seed_chooser: SeedNoBumpCore, seed_size: conf.seed_size };
                }
                seed += 1;
            }
        }
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
}

impl NBFunction<GenericCore, Bits8, BuildDefaultSeededHasher> {
    /// Constructs [`NBFunction`] for given `keys`, using a single thread and given loading factor.
    /// Recommended `loading_factor_1000` is from `970` (for fast building) to `990` (for small range).
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_st<K>(keys: &[K], loading_factor_1000: u16) -> Self where K: Hash {
        Self::with_slice_conf_se(keys, Conf::generic8_nobump(loading_factor_1000), ProdOfValues)
    }

    /// Constructs [`NBFunction`] for given `keys`, using multiple threads and given loading factor.
    /// Recommended `loading_factor_1000` is from `970` (for fast building) to `990` (for small range).
    /// 
    /// multithreading is used only for key hashing, sorting, and determining bucket sizes.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_mt<K>(keys: &[K], loading_factor_1000: u16) -> Self where K: Hash+Send+Sync {
        Self::with_slice_conf_threads_se(keys, Conf::generic8_nobump(loading_factor_1000),
        std::thread::available_parallelism().map_or(1, |v| v.into()), ProdOfValues)
    }
}