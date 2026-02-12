use std::{hash::Hash, io, usize};

use crate::{phast::{function::{build_level_from_slice_mt, build_level_from_slice_st, build_level_mt, build_level_st, Level, SeedEx}, seed_chooser::SeedOnlyNoBump, Params, ShiftOnlyWrapped}, seeds::{Bits8, SeedSize}};
use super::{bits_per_seed_to_100_bucket_size, builder::build_last_level, conf::Conf, seed_chooser::{SeedChooser, SeedOnly}, CompressedArray, DefaultCompressedArray};
use binout::{Serializer as _, VByte};
use bitm::BitAccess;
use dyn_size_of::GetSize;
use seedable_hash::{BuildDefaultSeededHasher, BuildSeededHasher};
use voracious_radix_sort::RadixSort;

/// PHast (Perfect Hashing made fast) - Minimal Perfect Hash Function
/// with very fast evaluation and size below 2 bits/key
/// developed by Piotr Beling and Peter Sanders.
/// 
/// The last layer (when the number of keys is small) is constructed using regular PHast.
/// This makes `Function2` compatible with almost all [`SeedChooser`]s (including non-wrapping `ShiftOnly`).
/// 
/// It can be used with the following [`SeedChooser`] (which specify a particular PHast variant):
/// [`ShiftOnly`] (PHast+ without wrapping),
/// [`ShiftOnlyWrapped`] (PHast+ with wrapping),
/// [`ShiftSeedWrapped`] (PHast/PHast+ hybrid),
/// [`SeedOnly`] (regular PHast).
/// 
/// Note that some [`SeedChooser`]s can also be used with [`Function`](crate::phast::Function).
/// 
/// See:
/// Piotr Beling, Peter Sanders, *PHast - Perfect Hashing made fast*, 2025, <https://arxiv.org/abs/2504.17918>
pub struct Function2<SS, SC = ShiftOnlyWrapped, CA = DefaultCompressedArray, S = BuildDefaultSeededHasher>
    where SS: SeedSize
{
    level0: SeedEx<SS::VecElement>,
    unassigned: CA,
    levels: Box<[Level<SS::VecElement>]>,
    hasher: S,
    last_level: Level<<Bits8 as SeedSize>::VecElement>,
    last_level_seed: u64,
    seed_chooser: SC,
    seed_size: SS,
}

impl<SC, SS: SeedSize, CA, S> GetSize for Function2<SS, SC, CA, S> where CA: GetSize {
    fn size_bytes_dyn(&self) -> usize {
        self.level0.size_bytes_dyn() +
            self.unassigned.size_bytes_dyn() +
            self.levels.size_bytes_dyn() +
            self.last_level.size_bytes_dyn()
    }
    fn size_bytes_content_dyn(&self) -> usize {
        self.level0.size_bytes_content_dyn() +
            self.unassigned.size_bytes_content_dyn() +
            self.levels.size_bytes_content_dyn() +
            self.last_level.size_bytes_content_dyn()
    }
    const USES_DYN_MEM: bool = true;
}

impl<SS: SeedSize, SC: SeedChooser, CA: CompressedArray, S: BuildSeededHasher> Function2<SS, SC, CA, S> {
    
    /// Returns value assigned to the given `key`.
    /// 
    /// The returned value is in the range from `0` (inclusive) to the number of elements in the input key collection (exclusive).
    /// `key` must come from the input key collection given during construction.
    #[inline(always)]   //inline(always) is important here
    pub fn get<K>(&self, key: &K) -> usize where K: Hash + ?Sized {
        let key_hash = self.hasher.hash_one(key, 0);
        let seed = unsafe { self.level0.seed_for(self.seed_size, key_hash) };
        if seed != 0 { return self.seed_chooser.f(key_hash, seed, &self.level0.conf); }

        for level_nr in 0..self.levels.len() {
            let l = &self.levels[level_nr];
            let key_hash = self.hasher.hash_one(key, level_nr as u64 + 1);
            let seed = unsafe { l.seeds.seed_for(self.seed_size, key_hash) };
            if seed != 0 {
                return self.unassigned.get(self.seed_chooser.f(key_hash, seed, &l.seeds.conf) + l.shift)
            }
        }

        let key_hash = self.hasher.hash_one(key, self.last_level_seed);
        let seed = unsafe { self.last_level.seeds.seed_for(Bits8, key_hash) };
        return self.unassigned.get(SeedOnlyNoBump.f(key_hash, seed, &self.last_level.seeds.conf) + self.last_level.shift)
    }

    /// Constructs [`Function`] for given `keys`, using a single thread and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_vec_p_hash_sc<K>(mut keys: Vec::<K>, params: &Params<SS>, hasher: S, seed_chooser: SC) -> Self where K: Hash {
        let number_of_keys = keys.len();
        Self::_new(|h| {
            let (level0, unassigned_values, unassigned_len) =
                build_level_st(&mut keys, params, h, seed_chooser, 0);
            (keys, level0, unassigned_values, unassigned_len)
        }, |keys, level_nr, h| {
            build_level_st(keys, params, h, seed_chooser, level_nr)
        }, hasher, seed_chooser, params.seed_size, number_of_keys)
    }

    /// Constructs [`Function`] for given `keys`, using multiple (given number of) threads and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_vec_p_threads_hash_sc<K>(mut keys: Vec::<K>, params: &Params<SS>, threads_num: usize, hasher: S, seed_chooser: SC) -> Self
        where K: Hash+Sync+Send, S: Sync, SC: Sync {
        if threads_num == 1 { return Self::with_vec_p_hash_sc(keys, params, hasher, seed_chooser); }
        let number_of_keys = keys.len();
        Self::_new(|h| {
            let (level0, unassigned_values, unassigned_len) =
                build_level_mt(&mut keys, params, threads_num, h, seed_chooser, 0);
            (keys, level0, unassigned_values, unassigned_len)
        }, |keys, level_nr, h| {
            build_level_mt(keys, params, threads_num, h, seed_chooser, level_nr)
        }, hasher, seed_chooser, params.seed_size, number_of_keys)
    }


    /// Constructs [`Function`] for given `keys`, using a single thread and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_slice_p_hash_sc<K>(keys: &[K], params: &Params<SS>, hasher: S, seed_chooser: SC) -> Self where K: Hash+Clone {
        Self::_new(|h| {
            build_level_from_slice_st(keys, params, h, seed_chooser, 0)
        }, |keys, level_nr, h| {
            build_level_st(keys, params, h, seed_chooser, level_nr)
        }, hasher, seed_chooser, params.seed_size, keys.len())
    }


    /// Constructs [`Function`] for given `keys`, using multiple (given number of) threads and given parameters:
    /// number of bits per seed, average bucket size (equals `bucket_size100/100.0`) and `hasher`.
    /// 
    /// `bits_per_seed_to_100_bucket_size` can be used to calculate good `bucket_size100`.
    /// `keys` cannot contain duplicates.
    pub fn with_slice_p_threads_hash_sc<K>(keys: &[K], params: &Params<SS>, threads_num: usize, hasher: S, seed_chooser: SC) -> Self
        where K: Hash+Sync+Send+Clone, S: Sync, SC: Sync {
        if threads_num == 1 { return Self::with_slice_p_hash_sc(keys, params, hasher, seed_chooser); }
        Self::_new(|h| {
            build_level_from_slice_mt(keys, params, threads_num, h, seed_chooser, 0)
        }, |keys, level_nr, h| {
            build_level_mt(keys, params, threads_num, h, seed_chooser, level_nr)
        }, hasher, seed_chooser, params.seed_size, keys.len())
    }

    #[inline]
    fn _new<K, BF, BL>(build_first: BF, build_level: BL, hasher: S, seed_chooser: SC, seed_size: SS, number_of_keys: usize) -> Self
        where BF: FnOnce(&S) -> (Vec::<K>, SeedEx<SS::VecElement>, Box<[u64]>, usize),
            BL: Fn(&mut Vec::<K>, u64, &S) -> (SeedEx<SS::VecElement>, Box<[u64]>, usize),
            K: Hash
        {
        let (mut keys, level0, unassigned_values, unassigned_len) = build_first(&hasher);
        //dbg!(keys.len(), unassigned_len, unassigned_values.bit_ones().count());
        debug_assert_eq!(unassigned_len, unassigned_values.bit_ones().count());
        //Self::finish_building(keys, bits_per_seed, bucket_size100, threads_num, hasher, level0, unassigned_values, unassigned_len)
        let mut level0_unassigned = unassigned_values.bit_ones();
        let mut unassigned = Vec::with_capacity(unassigned_len * 3 / 2);

        let mut levels = Vec::with_capacity(16);
        let mut last = 0;
        //let max_keys = 2048.max(SC::extra_shift(bits_p))
        while keys.len() > SC::FUNCTION2_THRESHOLD /*2048*2*/ {
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
        //dbg!(keys.len());   // TODO keys.len()==0
        let mut last_seed = levels.len() as u64+1;
        let last_shift;
        let last_seeds =
        if keys.is_empty() {
            last_shift = 0;
            SeedEx::<<Bits8 as SeedSize>::VecElement>{ seeds: Box::default(), conf: Conf { buckets_num: 0, slice_len_minus_one: 0, num_of_slices: 0 } }
        } else {
            let (last_seeds, unassigned_values, _unassigned_len) =
                Self::build_last_level(keys, &hasher, &mut last_seed);
            last_shift = unassigned.len();
            for i in 0..last_seeds.conf.output_range(SeedOnlyNoBump, Bits8.into()) {
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
            //drop(unassigned_values);
            last_seeds
        };
        debug_assert!(level0_unassigned.next().is_none());  // TODO
        drop(level0_unassigned);
        Self {
            level0,
            unassigned: CA::new(unassigned, last, number_of_keys),
            levels: levels.into_boxed_slice(),
            hasher,
            seed_chooser,
            last_level: Level { seeds: last_seeds, shift: last_shift },
            last_level_seed: last_seed,
            seed_size,
        }
    }

    #[inline(always)]
    fn build_last_level<K>(keys: Vec::<K>, hasher: &S, seed: &mut u64)
        -> (SeedEx<<Bits8 as SeedSize>::VecElement>, Box<[u64]>, usize)
        where K: Hash
    {
        let bits_per_seed = Bits8;
        let len100 = (keys.len()+10)*120;
        let conf = SeedOnly.conf_for_minimal((len100+50)/100,
            bits_per_seed.into(), 400, 0);  // TODO use turbo variant here
        let evaluator = SeedOnly.bucket_evaluator(bits_per_seed.into(), conf.slice_len());
        loop {
            let mut hashes: Box<[_]> = keys.iter().map(|k| hasher.hash_one(k, *seed)).collect();
            hashes.voracious_sort();    // maybe standard sort here?
            if let Some((seeds, unassigned_values, unassigned_len)) =
                build_last_level(&hashes, conf, bits_per_seed, evaluator.clone())
            {
                return (SeedEx{ seeds, conf }, unassigned_values, unassigned_len);
            }
            *seed += 1;
            //dbg!(*seed);
        }
    }

    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys,
    /// i.e. 1 + maximum value that minimal function can return.
    #[inline(always)] pub fn minimal_output_range(&self, num_of_keys: usize) -> usize { self.seed_chooser.minimal_output_range(num_of_keys) }

    /// Returns output range of `self`, i.e. 1 + maximum value that `self` can return.
    pub fn output_range(&self) -> usize {
        self.level0.conf.output_range(self.seed_chooser, self.seed_size.into())
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

    /// Writes `self` to the `output`.
    pub fn write(&self, output: &mut dyn io::Write) -> io::Result<()>
    {
        self.level0.write(output, self.seed_size)?;
        self.unassigned.write(output)?;
        VByte::write(output, self.levels.len())?;
        for level in &self.levels {
            level.write(output, self.seed_size)?;
        }
        VByte::write(output, self.last_level_seed)?;
        self.last_level.write(output, Bits8)?;
        Ok(())
    }

    /// Returns number of bytes which `write` will write.
    pub fn write_bytes(&self) -> usize {
        self.level0.write_bytes() +
        self.unassigned.write_bytes() +
        VByte::size(self.levels.len()) +
        self.levels.iter().map(|l| l.size_bytes()).sum::<usize>() +
        VByte::size(self.last_level_seed) +
        self.last_level.write_bytes()
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
        let last_level_seed = VByte::read(input)?;
        let last_level = Level::read::<Bits8>(input)?.1;
        Ok(Self { level0, unassigned, levels: levels.into_boxed_slice(), hasher, seed_chooser, seed_size, last_level, last_level_seed })
    }
}

impl<SS: SeedSize> Function2<SS, ShiftOnlyWrapped, DefaultCompressedArray, BuildDefaultSeededHasher> {

    /// Read `Self` from the `input`. Uses default hasher and seed chooser.
    pub fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Self::read_with_hasher_sc(input, BuildDefaultSeededHasher::default(), ShiftOnlyWrapped)
    }
}

impl Function2<Bits8, ShiftOnlyWrapped, DefaultCompressedArray, BuildDefaultSeededHasher> {
    /// Constructs [`Function`] for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_st<K>(keys: Vec::<K>) -> Self where K: Hash {
        Self::with_vec_p_hash_sc(keys, &Params::new(Bits8, bits_per_seed_to_100_bucket_size(8)),
        BuildDefaultSeededHasher::default(), ShiftOnlyWrapped)
    }

    /// Constructs [`Function`] for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_vec_mt<K>(keys: Vec::<K>) -> Self where K: Hash+Send+Sync {
        Self::with_vec_p_threads_hash_sc(keys, &Params::new(Bits8, bits_per_seed_to_100_bucket_size(8)),
        std::thread::available_parallelism().map_or(1, |v| v.into()), BuildDefaultSeededHasher::default(), ShiftOnlyWrapped)
    }

    /// Constructs [`Function`] for given `keys`, using a single thread.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_st<K>(keys: &[K]) -> Self where K: Hash+Clone {
        Self::with_slice_p_hash_sc(keys, &Params::new(Bits8, bits_per_seed_to_100_bucket_size(8)),
        BuildDefaultSeededHasher::default(), ShiftOnlyWrapped)
    }

    /// Constructs [`Function`] for given `keys`, using multiple threads.
    /// 
    /// `keys` cannot contain duplicates.
    pub fn from_slice_mt<K>(keys: &[K]) -> Self where K: Hash+Clone+Send+Sync {
        Self::with_slice_p_threads_hash_sc(keys, &Params::new(Bits8, bits_per_seed_to_100_bucket_size(8)),
        std::thread::available_parallelism().map_or(1, |v| v.into()), BuildDefaultSeededHasher::default(), ShiftOnlyWrapped)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::utils::tests::test_mphf;

    fn test_read_write<SS: SeedSize>(h: &Function2::<SS>) where SS::VecElement: std::fmt::Debug+PartialEq {
        let mut buff = Vec::new();
        h.write(&mut buff).unwrap();
        //assert_eq!(buff.len(), h.write_bytes());
        let read = Function2::<SS>::read(&mut &buff[..]).unwrap();
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
        let f = Function2::from_slice_st(&input);
        test_mphf(&input, |key| Some(f.get(key)));
        test_read_write(&f);
    }
}