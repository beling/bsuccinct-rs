use std::u16;

use crate::seeds::SeedSize;

use super::{builder::UsedValues, conf::Conf};

/// Choose best seed in bucket.
pub trait SeedChooser {
    fn best_seed<SS: SeedSize>(used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) -> u16;
}

/// Choose best seed without shift component.
pub struct SeedOnly;

const SMALL_BUCKET_LIMIT: usize = 8;

impl SeedOnly {
    fn best_seed_big<SS: SeedSize>(used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) -> u16 {
        let mut best_value = usize::MAX;
        let mut best_seed = 0;
        let mut values_used_by_seed = Vec::with_capacity(keys.len());
        let simd_keys = keys.len() / 4 * 4;
        //assert!(simd_keys <= keys.len());
        'outer: for seed in 1u16..conf.seeds_num() {    // seed=0 is special = no seed,
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
            if seed_value < best_value {
                values_used_by_seed.sort();
                if values_used_by_seed.windows(2).any(|v| v[0]==v[1]) {
                    continue;
                }
                best_value = seed_value;
                best_seed = seed;
            }
        }
        best_seed
    }

    #[inline]
    fn best_seed_small<SS: SeedSize>(used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) -> u16 {
        assert!(keys.len() <= SMALL_BUCKET_LIMIT);  // seems to speeds up a bit
        let mut best_value = usize::MAX;
        let mut best_seed = 0;
        let mut values_used_by_seed = arrayvec::ArrayVec::<_, SMALL_BUCKET_LIMIT>::new(); // Vec::with_capacity(keys.len());
        'outer: for seed in 1u16..conf.seeds_num() {    // seed=0 is special = no seed,
            values_used_by_seed.clear();
            for key in keys.iter().copied() {
                let value = conf.f(key, seed);
                if used_values.contain(value) { continue 'outer; }
                values_used_by_seed.push(value);
            }
            let seed_value = values_used_by_seed.iter().sum();
            if seed_value < best_value {
                values_used_by_seed.sort_unstable();
                for i in 1..values_used_by_seed.len() {
                    if values_used_by_seed[i-1] == values_used_by_seed[i] { continue 'outer; }
                }
                best_value = seed_value;
                best_seed = seed;
            }
        }
        best_seed
    }
}

impl SeedChooser for SeedOnly {
    #[inline]
    fn best_seed<SS: SeedSize>(used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) -> u16 {
        let best_seed = if keys.len() <= SMALL_BUCKET_LIMIT {
            Self::best_seed_small(used_values, keys, conf)
        } else {
            Self::best_seed_big(used_values, keys, conf)
        };
        if best_seed != 0 { // can assign seed to the bucket
            for key in keys {
                used_values.add(conf.f(*key, best_seed));
            }
        };
        best_seed
    }
}


pub struct ShiftOnly;

impl SeedChooser for ShiftOnly {
    fn best_seed<SS: SeedSize>(used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) -> u16 {
        let without_shift: Box<[usize]> = keys.iter()
            .map(|key| conf.slice_begin(*key) + conf.in_slice_noseed(*key))
            .collect();
        for shift in (0..256).step_by(64) {
            let mut used = 0;
            for first in &without_shift {
                used |= used_values.get64(first + shift);
            }
            if used != u64::MAX {
                return used.trailing_ones() as u16;
            }
        }
        u16::MAX    //??
    }
}