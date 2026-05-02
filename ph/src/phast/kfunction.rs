use std::{hash::Hash, io, usize};

use crate::{phast::{CoreConf, Generic, ProdOfValues, SeedChooserCore, SeedCore, SeedKCore, SeedOnly, SeedOnlyK, conf::{Conf, Core}, function::{Level, SeedEx, build_level_mt, build_level_st}}, seeds::{Bits8, SeedSize}};
use super::{builder::{build_mt, build_st}, conf::GenericCore, seed_chooser::SeedChooser, CompressedArray, DefaultCompressedArray, WINDOW_SIZE};
use binout::{Serializer, VByte};
use bitm::BitAccess;
use dyn_size_of::GetSize;
use seedable_hash::{BuildDefaultSeededHasher, BuildSeededHasher};
use voracious_radix_sort::RadixSort;
use rayon::prelude::*;

/// PHast (Perfect Hashing made fast) - (K-)Perfect Hash Function with very fast evaluation
/// developed by Piotr Beling and Peter Sanders.
/// Experimental.
/// 
/// See:
/// Piotr Beling, Peter Sanders, *PHast - Perfect Hashing made fast*, 2025, <https://arxiv.org/abs/2504.17918>
pub struct KFunction<C: Core, SS, SC = SeedKCore, CA = DefaultCompressedArray, S = BuildDefaultSeededHasher>
    where SS: SeedSize
{
    level0: SeedEx<SS::VecElement, C>,
    bumped_index_to_value: CA,
    //bumped_to_index: Perfect<Bits8, SeedOnly, S>,
    bumped_to_index: Box<[Level<<Bits8 as SeedSize>::VecElement, GenericCore>]>,
    hasher: S,
    seed_chooser: SC,
    seed_size: SS,
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

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore, CA: CompressedArray, S: BuildSeededHasher> KFunction<C, SS, SCC, CA, S> {
    
    //const L0_SEED: u64 = 0xFF_FF_FF_FE_FF_FF_FF_FE;

    /// Returns value assigned to the given `key`.
    /// 
    /// The returned value is in the range from `0` (inclusive) to the number of elements in the input key collection (exclusive).
    /// `key` must come from the input key collection given during construction.
    #[inline(always)]   //inline(always) is important here
    pub fn get<K>(&self, key: &K) -> usize where K: Hash + ?Sized {
        /*let key_hash = self.hasher.hash_one(key, 0);
        let seed = unsafe{ self.level0.seed_for(self.seed_size, key_hash) };
        if seed != 0 { return self.seed_chooser.f(key_hash, seed, &self.level0.conf); }*/
        if let Some(result) = self.seed_chooser.try_f(self.seed_size, &self.level0.seeds, self.hasher.hash_one(key, 0), &self.level0.core) {
            return result;
        }

        for level_nr in 0..self.bumped_to_index.len() {
            let l = &self.bumped_to_index[level_nr];
            if let Some(result) = SeedCore.try_f(Bits8, &l.seeds.seeds, self.hasher.hash_one(key, level_nr as u64 + 1), &l.seeds.core) {
                return self.bumped_index_to_value.get(result + l.shift);
            }
        }
        unreachable!()
    }

    /// Constructs [`KFunction`] for given `keys`, using a single thread and given parameters, `hasher` and `seed_chooser`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_vec_conf_sc<K, CC, SC>(mut keys: Vec::<K>, conf: Conf<SS, CC, S>, seed_chooser: SC) -> Self
        where K: Hash, CC: CoreConf<Core = C>, SC: SeedChooser<Core = SCC>
    {
        let number_of_keys = keys.len();
        Self::_new(|conf| {
            let (level0, unassigned_values) =
                Self::build_level0_st(&mut keys, conf, seed_chooser.clone());  //TODO number_of_keys/k
            (keys, level0, unassigned_values)
        }, |keys, level_nr, h| {
            build_level_st(keys, &Generic::new_for_bps(8), Bits8, h, SeedOnly(ProdOfValues), level_nr)
        }, conf, seed_chooser.core(), number_of_keys)
    }

    /// Constructs [`Function`] for given `keys`, using multiple (given number of) threads and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_vec_conf_threads_sc<K, CC, SC>(mut keys: Vec::<K>, conf: Conf<SS, CC, S>, threads_num: usize, seed_chooser: SC) -> Self
        where K: Hash+Sync+Send, S: Sync, SC: SeedChooser<Core = SCC>, CC: CoreConf<Core = C>
    {
        if threads_num == 1 { return Self::with_vec_conf_sc(keys, conf, seed_chooser); }
        let number_of_keys = keys.len();
        Self::_new(|conf| {
            let (level0, unassigned_values) =
                Self::build_level0_mt(&mut keys, conf, threads_num, seed_chooser.clone());  //TODO number_of_keys/k
            (keys, level0, unassigned_values)
        }, |keys, level_nr, h| {
            build_level_mt(keys, &Generic::new_for_bps(8), Bits8, threads_num, h, SeedOnly(ProdOfValues), level_nr)
        }, conf, seed_chooser.core(), number_of_keys)
    }

    /// Constructs [`Function`] for given `keys`, using a single thread and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_slice_conf_sc<K, CC, SC>(keys: &[K], conf: Conf<SS, CC, S>, seed_chooser: SC) -> Self
        where K: Hash+Clone, CC: CoreConf<Core = C>, SC: SeedChooser<Core = SCC>
    {
        Self::_new(|conf| {
            Self::build_level0_from_slice_st(keys, conf, seed_chooser.clone())
        }, |keys, level_nr, h| {
            build_level_st(keys, &Generic::new_for_bps(8), Bits8, h, SeedOnly(ProdOfValues), level_nr)
        }, conf, seed_chooser.core(), keys.len())
    }


    /// Constructs [`Function`] for given `keys`, using multiple (given number of) threads and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_slice_conf_threads_sc<K, CC, SC>(keys: &[K], conf: Conf<SS, CC, S>, threads_num: usize, seed_chooser: SC) -> Self
        where K: Hash+Sync+Send+Clone, S: Sync, SC: SeedChooser<Core = SCC>, CC: CoreConf<Core = C> {
        if threads_num == 1 { return Self::with_slice_conf_sc(keys, conf, seed_chooser); }
        Self::_new(|conf| {
            Self::build_level0_from_slice_mt(keys, conf, threads_num, seed_chooser.clone())
        }, |keys, level_nr, h| {
            build_level_mt(keys, &Generic::new_for_bps(8), Bits8, threads_num, h, SeedOnly(ProdOfValues), level_nr)
        }, conf, seed_chooser.core(), keys.len())
    }

    #[inline]
    fn build_level0_from_slice_st<K, CC, SC>(keys: &[K], conf: &Conf<SS, CC, S>, seed_chooser: SC)
        -> (Vec<K>, SeedEx<SS::VecElement, C>, Box<[u16]>)
        where K: Hash+Clone, CC: CoreConf<Core=C>, SC: SeedChooser<Core = SCC>
    {
        let mut hashes: Box<[_]> = keys.iter().map(|k| conf.hasher.hash_one(k, 0)).collect();
        hashes.voracious_sort();
        let core = seed_chooser.f_core_lf(hashes.len(), conf.loading_factor_1000, &conf.core_conf, conf.bits_per_seed());
        let (seeds, builder) =
            build_st(&hashes, core, conf.seed_size, seed_chooser.bucket_evaluator(conf.bits_per_seed(), core.slice_len()), seed_chooser);
        let (free_count, bumped_num) = builder.unassigned_values_k(&seeds);
        let mut keys_vec = Vec::with_capacity(bumped_num);
        drop(builder);
        keys_vec.extend(keys.into_iter().filter(|key| {
            unsafe { conf.seed_size.get_seed(&seeds, core.bucket_for(conf.hasher.hash_one(key, 0))) == 0 }
        }).cloned());
        (keys_vec, SeedEx{ seeds, core }, free_count)
    }

    #[inline]
    fn build_level0_from_slice_mt<K, CC, SC>(keys: &[K], conf: &Conf<SS, CC, S>, threads_num: usize, seed_chooser: SC)
        -> (Vec<K>, SeedEx<SS::VecElement, C>, Box<[u16]>)
        where K: Hash+Sync+Send+Clone, CC: CoreConf<Core=C>, S: Sync, SC: SeedChooser<Core = SCC>
    {
        let mut hashes: Box<[_]> = if keys.len() > 4*2048 {    //maybe better for string keys
            keys.par_iter().with_min_len(256).map(|k| conf.hasher.hash_one(k, 0)).collect()
        } else {
            keys.iter().map(|k| conf.hasher.hash_one(k, 0)).collect()
        };
        //radsort::unopt::sort(&mut hashes);
        hashes.voracious_mt_sort(threads_num);
        let core = seed_chooser.f_core_lf(hashes.len(), conf.loading_factor_1000, &conf.core_conf, conf.bits_per_seed());
        let (seeds, builder) =
            build_mt(&hashes, core, conf.seed_size, WINDOW_SIZE, seed_chooser.bucket_evaluator(conf.bits_per_seed(), core.slice_len()), seed_chooser, threads_num);
        let (free_count, bumped_num) = builder.unassigned_values_k(&seeds);
        let mut keys_vec = Vec::with_capacity(bumped_num);
        drop(builder);
        keys_vec.par_extend(keys.into_par_iter().filter(|key| {
            unsafe { conf.seed_size.get_seed(&seeds, core.bucket_for(conf.hasher.hash_one(key, 0))) == 0 }
        }).cloned());
        (keys_vec, SeedEx{ seeds, core }, free_count)
    }

    #[inline(always)]
    fn build_level0_st<K, CC, SC>(keys: &mut Vec::<K>, conf: &Conf<SS, CC, S>, seed_chooser: SC)
        -> (SeedEx<SS::VecElement, C>, Box<[u16]>)
        where K: Hash, CC: CoreConf<Core=C>, SC: SeedChooser<Core = SCC>
    {
        let mut hashes: Box<[_]> = keys.iter().map(|k| conf.hasher.hash_one(k, 0)).collect();
        hashes.voracious_sort();
        let core = seed_chooser.f_core_lf(hashes.len(), conf.loading_factor_1000, &conf.core_conf, conf.bits_per_seed());
        let (seeds, builder) =
            build_st(&hashes, core, conf.seed_size, seed_chooser.bucket_evaluator(conf.bits_per_seed(), core.slice_len()), seed_chooser);
        let (free_count, _) = builder.unassigned_values_k(&seeds);
        keys.retain(|key| {
            unsafe { conf.seed_size.get_seed(&seeds, core.bucket_for(conf.hasher.hash_one(key, 0))) == 0 }
        });
        (SeedEx{ seeds, core }, free_count)
    }

    #[inline]
    fn build_level0_mt<K, CC, SC>(keys: &mut Vec::<K>, conf: &Conf<SS, CC, S>, threads_num: usize, seed_chooser: SC)
        -> (SeedEx<SS::VecElement, C>, Box<[u16]>)
        where K: Hash+Sync+Send, CC: CoreConf<Core=C>, S: Sync, SC: SeedChooser<Core = SCC>
    {
        let mut hashes: Box<[_]> = if keys.len() > 4*2048 {    //maybe better for string keys
            //let mut k = Vec::with_capacity(keys.len());
            //k.par_extend(keys.par_iter().with_min_len(10000).map(|k| hasher.hash_one_s64(k, level_nr)));
            //k.into_boxed_slice()
            keys.par_iter().with_min_len(256).map(|k| conf.hasher.hash_one(k, 0)).collect()
        } else {
            keys.iter().map(|k| conf.hasher.hash_one(k, 0)).collect()
        };
        //radsort::unopt::sort(&mut hashes);
        hashes.voracious_mt_sort(threads_num);
        let core = seed_chooser.f_core_lf(hashes.len(), conf.loading_factor_1000, &conf.core_conf, conf.bits_per_seed());
        let (seeds, builder) =
            build_mt(&hashes, core, conf.seed_size, WINDOW_SIZE, seed_chooser.bucket_evaluator(conf.bits_per_seed(), core.slice_len()), seed_chooser, threads_num);
        let (free_count, bumped_num) = builder.unassigned_values_k(&seeds);
        drop(builder);
        let mut result = Vec::with_capacity(bumped_num);
        std::mem::swap(keys, &mut result);
        keys.par_extend(result.into_par_iter().filter(|key| {
            unsafe { conf.seed_size.get_seed(&seeds, core.bucket_for(conf.hasher.hash_one(key, 0))) == 0 }
        }));
        (SeedEx{ seeds, core }, free_count)
    }

    #[inline]
    fn _new<K, BF, BL, CC>(build_first: BF, build_level: BL, conf: Conf<SS, CC, S>, seed_chooser: SCC, number_of_keys: usize) -> Self
        where BF: FnOnce(&Conf<SS, CC, S>) -> (Vec::<K>, SeedEx<SS::VecElement, C>, Box<[u16]>),
            BL: Fn(&mut Vec::<K>, u64, &S) -> (SeedEx<<Bits8 as SeedSize>::VecElement, GenericCore>, Box<[u64]>),
            K: Hash, CC: CoreConf<Core = C>
        {
        let (mut keys, level0, mut level0_free_count) = build_first(&conf);
        //let bumped_to_index = build_rest(&keys, hasher);
        let mut bumped_index_to_value = Vec::with_capacity(keys.len() * 3 / 2);
        //let mut bumped_index_to_value = Vec::with_capacity(bumped_to_index.output_range());
        
        let mut levels = Vec::with_capacity(16);
        let mut last = 0;   // last value added to bumped_index_to_value
        while !keys.is_empty() {
            let keys_len = keys.len();
            debug_assert!(keys_len <= level0_free_count.iter().map(|v| *v as usize).sum::<usize>());

            let (seeds, level_free_values) = build_level(&mut keys, levels.len() as u64+1, &conf.hasher);
            let shift = bumped_index_to_value.len();
            for i in 0..keys_len {
                if !unsafe{level_free_values.get_bit_unchecked(i)} {
                    while level0_free_count[last] == 0 { last += 1; }
                    level0_free_count[last] -= 1;
                    bumped_index_to_value.push(last);
                } else {
                    bumped_index_to_value.push(if CA::MAX_FOR_UNUSED { usize::MAX } else {last});
                }
            }
            levels.push(Level { seeds, shift });
        }
        drop(level0_free_count);
        Self {
            level0,
            bumped_index_to_value: CA::new(bumped_index_to_value, last, number_of_keys),
            bumped_to_index: levels.into_boxed_slice(),
            hasher: conf.hasher,
            seed_chooser,
            seed_size: conf.seed_size
        }
    }

    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys,
    /// i.e. 1 + maximum value that minimal function can return.
    #[inline(always)] pub fn minimal_output_range(&self, num_of_keys: usize) -> usize { self.seed_chooser.minimal_output_range(num_of_keys) }

    /// Returns output range of `self`, i.e. `1` + maximum value that `self` can return.
    pub fn output_range(&self) -> usize {
        self.level0.core.output_range(self.seed_chooser, self.seed_size.into())
    }

    /// Returns maximum number of keys which can be mapped to the same value by `k`-[`Perfect`] function `self`.
    #[inline(always)] pub fn k(&self) -> u16 { self.seed_chooser.k() }

    /// Returns number of bytes which `write` will write.
    pub fn write_bytes(&self) -> usize {
        self.seed_chooser.write_bytes() +
        self.level0.write_bytes() +
        self.bumped_index_to_value.write_bytes() +
        VByte::size(self.bumped_to_index.len()) +
        self.bumped_to_index.iter().map(|l| l.size_bytes()).sum::<usize>()
    }

    /// Writes `self` to the `output`.
    pub fn write(&self, output: &mut dyn io::Write) -> io::Result<()>
    {
        self.seed_chooser.write(output)?;
        self.level0.write(output, self.seed_size)?;
        self.bumped_index_to_value.write(output)?;
        VByte::write(output, self.bumped_to_index.len())?;
        for level in &self.bumped_to_index {
            level.write(output, Bits8)?;
        }
        Ok(())
    }

    /// Read `Self` from the `input`. `hasher` must be the same as used by the structure written.
    pub fn read_with_hasher(input: &mut dyn io::Read, hasher: S) -> io::Result<Self> {
        let seed_chooser_core = SCC::read(input)?;
        let (seed_size, level0) = SeedEx::read(input)?;
        let unassigned = CA::read(input)?;
        let levels_num: usize = VByte::read(input)?;
        let mut levels = Vec::with_capacity(levels_num);
        for _ in 0..levels_num {
            levels.push(Level::read::<Bits8>(input)?.1);
        }
        Ok(Self { level0, bumped_index_to_value: unassigned, bumped_to_index: levels.into_boxed_slice(), hasher, seed_chooser: seed_chooser_core, seed_size })
    }
}

impl<C: Core, SS: SeedSize> KFunction<C, SS, SeedKCore, DefaultCompressedArray, BuildDefaultSeededHasher> {

    /// Read `Self` from the `input`. Uses default hasher and seed chooser.
    pub fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Self::read_with_hasher(input, BuildDefaultSeededHasher::default())
    }
}

// TODO switch Conf to ConfTurbo ?
impl KFunction<GenericCore, Bits8, SeedKCore, DefaultCompressedArray, BuildDefaultSeededHasher> {
    /// Constructs [`Function`] for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_st<K>(k: u16, keys: Vec::<K>) -> Self where K: Hash {
        Self::with_vec_conf_sc(keys, Conf::default_generic8(), SeedOnlyK::new(k))
    }

    /// Constructs [`Function`] for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_mt<K>(k: u16, keys: Vec::<K>) -> Self where K: Hash+Send+Sync {
        Self::with_vec_conf_threads_sc(keys, Conf::default_generic8(),
        std::thread::available_parallelism().map_or(1, |v| v.into()), SeedOnlyK::new(k))
    }

    /// Constructs [`Function`] for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_st<K>(k: u16, keys: &[K]) -> Self where K: Hash+Clone {
        Self::with_slice_conf_sc(keys, Conf::default_generic8(), SeedOnlyK::new(k))
    }

    /// Constructs [`Function`] for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_mt<K>(k: u16, keys: &[K]) -> Self where K: Hash+Clone+Send+Sync {
        Self::with_slice_conf_threads_sc(keys, Conf::default_generic8(),
        std::thread::available_parallelism().map_or(1, |v| v.into()), SeedOnlyK::new(k))
    }

}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::utils::tests::test_kmphf;

    fn test_read_write<C, SS>(h: &KFunction::<C, SS>)
         where C: Core + PartialEq + std::fmt::Debug, SS: SeedSize, SS::VecElement: std::fmt::Debug+PartialEq
    {
        let mut buff = Vec::new();
        h.write(&mut buff).unwrap();
        //assert_eq!(buff.len(), h.write_bytes());
        let read = KFunction::<C, SS>::read(&mut &buff[..]).unwrap();
        assert_eq!(h.level0.core, read.level0.core);
        assert_eq!(h.bumped_to_index.len(), read.bumped_to_index.len());
        for (hl, rl) in h.bumped_to_index.iter().zip(&read.bumped_to_index) {
            assert_eq!(hl.shift, rl.shift);
            assert_eq!(hl.seeds.core, rl.seeds.core);
            assert_eq!(hl.seeds.seeds, rl.seeds.seeds);
        }
    }

    #[test]
    fn test_small() {
        let input = [1, 2, 3, 4, 5];
        let f = KFunction::from_slice_st(2, &input);
        test_kmphf(2, &input, |key| Some(f.get(key)));
        test_read_write(&f);
    }

    #[test]
    fn test_small2() {
        let input = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21];
        let f = KFunction::from_slice_st(2, &input);
        test_kmphf(2, &input, |key| Some(f.get(key)));
        test_read_write(&f);
    }
}