use std::{hash::Hash, usize};

use crate::seeds::{Bits8, SeedSize};
use super::{bits_per_seed_to_100_bucket_size, builder::{build_mt, build_st}, conf::Conf, evaluator::Weights, stats::BuildStats, CompressedArray, CompressedBuilder, DefaultCompressedArray};
use bitm::BitAccess;
use dyn_size_of::GetSize;
use seedable_hash::{BuildDefaultSeededHasher, BuildSeededHasher};
use voracious_radix_sort::RadixSort;
use rayon::prelude::*;

struct SeedEx<SS: SeedSize> {
    seeds: Box<[SS::VecElement]>,
    conf: Conf<SS>,
}

impl<SS: SeedSize> SeedEx<SS> {
    #[inline]
    fn bucket_for(&self, key: u64) -> usize { self.conf.bucket_for(key) }

    #[inline]
    fn seed_for(&self, key: u64) -> u16 {
        //self.seeds.get_fragment(self.bucket_for(key), self.conf.bits_per_seed()) as u16
        self.conf.bits_per_seed.get_seed(&self.seeds, self.bucket_for(key))
    }

    #[inline]
    fn get(&self, key: u64, seed: u16) -> usize {
        self.conf.f(key, seed)
    }
}

impl<SS: SeedSize> GetSize for SeedEx<SS> {
    fn size_bytes_dyn(&self) -> usize { self.seeds.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.seeds.size_bytes_content_dyn() }
    const USES_DYN_MEM: bool = true;
}


struct Level<SS: SeedSize> {
    seeds: SeedEx<SS>,
    shift: usize
}

impl<SS: SeedSize> GetSize for Level<SS> {
    fn size_bytes_dyn(&self) -> usize { self.seeds.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.seeds.size_bytes_content_dyn() }
    const USES_DYN_MEM: bool = true;
}

/// PHast (Perfect Hashing with fast evaluation). Experimental.
/// 
/// Perfect hash function with very fast evaluation and size below 2 bits/key
/// developed by Peter Sanders and Piotr Beling.
pub struct Function<SS: SeedSize, CA = DefaultCompressedArray, S = BuildDefaultSeededHasher> {
    level0: SeedEx<SS>,
    unassigned: CA,
    levels: Box<[Level<SS>]>,
    hasher: S,
}

impl<SS: SeedSize, CA, S> GetSize for Function<SS, CA, S> where Level<SS>: GetSize, CA: GetSize {
    fn size_bytes_dyn(&self) -> usize {
        self.level0.size_bytes_dyn() +
            self.unassigned.size_bytes_dyn() +
            self.levels.size_bytes_dyn()
    }
    fn size_bytes_content_dyn(&self) -> usize {
        self.level0.size_bytes_content_dyn() +
            self.unassigned.size_bytes_content_dyn() +
            self.levels.size_bytes_content_dyn()
    }
    const USES_DYN_MEM: bool = true;
}

impl<SS: SeedSize, CA: CompressedArray, S: BuildSeededHasher> Function<SS, CA, S> {
    
    /// Returns value assigned to the given `key`.
    /// 
    /// The returned value is in the range from `0` (inclusive) to the number of elements in the input key collection (exclusive).
    /// `key` must come from the input key collection given during construction.
    #[inline]
    pub fn get<K>(&self, key: &K) -> usize where K: Hash + ?Sized {
        let key_hash = self.hasher.hash_one(key, 0);
        let seed = self.level0.seed_for(key_hash);
        if seed != 0 { return self.level0.get(key_hash, seed); }

        for level_nr in 0..self.levels.len() {
            let l = &self.levels[level_nr];
            let key_hash = self.hasher.hash_one(key, level_nr as u64 + 1);
            let seed = l.seeds.seed_for(key_hash);
            if seed != 0 {
                return self.unassigned.get(l.seeds.get(key_hash, seed) + l.shift)
            }
        }
        unreachable!()
    }

    /// Constructs [`Function`] for given `keys`, using a single thread and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_vec_bps_bs_hash<K, St: super::stats::BuildStats>(mut keys: Vec::<K>, bits_per_seed: SS, bucket_size100: u16, hasher: S, stats: St) -> Self where K: Hash {
        Self::_new(|h| {
            let (level0, unassigned_values, unassigned_len) =
                Self::build_level_st(&mut keys, bits_per_seed, bucket_size100, h, 0, stats);
            (keys, level0, unassigned_values, unassigned_len)
        }, |keys, level_nr, h| {
            Self::build_level_st(keys, bits_per_seed, bucket_size100, h, level_nr, ())
        }, hasher)
    }

    /// Constructs [`Function`] for given `keys`, using multiple (given number of) threads and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_vec_bps_bs_threads_hash<K, St>(mut keys: Vec::<K>, bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: S, stats: St) -> Self
        where K: Hash+Sync+Send, S: Sync, St: BuildStats
    {
        if threads_num == 1 { return Self::with_vec_bps_bs_hash(keys, bits_per_seed, bucket_size100, hasher, stats); }
        Self::_new(|h| {
            let (level0, unassigned_values, unassigned_len) =
                Self::build_level_mt(&mut keys, bits_per_seed, bucket_size100, threads_num, &h, 0, stats);
            (keys, level0, unassigned_values, unassigned_len)
        }, |keys, level_nr, h| {
            Self::build_level_mt(keys, bits_per_seed, bucket_size100, threads_num, &h, level_nr, ())
        }, hasher)
    }


    /// Constructs [`Function`] for given `keys`, using a single thread and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_slice_bps_bs_hash<K, St>(keys: &[K], bits_per_seed: SS, bucket_size100: u16, hasher: S, stats: St) -> Self where K: Hash+Clone, St: BuildStats {
        Self::_new(|h| {
            Self::build_level_from_slice_st(keys, bits_per_seed, bucket_size100, h, 0, stats)
        }, |keys, level_nr, h| {
            Self::build_level_st(keys, bits_per_seed, bucket_size100, &h, level_nr, ())
        }, hasher)
    }


    /// Constructs [`Function`] for given `keys`, using multiple (given number of) threads and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_slice_bps_bs_threads_hash<K, St>(keys: &[K], bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: S, stats: St) -> Self
        where K: Hash+Sync+Send+Clone, S: Sync, St: BuildStats 
    {
        if threads_num == 1 { return Self::with_slice_bps_bs_hash(keys, bits_per_seed, bucket_size100, hasher, stats); }
        Self::_new(|h| {
            Self::build_level_from_slice_mt(keys, bits_per_seed, bucket_size100, threads_num, h, 0)
        }, |keys, level_nr, h| {
            Self::build_level_mt(keys, bits_per_seed, bucket_size100, threads_num, &h, level_nr, ())
        }, hasher)
    }

    #[inline]
    fn _new<K, BF, BL>(build_first: BF, build_level: BL, hasher: S) -> Self
        where BF: FnOnce(&S) -> (Vec::<K>, SeedEx<SS>, Box<[u64]>, usize),
            BL: Fn(&mut Vec::<K>, u64, &S) -> (SeedEx<SS>, Box<[u64]>, usize),
        {
        let (mut keys, level0, unassigned_values, unassigned_len) = build_first(&hasher);
        //Self::finish_building(keys, bits_per_seed, bucket_size100, threads_num, hasher, level0, unassigned_values, unassigned_len)
        let mut level0_unassigned = unassigned_values.bit_ones();
        let mut unassigned = Vec::with_capacity(unassigned_len * 3 / 2);

        let mut levels = Vec::with_capacity(16);
        let mut last = 0;
        while !keys.is_empty() {
            let keys_len = keys.len();
            let (seeds, unassigned_values, _unassigned_len) =
                build_level(&mut keys, levels.len() as u64+1, &hasher);
            let shift = unassigned.len();
            for i in 0..keys_len {
                if !unsafe{unassigned_values.get_bit_unchecked(i)} {
                    last = level0_unassigned.next().unwrap();                    
                }
                unassigned.push(last);
            }
            levels.push(Level { seeds, shift });
        }
        debug_assert!(level0_unassigned.next().is_none());
        drop(level0_unassigned);

        let mut builder = CA::Builder::new(unassigned.len(), last);
        builder.push_all(unassigned);

        Self {
            level0,
            unassigned: CA::finish(builder),
            levels: levels.into_boxed_slice(),
            hasher,
        }
    }

    #[inline]
    fn build_level_from_slice_st<K, St: BuildStats>(keys: &[K], bits_per_seed: SS, bucket_size100: u16, hasher: &S, level_nr: u64, mut stats: St)
        -> (Vec<K>, SeedEx<SS>, Box<[u64]>, usize)
        where K: Hash+Clone
    {
        stats.pre_hash();
        let mut hashes: Box<[_]> = keys.iter().map(|k| hasher.hash_one(k, level_nr)).collect();
        stats.pre_sort();
        //radsort::unopt::sort(&mut hashes);
        hashes.voracious_sort();
        stats.pre_seeding();
        let conf = Conf::new(hashes.len(), bits_per_seed, bucket_size100);
        let (seeds, unassigned_values, unassigned_len) =
            build_st(&hashes, conf, Weights::new(conf.bits_per_seed(), conf.partition_size()));
        stats.pre_keys_removing();
        let mut keys_vec = Vec::with_capacity(unassigned_len);
        keys_vec.extend(keys.into_iter().filter(|key| {
            bits_per_seed.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, level_nr))) == 0
        }).cloned());
        stats.post_keys_removing();
        (keys_vec, SeedEx::<SS>{ seeds, conf }, unassigned_values, unassigned_len)
    }

    #[inline]
    fn build_level_from_slice_mt<K>(keys: &[K], bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: &S, level_nr: u64)
        -> (Vec<K>, SeedEx<SS>, Box<[u64]>, usize)
        where K: Hash+Sync+Send+Clone, S: Sync
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
        let conf = Conf::new(hashes.len(), bits_per_seed, bucket_size100);
        let (seeds, unassigned_values, unassigned_len) =
            build_mt(&hashes, conf, bucket_size100, 256, Weights::new(conf.bits_per_seed(), conf.partition_size()), threads_num);
        let mut keys_vec = Vec::with_capacity(unassigned_len);
        keys_vec.par_extend(keys.into_par_iter().filter(|key| {
            bits_per_seed.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, level_nr))) == 0
        }).cloned());
        (keys_vec, SeedEx::<SS>{ seeds, conf }, unassigned_values, unassigned_len)
    }

    #[inline(always)]
    fn build_level_st<K, St: BuildStats>(keys: &mut Vec::<K>, bits_per_seed: SS, bucket_size100: u16, hasher: &S, level_nr: u64, mut stats: St)
        -> (SeedEx<SS>, Box<[u64]>, usize)
        where K: Hash
    {
        stats.pre_hash();
        let mut hashes: Box<[_]> = keys.iter().map(|k| hasher.hash_one(k, level_nr)).collect();
        stats.pre_sort();
        hashes.voracious_sort();
        stats.pre_seeding();
        let conf = Conf::new(hashes.len(), bits_per_seed, bucket_size100);
        let (seeds, unassigned_values, unassigned_len) =
            build_st(&hashes, conf, Weights::new(conf.bits_per_seed(), conf.partition_size()));
        stats.pre_keys_removing();
        keys.retain(|key| {
            bits_per_seed.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, level_nr))) == 0
        });
        stats.post_keys_removing();
        (SeedEx::<SS>{ seeds, conf }, unassigned_values, unassigned_len)
    }

    #[inline]
    fn build_level_mt<K, St: BuildStats>(keys: &mut Vec::<K>, bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: &S, level_nr: u64, mut stats: St)
        -> (SeedEx<SS>, Box<[u64]>, usize)
        where K: Hash+Sync+Send, S: Sync
    {
        stats.pre_hash();
        let mut hashes: Box<[_]> = if keys.len() > 4*2048 {    //maybe better for string keys
            //let mut k = Vec::with_capacity(keys.len());
            //k.par_extend(keys.par_iter().with_min_len(10000).map(|k| hasher.hash_one_s64(k, level_nr)));
            //k.into_boxed_slice()
            keys.par_iter().with_min_len(256).map(|k| hasher.hash_one(k, level_nr)).collect()
        } else {
            keys.iter().map(|k| hasher.hash_one(k, level_nr)).collect()
        };
        //radsort::unopt::sort(&mut hashes);
        stats.pre_sort();
        hashes.voracious_mt_sort(threads_num);
        stats.pre_seeding();
        let conf = Conf::new(hashes.len(), bits_per_seed, bucket_size100);
        let (seeds, unassigned_values, unassigned_len) =
            build_mt(&hashes, conf, bucket_size100, 256, Weights::new(conf.bits_per_seed(), conf.partition_size()), threads_num);
        stats.pre_keys_removing();
        let mut result = Vec::with_capacity(unassigned_len);
        std::mem::swap(keys, &mut result);
        keys.par_extend(result.into_par_iter().filter(|key| {
            bits_per_seed.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, level_nr))) == 0
        }));
        stats.post_keys_removing();
        (SeedEx::<SS>{ seeds, conf }, unassigned_values, unassigned_len)
    }

    /*#[inline(always)]
    fn finish_building<K>(mut keys: Vec::<K>, bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: S, level0: SeedEx<SS>, unassigned_values: Box<[u64]>, unassigned_len: usize) -> Self where K: Hash+Sync+Send, S: Sync {
        let mut level0_unassigned = unassigned_values.bit_ones();
        let mut unassigned = Vec::with_capacity(unassigned_len * 3 / 2);

        let mut levels = Vec::with_capacity(16);
        let mut last = 0;
        while !keys.is_empty() {
            let keys_len = keys.len();
            let (seeds, unassigned_values, _unassigned_len) =
                Self::build_level(&mut keys, bits_per_seed, bucket_size100, threads_num, &hasher, levels.len() as u32+1);
            let shift = unassigned.len();
            for i in 0..keys_len {
                if !unsafe{unassigned_values.get_bit_unchecked(i)} {
                    last = level0_unassigned.next().unwrap();                    
                }
                unassigned.push(last);
            }
            levels.push(Level { seeds, shift });
        }
        debug_assert!(level0_unassigned.next().is_none());
        drop(level0_unassigned);

        let mut builder = CA::Builder::new(unassigned.len(), last);
        builder.push_all(unassigned);

        Self {
            level0,
            unassigned: CA::finish(builder),
            levels: levels.into_boxed_slice(),
            hasher,
        }
    }*/

    /*pub fn new2<K>(mut keys: Vec::<K>, bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: S) -> Self where K: Hash+Sync+Send, S: Sync {
        let keys_len = keys.len();
        let (level0, unassigned_values, _unassigned_len) =
            Self::build_level(&mut keys, bits_per_seed, bucket_size100, threads_num, &hasher, 0);
        let largest_unassigned = bitmap_largest(&unassigned_values, keys_len);

        let mut levels_data = Vec::with_capacity(16);
        let mut total_len = 0;
        while !keys.is_empty() {
            let keys_len = keys.len();
            let (seeds, unassigned_values, _unassigned_len) =
                Self::build_level(&mut keys, bits_per_seed, bucket_size100, threads_num, &hasher, levels_data.len() as u32+1);
            levels_data.push((seeds, unassigned_values, keys_len, total_len));
            total_len += keys_len;
        }
        let mut levels = Vec::with_capacity(levels_data.len());
        let mut builder = CA::Builder::new(total_len, largest_unassigned);
        let mut level0_unassigned = unassigned_values.bit_ones();
        let mut last = 0;
        for (seeds, unassigned_values, keys_len, shift) in levels_data {
            for i in 0..keys_len {
                if !unsafe{unassigned_values.get_bit_unchecked(i)} {
                    last = level0_unassigned.next().unwrap();                    
                }
                builder.push(last);
            }
            levels.push(Level { seeds, shift });
        }
        debug_assert!(level0_unassigned.next().is_none());
        drop(level0_unassigned);

        Self {
            level0,
            unassigned: CA::finish(builder),
            levels: levels.into_boxed_slice(),
            hasher,
        }
    }*/
}

impl Function<Bits8, DefaultCompressedArray, BuildDefaultSeededHasher> {
    /// Constructs [`Function`] for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_st<K, St>(keys: Vec::<K>, stats: St) -> Self where K: Hash, St: BuildStats {
        Self::with_vec_bps_bs_hash(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        BuildDefaultSeededHasher::default(), stats)
    }

    /// Constructs [`Function`] for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_mt<K, St>(keys: Vec::<K>, stats: St) -> Self where K: Hash+Send+Sync, St: BuildStats {
        Self::with_vec_bps_bs_threads_hash(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        std::thread::available_parallelism().map_or(1, |v| v.into()), BuildDefaultSeededHasher::default(),
        stats)
    }

    /// Constructs [`Function`] for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_st<K, St>(keys: &[K], stats: St) -> Self where K: Hash+Clone, St: BuildStats {
        Self::with_slice_bps_bs_hash(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        BuildDefaultSeededHasher::default(), stats)
    }

    /// Constructs [`Function`] for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_mt<K, St>(keys: &[K], stats: St) -> Self where K: Hash+Clone+Send+Sync, St: BuildStats {
        Self::with_slice_bps_bs_threads_hash(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        std::thread::available_parallelism().map_or(1, |v| v.into()), BuildDefaultSeededHasher::default(),
        stats)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::fmt::Display;

    use bitm::{BitAccess, BitVec};

    use super::*;

    fn mphf_test<K: Display+Hash, SS: SeedSize, CA: CompressedArray, S: BuildSeededHasher>(f: &Function<SS, CA, S>, keys: &[K]) {
        let expected_range = keys.len();
        let mut seen_values = Box::with_zeroed_bits(expected_range);
        for key in keys {
            let v = f.get(&key);
            assert!(v < expected_range, "f({key})={v} exceeds maximum value {}", expected_range-1);
            assert!(!seen_values.get_bit(v as usize), "f returned the same value {v} for {key} and another key");
            seen_values.set_bit(v as usize);
        }
    }
    
    #[test]
    fn test_small() {
        let input = [1, 2, 3, 4, 5];
        let f = Function::from_slice_st(&input, ());
        mphf_test(&f, &input);
    }
}