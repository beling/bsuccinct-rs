use dyn_size_of::GetSize;
use seedable_hash::{BuildDefaultSeededHasher, BuildSeededHasher};
use voracious_radix_sort::RadixSort;
use std::hash::Hash;
use rayon::prelude::*;

use crate::{phast::{bits_per_seed_to_100_bucket_size, builder::{build_mt, build_st}, evaluator::Weights, function::{Level, SeedEx}, SeedChooser, SeedOnly, SeedOnlyK, WINDOW_SIZE}, seeds::{Bits8, SeedSize}};

/// PHast (Perfect Hashing with fast evaluation) Perfect (not necessary minimal) Hash Function.
/// Experimental.
/// 
/// Perfect Hash Function with very fast evaluation developed by Piotr Beling and Peter Sanders.
/// 
/// See:
/// Piotr Beling, Peter Sanders, *PHast - Perfect Hashing with fast evaluation*, 2025, <https://arxiv.org/abs/2504.17918>
pub struct Perfect<SS: SeedSize, SC = SeedOnly, S = BuildDefaultSeededHasher>
{
    level0: SeedEx<SS::VecElement>,
    levels: Box<[Level<SS::VecElement>]>,
    hasher: S,
    seed_chooser: SC,
    seed_size: SS
}

impl<SC, SS: SeedSize, S> GetSize for Perfect<SS, SC, S> {
    fn size_bytes_dyn(&self) -> usize {
        self.level0.size_bytes_dyn() + self.levels.size_bytes_dyn()
    }
    fn size_bytes_content_dyn(&self) -> usize {
        self.level0.size_bytes_content_dyn() + self.levels.size_bytes_content_dyn()
    }
    const USES_DYN_MEM: bool = true;
}

impl<SS: SeedSize, SC: SeedChooser, S: BuildSeededHasher> Perfect<SS, SC, S> {
    
    /// Returns value assigned to the given `key`.
    /// 
    /// The returned value is in the range from `0` (inclusive) to the number of elements in the input key collection (exclusive).
    /// `key` must come from the input key collection given during construction.
    #[inline(always)]   //inline(always) is important here
    pub fn get<K>(&self, key: &K) -> usize where K: Hash + ?Sized {
        let key_hash = self.hasher.hash_one(key, 0);
        let seed = self.level0.seed_for(self.seed_size, key_hash);
        if seed != 0 { return self.seed_chooser.f(key_hash, seed, &self.level0.conf); }

        for level_nr in 0..self.levels.len() {
            let l = &self.levels[level_nr];
            let key_hash = self.hasher.hash_one(key, level_nr as u64 + 1);
            let seed = l.seeds.seed_for(self.seed_size, key_hash);
            if seed != 0 {
                return self.seed_chooser.f(key_hash, seed, &l.seeds.conf) + l.shift
            }
        }

        unreachable!("phast::Perfect::get called for key not included in the input set")
    }

    /// Constructs [`Perfect`] function for given `keys`, using a single thread and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_vec_bps_bs_hash_sc<K>(mut keys: Vec::<K>, bits_per_seed: SS, bucket_size100: u16, hasher: S, seed_chooser: SC) -> Self where K: Hash {
        Self::_new(|h| {
            let level0 = Self::build_level_st(&mut keys, bits_per_seed, bucket_size100, h, seed_chooser, 0);
            (keys, level0)
        }, |keys, level_nr, h| {
            Self::build_level_st(keys, bits_per_seed, bucket_size100, h, seed_chooser, level_nr)
        }, hasher, seed_chooser, bits_per_seed)
    }

    /// Constructs [`Perfect`] function for given `keys`, using multiple (given number of) threads and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_vec_bps_bs_threads_hash_sc<K>(mut keys: Vec::<K>, bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: S, seed_chooser: SC) -> Self
        where K: Hash+Sync+Send, S: Sync, SC: Sync {
        if threads_num == 1 { return Self::with_vec_bps_bs_hash_sc(keys, bits_per_seed, bucket_size100, hasher, seed_chooser); }
        Self::_new(|h| {
            let level0 = Self::build_level_mt(&mut keys, bits_per_seed, bucket_size100, threads_num, &h, seed_chooser, 0);
            (keys, level0)
        }, |keys, level_nr, h| {
            Self::build_level_mt(keys, bits_per_seed, bucket_size100, threads_num, &h, seed_chooser, level_nr)
        }, hasher, seed_chooser, bits_per_seed)
    }


    /// Constructs [`Perfect`] function for given `keys`, using a single thread and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_slice_bps_bs_hash_sc<K>(keys: &[K], bits_per_seed: SS, bucket_size100: u16, hasher: S, seed_chooser: SC) -> Self where K: Hash+Clone {
        Self::_new(|h| {
            Self::build_level_from_slice_st(keys, bits_per_seed, bucket_size100, h, seed_chooser, 0)
        }, |keys, level_nr, h| {
            Self::build_level_st(keys, bits_per_seed, bucket_size100, &h, seed_chooser, level_nr)
        }, hasher, seed_chooser, bits_per_seed)
    }


    /// Constructs [`Perfect`] function for given `keys`, using multiple (given number of) threads and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_slice_bps_bs_threads_hash_sc<K>(keys: &[K], bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: S, seed_chooser: SC) -> Self
        where K: Hash+Sync+Send+Clone, S: Sync, SC: Sync {
        if threads_num == 1 { return Self::with_slice_bps_bs_hash_sc(keys, bits_per_seed, bucket_size100, hasher, seed_chooser); }
        Self::_new(|h| {
            Self::build_level_from_slice_mt(keys, bits_per_seed, bucket_size100, threads_num, h, seed_chooser, 0)
        }, |keys, level_nr, h| {
            Self::build_level_mt(keys, bits_per_seed, bucket_size100, threads_num, h, seed_chooser, level_nr)
        }, hasher, seed_chooser, bits_per_seed)
    }

    #[inline]
    fn _new<K, BF, BL>(build_first: BF, build_level: BL, hasher: S, seed_chooser: SC, seed_size: SS) -> Self
        where BF: FnOnce(&S) -> (Vec::<K>, SeedEx<SS::VecElement>),
            BL: Fn(&mut Vec::<K>, u64, &S) -> SeedEx<SS::VecElement>,
            K: Hash
        {
        let (mut keys, level0) = build_first(&hasher);
        let mut shift = level0.conf.output_range(seed_chooser, seed_size);
        let mut levels = Vec::with_capacity(16);
        while !keys.is_empty() {
            let seeds = build_level(&mut keys, levels.len() as u64+1, &hasher);
            let out_range = seeds.conf.output_range(seed_chooser, seed_size);
            levels.push(Level { seeds, shift });
            shift += out_range;
        }
        Self {
            level0,
            levels: levels.into_boxed_slice(),
            hasher,
            seed_chooser,
            seed_size,
        }
    }

    #[inline]
    fn build_level_from_slice_st<K>(keys: &[K], bits_per_seed: SS, bucket_size100: u16, hasher: &S, seed_chooser: SC, level_nr: u64)
        -> (Vec<K>, SeedEx<SS::VecElement>)
        where K: Hash+Clone
    {
        let mut hashes: Box<[_]> = keys.iter().map(|k| hasher.hash_one(k, level_nr)).collect();
        //radsort::unopt::sort(&mut hashes);
        hashes.voracious_sort();
        let conf = seed_chooser.conf_for_minimal(hashes.len(), bits_per_seed, bucket_size100);
        let (seeds, builder) =
            build_st(&hashes, conf, bits_per_seed, Weights::new(bits_per_seed.into(), conf.slice_len()), seed_chooser);
        let mut keys_vec = Vec::with_capacity(builder.unassigned_len(&seeds));
        drop(builder);
        keys_vec.extend(keys.into_iter().filter(|key| {
            bits_per_seed.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, level_nr))) == 0
        }).cloned());
        (keys_vec, SeedEx{ seeds, conf })
    }

    #[inline]
    fn build_level_from_slice_mt<K>(keys: &[K], bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: &S, seed_chooser: SC, level_nr: u64)
        -> (Vec<K>, SeedEx<SS::VecElement>)
        where K: Hash+Sync+Send+Clone, S: Sync, SC: Sync
    {
        let mut hashes: Box<[_]> = if keys.len() > 4*2048 {    //maybe better for string keys
            keys.par_iter().with_min_len(256).map(|k| hasher.hash_one(k, level_nr)).collect()
        } else {
            keys.iter().map(|k| hasher.hash_one(k, level_nr)).collect()
        };
        //radsort::unopt::sort(&mut hashes);
        hashes.voracious_mt_sort(threads_num);
        let conf = seed_chooser.conf_for_minimal(hashes.len(), bits_per_seed, bucket_size100);
        let (seeds, builder) =
            build_mt(&hashes, conf, bits_per_seed, bucket_size100, WINDOW_SIZE, Weights::new(bits_per_seed.into(), conf.slice_len()), seed_chooser, threads_num);
        let mut keys_vec = Vec::with_capacity(builder.unassigned_len(&seeds));
        drop(builder);
        keys_vec.par_extend(keys.into_par_iter().filter(|key| {
            bits_per_seed.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, level_nr))) == 0
        }).cloned());
        (keys_vec, SeedEx{ seeds, conf })
    }

    #[inline(always)]
    fn build_level_st<K>(keys: &mut Vec::<K>, bits_per_seed: SS, bucket_size100: u16, hasher: &S, seed_chooser: SC, level_nr: u64) -> SeedEx<SS::VecElement>
        where K: Hash
    {
        let mut hashes: Box<[_]> = keys.iter().map(|k| hasher.hash_one(k, level_nr)).collect();
        hashes.voracious_sort();
        let conf = seed_chooser.conf_for_minimal(hashes.len(), bits_per_seed, bucket_size100);
        let (seeds, _) =
            build_st(&hashes, conf, bits_per_seed, Weights::new(bits_per_seed.into(), conf.slice_len()), seed_chooser);
        keys.retain(|key| {
            bits_per_seed.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, level_nr))) == 0
        });
        SeedEx{ seeds, conf }
    }

    #[inline]
    fn build_level_mt<K>(keys: &mut Vec::<K>, bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: &S, seed_chooser: SC, level_nr: u64)
        -> SeedEx<SS::VecElement>
        where K: Hash+Sync+Send, S: Sync, SC: Sync
    {
        let mut hashes: Box<[_]> = if keys.len() > 4*2048 {    //maybe better for string keys
            //let mut k = Vec::with_capacity(keys.len());
            //k.par_extend(keys.par_iter().with_min_len(10000).map(|k| hasher.hash_one_s64(k, level_nr)));
            //k.into_boxed_slice()
            keys.par_iter().with_min_len(256).map(|k| hasher.hash_one(k, level_nr)).collect()
        } else {
            keys.iter().map(|k| hasher.hash_one(k, level_nr)).collect()
        };
        //radsort::unopt::sort(&mut hashes);
        hashes.voracious_mt_sort(threads_num);
        let conf = seed_chooser.conf_for_minimal(hashes.len(), bits_per_seed, bucket_size100);
        let (seeds, builder) =
            build_mt(&hashes, conf, bits_per_seed, bucket_size100, WINDOW_SIZE, Weights::new(bits_per_seed.into(), conf.slice_len()), seed_chooser, threads_num);
        let mut result = Vec::with_capacity(builder.unassigned_len(&seeds));
        drop(builder);
        std::mem::swap(keys, &mut result);
        keys.par_extend(result.into_par_iter().filter(|key| {
            bits_per_seed.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, level_nr))) == 0
        }));
        SeedEx{ seeds, conf }
    }

    /// Returns maximum number of keys which can be mapped to the same value by `k`-[`Perfect`] function `self`.
    #[inline(always)] pub fn k(&self) -> u8 { self.seed_chooser.k() }

    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys,
    /// i.e. 1 + maximum value that minimal function can return.
    #[inline(always)] pub fn minimal_output_range(&self, num_of_keys: usize) -> usize { self.seed_chooser.minimal_output_range(num_of_keys) }

    /// Returns output range of `self`, i.e. 1 + maximum value that `self` can return.
    pub fn output_range(&self) -> usize {
        if let Some(last_level) = self.levels.last() {
            last_level.shift + last_level.seeds.conf.output_range(self.seed_chooser, self.seed_size)
        } else {
            self.level0.conf.output_range(self.seed_chooser, self.seed_size)
        }
    }
}

impl Perfect<Bits8, SeedOnly, BuildDefaultSeededHasher> {
    /// Constructs [`Perfect`] function for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_st<K>(keys: Vec::<K>) -> Self where K: Hash {
        Self::with_vec_bps_bs_hash_sc(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        BuildDefaultSeededHasher::default(), SeedOnly)
    }

    /// Constructs [`Perfect`] function for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_mt<K>(keys: Vec::<K>) -> Self where K: Hash+Send+Sync {
        Self::with_vec_bps_bs_threads_hash_sc(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        std::thread::available_parallelism().map_or(1, |v| v.into()), BuildDefaultSeededHasher::default(), SeedOnly)
    }

    /// Constructs [`Perfect`] function for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_st<K>(keys: &[K]) -> Self where K: Hash+Clone {
        Self::with_slice_bps_bs_hash_sc(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        BuildDefaultSeededHasher::default(), SeedOnly)
    }

    /// Constructs [`Perfect`] function for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_mt<K>(keys: &[K]) -> Self where K: Hash+Clone+Send+Sync {
        Self::with_slice_bps_bs_threads_hash_sc(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        std::thread::available_parallelism().map_or(1, |v| v.into()), BuildDefaultSeededHasher::default(), SeedOnly)
    }
}

impl Perfect<Bits8, SeedOnlyK, BuildDefaultSeededHasher> {
    /// Constructs `k`-[`Perfect`] function for given `keys`, using a single thread.
    /// `k`-[`Perfect`] function maps `k` or less different keys to each value.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn k_from_vec_st<K>(k: u8, keys: Vec::<K>) -> Self where K: Hash {
        Self::with_vec_bps_bs_hash_sc(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        BuildDefaultSeededHasher::default(), SeedOnlyK(k))
    }

    /// Constructs `k`-[`Perfect`] function for given `keys`, using multiple threads.
    /// `k`-[`Perfect`] function maps `k` or less different keys to each value.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn k_from_vec_mt<K>(k: u8, keys: Vec::<K>) -> Self where K: Hash+Send+Sync {
        Self::with_vec_bps_bs_threads_hash_sc(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        std::thread::available_parallelism().map_or(1, |v| v.into()), BuildDefaultSeededHasher::default(), SeedOnlyK(k))
    }

    /// Constructs `k`-[`Perfect`] function for given `keys`, using a single thread.
    /// `k`-[`Perfect`] function maps `k` or less different keys to each value.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn k_from_slice_st<K>(k: u8, keys: &[K]) -> Self where K: Hash+Clone {
        Self::with_slice_bps_bs_hash_sc(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        BuildDefaultSeededHasher::default(), SeedOnlyK(k))
    }

    /// Constructs `k`-[`Perfect`] function for given `keys`, using multiple threads.
    /// `k`-[`Perfect`] function maps `k` or less different keys to each value.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn k_from_slice_mt<K>(k: u8, keys: &[K]) -> Self where K: Hash+Clone+Send+Sync {
        Self::with_slice_bps_bs_threads_hash_sc(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        std::thread::available_parallelism().map_or(1, |v| v.into()), BuildDefaultSeededHasher::default(), SeedOnlyK(k))
    }
}


#[cfg(test)]
pub(crate) mod tests {
    use std::fmt::Display;

    use bitm::{BitAccess, BitVec};

    use super::*;

    fn phf_test<SC, K: Display+Hash, SS: SeedSize, S: BuildSeededHasher>(f: &Perfect<SS, SC, S>, keys: &[K])
        where SC: SeedChooser
    {
        let expected_range = 8 * keys.len();
        let mut seen_values = Box::with_zeroed_bits(expected_range);
        for key in keys {
            let v = f.get(&key);
            assert!(v < expected_range, "f({key})={v} exceeds 8*number of keys = {}", expected_range-1);
            assert!(!seen_values.get_bit(v as usize), "f returned the same value {v} for {key} and another key");
            seen_values.set_bit(v as usize);
        }
    }

    fn kphf_test<K: Display+Hash, SS: SeedSize, S: BuildSeededHasher>(f: &Perfect<SS, SeedOnlyK, S>, keys: &[K]) {
        let k = f.k();
        let expected_range = 8 * keys.len() / k as usize;
        let mut seen_values = vec![0; expected_range];
        for key in keys {
            let v = f.get(&key);
            assert!(v < expected_range, "f({key})={v} exceeds 8*number of keys = {}", expected_range-1);
            assert!(seen_values[v as usize] < k, "f returned the same value {v} for {key} and {k} another keys");
            seen_values[v as usize] += 1;
        }
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