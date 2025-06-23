use dyn_size_of::GetSize;
use seedable_hash::{BuildDefaultSeededHasher, BuildSeededHasher};
use voracious_radix_sort::RadixSort;

use crate::{phast::{builder::{build_mt, build_st, BuildConf}, conf::Conf, evaluator::{BucketToActivateEvaluator, Weights}, function::SeedEx, SeedChooser, SeedOnly, WINDOW_SIZE}, seeds::SeedSize};
use std::hash::Hash;

/// Map-or-bump function that assigns different numbers to some keys and `None` to other.
pub struct Partial<SS, SC = SeedOnly, S = BuildDefaultSeededHasher> where SS: SeedSize {
    seeds: SeedEx<SS>,
    hasher: S,
    seed_chooser: SC
}

impl<SC, SS: SeedSize, S> GetSize for Partial<SS, SC, S> where SeedEx<SS>: GetSize {
    fn size_bytes_dyn(&self) -> usize { self.seeds.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.seeds.size_bytes_content_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl<SS: SeedSize, SC: SeedChooser, S: BuildSeededHasher> Partial<SS, SC, S> {
    /// Returns value assigned to the given `key` or `None`.
    /// 
    /// The returned value is in the range from `0` (inclusive) to the number of elements in the input key collection (exclusive).
    /// `key` must come from the input key collection given during construction.
    #[inline(always)]
    pub fn get<K>(&self, key: &K) -> Option<usize> where K: Hash + ?Sized {
        let key_hash = self.hasher.hash_one(key, 0);
        let seed = self.seeds.seed_for(key_hash);
        (seed != 0).then(|| self.seed_chooser.f(key_hash, seed, &self.seeds.conf))
    }

    fn build<'k, BE, GetBE>(hashes: &'k mut [u64], bits_per_seed: SS, bucket_size100: u16, hasher: S, seed_chooser: SC, bucket_evaluator: GetBE)
     -> (Self, BuildConf<'k, BE, SS, SC>)
        where BE: BucketToActivateEvaluator, GetBE: FnOnce(u16, u8) -> BE
    {
        hashes.voracious_sort();
        let conf = seed_chooser.conf_for_minimal(hashes.len(), bits_per_seed, bucket_size100);
        let (seeds, build_conf) = build_st(hashes, conf, bucket_evaluator(&conf), seed_chooser);
        (Self {
            seeds: SeedEx::<SS>{ seeds, conf },
            hasher,
            seed_chooser,
        }, build_conf)
    }

    fn build_mt<'k, BE, GetBE>(hashes: &'k mut [u64], bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: S, seed_chooser: SC, bucket_evaluator: GetBE)
     -> (Self, BuildConf<'k, BE, SS, SC>)
        where BE: BucketToActivateEvaluator + Sync, SC: Sync, BE::Value: Send, GetBE: FnOnce(&Conf<SS>) -> BE
    {
        if threads_num == 1 { return Self::build(hashes, bits_per_seed, bucket_size100, hasher, seed_chooser, bucket_evaluator); }
        hashes.voracious_mt_sort(threads_num);
        let conf = seed_chooser.conf_for_minimal(hashes.len(), bits_per_seed, bucket_size100);
        let (seeds, build_conf) = build_mt(hashes, conf, bucket_size100, WINDOW_SIZE, bucket_evaluator(&conf), seed_chooser, threads_num);
        (Self {
            seeds: SeedEx::<SS>{ seeds, conf },
            hasher,
            seed_chooser,
        }, build_conf)
    }

    /// Returns [`Partial`] function and number of keys with unassigned values for given `keys`,
    /// using a single thread and given parameters.
    pub fn with_hashes_bps_bs_hash_sc_be_u<'k, K, BE>(keys: impl Iterator<Item = K>, bits_per_seed: SS, bucket_size100: u16, hasher: S, seed_chooser: SC, bucket_evaluator: BE)
     -> (Self, usize)
        where K: Hash, BE: BucketToActivateEvaluator
    {
        let mut hashes: Box<[_]> = keys.map(|k| hasher.hash_one(k, 0)).collect();
        let (f, build_conf) = Self::build(&mut hashes, bits_per_seed, bucket_size100, hasher, seed_chooser, |_| bucket_evaluator);
        let unassigned = build_conf.unassigned_len(&f.seeds.seeds);
        (f, unassigned)
    }

    /// Returns [`Partial`] function for given `keys`, using a single thread and given parameters.
    pub fn with_hashes_bps_bs_hash_sc_be<'k, K, BE>(keys: impl Iterator<Item = K>, bits_per_seed: SS, bucket_size100: u16, hasher: S, seed_chooser: SC, bucket_evaluator: BE)
     -> Self
        where K: Hash, BE: BucketToActivateEvaluator
    {
        let mut hashes: Box<[_]> = keys.map(|k| hasher.hash_one(k, 0)).collect();
        let (f, _) = Self::build(&mut hashes, bits_per_seed, bucket_size100, hasher, seed_chooser, |_| bucket_evaluator);
        f
    }

    /// Returns [`Partial`] function and number of keys with unassigned values for given `keys`,
    /// using a single thread and given parameters.
    pub fn with_hashes_bps_bs_hash_sc_u<'k, K, BE>(keys: impl Iterator<Item = K>, bits_per_seed: SS, bucket_size100: u16, hasher: S, seed_chooser: SC)
     -> (Self, usize) where K: Hash
    {
        let mut hashes: Box<[_]> = keys.map(|k| hasher.hash_one(k, 0)).collect();
        let (f, build_conf) = Self::build(&mut hashes, bits_per_seed, bucket_size100, hasher, seed_chooser, |bps, sl| Weights::new(bps, sl));
        let unassigned = build_conf.unassigned_len(&f.seeds.seeds);
        (f, unassigned)
    }

    /// Returns [`Partial`] function for given `keys`, using a single thread and given parameters.
    pub fn with_hashes_bps_bs_hash_sc<'k, K, BE>(keys: impl Iterator<Item = K>, bits_per_seed: SS, bucket_size100: u16, hasher: S, seed_chooser: SC)
     -> Self where K: Hash
    {
        let mut hashes: Box<[_]> = keys.map(|k| hasher.hash_one(k, 0)).collect();
        let (f, _) = Self::build(&mut hashes, bits_per_seed, bucket_size100, hasher, seed_chooser, |conf| Weights::new(conf.bits_per_seed(), conf.slice_len()));
        f
    }
}