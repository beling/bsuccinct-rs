use std::{hash::Hash, usize};

use crate::seeds::{Bits8, SeedSize};
use super::{bits_per_seed_to_100_bucket_size, builder::{build_mt, build_st}, conf::Conf, evaluator::Weights, seed_chooser::{SeedChooser, SeedOnly}, CompressedArray, DefaultCompressedArray, WINDOW_SIZE};
use bitm::BitAccess;
use dyn_size_of::GetSize;
use seedable_hash::{BuildDefaultSeededHasher, BuildSeededHasher};
use voracious_radix_sort::RadixSort;
use rayon::prelude::*;

/// Represents map-or-bump function.
pub(crate) struct SeedEx<SS: SeedSize> {
    pub(crate) seeds: Box<[SS::VecElement]>,
    pub(crate) conf: Conf<SS>,
}

impl<SS: SeedSize> SeedEx<SS> {
    #[inline(always)]
    pub(crate) fn bucket_for(&self, key: u64) -> usize { self.conf.bucket_for(key) }

    #[inline(always)]
    pub(crate) fn seed_for(&self, key: u64) -> u16 {
        //self.seeds.get_fragment(self.bucket_for(key), self.conf.bits_per_seed()) as u16
        self.conf.bits_per_seed.get_seed(&self.seeds, self.bucket_for(key))
    }
}

impl<SS: SeedSize> GetSize for SeedEx<SS> {
    fn size_bytes_dyn(&self) -> usize { self.seeds.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.seeds.size_bytes_content_dyn() }
    const USES_DYN_MEM: bool = true;
}


pub(crate) struct Level<SS: SeedSize> {
    pub(crate) seeds: SeedEx<SS>,
    pub(crate) shift: usize
}

impl<SS: SeedSize> GetSize for Level<SS> {
    fn size_bytes_dyn(&self) -> usize { self.seeds.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.seeds.size_bytes_content_dyn() }
    const USES_DYN_MEM: bool = true;
}

/// PHast (Perfect Hashing with fast evaluation) Minimal Perfect Hash Function.
/// Experimental.
/// 
/// Minimal Perfect Hash Function with very fast evaluation and size below 2 bits/key
/// developed by Piotr Beling and Peter Sanders.
/// 
/// See:
/// Piotr Beling, Peter Sanders, *PHast - Perfect Hashing with fast evaluation*, 2025, <https://arxiv.org/abs/2504.17918>
pub struct Function<SS, SC = SeedOnly, CA = DefaultCompressedArray, S = BuildDefaultSeededHasher>
    where SS: SeedSize
{
    level0: SeedEx<SS>,
    unassigned: CA,
    levels: Box<[Level<SS>]>,
    hasher: S,
    seed_chooser: SC
}

impl<SC, SS: SeedSize, CA, S> GetSize for Function<SS, SC, CA, S> where Level<SS>: GetSize, CA: GetSize {
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

impl<SS: SeedSize, SC: SeedChooser, CA: CompressedArray, S: BuildSeededHasher> Function<SS, SC, CA, S> {
    
    /// Returns value assigned to the given `key`.
    /// 
    /// The returned value is in the range from `0` (inclusive) to the number of elements in the input key collection (exclusive).
    /// `key` must come from the input key collection given during construction.
    #[inline(always)]   //inline(always) is important here
    pub fn get<K>(&self, key: &K) -> usize where K: Hash + ?Sized {
        let key_hash = self.hasher.hash_one(key, 0);
        let seed = self.level0.seed_for(key_hash);
        if seed != 0 { return self.seed_chooser.f(key_hash, seed, &self.level0.conf); }

        for level_nr in 0..self.levels.len() {
            let l = &self.levels[level_nr];
            let key_hash = self.hasher.hash_one(key, level_nr as u64 + 1);
            let seed = l.seeds.seed_for(key_hash);
            if seed != 0 {
                return self.unassigned.get(self.seed_chooser.f(key_hash, seed, &l.seeds.conf) + l.shift)
            }
        }
        unreachable!()
    }

    /// Constructs [`Function`] for given `keys`, using a single thread and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_vec_bps_bs_hash_sc<K>(mut keys: Vec::<K>, bits_per_seed: SS, bucket_size100: u16, hasher: S, seed_chooser: SC) -> Self where K: Hash {
        let number_of_keys = keys.len();
        Self::_new(|h| {
            let (level0, unassigned_values, unassigned_len) =
                Self::build_level_st(&mut keys, bits_per_seed, bucket_size100, h, seed_chooser, 0);
            (keys, level0, unassigned_values, unassigned_len)
        }, |keys, level_nr, h| {
            Self::build_level_st(keys, bits_per_seed, bucket_size100, h, seed_chooser, level_nr)
        }, hasher, seed_chooser, number_of_keys)
    }

    /// Constructs [`Function`] for given `keys`, using multiple (given number of) threads and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_vec_bps_bs_threads_hash_sc<K>(mut keys: Vec::<K>, bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: S, seed_chooser: SC) -> Self
        where K: Hash+Sync+Send, S: Sync, SC: Sync {
        if threads_num == 1 { return Self::with_vec_bps_bs_hash_sc(keys, bits_per_seed, bucket_size100, hasher, seed_chooser); }
        let number_of_keys = keys.len();
        Self::_new(|h| {
            let (level0, unassigned_values, unassigned_len) =
                Self::build_level_mt(&mut keys, bits_per_seed, bucket_size100, threads_num, &h, seed_chooser, 0);
            (keys, level0, unassigned_values, unassigned_len)
        }, |keys, level_nr, h| {
            Self::build_level_mt(keys, bits_per_seed, bucket_size100, threads_num, &h, seed_chooser, level_nr)
        }, hasher, seed_chooser, number_of_keys)
    }


    /// Constructs [`Function`] for given `keys`, using a single thread and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_slice_bps_bs_hash_sc<K>(keys: &[K], bits_per_seed: SS, bucket_size100: u16, hasher: S, seed_chooser: SC) -> Self where K: Hash+Clone {
        Self::_new(|h| {
            Self::build_level_from_slice_st(keys, bits_per_seed, bucket_size100, h, seed_chooser, 0)
        }, |keys, level_nr, h| {
            Self::build_level_st(keys, bits_per_seed, bucket_size100, &h, seed_chooser, level_nr)
        }, hasher, seed_chooser, keys.len())
    }


    /// Constructs [`Function`] for given `keys`, using multiple (given number of) threads and given parameters:
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
            Self::build_level_mt(keys, bits_per_seed, bucket_size100, threads_num, &h, seed_chooser, level_nr)
        }, hasher, seed_chooser, keys.len())
    }

    #[inline]
    fn _new<K, BF, BL>(build_first: BF, build_level: BL, hasher: S, seed_chooser: SC, number_of_keys: usize) -> Self
        where BF: FnOnce(&S) -> (Vec::<K>, SeedEx<SS>, Box<[u64]>, usize),
            BL: Fn(&mut Vec::<K>, u64, &S) -> (SeedEx<SS>, Box<[u64]>, usize),
            K: Hash
        {
        let (mut keys, level0, unassigned_values, unassigned_len) = build_first(&hasher);
        //Self::finish_building(keys, bits_per_seed, bucket_size100, threads_num, hasher, level0, unassigned_values, unassigned_len)
        let mut level0_unassigned = unassigned_values.bit_ones();
        let mut unassigned = Vec::with_capacity(unassigned_len * 3 / 2);

        let mut levels = Vec::with_capacity(16);
        let mut last = 0;
        while !keys.is_empty() {
            let keys_len = keys.len();
            //println!("{keys_len} {:.2}% keys bumped, {} {}% in {} self-collided buckets",
            //    keys_len as f64 / 100000.0,
                //crate::phast::seed_chooser::SELF_COLLISION_KEYS.load(std::sync::atomic::Ordering::SeqCst),
                //crate::phast::seed_chooser::SELF_COLLISION_KEYS.load(std::sync::atomic::Ordering::SeqCst) * 100 / keys_len as u64,
                //crate::phast::seed_chooser::SELF_COLLISION_BUCKETS.load(std::sync::atomic::Ordering::SeqCst));
            let (seeds, unassigned_values, _unassigned_len) =
                build_level(&mut keys, levels.len() as u64+1, &hasher);
            let shift = unassigned.len();
            for i in 0..keys_len {
                if CA::MAX_FOR_UNUSED {
                    if !unsafe{unassigned_values.get_bit_unchecked(i)} {
                        last = level0_unassigned.next().unwrap();
                        unassigned.push(last);
                    } else {
                        unassigned.push(usize::MAX);
                    }
                } else {
                    if !unsafe{unassigned_values.get_bit_unchecked(i)} {
                        last = level0_unassigned.next().unwrap();
                    }
                    unassigned.push(last);
                }
            }
            levels.push(Level { seeds, shift });
        }
        debug_assert!(level0_unassigned.next().is_none());
        drop(level0_unassigned);
        Self {
            level0,
            unassigned: CA::new(unassigned, last, number_of_keys),
            levels: levels.into_boxed_slice(),
            hasher,
            seed_chooser,
        }
    }

    #[inline]
    fn build_level_from_slice_st<K>(keys: &[K], bits_per_seed: SS, bucket_size100: u16, hasher: &S, seed_chooser: SC, level_nr: u64)
        -> (Vec<K>, SeedEx<SS>, Box<[u64]>, usize)
        where K: Hash+Clone
    {
        let mut hashes: Box<[_]> = keys.iter().map(|k| hasher.hash_one(k, level_nr)).collect();
        //radsort::unopt::sort(&mut hashes);
        hashes.voracious_sort();
        let conf = seed_chooser.conf_for_minimal(hashes.len(), bits_per_seed, bucket_size100);
        let (seeds, builder) =
            build_st(&hashes, conf, Weights::new(conf.bits_per_seed(), conf.slice_len()), seed_chooser);
        let (unassigned_values, unassigned_len) = builder.unassigned_values(&seeds);
        drop(builder);
        let mut keys_vec = Vec::with_capacity(unassigned_len);
        keys_vec.extend(keys.into_iter().filter(|key| {
            bits_per_seed.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, level_nr))) == 0
        }).cloned());
        (keys_vec, SeedEx::<SS>{ seeds, conf }, unassigned_values, unassigned_len)
    }

    #[inline]
    fn build_level_from_slice_mt<K>(keys: &[K], bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: &S, seed_chooser: SC, level_nr: u64)
        -> (Vec<K>, SeedEx<SS>, Box<[u64]>, usize)
        where K: Hash+Sync+Send+Clone, S: Sync, SC: Sync
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
            build_mt(&hashes, conf, bucket_size100, WINDOW_SIZE, Weights::new(conf.bits_per_seed(), conf.slice_len()), seed_chooser, threads_num);
        let (unassigned_values, unassigned_len) = builder.unassigned_values(&seeds);
        drop(builder);
        let mut keys_vec = Vec::with_capacity(unassigned_len);
        keys_vec.par_extend(keys.into_par_iter().filter(|key| {
            bits_per_seed.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, level_nr))) == 0
        }).cloned());
        (keys_vec, SeedEx::<SS>{ seeds, conf }, unassigned_values, unassigned_len)
    }

    #[inline(always)]
    fn build_level_st<K>(keys: &mut Vec::<K>, bits_per_seed: SS, bucket_size100: u16, hasher: &S, seed_chooser: SC, level_nr: u64)
        -> (SeedEx<SS>, Box<[u64]>, usize)
        where K: Hash
    {
        let mut hashes: Box<[_]> = keys.iter().map(|k| hasher.hash_one(k, level_nr)).collect();
        hashes.voracious_sort();
        let conf = seed_chooser.conf_for_minimal(hashes.len(), bits_per_seed, bucket_size100);
        let (seeds, builder) =
            build_st(&hashes, conf, Weights::new(conf.bits_per_seed(), conf.slice_len()), seed_chooser);
        let (unassigned_values, unassigned_len) = builder.unassigned_values(&seeds);
        drop(builder);
        keys.retain(|key| {
            bits_per_seed.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, level_nr))) == 0
        });
        (SeedEx::<SS>{ seeds, conf }, unassigned_values, unassigned_len)
    }

    #[inline]
    fn build_level_mt<K>(keys: &mut Vec::<K>, bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: &S, seed_chooser: SC, level_nr: u64)
        -> (SeedEx<SS>, Box<[u64]>, usize)
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
            build_mt(&hashes, conf, bucket_size100, WINDOW_SIZE, Weights::new(conf.bits_per_seed(), conf.slice_len()), seed_chooser, threads_num);
        let (unassigned_values, unassigned_len) = builder.unassigned_values(&seeds);
        drop(builder);
        let mut result = Vec::with_capacity(unassigned_len);
        std::mem::swap(keys, &mut result);
        keys.par_extend(result.into_par_iter().filter(|key| {
            bits_per_seed.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, level_nr))) == 0
        }));
        (SeedEx::<SS>{ seeds, conf }, unassigned_values, unassigned_len)
    }

    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys,
    /// i.e. 1 + maximum value that minimal function can return.
    #[inline(always)] pub fn minimal_output_range(&self, num_of_keys: usize) -> usize { self.seed_chooser.minimal_output_range(num_of_keys) }

    /// Returns output range of `self`, i.e. 1 + maximum value that `self` can return.
    pub fn output_range(&self) -> usize {
        self.level0.conf.output_range(self.seed_chooser)
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

impl Function<Bits8, SeedOnly, DefaultCompressedArray, BuildDefaultSeededHasher> {
    /// Constructs [`Function`] for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_st<K>(keys: Vec::<K>) -> Self where K: Hash {
        Self::with_vec_bps_bs_hash_sc(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        BuildDefaultSeededHasher::default(), SeedOnly)
    }

    /// Constructs [`Function`] for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_mt<K>(keys: Vec::<K>) -> Self where K: Hash+Send+Sync {
        Self::with_vec_bps_bs_threads_hash_sc(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        std::thread::available_parallelism().map_or(1, |v| v.into()), BuildDefaultSeededHasher::default(), SeedOnly)
    }

    /// Constructs [`Function`] for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_st<K>(keys: &[K]) -> Self where K: Hash+Clone {
        Self::with_slice_bps_bs_hash_sc(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        BuildDefaultSeededHasher::default(), SeedOnly)
    }

    /// Constructs [`Function`] for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_mt<K>(keys: &[K]) -> Self where K: Hash+Clone+Send+Sync {
        Self::with_slice_bps_bs_threads_hash_sc(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        std::thread::available_parallelism().map_or(1, |v| v.into()), BuildDefaultSeededHasher::default(), SeedOnly)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::utils::tests::test_mphf;

    #[test]
    fn test_small() {
        let input = [1, 2, 3, 4, 5];
        let f = Function::from_slice_st(&input);
        test_mphf(&input, |key| Some(f.get(key) as u64));
    }
}