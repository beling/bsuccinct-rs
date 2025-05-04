use crate::seeds::SeedSize;

use super::{builder::UsedValues, conf::Conf};

/// Choose best seed in bucket.
pub trait SeedChooser {
    const BUMPING: bool = true;
    const FIRST_SEED: u16 = if Self::BUMPING { 1 } else { 0 };

    fn conf<SS: SeedSize>(output_range: usize, bits_per_seed: SS, bucket_size_100: u16) -> Conf<SS> {
        let max_shift = Self::extra_shift(bits_per_seed);
        let slice_len = match output_range.saturating_sub(max_shift as usize) {
            n @ 0..64 => (n/2+1).next_power_of_two() as u16,
            64..1300 => 64,
            1300..1750 => 128,
            1750..7500 => 256,
            7500..150000 => 512,
            _ if bits_per_seed.into() < 7 => 512,
            _ => 1024
        };
        Conf::<SS>::new(output_range, bits_per_seed, bucket_size_100, slice_len, max_shift)
    }

    /// How much the chooser can add to value over slice length.
    #[inline(always)] fn extra_shift<SS: SeedSize>(_seed_size: SS) -> u16 { 0 }

    /// Returns function value for given primary code and seed.
    fn f<SS: SeedSize>(primary_code: u64, seed: u16, conf: &Conf<SS>) -> usize;
    
    /// Returns best seed to store in seeds array or `u16::MAX` if `NO_BUMPING` is `true` and there is no feasible seed.
    fn best_seed<SS: SeedSize>(used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) -> u16;
}

#[inline(always)]
fn best_seed_big<SC: SeedChooser, SS: SeedSize>(best_value: &mut usize, best_seed: &mut u16, used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) {
    let mut values_used_by_seed = Vec::with_capacity(keys.len());
    let simd_keys = keys.len() / 4 * 4;
    //assert!(simd_keys <= keys.len());
    'outer: for seed in SC::FIRST_SEED..conf.seeds_num() {    // seed=0 is special = no seed,
        values_used_by_seed.clear();
        for i in (0..simd_keys).step_by(4) {
            let values = [
                SC::f(keys[i], seed, conf),
                SC::f(keys[i+1], seed, conf),
                SC::f(keys[i+2], seed, conf),
                SC::f(keys[i+3], seed, conf),
            ];
            let contains = [
                used_values.contain(values[0]),
                used_values.contain(values[1]),
                used_values.contain(values[2]),
                used_values.contain(values[3]),
            ];
            if contains.iter().any(|b| *b) { continue 'outer; }
            //if contains[0] || contains[1] || contains[2] || contains[3] { continue 'outer; }
            values_used_by_seed.push(values[0]);
            values_used_by_seed.push(values[1]);
            values_used_by_seed.push(values[2]);
            values_used_by_seed.push(values[3]);
        }
        //assert!(keys.len() - simd_keys < 4);
        for i in simd_keys..keys.len() {
            let value = SC::f(keys[i], seed, conf);
            if used_values.contain(value) { continue 'outer; }
            values_used_by_seed.push(value);
        }
        let seed_value = values_used_by_seed.iter().sum();
        if seed_value < *best_value {
            values_used_by_seed.sort();
            if values_used_by_seed.windows(2).any(|v| v[0]==v[1]) {
                //SELF_COLLISION_KEYS.fetch_add(keys.len() as u64, std::sync::atomic::Ordering::Relaxed);
                //SELF_COLLISION_BUCKETS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                //if SC::BUMPING { return; }
                continue;
            }
            *best_value = seed_value;
            *best_seed = seed;
        }
    }
}

#[inline(always)]
fn best_seed_small<SC: SeedChooser, SS: SeedSize>(best_value: &mut usize, best_seed: &mut u16, used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) {
    assert!(keys.len() <= SMALL_BUCKET_LIMIT);  // seems to speeds up a bit
    let mut values_used_by_seed = arrayvec::ArrayVec::<_, SMALL_BUCKET_LIMIT>::new(); // Vec::with_capacity(keys.len());
    'outer: for seed in SC::FIRST_SEED..conf.seeds_num() {    // seed=0 is special = no seed,
        values_used_by_seed.clear();
        for key in keys.iter().copied() {
            let value = SC::f(key, seed, conf);
            if used_values.contain(value) { continue 'outer; }
            values_used_by_seed.push(value);
        }
        let seed_value = values_used_by_seed.iter().sum();
        if seed_value < *best_value {
            values_used_by_seed.sort_unstable();
            for i in 1..values_used_by_seed.len() {
                if values_used_by_seed[i-1] == values_used_by_seed[i] {
                    //SELF_COLLISION_KEYS.fetch_add(keys.len() as u64, std::sync::atomic::Ordering::Relaxed);
                    //SELF_COLLISION_BUCKETS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    //if SC::BUMPING { return; }
                    continue 'outer;
                }
            }
            *best_value = seed_value;
            *best_seed = seed;
        }
    }
}

/// Choose best seed without shift component.
pub struct SeedOnly;

const SMALL_BUCKET_LIMIT: usize = 8;

impl SeedChooser for SeedOnly {
    #[inline(always)] fn f<SS: SeedSize>(primary_code: u64, seed: u16, conf: &Conf<SS>) -> usize {
        conf.f(primary_code, seed)
    }

    #[inline(always)]
    fn best_seed<SS: SeedSize>(used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) -> u16 {
        let mut best_seed = 0;
        let mut best_value = usize::MAX;
        if keys.len() <= SMALL_BUCKET_LIMIT {
            best_seed_small::<Self, _>(&mut best_value, &mut best_seed, used_values, keys, conf)
        } else {
            best_seed_big::<Self, _>(&mut best_value, &mut best_seed, used_values, keys, conf)
        };
        if best_seed != 0 { // can assign seed to the bucket
            for key in keys {
                used_values.add(conf.f(*key, best_seed));
            }
        };
        best_seed
    }
}

/// Choose best seed without shift component.
pub struct SeedOnlyNoBump;

impl SeedChooser for SeedOnlyNoBump {
    const BUMPING: bool = false;
    const FIRST_SEED: u16 = 0;

    #[inline(always)] fn f<SS: SeedSize>(primary_code: u64, seed: u16, conf: &Conf<SS>) -> usize {
        conf.f_nobump(primary_code, seed)
    }

    #[inline]
    fn best_seed<SS: SeedSize>(used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) -> u16 {
        //let _: [(); Self::FIRST_SEED as usize] = [];
        let mut best_seed = u16::MAX;
        let mut best_value = usize::MAX;
        if keys.len() <= SMALL_BUCKET_LIMIT {
            best_seed_small::<Self, _>(&mut best_value, &mut best_seed, used_values, keys, conf)
        } else {
            best_seed_big::<Self, _>(&mut best_value, &mut best_seed, used_values, keys, conf)
        };
        if best_seed != u16::MAX { // can assign seed to the bucket
            for key in keys {
                used_values.add(conf.f_nobump(*key, best_seed));
            }
        };
        best_seed
    }
}

pub struct ShiftOnly;

//pub static SELF_COLLISION_KEYS: AtomicU64 = AtomicU64::new(0);
//pub static SELF_COLLISION_BUCKETS: AtomicU64 = AtomicU64::new(0);

impl SeedChooser for ShiftOnly {
    fn conf<SS: SeedSize>(output_range: usize, bits_per_seed: SS, bucket_size_100: u16) -> Conf<SS> {
        let max_shift = Self::extra_shift(bits_per_seed);
        let slice_len = match output_range.saturating_sub(max_shift as usize) {
            n @ 0..64 => (n/2+1).next_power_of_two() as u16,
            64..1300 => 64,
            1300..1750 => 128,
            1750..7500 => 256,
            _ => 512,
            //7500..150000 => 512,
            //_ => 1024,
            //150000..250000 => 1024,
            //_ => 2048,
        };
        Conf::<SS>::new(output_range, bits_per_seed, bucket_size_100, slice_len, max_shift)
    }

    #[inline(always)] fn extra_shift<SS: SeedSize>(seed_size: SS) -> u16 {
        (1 << seed_size.into()) - 2
    }

    #[inline(always)] fn f<SS: SeedSize>(primary_code: u64, seed: u16, conf: &Conf<SS>) -> usize {
        conf.f_shift(primary_code, seed)
    }

    #[inline]
    fn best_seed<SS: SeedSize>(used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) -> u16 {
        let mut without_shift: Box<[usize]> = keys.iter()
            .map(|key| conf.slice_begin(*key) + conf.in_slice_noseed(*key))
            .collect();
        without_shift.sort_unstable();  // maybe it is better to postpone self-collision test?
        for i in 1..without_shift.len() {
            if without_shift[i-1] == without_shift[i] { // self-collision?
                //SELF_COLLISION_KEYS.fetch_add(keys.len() as u64, std::sync::atomic::Ordering::Relaxed);
                //SELF_COLLISION_BUCKETS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return 0;
            }
        }
        let last_shift = conf.seeds_num()-1;
        for shift in (0..last_shift).step_by(64) {
            let mut used = 0;
            for first in &without_shift {
                used |= used_values.get64(first + shift as usize);
            }
            if used != u64::MAX {
                let total_shift = shift + used.trailing_ones() as u16;
                if total_shift == last_shift { return 0; }   //total_shift+1 is too large
                for first in &without_shift {
                    used_values.add(*first + total_shift as usize);
                }
                return total_shift as u16 + 1;
            }
        }
        0
    }
}


pub struct ShiftOnlyX2;

impl SeedChooser for ShiftOnlyX2 {
    fn conf<SS: SeedSize>(output_range: usize, bits_per_seed: SS, bucket_size_100: u16) -> Conf<SS> {
        let max_shift = Self::extra_shift(bits_per_seed);
        let slice_len = match output_range.saturating_sub(max_shift as usize) {
            n @ 0..64 => (n/2+1).next_power_of_two() as u16,
            64..1300 => 64,
            1300..1750 => 128,
            1750..7500 => 256,
            //_ => 256,
            //_ => 512,
            7500..150000 => 512,
            _ => 1024,
            //150000..250000 => 1024,
            //_ => 2048,
        };
        Conf::<SS>::new(output_range, bits_per_seed, bucket_size_100, slice_len, max_shift)
    }

    #[inline(always)] fn extra_shift<SS: SeedSize>(seed_size: SS) -> u16 {
        let largest_seed = 1 << seed_size.into();
        2 * (largest_seed - 2)
    }

    #[inline(always)] fn f<SS: SeedSize>(primary_code: u64, seed: u16, conf: &Conf<SS>) -> usize {
        conf.slice_begin(primary_code) + conf.in_slice_noseed(primary_code) + (seed-1) as usize*2
    }

    #[inline]
    fn best_seed<SS: SeedSize>(used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) -> u16 {
        let mut without_shift: Box<[usize]> = keys.iter()
            .map(|key| conf.slice_begin(*key) + conf.in_slice_noseed(*key))
            .collect();
        without_shift.sort_unstable();  // maybe it is better to postpone self-collision test?
        for i in 1..without_shift.len() {
            if without_shift[i-1] == without_shift[i] { // self-collision?
                //SELF_COLLISION_KEYS.fetch_add(keys.len() as u64, std::sync::atomic::Ordering::Relaxed);
                //SELF_COLLISION_BUCKETS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return 0;
            }
        }
        let last_shift = (conf.seeds_num()<<1)-2;
        for shift in (0..last_shift).step_by(64) {
            let mut used = 0xAAAA_AAAA_AAAA_AAAA;
            for first in &without_shift {
                used |= used_values.get64(first + shift as usize);
            }
            if used != u64::MAX {
                let total_shift = shift + used.trailing_ones() as u16;
                if total_shift == last_shift { return 0; }   //TODO check
                for first in &without_shift {
                    used_values.add(*first + total_shift as usize);
                }
                return total_shift as u16 / 2 + 1;
            }
        }
        0
    }
}