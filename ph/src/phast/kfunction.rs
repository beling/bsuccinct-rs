use std::{hash::Hash, io, usize};

use crate::{phast::{Generic, Perfect, ProdOfValues, SeedOnlyK, conf::{Conf, Core}, function::SeedEx}, seeds::{Bits8, SeedSize}};
use super::{bits_per_seed_to_100_bucket_size, builder::{build_mt, build_st}, conf::GenericCore, seed_chooser::{SeedChooser, SeedOnly}, CompressedArray, DefaultCompressedArray, WINDOW_SIZE};
use binout::{Serializer, VByte};
use bitm::BitAccess;
use dyn_size_of::GetSize;
use seedable_hash::{BuildDefaultSeededHasher, BuildSeededHasher};
use voracious_radix_sort::RadixSort;
use rayon::prelude::*;

/// PHast (Perfect Hashing made fast) - (K-)Perfect Hash Function
/// with very fast evaluation and size below 2 bits/key
/// developed by Piotr Beling and Peter Sanders.
/// Experimental.
/// 
/// See:
/// Piotr Beling, Peter Sanders, *PHast - Perfect Hashing made fast*, 2025, <https://arxiv.org/abs/2504.17918>
pub struct KFunction<C: Core, SS, SC = SeedOnlyK, CA = DefaultCompressedArray, S = BuildDefaultSeededHasher>
    where SS: SeedSize
{
    level0: SeedEx<SS::VecElement, C>,
    bumped_index_to_value: CA,
    bumped_to_index: Perfect<Bits8, SeedOnly, S>,
    seed_chooser: SC,
    seed_size: SS,  // seed size, K=2**bits_per_seed
}

impl<C: Core, SS: SeedSize, SC, CA, S> GetSize for KFunction<C, SS, SC, CA, S> where CA: GetSize {
    fn size_bytes_dyn(&self) -> usize {
        self.level0.size_bytes_dyn() +
            self.bumped_index_to_value.size_bytes_dyn() +
            self.bumped_to_index.size_bytes_dyn()
    }
    fn size_bytes_content_dyn(&self) -> usize {
        self.level0.size_bytes_content_dyn() +
            self.bumped_index_to_value.size_bytes_content_dyn() +
            self.bumped_to_index.size_bytes_content_dyn()
    }
    const USES_DYN_MEM: bool = true;
}

impl<C: Core, SS: SeedSize, SC: SeedChooser, CA: CompressedArray, S: BuildSeededHasher> KFunction<C, SS, SC, CA, S> {
    
    const L0_SEED: u64 = 0xFF_FF_FF_FE_FF_FF_FF_FE;

    /// Returns value assigned to the given `key`.
    /// 
    /// The returned value is in the range from `0` (inclusive) to the number of elements in the input key collection (exclusive).
    /// `key` must come from the input key collection given during construction.
    #[inline(always)]   //inline(always) is important here
    pub fn get<K>(&self, key: &K) -> usize where K: Hash + ?Sized {
        /* TODO turbo support: let slice_begin = self.level0.slice_begin(key_hash);
        let seed = self.level0.seed_for_slice(slice_begin);
        if seed != 0 { return SC::f_slice(key_hash, slice_begin, seed, &self.level0.conf); } */

        /*let key_hash = self.hasher.hash_one(key, Self::L0_SEED);
        let seed = unsafe{ self.level0.seed_for(self.seed_size, key_hash) };
        if seed != 0 { return self.seed_chooser.f(key_hash, seed, &self.level0.conf); }*/
        if let Some(result) = self.seed_chooser.try_f(self.seed_size, &self.level0.seeds, self.bumped_to_index.hasher.hash_one(key, Self::L0_SEED), &self.level0.conf) {
            return result;
        }
        self.bumped_index_to_value.get(self.bumped_to_index.get(key))
    }

    /// Constructs [`KFunction`] for given `keys`, using a single thread and given parameters, `hasher` and `seed_chooser`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_vec_p_hash_sc<K, P>(mut keys: Vec::<K>, params: &P, hasher: S, seed_chooser: SC) -> Self
        where K: Hash, P: Conf<Core = C, SeedSize = SS>
    {
        let number_of_keys = keys.len();
        Self::_new(|h| {
            let (level0, unassigned_values, unassigned_len) =
                build_level_st(&mut keys, params, h, seed_chooser.clone(), 0);
            (keys, level0, unassigned_values, unassigned_len)
        }, |keys, level_nr, h| {
            build_level_st(keys, params, h, seed_chooser.clone(), level_nr)
        }, hasher, seed_chooser.clone(), params.seed_size(), number_of_keys)
    }

    /// Constructs [`Function`] for given `keys`, using multiple (given number of) threads and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_vec_p_threads_hash_sc<K, P>(mut keys: Vec::<K>, params: &P, threads_num: usize, hasher: S, seed_chooser: SC) -> Self
        where K: Hash+Sync+Send, S: Sync, SC: Sync, P: Conf<Core = C, SeedSize = SS>
    {
        if threads_num == 1 { return Self::with_vec_p_hash_sc(keys, params, hasher, seed_chooser); }
        let number_of_keys = keys.len();
        Self::_new(|h| {
            let (level0, unassigned_values, unassigned_len) =
                build_level_mt(&mut keys, params, threads_num, h, seed_chooser.clone(), 0);
            (keys, level0, unassigned_values, unassigned_len)
        }, |keys, level_nr, h| {
            build_level_mt(keys, params, threads_num, h, seed_chooser.clone(), level_nr)
        }, hasher, seed_chooser.clone(), params.seed_size(), number_of_keys)
    }


    /// Constructs [`Function`] for given `keys`, using a single thread and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_slice_p_hash_sc<K, P>(keys: &[K], params: &P, hasher: S, seed_chooser: SC) -> Self
        where K: Hash+Clone, P: Conf<SeedSize = SS, Core = C>
    {
        Self::_new(|h| {
            build_level_from_slice_st(keys, params, h, seed_chooser.clone(), 0)
        }, |keys, level_nr, h| {
            build_level_st(keys, params, h, seed_chooser.clone(), level_nr)
        }, hasher, seed_chooser.clone(), params.seed_size(), keys.len())
    }


    /// Constructs [`Function`] for given `keys`, using multiple (given number of) threads and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_slice_p_threads_hash_sc<K, P>(keys: &[K], params: &P, threads_num: usize, hasher: S, seed_chooser: SC) -> Self
        where K: Hash+Sync+Send+Clone, S: Sync, SC: Sync, P: Conf<SeedSize = SS, Core = C> {
        if threads_num == 1 { return Self::with_slice_p_hash_sc(keys, params, hasher, seed_chooser); }
        Self::_new(|h| {
            build_level_from_slice_mt(keys, params, threads_num, h, seed_chooser.clone(), 0)
        }, |keys, level_nr, h| {
            build_level_mt(keys, params, threads_num, h, seed_chooser.clone(), level_nr)
        }, hasher, seed_chooser.clone(), params.seed_size(), keys.len())
    }

    #[inline]
    fn build_from_slice_st<K>(keys: &[K], params: &Generic<SS>, output_range: usize, hasher: &S, seed_chooser: SC)
        -> (Vec<K>, SeedEx<SS::VecElement>, Box<[u16]>)
        where K: Hash+Clone
    {
        let mut hashes: Box<[_]> = keys.iter().map(|k| hasher.hash_one(k, Self::L0_SEED)).collect();
        hashes.voracious_sort();
        let conf = seed_chooser.conf_p(output_range, hashes.len(), params);
        let (seeds, builder) =
            build_st(&hashes, conf, params.seed_size, seed_chooser.bucket_evaluator(params.bits_per_seed(), conf.slice_len()), seed_chooser);
        let (free_count, bumped_num) = builder.unassigned_values_k(&seeds);
        let mut keys_vec = Vec::with_capacity(bumped_num);
        drop(builder);
        keys_vec.extend(keys.into_iter().filter(|key| {
            unsafe { params.seed_size.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, Self::L0_SEED))) == 0 }
        }).cloned());
        (keys_vec, SeedEx{ seeds, conf }, free_count)
    }

    #[inline]
    fn build_level_from_slice_mt<K>(keys: &[K], params: &Generic<SS>, threads_num: usize, output_range: usize, hasher: &S, seed_chooser: SC)
        -> (Vec<K>, SeedEx<SS::VecElement>, Box<[u16]>)
        where K: Hash+Sync+Send+Clone, S: Sync, SC: Sync
    {
        let mut hashes: Box<[_]> = if keys.len() > 4*2048 {    //maybe better for string keys
            keys.par_iter().with_min_len(256).map(|k| hasher.hash_one(k, Self::L0_SEED)).collect()
        } else {
            keys.iter().map(|k| hasher.hash_one(k, Self::L0_SEED)).collect()
        };
        //radsort::unopt::sort(&mut hashes);
        hashes.voracious_mt_sort(threads_num);
        let conf = seed_chooser.conf_p(output_range, hashes.len(), params);
        let (seeds, builder) =
            build_mt(&hashes, conf, params.seed_size, WINDOW_SIZE, seed_chooser.bucket_evaluator(params.bits_per_seed(), conf.slice_len()), seed_chooser, threads_num);
        let (free_count, bumped_num) = builder.unassigned_values_k(&seeds);
        let mut keys_vec = Vec::with_capacity(bumped_num);
        drop(builder);
        keys_vec.par_extend(keys.into_par_iter().filter(|key| {
            unsafe { params.seed_size.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, Self::L0_SEED))) == 0 }
        }).cloned());
        (keys_vec, SeedEx{ seeds, conf }, free_count)
    }

    #[inline(always)]
    fn build_level_st<K>(keys: &mut Vec::<K>, params: &Generic<SS>, output_range: usize, hasher: &S, seed_chooser: SC)
        -> (SeedEx<SS::VecElement>, Box<[u16]>)
        where K: Hash
    {
        let mut hashes: Box<[_]> = keys.iter().map(|k| hasher.hash_one(k, Self::L0_SEED)).collect();
        hashes.voracious_sort();
        let conf = seed_chooser.conf_p(output_range, hashes.len(), params);
        let (seeds, builder) =
            build_st(&hashes, conf, params.seed_size, seed_chooser.bucket_evaluator(params.bits_per_seed(), conf.slice_len()), seed_chooser);
        let (free_count, _) = builder.unassigned_values_k(&seeds);
        keys.retain(|key| {
            unsafe { params.seed_size.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, Self::L0_SEED))) == 0 }
        });
        (SeedEx{ seeds, conf }, free_count)
    }

    #[inline]
    fn build_level_mt<K>(keys: &mut Vec::<K>, params: &Generic<SS>, threads_num: usize, output_range: usize, hasher: &S, seed_chooser: SC)
        -> (SeedEx<SS::VecElement>, Box<[u16]>)
        where K: Hash+Sync+Send, S: Sync, SC: Sync
    {
        let mut hashes: Box<[_]> = if keys.len() > 4*2048 {    //maybe better for string keys
            //let mut k = Vec::with_capacity(keys.len());
            //k.par_extend(keys.par_iter().with_min_len(10000).map(|k| hasher.hash_one_s64(k, level_nr)));
            //k.into_boxed_slice()
            keys.par_iter().with_min_len(256).map(|k| hasher.hash_one(k, Self::L0_SEED)).collect()
        } else {
            keys.iter().map(|k| hasher.hash_one(k, Self::L0_SEED)).collect()
        };
        //radsort::unopt::sort(&mut hashes);
        hashes.voracious_mt_sort(threads_num);
        let conf = seed_chooser.conf_p(output_range, hashes.len(), params);
        let (seeds, builder) =
            build_mt(&hashes, conf, params.seed_size, WINDOW_SIZE, seed_chooser.bucket_evaluator(params.bits_per_seed(), conf.slice_len()), seed_chooser, threads_num);
        let (free_count, bumped_num) = builder.unassigned_values_k(&seeds);
        drop(builder);
        let mut result = Vec::with_capacity(bumped_num);
        std::mem::swap(keys, &mut result);
        keys.par_extend(result.into_par_iter().filter(|key| {
            unsafe { params.seed_size.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, Self::L0_SEED))) == 0 }
        }));
        (SeedEx{ seeds, conf }, free_count)
    }

    #[inline]
    fn _new<K, BF, BL>(build_first: BF, build_rest: BL, hasher: S, seed_chooser: SC, seed_size: SS, number_of_keys: usize) -> Self
        where BF: FnOnce(&S) -> (Vec::<K>, SeedEx<SS::VecElement, C>, Box<[u16]>),
            BL: Fn(&[K], S) -> Perfect<Bits8, SeedOnly, S>,
            K: Hash
        {
        let (mut keys, level0, mut free_count) = build_first(&hasher);
        let bumped_to_index = build_rest(&keys, hasher);

        let mut free_value = 0;

        //let mut bumped_index_to_value = Vec::with_capacity(keys.len() * 3 / 2);
        let mut bumped_index_to_value = Vec::with_capacity(bumped_to_index.output_range());

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
            let shift = bumped_index_to_value.len();
            for i in 0..keys_len {
                if CA::MAX_FOR_UNUSED {
                    if !unsafe{unassigned_values.get_bit_unchecked(i)} {
                        last = level0_unassigned.next().unwrap();
                        bumped_index_to_value.push(last);
                    } else {
                        bumped_index_to_value.push(usize::MAX);
                    }
                } else {
                    if !unsafe{unassigned_values.get_bit_unchecked(i)} {
                        last = level0_unassigned.next().unwrap();
                    }
                    bumped_index_to_value.push(last);
                }
            }
            levels.push(Level { seeds, shift });
        }
        debug_assert!(level0_unassigned.next().is_none());
        drop(level0_unassigned);
        Self {
            level0,
            unassigned: CA::new(bumped_index_to_value, last, number_of_keys),
            levels: levels.into_boxed_slice(),
            hasher,
            seed_chooser,
            seed_size
        }
    }

    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys,
    /// i.e. 1 + maximum value that minimal function can return.
    #[inline(always)] pub fn minimal_output_range(&self, num_of_keys: usize) -> usize { self.seed_chooser.minimal_output_range(num_of_keys) }

    /// Returns output range of `self`, i.e. 1 + maximum value that `self` can return.
    pub fn output_range(&self) -> usize {
        self.level0.conf.output_range(&self.seed_chooser, self.seed_size.into())
    }

    /// Returns maximum number of keys which can be mapped to the same value by `k`-[`Perfect`] function `self`.
    #[inline(always)] pub fn k(&self) -> u16 { self.seed_chooser.k() }

    /// Returns number of bytes which `write` will write.
    pub fn write_bytes(&self) -> usize {
        self.level0.write_bytes() +
        self.unassigned.write_bytes() +
        VByte::size(self.levels.len()) +
        self.levels.iter().map(|l| l.size_bytes()).sum::<usize>()
    }

    /// Writes `self` to the `output`.
    pub fn write(&self, output: &mut dyn io::Write) -> io::Result<()>
    {
        self.level0.write(output, self.seed_size)?;
        self.unassigned.write(output)?;
        VByte::write(output, self.levels.len())?;
        for level in &self.levels {
            level.write(output, self.seed_size)?;
        }
        Ok(())
    }

    /// Read `Self` from the `input`. `hasher` and `seed_chooser` must be the same as used by the structure written.
    pub fn read_with_hasher_sc(input: &mut dyn io::Read, hasher: S, seed_chooser: SC) -> io::Result<Self> {
        let (seed_size, level0) = SeedEx::read(input)?;
        let unassigned = CA::read(input)?;
        let levels_num: usize = VByte::read(input)?;
        let mut levels = Vec::with_capacity(levels_num);
        for _ in 0..levels_num {
            levels.push(Level::read::<SS>(input)?.1);
        }
        Ok(Self { level0, unassigned, levels: levels.into_boxed_slice(), hasher, seed_chooser, seed_size })
    }
}

impl<C: Core, SS: SeedSize> Function<C, SS, SeedOnly, DefaultCompressedArray, BuildDefaultSeededHasher> {

    /// Read `Self` from the `input`. Uses default hasher and seed chooser.
    pub fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Self::read_with_hasher_sc(input, BuildDefaultSeededHasher::default(), SeedOnly(ProdOfValues))
    }
}

// TODO switch Conf to ConfTurbo ?
impl Function<GenericCore, Bits8, SeedOnly, DefaultCompressedArray, BuildDefaultSeededHasher> {
    /// Constructs [`Function`] for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_st<K>(keys: Vec::<K>) -> Self where K: Hash {
        Self::with_vec_p_hash_sc(keys, &Generic::new(Bits8::default(), bits_per_seed_to_100_bucket_size(8)),
        BuildDefaultSeededHasher::default(), SeedOnly(ProdOfValues))
    }

    /// Constructs [`Function`] for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_mt<K>(keys: Vec::<K>) -> Self where K: Hash+Send+Sync {
        Self::with_vec_p_threads_hash_sc(keys, &Generic::new(Bits8::default(), bits_per_seed_to_100_bucket_size(8)),
        std::thread::available_parallelism().map_or(1, |v| v.into()), BuildDefaultSeededHasher::default(), SeedOnly(ProdOfValues))
    }

    /// Constructs [`Function`] for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_st<K>(keys: &[K]) -> Self where K: Hash+Clone {
        Self::with_slice_p_hash_sc(keys, &Generic::new(Bits8::default(), bits_per_seed_to_100_bucket_size(8)),
        BuildDefaultSeededHasher::default(), SeedOnly(ProdOfValues))
    }

    /// Constructs [`Function`] for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_mt<K>(keys: &[K]) -> Self where K: Hash+Clone+Send+Sync {
        Self::with_slice_p_threads_hash_sc(keys, &Generic::new(Bits8::default(), bits_per_seed_to_100_bucket_size(8)),
        std::thread::available_parallelism().map_or(1, |v| v.into()), BuildDefaultSeededHasher::default(), SeedOnly(ProdOfValues))
    }

}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::utils::tests::test_mphf;

    fn test_read_write<C, SS>(h: &Function::<C, SS>)
         where C: Core + PartialEq + std::fmt::Debug, SS: SeedSize, SS::VecElement: std::fmt::Debug+PartialEq
    {
        let mut buff = Vec::new();
        h.write(&mut buff).unwrap();
        //assert_eq!(buff.len(), h.write_bytes());
        let read = Function::<C, SS>::read(&mut &buff[..]).unwrap();
        assert_eq!(h.level0.conf, read.level0.conf);
        assert_eq!(h.levels.len(), read.levels.len());
        for (hl, rl) in h.levels.iter().zip(&read.levels) {
            assert_eq!(hl.shift, rl.shift);
            assert_eq!(hl.seeds.conf, rl.seeds.conf);
            assert_eq!(hl.seeds.seeds, rl.seeds.seeds);
        }
    }

    #[test]
    fn test_small() {
        let input = [1, 2, 3, 4, 5];
        let f = Function::from_slice_st(&input);
        test_mphf(&input, |key| Some(f.get(key)));
        test_read_write(&f);
    }

    #[test]
    fn test_small2() {
        let input = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21];
        let f = Function::from_slice_st(&input);
        test_mphf(&input, |key| Some(f.get(key)));
        test_read_write(&f);
    }
}