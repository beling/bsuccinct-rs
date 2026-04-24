use dyn_size_of::GetSize;
use voracious_radix_sort::RadixSort;

use crate::{phast::{Conf, SeedChooser, SeedChooserCore, SeedCore, WINDOW_SIZE, builder::{BuildConf, build_mt, build_st}, conf::{Core, CoreConf}, evaluator::BucketToActivateEvaluator, function::SeedEx}, seeds::SeedSize};
use std::hash::{BuildHasher, Hash, RandomState};

/// Map-or-bump function that assigns different numbers to some keys and `None` to other.
/// 
/// Can be used with any seed chooser (which specify a particular PHast variant):
/// [`ShiftOnlyWrapped`], [`ShiftSeedWrapped`], [`SeedOnly`], [`SeedOnlyK`].
/// 
/// See:
/// Piotr Beling, Peter Sanders, *PHast - Perfect Hashing made fast*, 2025, <https://arxiv.org/abs/2504.17918>
pub struct Partial<C, SS, SCC = SeedCore, S = RandomState> where C: Core, SS: SeedSize {
    seeds: SeedEx<SS::VecElement, C>,
    hasher: S,
    seed_chooser: SCC,
    seed_size: SS,
}

impl<C: Core, SC, SS: SeedSize, S> GetSize for Partial<C, SS, SC, S> {
    fn size_bytes_dyn(&self) -> usize { self.seeds.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.seeds.size_bytes_content_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> Partial<C, SS, SCC, ()> {
    pub fn with_hashes_bps_conf_sc_be_u<'k, BE, SC>(hashes: &'k mut [u64], seed_size: SS, core: C, seed_chooser: SC, bucket_evaluator: BE) -> (Self, usize)
        where BE: BucketToActivateEvaluator, SC: SeedChooser<Core = SCC>
    {
        let (f, build_conf) = Self::build_st(hashes, seed_size, core, (), seed_chooser, bucket_evaluator);
        let unassigned = build_conf.bumped_len(&f.seeds.seeds);
        (f, unassigned)
    }

    pub fn with_hashes_bps_conf_sc_u<'k, SC>(hashes: &'k mut [u64], seed_size: SS, core: C, seed_chooser: SC) -> (Self, usize)
        where SC: SeedChooser<Core=SCC>
    {
        let bucket_evaluator = seed_chooser.bucket_evaluator(seed_size.into(), core.slice_len());
        Self::with_hashes_bps_conf_sc_be_u(hashes, seed_size, core, seed_chooser, bucket_evaluator)
    }

    pub fn with_hashes_bps_conf_bs_threads_sc_be_u<'k, BE, SC>(hashes: &'k mut [u64], seed_size: SS, conf: C, threads_num: usize, seed_chooser: SC, bucket_evaluator: BE) -> (Self, usize)
        where BE: BucketToActivateEvaluator + Sync, SC: SeedChooser<Core = SCC>, BE::Value: Send
    {
        let (f, build_conf) = Self::build_mt(hashes, seed_size, conf, threads_num, (), seed_chooser, bucket_evaluator);
        let unassigned = build_conf.bumped_len(&f.seeds.seeds);
        (f, unassigned)
    }


    pub fn with_hashes_bps_conf_sc_be<'k, BE, SC>(hashes: &'k mut [u64], seed_size: SS, conf: C, seed_chooser: SC, bucket_evaluator: BE) -> Self
        where BE: BucketToActivateEvaluator, SC: SeedChooser<Core = SCC>
    {
        Self::build_st(hashes, seed_size, conf, (), seed_chooser, bucket_evaluator).0
    }

    pub fn with_hashes_bps_conf_bs_threads_sc_be<'k, BE, SC>(hashes: &'k mut [u64], seed_size: SS, conf: C, threads_num: usize, seed_chooser: SC, bucket_evaluator: BE) -> Self
        where BE: BucketToActivateEvaluator + Sync, SC: SeedChooser<Core = SCC>, BE::Value: Send
    {
        Self::build_mt(hashes, seed_size, conf, threads_num, (), seed_chooser, bucket_evaluator).0
    }


    pub fn with_hashes_p_sc_be_u<'k, CC, BE, SC>(hashes: &'k mut [u64], params: &Conf<SS, CC>, seed_chooser: SC, bucket_evaluator: BE) -> (Self, usize)
        where CC: CoreConf<Core = C>, SC: SeedChooser<Core = SCC>, BE: BucketToActivateEvaluator
    {
        let conf = seed_chooser.minimal_f_core(hashes.len(), &params.core_conf, params.bits_per_seed());
        Self::with_hashes_bps_conf_sc_be_u(hashes, params.seed_size, conf, seed_chooser, bucket_evaluator)
    }

    pub fn with_hashes_p_threads_sc_be_u<'k, CC, BE, SC>(hashes: &'k mut [u64], params: &Conf<SS, CC>, threads_num: usize, seed_chooser: SC, bucket_evaluator: BE) -> (Self, usize)
        where CC: CoreConf<Core = C>, BE: BucketToActivateEvaluator + Sync, SC: SeedChooser<Core = SCC>, BE::Value: Send
    {
        let conf = seed_chooser.minimal_f_core(hashes.len(), &params.core_conf, params.bits_per_seed());
        Self::with_hashes_bps_conf_bs_threads_sc_be_u(hashes, params.seed_size, conf, threads_num, seed_chooser, bucket_evaluator)
    }


    pub fn with_hashes_p_sc_be<'k, CC, BE, SC>(hashes: &'k mut [u64], params: &Conf<SS, CC>, seed_chooser: SC, bucket_evaluator: BE) -> Self
        where CC: CoreConf<Core = C>, BE: BucketToActivateEvaluator, SC: SeedChooser<Core = SCC>
    {
        let conf = seed_chooser.minimal_f_core(hashes.len(), &params.core_conf, params.bits_per_seed());
        Self::with_hashes_bps_conf_sc_be(hashes, params.seed_size, conf, seed_chooser, bucket_evaluator)
    }

    pub fn with_hashes_p_threads_sc_be<'k, CC, BE, SC>(hashes: &'k mut [u64], params: &Conf<SS, CC>, threads_num: usize, seed_chooser: SC, bucket_evaluator: BE) -> Self
        where CC: CoreConf<Core = C>, BE: BucketToActivateEvaluator + Sync, SC: SeedChooser<Core = SCC>, BE::Value: Send
    {
        let conf = seed_chooser.minimal_f_core(hashes.len(), &params.core_conf, params.bits_per_seed());
        Self::with_hashes_bps_conf_bs_threads_sc_be(hashes, params.seed_size, conf, threads_num, seed_chooser, bucket_evaluator)
    }


    pub fn with_hashes_p_sc_u<'k, CC, BE, SC>(hashes: &'k mut [u64], params: &Conf<SS, CC>, seed_chooser: SC) -> (Self, usize)
        where CC: CoreConf<Core = C>, SC: SeedChooser<Core = SCC>, BE: BucketToActivateEvaluator
    {
        let conf = seed_chooser.minimal_f_core(hashes.len(), &params.core_conf, params.bits_per_seed());
        let bucket_evaluator = seed_chooser.bucket_evaluator(params.bits_per_seed(), conf.slice_len());
        Self::with_hashes_bps_conf_sc_be_u(hashes, params.seed_size, conf, seed_chooser, bucket_evaluator)
    }

    pub fn with_hashes_p_threads_sc_u<'k, CC, BE, SC>(hashes: &'k mut [u64], params: &Conf<SS, CC>, threads_num: usize, seed_chooser: SC) -> (Self, usize)
        where CC: CoreConf<Core = C>, BE: BucketToActivateEvaluator + Sync, SC: SeedChooser<Core = SCC>, BE::Value: Send
    {
        let conf = seed_chooser.minimal_f_core(hashes.len(), &params.core_conf, params.bits_per_seed());
        let bucket_evaluator = seed_chooser.bucket_evaluator(params.bits_per_seed(), conf.slice_len());
        Self::with_hashes_bps_conf_bs_threads_sc_be_u(hashes, params.seed_size, conf, threads_num, seed_chooser, bucket_evaluator)
    }


    pub fn with_hashes_p_sc<'k, CC, SC>(hashes: &'k mut [u64], params: &Conf<SS, CC>, seed_chooser: SC) -> Self
        where CC: CoreConf<Core = C>, SC: SeedChooser<Core = SCC>
    {
        let conf = seed_chooser.minimal_f_core(hashes.len(), &params.core_conf, params.bits_per_seed());
        let bucket_evaluator = seed_chooser.bucket_evaluator(params.bits_per_seed(), conf.slice_len());
        Self::with_hashes_bps_conf_sc_be(hashes, params.seed_size, conf, seed_chooser, bucket_evaluator)
    }

    pub fn with_hashes_p_threads_sc<'k, CC, SC>(hashes: &'k mut [u64], params: &Conf<SS, CC>, threads_num: usize, seed_chooser: SC) -> Self
        where CC: CoreConf<Core = C>, SC: SeedChooser<Core = SCC>
    {
        let conf = seed_chooser.minimal_f_core(hashes.len(), &params.core_conf, params.bits_per_seed());
        let bucket_evaluator = seed_chooser.bucket_evaluator(params.bits_per_seed(), conf.slice_len());
        Self::with_hashes_bps_conf_bs_threads_sc_be(hashes, params.seed_size, conf, threads_num, seed_chooser, bucket_evaluator)
    }
}



impl<C: Core, SS: SeedSize, SCC: SeedChooserCore, S> Partial<C, SS, SCC, S> {
    /// Returns value assigned to the given key hash or `None`.
    #[inline(always)]
    pub fn get_for_hash(&self, key_hash: u64) -> Option<usize> {
        let seed = unsafe { self.seeds.seed_for(self.seed_size, key_hash) };
        (seed != 0).then(|| self.seed_chooser.f(key_hash, seed, &self.seeds.core))
    }

    fn build_st<'k, BE, SC>(hashes: &'k mut [u64], seed_size: SS, conf: C, hasher: S, seed_chooser: SC, bucket_evaluator: BE)
     -> (Self, BuildConf<'k, C, BE, SS, SC>)
        where BE: BucketToActivateEvaluator, C: Core, SC: SeedChooser<Core=SCC>
    {
        hashes.voracious_sort();
        let (seeds, build_conf) = build_st(hashes, conf, seed_size, bucket_evaluator, seed_chooser.clone());
        (Self {
            seeds: SeedEx{ seeds, core: conf },
            hasher,
            seed_chooser: seed_chooser.core(),
            seed_size
        }, build_conf)
    }

    fn build_mt<'k, BE, SC>(hashes: &'k mut [u64], seed_size: SS, conf: C, threads_num: usize, hasher: S, seed_chooser: SC, bucket_evaluator: BE)
     -> (Self, BuildConf<'k, C, BE, SS, SC>)
        where BE: BucketToActivateEvaluator + Sync, SC: SeedChooser<Core=SCC>, BE::Value: Send, C: Core
    {
        if threads_num == 1 { return Self::build_st(hashes, seed_size, conf, hasher, seed_chooser, bucket_evaluator); }
        hashes.voracious_mt_sort(threads_num);
        let (seeds, build_conf) = build_mt(hashes, conf, seed_size, WINDOW_SIZE, bucket_evaluator, seed_chooser.clone(), threads_num);
        (Self {
            seeds: SeedEx{ seeds, core: conf },
            hasher,
            seed_chooser: seed_chooser.core(),
            seed_size,
        }, build_conf)
    }
}

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore, S> Partial<C, SS, SCC, S> {
    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys,
    /// i.e. 1 + maximum value that minimal function can return.
    #[inline(always)] pub fn minimal_output_range(&self, num_of_keys: u32) -> usize {
        self.seed_chooser.minimal_output_range(num_of_keys as usize)
    }

    /// Returns output range of `self`, i.e. 1 + maximum value that `self` can return.
    pub fn output_range(&self) -> usize {
        self.seeds.core.output_range(self.seed_chooser, self.seed_size.into())
    }
}

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore, S: BuildHasher> Partial<C, SS, SCC, S> {
    /// Returns value assigned to the given `key` or `None`.
    /// 
    /// The returned value is in the range from `0` (inclusive) to the number of elements in the input key collection (exclusive).
    /// `key` must come from the input key collection given during construction.
    #[inline(always)]
    pub fn get<K>(&self, key: &K) -> Option<usize> where K: Hash + ?Sized {
        self.get_for_hash(self.hasher.hash_one(key))
    }

    /// Returns [`Partial`] function and number of keys with unassigned values for given `keys`,
    /// using a single thread and given parameters.
    pub fn with_keys_p_hash_sc_be_u<'k, K, CC, BE, SC>(keys: impl Iterator<Item = K>, params: &Conf<SS, CC>, hasher: S, seed_chooser: SC, bucket_evaluator: BE)
     -> (Self, usize)
        where K: Hash, CC: CoreConf<Core = C>, BE: BucketToActivateEvaluator, SC: SeedChooser<Core=SCC>
    {
        let mut hashes: Box<[_]> = keys.map(|k| hasher.hash_one(k)).collect();
        let conf = seed_chooser.minimal_f_core(hashes.len(), &params.core_conf, params.bits_per_seed());
        let (f, build_conf) = Self::build_st(&mut hashes, params.seed_size, conf, hasher, seed_chooser, bucket_evaluator);
        let unassigned = build_conf.bumped_len(&f.seeds.seeds);
        (f, unassigned)
    }
}