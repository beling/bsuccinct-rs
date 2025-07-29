use dyn_size_of::GetSize;
use voracious_radix_sort::RadixSort;

use crate::{phast::{builder::{build_mt, build_st, BuildConf}, conf::Conf, evaluator::BucketToActivateEvaluator, function::SeedEx, Params, SeedChooser, SeedOnly, WINDOW_SIZE}, seeds::SeedSize};
use std::hash::{BuildHasher, Hash, RandomState};

/// Map-or-bump function that assigns different numbers to some keys and `None` to other.
/// 
/// Can be used with any seed chooser (which specify a particular PHast variant):
/// [`ShiftOnlyWrapped`], [`ShiftSeedWrapped`], [`SeedOnly`], [`SeedOnlyK`].
/// 
/// See:
/// Piotr Beling, Peter Sanders, *PHast - Perfect Hashing made fast*, 2025, <https://arxiv.org/abs/2504.17918>
pub struct Partial<SS, SC = SeedOnly, S = RandomState> where SS: SeedSize {
    seeds: SeedEx<SS::VecElement>,
    hasher: S,
    seed_chooser: SC,
    seed_size: SS,
}

impl<SC, SS: SeedSize, S> GetSize for Partial<SS, SC, S> {
    fn size_bytes_dyn(&self) -> usize { self.seeds.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.seeds.size_bytes_content_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl<SS: SeedSize, SC: SeedChooser> Partial<SS, SC, ()> {
    pub fn with_hashes_bps_conf_sc_be_u<'k, BE>(hashes: &'k mut [u64], seed_size: SS, conf: Conf, seed_chooser: SC, bucket_evaluator: BE) -> (Self, usize)
        where BE: BucketToActivateEvaluator
    {
        let (f, build_conf) = Self::build_st(hashes, seed_size, conf, (), seed_chooser, bucket_evaluator);
        let unassigned = build_conf.unassigned_len(&f.seeds.seeds);
        (f, unassigned)
    }

    pub fn with_hashes_bps_conf_bs_threads_sc_be_u<'k, BE>(hashes: &'k mut [u64], seed_size: SS, conf: Conf, threads_num: usize, seed_chooser: SC, bucket_evaluator: BE) -> (Self, usize)
        where BE: BucketToActivateEvaluator + Sync, SC: Sync, BE::Value: Send
    {
        let (f, build_conf) = Self::build_mt(hashes, seed_size, conf, threads_num, (), seed_chooser, bucket_evaluator);
        let unassigned = build_conf.unassigned_len(&f.seeds.seeds);
        (f, unassigned)
    }


    pub fn with_hashes_bps_conf_sc_be<'k, BE>(hashes: &'k mut [u64], seed_size: SS, conf: Conf, seed_chooser: SC, bucket_evaluator: BE) -> Self
        where BE: BucketToActivateEvaluator
    {
        Self::build_st(hashes, seed_size, conf, (), seed_chooser, bucket_evaluator).0
    }

    pub fn with_hashes_bps_conf_bs_threads_sc_be<'k, BE>(hashes: &'k mut [u64], seed_size: SS, conf: Conf, threads_num: usize, seed_chooser: SC, bucket_evaluator: BE) -> Self
        where BE: BucketToActivateEvaluator + Sync, SC: Sync, BE::Value: Send
    {
        Self::build_mt(hashes, seed_size, conf, threads_num, (), seed_chooser, bucket_evaluator).0
    }


    pub fn with_hashes_p_sc_be_u<'k, BE>(hashes: &'k mut [u64], params: &Params<SS>, seed_chooser: SC, bucket_evaluator: BE) -> (Self, usize)
        where BE: BucketToActivateEvaluator
    {
        let conf = seed_chooser.conf_for_minimal_p(hashes.len(), params);
        Self::with_hashes_bps_conf_sc_be_u(hashes, params.seed_size, conf, seed_chooser, bucket_evaluator)
    }

    pub fn with_hashes_p_threads_sc_be_u<'k, BE>(hashes: &'k mut [u64], params: &Params<SS>, threads_num: usize, seed_chooser: SC, bucket_evaluator: BE) -> (Self, usize)
        where BE: BucketToActivateEvaluator + Sync, SC: Sync, BE::Value: Send
    {
        let conf = seed_chooser.conf_for_minimal_p(hashes.len(), params);
        Self::with_hashes_bps_conf_bs_threads_sc_be_u(hashes, params.seed_size, conf, threads_num, seed_chooser, bucket_evaluator)
    }


    pub fn with_hashes_p_sc_be<'k, BE>(hashes: &'k mut [u64], params: &Params<SS>, seed_chooser: SC, bucket_evaluator: BE) -> Self
        where BE: BucketToActivateEvaluator
    {
        let conf = seed_chooser.conf_for_minimal_p(hashes.len(), params);
        Self::with_hashes_bps_conf_sc_be(hashes, params.seed_size, conf, seed_chooser, bucket_evaluator)
    }

    pub fn with_hashes_p_threads_sc_be<'k, BE>(hashes: &'k mut [u64], params: &Params<SS>, threads_num: usize, seed_chooser: SC, bucket_evaluator: BE) -> Self
        where BE: BucketToActivateEvaluator + Sync, SC: Sync, BE::Value: Send
    {
        let conf = seed_chooser.conf_for_minimal_p(hashes.len(), params);
        Self::with_hashes_bps_conf_bs_threads_sc_be(hashes, params.seed_size, conf, threads_num, seed_chooser, bucket_evaluator)
    }


    pub fn with_hashes_p_sc_u<'k, BE>(hashes: &'k mut [u64], params: &Params<SS>, seed_chooser: SC) -> (Self, usize)
        where BE: BucketToActivateEvaluator
    {
        let conf = seed_chooser.conf_for_minimal_p(hashes.len(), params);
        let bucket_evaluator = seed_chooser.bucket_evaluator(params.bits_per_seed(), conf.slice_len());
        Self::with_hashes_bps_conf_sc_be_u(hashes, params.seed_size, conf, seed_chooser, bucket_evaluator)
    }

    pub fn with_hashes_p_threads_sc_u<'k, BE>(hashes: &'k mut [u64], params: &Params<SS>, threads_num: usize, seed_chooser: SC) -> (Self, usize)
        where BE: BucketToActivateEvaluator + Sync, SC: Sync, BE::Value: Send
    {
        let conf = seed_chooser.conf_for_minimal_p(hashes.len(), params);
        let bucket_evaluator = seed_chooser.bucket_evaluator(params.bits_per_seed(), conf.slice_len());
        Self::with_hashes_bps_conf_bs_threads_sc_be_u(hashes, params.seed_size, conf, threads_num, seed_chooser, bucket_evaluator)
    }


    pub fn with_hashes_p_sc<'k>(hashes: &'k mut [u64], params: &Params<SS>, seed_chooser: SC) -> Self
    {
        let conf = seed_chooser.conf_for_minimal_p(hashes.len(), params);
        let bucket_evaluator = seed_chooser.bucket_evaluator(params.bits_per_seed(), conf.slice_len());
        Self::with_hashes_bps_conf_sc_be(hashes, params.seed_size, conf, seed_chooser, bucket_evaluator)
    }

    pub fn with_hashes_p_threads_sc<'k>(hashes: &'k mut [u64], params: &Params<SS>, threads_num: usize, seed_chooser: SC) -> Self
        where SC: Sync
    {
        let conf = seed_chooser.conf_for_minimal_p(hashes.len(), params);
        let bucket_evaluator = seed_chooser.bucket_evaluator(params.bits_per_seed(), conf.slice_len());
        Self::with_hashes_bps_conf_bs_threads_sc_be(hashes, params.seed_size, conf, threads_num, seed_chooser, bucket_evaluator)
    }
}



impl<SS: SeedSize, SC: SeedChooser, S> Partial<SS, SC, S> {
    /// Returns value assigned to the given key hash or `None`.
    #[inline(always)]
    pub fn get_for_hash(&self, key_hash: u64) -> Option<usize> {
        let seed = self.seeds.seed_for(self.seed_size, key_hash);
        (seed != 0).then(|| self.seed_chooser.f(key_hash, seed, &self.seeds.conf))
    }

    fn build_st<'k, BE>(hashes: &'k mut [u64], seed_size: SS, conf: Conf, hasher: S, seed_chooser: SC, bucket_evaluator: BE)
     -> (Self, BuildConf<'k, BE, SS, SC>)
        where BE: BucketToActivateEvaluator
    {
        hashes.voracious_sort();
        let (seeds, build_conf) = build_st(hashes, conf, seed_size, bucket_evaluator, seed_chooser);
        (Self {
            seeds: SeedEx{ seeds, conf },
            hasher,
            seed_chooser,
            seed_size
        }, build_conf)
    }

    fn build_mt<'k, BE>(hashes: &'k mut [u64], seed_size: SS, conf: Conf, threads_num: usize, hasher: S, seed_chooser: SC, bucket_evaluator: BE)
     -> (Self, BuildConf<'k, BE, SS, SC>)
        where BE: BucketToActivateEvaluator + Sync, SC: Sync, BE::Value: Send
    {
        if threads_num == 1 { return Self::build_st(hashes, seed_size, conf, hasher, seed_chooser, bucket_evaluator); }
        hashes.voracious_mt_sort(threads_num);
        let (seeds, build_conf) = build_mt(hashes, conf, seed_size, WINDOW_SIZE, bucket_evaluator, seed_chooser, threads_num);
        (Self {
            seeds: SeedEx{ seeds, conf },
            hasher,
            seed_chooser,
            seed_size,
        }, build_conf)
    }
}

impl<SS: SeedSize, SC: SeedChooser, S> Partial<SS, SC, S> {
    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys,
    /// i.e. 1 + maximum value that minimal function can return.
    #[inline(always)] pub fn minimal_output_range(&self, num_of_keys: u32) -> usize {
        self.seed_chooser.minimal_output_range(num_of_keys as usize)
    }

    /// Returns output range of `self`, i.e. 1 + maximum value that `self` can return.
    pub fn output_range(&self) -> usize {
        self.seeds.conf.output_range(self.seed_chooser, self.seed_size.into())
    }
}

impl<SS: SeedSize, SC: SeedChooser, S: BuildHasher> Partial<SS, SC, S> {
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
    pub fn with_keys_p_hash_sc_be_u<'k, K, BE>(keys: impl Iterator<Item = K>, params: &Params<SS>, hasher: S, seed_chooser: SC, bucket_evaluator: BE)
     -> (Self, usize)
        where K: Hash, BE: BucketToActivateEvaluator
    {
        let mut hashes: Box<[_]> = keys.map(|k| hasher.hash_one(k)).collect();
        let conf = seed_chooser.conf_for_minimal_p(hashes.len(), params);
        let (f, build_conf) = Self::build_st(&mut hashes, params.seed_size, conf, hasher, seed_chooser, bucket_evaluator);
        let unassigned = build_conf.unassigned_len(&f.seeds.seeds);
        (f, unassigned)
    }
}