use std::{hash::Hash, usize};

use crate::seeds::{Bits8, SeedSize};
use super::{bits_per_seed_to_100_bucket_size, builder::build_mt, conf::Conf, evaluator::Weights, CompressedArray, DefaultCompressedArray};
use bitm::{BitAccess, BitVec};
use dyn_size_of::GetSize;
use seedable_hash::{BuildDefaultSeededHasher, BuildSeededHasher};
use voracious_radix_sort::RadixSort;

pub struct Level<CA, SS: SeedSize> {
    seeds: Box<[SS::VecElement]>,
    conf: Conf<SS>,
    unassigned_values: CA
}

impl<CA, SS: SeedSize> Level<CA, SS> {
    #[inline]
    fn bucket_for(&self, key: u64) -> usize { self.conf.bucket_for(key) }

    #[inline]
    fn seed_for(&self, key: u64) -> u16 {
        //self.seeds.get_fragment(self.bucket_for(key), self.conf.bits_per_seed()) as u16
        self.conf.bits_per_seed.get_seed(&self.seeds, self.bucket_for(key))
    }

    #[inline]
    pub fn get(&self, key: u64, seed: u16) -> usize {
        self.conf.f(key, seed)
    }
}

impl<CA: GetSize, SS: SeedSize> GetSize for Level<CA, SS> {
    fn size_bytes_dyn(&self) -> usize { self.seeds.size_bytes_dyn() + self.unassigned_values.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.seeds.size_bytes_content_dyn() + self.unassigned_values.size_bytes_content_dyn() }
    const USES_DYN_MEM: bool = true;
}

/// PHast (Perfect Hashing with fast evaluation).
/// 
/// Perfect hash function with very fast evaluation and size below 2 bits/key
/// developed by Peter Sanders and Piotr Beling.
pub struct Function<SS: SeedSize, CA = DefaultCompressedArray, S = BuildDefaultSeededHasher> {
    pub levels: Box<[Level<CA, SS>]>,
    pub hasher: S
}

impl<SS: SeedSize, CA, S> GetSize for Function<SS, CA, S> where Level<CA, SS>: GetSize {
    fn size_bytes_dyn(&self) -> usize { self.levels.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.levels.size_bytes_content_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl Function<Bits8, DefaultCompressedArray, BuildDefaultSeededHasher> {
    pub fn new<K>(keys: Vec::<K>) -> Self where K: Hash {
        Self::with_keys_bps_bs_threads_hash(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
            std::thread::available_parallelism().map_or(1, |v| v.into()), BuildDefaultSeededHasher::default())
    }

    pub fn new_st<K>(keys: Vec::<K>) -> Self where K: Hash {
        Self::with_keys_bps_bs_threads_hash(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8),
        1, BuildDefaultSeededHasher::default())
    }
}

impl<SS: SeedSize, CA: CompressedArray> Function<SS, CA, BuildDefaultSeededHasher> {
    pub fn with_keys_bps<K>(keys: Vec::<K>, bits_per_seed: SS) -> Self where K: Hash {
        Self::with_keys_bps_bs_threads_hash(keys, bits_per_seed, bits_per_seed_to_100_bucket_size(bits_per_seed.into()),
        std::thread::available_parallelism().map_or(1, |v| v.into()), BuildDefaultSeededHasher::default())
    }
}

/*impl<SS: SeedSize, CA: CompressedArray, S: BuildSeededHasher + Sync> Function<SS, CA, S> {
    pub fn with_keyset_bps_bs_threads_hash<K, KS: KeySet<K>>(mut keys: KS, bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: S) -> Self where K: Hash {
        let mut levels = Vec::new();
        let use_mt = threads_num > 1;
        while keys.keys_len() != 0 {
            let level_nr = levels.len() as u32;
            let mut hashes = keys.maybe_par_map_each_key(|k| hasher.hash_one(k, level_nr), |_| true /* TODO */, use_mt);
            //radsort::unopt::sort(&mut hashes);
            hashes.voracious_mt_sort(threads_num);
            let conf = Conf::new(hashes.len(), bits_per_seed, bucket_size100);
            let seeds =
                build_mt(&hashes, conf, bucket_size100, 256, Weights::new(conf.bits_per_seed(), conf.partition_size()), threads_num);
            let keys_len = keys.keys_len();
            let mut unassigned_values = Box::with_filled_bits(keys_len);
            if keys_len % 64 != 0 {
                *unassigned_values.last_mut().unwrap() >>= 64 - keys_len % 64;
            }
            let mut unassigned_len = keys_len;
            keys.maybe_par_retain_keys(|key| { // TODO use unsorted hashes
                let key_hash = hasher.hash_one(key, level_nr);
                //let seed = seeds.get_fragment(conf.bucket_for(key_hash), bits_per_seed) as u16;
                let seed = bits_per_seed.get_seed(&seeds, conf.bucket_for(key_hash));
                if seed == 0 {
                    true
                } else {
                    let value = conf.f(key_hash, seed);
                    unassigned_values.clear_bit(value);
                    unassigned_len -= 1;
                    false
                }
            }, |_| true, || 0 /* TODO */, use_mt);
            //let unassigned_values: EliasFano = EliasFano::new(&unassigned_values, keys_len, unassigned_len);
            let unassigned_values = CA::new(&unassigned_values, keys_len, unassigned_len);
            levels.push(Level {
                seeds,
                conf,
                unassigned_values,
            });
        }
        Self {
            levels: levels.into_boxed_slice(), hasher
        }
    }
}*/

impl<SS: SeedSize, CA: CompressedArray, S: BuildSeededHasher> Function<SS, CA, S> {
    /*    pub fn get<K>(&self, key: &K) -> usize where K: Hash + ?Sized {
        let key_hash = self.hasher.hash_one(key, 0);
        let seed = self.seeds.get_fragment(self.conf.bucket_for(key_hash), self.conf.bits_per_seed) as u16;
        if seed != 0 { return self.conf.f(key_hash, seed); }
        for level_nr in 0..self.levels.len() {
            let key_hash = self.hasher.hash_one(key, level_nr as u32);
            let seed = self.levels[level_nr].seed_for(key_hash);
            if seed != 0 {
                let mut index = self.levels[level_nr].get(key_hash, seed);
                for level_nr in (0..=level_nr).rev() {
                    index = self.levels[level_nr].unassigned_values.get_or_panic(index) as usize;
                }
                return index;
            }
        }
        unreachable!()
    } */


    pub fn get<K>(&self, key: &K) -> usize where K: Hash + ?Sized {
        let key_hash = self.hasher.hash_one(key, 0);
        let seed = self.levels[0].seed_for(key_hash);
        if seed != 0 { return self.levels[0].get(key_hash, seed); }
        for level_nr in 1..self.levels.len() {
            let key_hash = self.hasher.hash_one(key, level_nr as u32);
            let seed = self.levels[level_nr].seed_for(key_hash);
            if seed != 0 {
                let mut index = self.levels[level_nr].get(key_hash, seed);
                for level_nr in (0..level_nr).rev() {
                    //index = self.levels[level_nr].unassigned_values.get_or_panic(index) as usize;
                    index = self.levels[level_nr].unassigned_values.get(index);
                }
                return index;
            }
        }
        unreachable!()
    }

    pub fn with_keys_bps_bs_threads_hash<K>(mut keys: Vec::<K>, bits_per_seed: SS, bucket_size100: u16, threads_num: usize, hasher: S) -> Self where K: Hash {
        let mut levels = Vec::new();
        while !keys.is_empty() {
            let level_nr = levels.len() as u32;
            let mut hashes: Box<[_]> = keys.iter().map(|k| hasher.hash_one(k, level_nr)).collect();
            //radsort::unopt::sort(&mut hashes);
            hashes.voracious_mt_sort(threads_num);
            let conf = Conf::new(hashes.len(), bits_per_seed, bucket_size100);
            let seeds =
                build_mt(&hashes, conf, bucket_size100, 256, Weights::new(conf.bits_per_seed(), conf.partition_size()), threads_num);
            let keys_len = keys.len();
            let mut unassigned_values = Box::with_filled_bits(keys_len);
            if keys_len % 64 != 0 {
                *unassigned_values.last_mut().unwrap() >>= 64 - keys_len % 64;
            }
            let mut unassigned_len = keys_len;
            keys.retain(|key| { // TODO use unsorted hashes
                let key_hash = hasher.hash_one(key, level_nr);
                //let seed = seeds.get_fragment(conf.bucket_for(key_hash), bits_per_seed) as u16;
                let seed = bits_per_seed.get_seed(&seeds, conf.bucket_for(key_hash));
                if seed == 0 {
                    true
                } else {
                    let value = conf.f(key_hash, seed);
                    unassigned_values.clear_bit(value);
                    unassigned_len -= 1;
                    false
                }
            });
            //let unassigned_values: EliasFano = EliasFano::new(&unassigned_values, keys_len, unassigned_len);
            let unassigned_values = CA::new(&unassigned_values, keys_len, unassigned_len);
            levels.push(Level {
                seeds,
                conf,
                unassigned_values,
            });
        }
        Self {
            levels: levels.into_boxed_slice(), hasher
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::fmt::Display;

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
        let f = Function::new(input.to_vec());
        mphf_test(&f, &input);
    }
}