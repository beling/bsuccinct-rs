use std::{hash::Hash, io, usize};

use crate::{phast::{Conf, Generic, ProdOfValues, SeedChooserCore, conf::{Core, CoreConf}, seed_chooser::SeedCore}, seeds::{Bits8, SeedSize}};
use super::{bits_per_seed_to_100_bucket_size, builder::{build_mt, build_st}, conf::GenericCore, seed_chooser::{SeedChooser, SeedOnly}, CompressedArray, DefaultCompressedArray, WINDOW_SIZE};
use binout::{Serializer, VByte};
use bitm::BitAccess;
use dyn_size_of::GetSize;
use seedable_hash::{BuildDefaultSeededHasher, BuildSeededHasher};
use voracious_radix_sort::RadixSort;
use rayon::prelude::*;

/// Represents map-or-bump function.
pub(crate) struct SeedEx<SSVecElement, C = GenericCore> {
    pub(crate) seeds: Box<[SSVecElement]>,
    pub(crate) core: C,
}

impl<SSVecElement, C: Core> SeedEx<SSVecElement, C> {
    #[inline(always)]
    pub(crate) unsafe fn seed_for<SS>(&self, seed_size: SS, key: u64) -> u16 where SS: SeedSize<VecElement=SSVecElement> {
        //self.seeds.get_fragment(self.bucket_for(key), self.conf.bits_per_seed()) as u16
        seed_size.get_seed(&self.seeds, self.core.bucket_for(key))
    }

    /// Writes `self` to the `output`.
    pub fn write<SS: SeedSize<VecElement = SSVecElement>>(&self, output: &mut dyn io::Write, seed_size: SS) -> io::Result<()>
    {
        self.core.write(output)?;
        seed_size.write_seed_vec(output, &self.seeds)
    }

    /// Reads seed size and `Self` from the `input`.
    pub fn read<SS: SeedSize<VecElement = SSVecElement>>(input: &mut dyn io::Read) -> io::Result<(SS, Self)> {
        let conf = C::read(input)?;
        let (seed_size, seeds) = SS::read_seed_vec(input, conf.buckets_num())?;
        Ok((seed_size, Self{ seeds, core: conf }))
    }
}

impl<SSVecElement: GetSize, C: Core> SeedEx<SSVecElement, C> {
    /// Returns number of bytes which `write` will write.
    pub fn write_bytes(&self) -> usize {
        self.core.write_bytes() + GetSize::size_bytes_dyn(&self.seeds)
    }
}

impl<SSVecElement: GetSize, C> GetSize for SeedEx<SSVecElement, C> {
    fn size_bytes_dyn(&self) -> usize { self.seeds.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.seeds.size_bytes_content_dyn() }
    const USES_DYN_MEM: bool = true;
}


pub(crate) struct Level<SSVecElement, C = GenericCore> {
    pub(crate) seeds: SeedEx<SSVecElement, C>,
    pub(crate) shift: usize
}

impl<SSVecElement: GetSize, C> GetSize for Level<SSVecElement, C> {
    fn size_bytes_dyn(&self) -> usize { self.seeds.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.seeds.size_bytes_content_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl<SSVecElement: GetSize, C: Core> Level<SSVecElement, C> {
    pub fn write_bytes(&self) -> usize {
        VByte::size(self.shift) + self.seeds.write_bytes()
    }
}

impl<SSVecElement, C: Core> Level<SSVecElement, C> {
    /// Writes `self` to the `output`.
    pub fn write<SS: SeedSize<VecElement = SSVecElement>>(&self, output: &mut dyn io::Write, seed_size: SS) -> io::Result<()>
    {
        VByte::write(output, self.shift)?;
        self.seeds.write(output, seed_size)
    }

    /// Reads seed size and `Self` from the `input`.
    pub fn read<SS: SeedSize<VecElement = SSVecElement>>(input: &mut dyn io::Read) -> io::Result<(SS, Self)> {
        let shift = VByte::read(input)?;
        SeedEx::<SSVecElement, C>::read::<SS>(input).map(|(ss, seeds)| (ss, Self{ seeds, shift }))
    }
}

#[inline]
pub(crate) fn build_level_from_slice_st<K, SS, CC, SC, S>(keys: &[K], params: &Conf<SS, CC>, hasher: &S, seed_chooser: SC, level_nr: u64)
    -> (Vec<K>, SeedEx<SS::VecElement, CC::Core>, Box<[u64]>)
    where K: Hash+Clone, SC: SeedChooser, SS: SeedSize, CC: CoreConf, S: BuildSeededHasher
{
    let mut hashes: Box<[_]> = keys.iter().map(|k| hasher.hash_one(k, level_nr)).collect();
    //radsort::unopt::sort(&mut hashes);
    hashes.voracious_sort();
    let core = seed_chooser.conf_for_minimal_p(hashes.len(), &params.core_conf, params.bits_per_seed());
    let (seeds, builder) =
        build_st(&hashes, core, params.seed_size, seed_chooser.bucket_evaluator(params.bits_per_seed(), core.slice_len()), seed_chooser);
    let (unassigned_values, bumped_len) = builder.unassigned_values(&seeds);
    drop(builder);
    let mut keys_vec = Vec::with_capacity(bumped_len);
    keys_vec.extend(keys.into_iter().filter(|key| {
        unsafe { params.seed_size.get_seed(&seeds, core.bucket_for(hasher.hash_one(key, level_nr))) == 0 }
    }).cloned());
    (keys_vec, SeedEx{ seeds, core }, unassigned_values)
}

#[inline]
pub(crate) fn build_level_from_slice_mt<K, SS, CC, SC, S>(keys: &[K], params: &Conf<SS, CC>, threads_num: usize, hasher: &S, seed_chooser: SC, level_nr: u64)
    -> (Vec<K>, SeedEx<SS::VecElement, CC::Core>, Box<[u64]>)
    where K: Hash+Sync+Send+Clone, SC: SeedChooser, SS: SeedSize, CC: CoreConf, SC: SeedChooser, S: BuildSeededHasher+Sync
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
    let conf = seed_chooser.conf_for_minimal_p(hashes.len(), &params.core_conf, params.bits_per_seed());
    let (seeds, builder) =
        build_mt(&hashes, conf, params.seed_size, WINDOW_SIZE, seed_chooser.bucket_evaluator(params.bits_per_seed(), conf.slice_len()), seed_chooser, threads_num);
    let (unassigned_values, bumped_len) = builder.unassigned_values(&seeds);
    drop(builder);
    let mut keys_vec = Vec::with_capacity(bumped_len);
    keys_vec.par_extend(keys.into_par_iter().filter(|key| {
        unsafe { params.seed_size.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, level_nr))) == 0 }
    }).cloned());
    (keys_vec, SeedEx{ seeds, core: conf }, unassigned_values)
}

#[inline(always)]
pub(crate) fn build_level_st<K, SS, CC, SC, S>(keys: &mut Vec::<K>, params: &Conf<SS, CC>, hasher: &S, seed_chooser: SC, level_nr: u64)
    -> (SeedEx<SS::VecElement, CC::Core>, Box<[u64]>)
    where K: Hash,SC: SeedChooser, SS: SeedSize, SC: SeedChooser, CC: CoreConf, S: BuildSeededHasher
{
    let mut hashes: Box<[_]> = keys.iter().map(|k| hasher.hash_one(k, level_nr)).collect();
    hashes.voracious_sort();
    let conf = seed_chooser.conf_for_minimal_p(hashes.len(), &params.core_conf, params.bits_per_seed());
    let (seeds, builder) =
        build_st(&hashes, conf, params.seed_size, seed_chooser.bucket_evaluator(params.bits_per_seed(), conf.slice_len()), seed_chooser);
    let (unassigned_values, _) = builder.unassigned_values(&seeds);
    drop(builder);
    keys.retain(|key| {
        unsafe { params.seed_size.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, level_nr))) == 0 }
    });
    (SeedEx{ seeds, core: conf }, unassigned_values)
}

#[inline]
pub(crate) fn build_level_mt<K, SS, CC, SC, S>(keys: &mut Vec::<K>, params: &Conf<SS, CC>, threads_num: usize, hasher: &S, seed_chooser: SC, level_nr: u64)
    -> (SeedEx<SS::VecElement, CC::Core>, Box<[u64]>)
    where K: Hash+Sync+Send, SC: SeedChooser, SS: SeedSize, CC: CoreConf, SC: SeedChooser, S: BuildSeededHasher+Sync
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
    let conf = seed_chooser.conf_for_minimal_p(hashes.len(), &params.core_conf, params.bits_per_seed());
    let (seeds, builder) =
        build_mt(&hashes, conf, params.seed_size, WINDOW_SIZE, seed_chooser.bucket_evaluator(params.bits_per_seed(), conf.slice_len()), seed_chooser, threads_num);
    let (unassigned_values, bumped_len) = builder.unassigned_values(&seeds);
    drop(builder);
    let mut result = Vec::with_capacity(bumped_len);
    std::mem::swap(keys, &mut result);
    keys.par_extend(result.into_par_iter().filter(|key| {
        unsafe { params.seed_size.get_seed(&seeds, conf.bucket_for(hasher.hash_one(key, level_nr))) == 0 }
    }));
    (SeedEx{ seeds, core: conf }, unassigned_values)
}

/// PHast (Perfect Hashing made fast) - Minimal Perfect Hash Function
/// with very fast evaluation and size below 2 bits/key
/// developed by Piotr Beling and Peter Sanders.
/// 
/// It can be used with the following [`SeedChooser`] (which specify a particular PHast variant):
/// [`ShiftOnlyWrapped`] (PHast+ with wrapping),
/// [`ShiftSeedWrapped`] (PHast/PHast+ hybrid),
/// [`SeedOnly`] (regular PHast).
/// 
/// Note that some [`SeedChooser`]s can be used only with [`Function2`](crate::phast::Function2).
/// 
/// See:
/// Piotr Beling, Peter Sanders, *PHast - Perfect Hashing made fast*, 2025, <https://arxiv.org/abs/2504.17918>
pub struct Function<C: Core, SS, SC = SeedCore, CA = DefaultCompressedArray, S = BuildDefaultSeededHasher>
    where SS: SeedSize
{
    level0: SeedEx<SS::VecElement, C>,
    bumped_index_to_value: CA,
    bumped_to_index: Box<[Level<SS::VecElement, C>]>,
    hasher: S,
    seed_chooser: SC,
    seed_size: SS,  // seed size, K=2**bits_per_seed
}

impl<C: Core, SS: SeedSize, SC, CA, S> GetSize for Function<C, SS, SC, CA, S> where CA: GetSize {
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

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore, CA: CompressedArray, S: BuildSeededHasher> Function<C, SS, SCC, CA, S> {
    
    /// Returns value assigned to the given `key`.
    /// 
    /// The returned value is in the range from `0` (inclusive) to the number of elements in the input key collection (exclusive).
    /// `key` must come from the input key collection given during construction.
    #[inline(always)]   //inline(always) is important here
    pub fn get<K>(&self, key: &K) -> usize where K: Hash + ?Sized {
        /* TODO turbo support: let slice_begin = self.level0.slice_begin(key_hash);
        let seed = self.level0.seed_for_slice(slice_begin);
        if seed != 0 { return SC::f_slice(key_hash, slice_begin, seed, &self.level0.conf); } */

        /*let key_hash = self.hasher.hash_one(key, 0);
        let seed = unsafe{ self.level0.seed_for(self.seed_size, key_hash) };
        if seed != 0 { return self.seed_chooser.f(key_hash, seed, &self.level0.conf); }*/
        if let Some(result) = self.seed_chooser.try_f(self.seed_size, &self.level0.seeds, self.hasher.hash_one(key, 0), &self.level0.core) {
            return result;
        }

        for level_nr in 0..self.bumped_to_index.len() {
            let l = &self.bumped_to_index[level_nr];
            /*let key_hash = self.hasher.hash_one(key, level_nr as u64 + 1);
            let seed = unsafe { l.seeds.seed_for(self.seed_size, key_hash) };
            if seed != 0 {
                return self.unassigned.get(self.seed_chooser.f(key_hash, seed, &l.seeds.conf) + l.shift)
            }*/
            if let Some(result) = self.seed_chooser.try_f(self.seed_size, &l.seeds.seeds, self.hasher.hash_one(key, level_nr as u64 + 1), &l.seeds.core) {
                return self.bumped_index_to_value.get(result + l.shift);
            }
        }
        unreachable!()
    }

    /// Constructs [`Function`] for given `keys`, using a single thread and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_vec_p_hash_sc<K, CC, SC>(mut keys: Vec::<K>, params: &Conf<SS, CC>, hasher: S, seed_chooser: SC) -> Self
        where K: Hash, CC: CoreConf<Core = C>, SC: SeedChooser<Core = SCC>
    {
        let number_of_keys = keys.len();
        Self::_new(|h| {
            let (level0, unassigned_values) =
                build_level_st(&mut keys, params, h, seed_chooser.clone(), 0);
            (keys, level0, unassigned_values)
        }, |keys, level_nr, h| {
            build_level_st(keys, params, h, seed_chooser.clone(), level_nr)
        }, hasher, seed_chooser.core(), params.seed_size, number_of_keys)
    }

    /// Constructs [`Function`] for given `keys`, using multiple (given number of) threads and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_vec_p_threads_hash_sc<K, CC, SC>(mut keys: Vec::<K>, params: &Conf<SS, CC>, threads_num: usize, hasher: S, seed_chooser: SC) -> Self
        where K: Hash+Sync+Send, S: Sync, SC: SeedChooser<Core = SCC>, CC: CoreConf<Core = C>
    {
        if threads_num == 1 { return Self::with_vec_p_hash_sc(keys, params, hasher, seed_chooser); }
        let number_of_keys = keys.len();
        Self::_new(|h| {
            let (level0, unassigned_values) =
                build_level_mt(&mut keys, params, threads_num, h, seed_chooser.clone(), 0);
            (keys, level0, unassigned_values)
        }, |keys, level_nr, h| {
            build_level_mt(keys, params, threads_num, h, seed_chooser.clone(), level_nr)
        }, hasher, seed_chooser.core(), params.seed_size, number_of_keys)
    }


    /// Constructs [`Function`] for given `keys`, using a single thread and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_slice_p_hash_sc<K, CC, SC>(keys: &[K], params: &Conf<SS, CC>, hasher: S, seed_chooser: SC) -> Self
        where K: Hash+Clone, CC: CoreConf<Core = C>, SC: SeedChooser<Core = SCC>
    {
        Self::_new(|h| {
            build_level_from_slice_st(keys, params, h, seed_chooser.clone(), 0)
        }, |keys, level_nr, h| {
            build_level_st(keys, params, h, seed_chooser.clone(), level_nr)
        }, hasher, seed_chooser.core(), params.seed_size, keys.len())
    }


    /// Constructs [`Function`] for given `keys`, using multiple (given number of) threads and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_slice_p_threads_hash_sc<K, CC, SC>(keys: &[K], params: &Conf<SS, CC>, threads_num: usize, hasher: S, seed_chooser: SC) -> Self
        where K: Hash+Sync+Send+Clone, S: Sync, SC: SeedChooser<Core = SCC>, CC: CoreConf<Core = C> {
        if threads_num == 1 { return Self::with_slice_p_hash_sc(keys, params, hasher, seed_chooser); }
        Self::_new(|h| {
            build_level_from_slice_mt(keys, params, threads_num, h, seed_chooser.clone(), 0)
        }, |keys, level_nr, h| {
            build_level_mt(keys, params, threads_num, h, seed_chooser.clone(), level_nr)
        }, hasher, seed_chooser.core(), params.seed_size, keys.len())
    }

    #[inline]
    fn _new<K, BF, BL>(build_first: BF, build_level: BL, hasher: S, seed_chooser: SCC, seed_size: SS, number_of_keys: usize) -> Self
        where BF: FnOnce(&S) -> (Vec::<K>, SeedEx<SS::VecElement, C>, Box<[u64]>),
            BL: Fn(&mut Vec::<K>, u64, &S) -> (SeedEx<SS::VecElement, C>, Box<[u64]>),
            K: Hash,
        {
        let (mut keys, level0, unassigned_values) = build_first(&hasher);
        debug_assert_eq!(keys.len(), unassigned_values.bit_ones().count()); // only true for output range = number of keys
        //dbg!(unassigned_len, keys.len(), unassigned_values.bit_ones().count());
        //Self::finish_building(keys, bits_per_seed, bucket_size100, threads_num, hasher, level0, unassigned_values, unassigned_len)
        let mut level0_unassigned = unassigned_values.bit_ones();
        let mut bumped_index_to_value = Vec::with_capacity(keys.len() * 3 / 2);

        let mut levels = Vec::with_capacity(16);
        let mut last = 0;   // last value added to bumped_index_to_value
        while !keys.is_empty() {
            let keys_len = keys.len();

            //println!("{keys_len} {:.2}% keys bumped, {} {}% in {} self-collided buckets",
            //    keys_len as f64 / 100000.0,
                //crate::phast::seed_chooser::SELF_COLLISION_KEYS.load(std::sync::atomic::Ordering::SeqCst),
                //crate::phast::seed_chooser::SELF_COLLISION_KEYS.load(std::sync::atomic::Ordering::SeqCst) * 100 / keys_len as u64,
                //crate::phast::seed_chooser::SELF_COLLISION_BUCKETS.load(std::sync::atomic::Ordering::SeqCst));
            let (seeds, level_free_values) =
                build_level(&mut keys, levels.len() as u64+1, &hasher);
            let shift = bumped_index_to_value.len();
            for i in 0..keys_len {
                if CA::MAX_FOR_UNUSED {
                    if !unsafe{level_free_values.get_bit_unchecked(i)} {
                        last = level0_unassigned.next().unwrap();
                        bumped_index_to_value.push(last);
                    } else {
                        bumped_index_to_value.push(usize::MAX);
                    }
                } else {
                    if !unsafe{level_free_values.get_bit_unchecked(i)} {
                        last = level0_unassigned.next().unwrap();
                    }
                    bumped_index_to_value.push(last);
                }
            }
            levels.push(Level { seeds, shift });
        }
        debug_assert!(level0_unassigned.next().is_none());  // only true for output range = number of keys
        drop(level0_unassigned);
        Self {
            level0,
            bumped_index_to_value: CA::new(bumped_index_to_value, last, number_of_keys),
            bumped_to_index: levels.into_boxed_slice(),
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
        self.level0.core.output_range(self.seed_chooser, self.seed_size.into())
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

    /// Returns number of bytes which `write` will write.
    pub fn write_bytes(&self) -> usize {
        self.level0.write_bytes() +
        self.bumped_index_to_value.write_bytes() +
        VByte::size(self.bumped_to_index.len()) +
        self.bumped_to_index.iter().map(|l| l.size_bytes()).sum::<usize>()
    }

    /// Writes `self` to the `output`.
    pub fn write(&self, output: &mut dyn io::Write) -> io::Result<()>
    {
        self.level0.write(output, self.seed_size)?;
        self.bumped_index_to_value.write(output)?;
        VByte::write(output, self.bumped_to_index.len())?;
        for level in &self.bumped_to_index {
            level.write(output, self.seed_size)?;
        }
        Ok(())
    }

    /// Read `Self` from the `input`. `hasher` and `seed_chooser` must be the same as used by the structure written.
    pub fn read_with_hasher_sc(input: &mut dyn io::Read, hasher: S, seed_chooser_core: SCC) -> io::Result<Self> {
        let (seed_size, level0) = SeedEx::read(input)?;
        let unassigned = CA::read(input)?;
        let levels_num: usize = VByte::read(input)?;
        let mut levels = Vec::with_capacity(levels_num);
        for _ in 0..levels_num {
            levels.push(Level::read::<SS>(input)?.1);
        }
        Ok(Self { level0, bumped_index_to_value: unassigned, bumped_to_index: levels.into_boxed_slice(), hasher, seed_chooser: seed_chooser_core, seed_size })
    }
}

impl<C: Core, SS: SeedSize> Function<C, SS, SeedCore, DefaultCompressedArray, BuildDefaultSeededHasher> {

    /// Read `Self` from the `input`. Uses default hasher and seed chooser.
    pub fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Self::read_with_hasher_sc(input, BuildDefaultSeededHasher::default(), SeedCore)
    }
}

// TODO switch Conf to ConfTurbo ?
impl Function<GenericCore, Bits8, SeedCore, DefaultCompressedArray, BuildDefaultSeededHasher> {
    /// Constructs [`Function`] for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_st<K>(keys: Vec::<K>) -> Self where K: Hash {
        Self::with_vec_p_hash_sc(keys, &Conf { seed_size: Bits8::default(), core_conf: Generic::new(bits_per_seed_to_100_bucket_size(8)) },
        BuildDefaultSeededHasher::default(), SeedOnly(ProdOfValues))
    }

    /// Constructs [`Function`] for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_mt<K>(keys: Vec::<K>) -> Self where K: Hash+Send+Sync {
        Self::with_vec_p_threads_hash_sc(keys, &Conf { seed_size: Bits8::default(), core_conf: Generic::new(bits_per_seed_to_100_bucket_size(8)) },
        std::thread::available_parallelism().map_or(1, |v| v.into()), BuildDefaultSeededHasher::default(), SeedOnly(ProdOfValues))
    }

    /// Constructs [`Function`] for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_st<K>(keys: &[K]) -> Self where K: Hash+Clone {
        Self::with_slice_p_hash_sc(keys, &Conf { seed_size: Bits8::default(), core_conf: Generic::new(bits_per_seed_to_100_bucket_size(8)) },
        BuildDefaultSeededHasher::default(), SeedOnly(ProdOfValues))
    }

    /// Constructs [`Function`] for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_mt<K>(keys: &[K]) -> Self where K: Hash+Clone+Send+Sync {
        Self::with_slice_p_threads_hash_sc(keys, &Conf { seed_size: Bits8::default(), core_conf: Generic::new(bits_per_seed_to_100_bucket_size(8)) },
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