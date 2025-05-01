use std::{u16, usize};

use crate::seeds::SeedSize;

use super::{builder::UsedValues, conf::Conf};

/// Choose best seed in bucket.
pub trait SeedChooser {
    const NO_BUMPING: bool = false;

    fn extra_shift<SS: SeedSize>(seed_size: SS) -> u16;

    /// Returns function value for given primary code and seed.
    fn f<SS: SeedSize>(primary_code: u64, seed: u16, conf: &Conf<SS>) -> usize;
    
    fn best_seed<SS: SeedSize>(used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) -> u16;
}

fn best_seed_big<SS: SeedSize>(best_value: &mut usize, best_seed: &mut u16, first_seed: u16, used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) {
    let mut values_used_by_seed = Vec::with_capacity(keys.len());
    let simd_keys = keys.len() / 4 * 4;
    //assert!(simd_keys <= keys.len());
    'outer: for seed in first_seed..conf.seeds_num() {    // seed=0 is special = no seed,
        values_used_by_seed.clear();
        for i in (0..simd_keys).step_by(4) {
            let values = [
                conf.f(keys[i], seed),
                conf.f(keys[i+1], seed),
                conf.f(keys[i+2], seed),
                conf.f(keys[i+3], seed),
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
            let value = conf.f(keys[i], seed);
            if used_values.contain(value) { continue 'outer; }
            values_used_by_seed.push(value);
        }
        let seed_value = values_used_by_seed.iter().sum();
        if seed_value < *best_value {
            values_used_by_seed.sort();
            if values_used_by_seed.windows(2).any(|v| v[0]==v[1]) {
                continue;
            }
            *best_value = seed_value;
            *best_seed = seed;
        }
    }
}

#[inline]
fn best_seed_small<SS: SeedSize>(best_value: &mut usize, best_seed: &mut u16, first_seed: u16, used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) {
    assert!(keys.len() <= SMALL_BUCKET_LIMIT);  // seems to speeds up a bit
    let mut values_used_by_seed = arrayvec::ArrayVec::<_, SMALL_BUCKET_LIMIT>::new(); // Vec::with_capacity(keys.len());
    'outer: for seed in first_seed..conf.seeds_num() {    // seed=0 is special = no seed,
        values_used_by_seed.clear();
        for key in keys.iter().copied() {
            let value = conf.f(key, seed);
            if used_values.contain(value) { continue 'outer; }
            values_used_by_seed.push(value);
        }
        let seed_value = values_used_by_seed.iter().sum();
        if seed_value < *best_value {
            values_used_by_seed.sort_unstable();
            for i in 1..values_used_by_seed.len() {
                if values_used_by_seed[i-1] == values_used_by_seed[i] { continue 'outer; }
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
    #[inline(always)] fn extra_shift<SS: SeedSize>(_seed_size: SS) -> u16 { 0 }

    #[inline(always)] fn f<SS: SeedSize>(primary_code: u64, seed: u16, conf: &Conf<SS>) -> usize {
        conf.f(primary_code, seed)
    }

    #[inline]
    fn best_seed<SS: SeedSize>(used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) -> u16 {
        let mut best_seed = 0;
        let mut best_value = usize::MAX;
        if keys.len() <= SMALL_BUCKET_LIMIT {
            best_seed_small(&mut best_value, &mut best_seed, 1, used_values, keys, conf)
        } else {
            best_seed_big(&mut best_value, &mut best_seed, 1, used_values, keys, conf)
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
    const NO_BUMPING: bool = true;

    #[inline(always)] fn extra_shift<SS: SeedSize>(_seed_size: SS) -> u16 { 0 }

    #[inline(always)] fn f<SS: SeedSize>(primary_code: u64, seed: u16, conf: &Conf<SS>) -> usize {
        conf.f(primary_code, seed)
    }

    #[inline]
    fn best_seed<SS: SeedSize>(used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) -> u16 {
        let mut best_seed = u16::MAX;
        let mut best_value = usize::MAX;
        if keys.len() <= SMALL_BUCKET_LIMIT {
            best_seed_small(&mut best_value, &mut best_seed, 0, used_values, keys, conf)
        } else {
            best_seed_big(&mut best_value, &mut best_seed, 0, used_values, keys, conf)
        };
        if best_seed != u16::MAX { // can assign seed to the bucket
            for key in keys {
                used_values.add(conf.f(*key, best_seed));
            }
        };
        best_seed
    }
}

pub struct ShiftOnly;

impl SeedChooser for ShiftOnly {
    #[inline(always)] fn extra_shift<SS: SeedSize>(seed_size: SS) -> u16 {
        (1 << seed_size.into()) - 1
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
                return 0;
            }
        }
        let seeds_num = conf.seeds_num();
        for shift in (0..seeds_num).step_by(64) {
            let mut used = 0;
            for first in &without_shift {
                used |= used_values.get64(first + shift as usize);
            }
            if used != u64::MAX {
                let total_shift = shift + used.trailing_ones() as u16;
                if total_shift == seeds_num { return 0; }
                for first in &without_shift {
                    used_values.add(*first + total_shift as usize);
                }
                return total_shift as u16 + 1;
            }
        }
        0
    }
}