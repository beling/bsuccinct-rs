use std::io;

use crate::{fmph::SeedSize, phast::{ComparableF64, Core, SeedChooser, SeedChooserCore, SeedEvaluator, cyclic::{GenericUsedValue, UsedValueSet}}};


#[derive(Clone, Copy)]
pub struct ProdOfValues;

impl SeedEvaluator for ProdOfValues {   // bumps 1.17% for S=8, lambda=4.5

    type Value = ComparableF64;

    const MAX: Self::Value = ComparableF64(f64::MAX);
        
    type BucketData = usize;
    
    fn for_bucket<C: Core>(&self, _bucket_nr: usize, _first_bucket_in_window: usize, core: &C) -> Self::BucketData {
       core.slice_begin_for_bucket(_bucket_nr).wrapping_sub(95)
    }

    fn eval(&self, values_used_by_seed: &[usize], to_extract: Self::BucketData) -> Self::Value {
        /*values_used_by_seed.iter().map(|v| {    // simple sume gives 1.921
            //2048.0 * ((v - min) as f64).log2()    // 1.905
            4096.0 * (v.wrapping_sub(self.min_bucket_value_minus_100) as f64).log2()    // 1.905 (0,2) 1.903 (10) 1.901 (20) 1.900 (30) 1.899 (40) 1.898 (50,60,80,100,120,150), 1.899 (200), 1.900 (250), 1.901 (300)
            //2048.0 * ((v - min + 5) as f64).sqrt()  // 1.902 (0,5,10), 1.903 (30,50), 1.905 (100)
        }).sum::<f64>() as usize*/
        ComparableF64(values_used_by_seed.iter().map(|v| {    // simple sume gives 1.921
            //2048.0 * ((v - min) as f64).log2()    // 1.905
            v.wrapping_sub(to_extract) as f64    // 1.905 (0,2) 1.903 (10) 1.901 (20) 1.900 (30) 1.899 (40) 1.898 (50,60,80,100,120,150), 1.899 (200), 1.900 (250), 1.901 (300)
            //2048.0 * ((v - min + 5) as f64).sqrt()  // 1.902 (0,5,10), 1.903 (30,50), 1.905 (100)
        }).product())
    }
    
}

#[derive(Clone)]
pub struct SumOfValues;

impl SeedEvaluator for SumOfValues {
    type Value = usize;

    const MAX: Self::Value = usize::MAX;

    type BucketData = ();

    #[inline] fn for_bucket<C: Core>(&self, _bucket_nr: usize, _first_bucket_in_window: usize, _core: &C) -> Self::BucketData { () }

    fn eval(&self, values_used_by_seed: &[usize], _bucket_data: Self::BucketData) -> Self::Value {
        values_used_by_seed.iter().sum()
    }
}

#[inline(always)]
fn best_seed_big<SC: SeedChooser, SE: SeedEvaluator, C: Core>(seed_chooser: &SC, seed_evaluator: SE, best_value: &mut SE::Value, best_seed: &mut u16, used_values: &mut UsedValueSet, keys: &[u64], conf: &C, seeds_num: u16, bucket_nr: usize, first_bucket_in_window: usize) {
    let mut values_used_by_seed = Vec::with_capacity(keys.len());
    let simd_keys = keys.len() / 4 * 4;
    //assert!(simd_keys <= keys.len());
    let seed_eval_data = seed_evaluator.for_bucket(bucket_nr, first_bucket_in_window, conf);
    'outer: for seed in SC::Core::FIRST_SEED..seeds_num {    // seed=0 is special = no seed,
        values_used_by_seed.clear();
        for i in (0..simd_keys).step_by(4) {
            let values = [
                seed_chooser.f(keys[i], seed, conf),
                seed_chooser.f(keys[i+1], seed, conf),
                seed_chooser.f(keys[i+2], seed, conf),
                seed_chooser.f(keys[i+3], seed, conf),
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
            let value = seed_chooser.f(keys[i], seed, conf);
            if used_values.contain(value) { continue 'outer; }
            values_used_by_seed.push(value);
        }
        let seed_value = seed_evaluator.eval(&values_used_by_seed, seed_eval_data);
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
fn best_seed_small<SC: SeedChooser, SE: SeedEvaluator, C: Core>(seed_chooser: &SC, seed_evaluator: SE, best_value: &mut SE::Value, best_seed: &mut u16, used_values: &mut UsedValueSet, keys: &[u64], conf: &C, seeds_num: u16, bucket_nr: usize, first_bucket_in_window: usize) {
    assert!(keys.len() <= SMALL_BUCKET_LIMIT);  // seems to speeds up a bit
    let mut values_used_by_seed = arrayvec::ArrayVec::<_, SMALL_BUCKET_LIMIT>::new(); // Vec::with_capacity(keys.len());
    let seed_eval_data = seed_evaluator.for_bucket(bucket_nr, first_bucket_in_window, conf);
    'outer: for seed in SC::Core::FIRST_SEED..seeds_num {    // seed=0 is special = no seed,
        values_used_by_seed.clear();
        for key in keys.iter().copied() {
            let value = seed_chooser.f(key, seed, conf);
            if used_values.contain(value) { continue 'outer; }
            values_used_by_seed.push(value);
        }
        let seed_value = seed_evaluator.eval(&values_used_by_seed, seed_eval_data);
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

const SMALL_BUCKET_LIMIT: usize = 8;

#[derive(Clone, Copy)]
pub struct SeedCore;

impl SeedChooserCore for SeedCore {
    #[inline(always)] fn f<C: Core>(&self, primary_code: u64, seed: u16, conf: &C) -> usize {
        conf.f(primary_code, seed)
    }

    #[inline(always)]
    fn try_f<SS, C>(&self, seed_size: SS, seeds: &[SS::VecElement], primary_code: u64, conf: &C) -> Option<usize> where SS: SeedSize, C: Core {
        conf.try_f(seed_size, seeds, primary_code)
    }

    /// Read `Self` from the `input`.
    #[inline(always)] fn read(_input: &mut dyn io::Read) -> io::Result<Self> { Ok(Self) }
}

/// [`SeedChooser`] to build (1-)perfect functions.
/// 
/// Can be used with any function type: [`Function`], [`Function2`], [`Perfect`].
/// 
/// It chooses best seed with quite strong hasher, without shift component,
/// which should lead to small size, but long construction time.
#[derive(Clone, Copy)]
pub struct SeedOnly<SE: SeedEvaluator = ProdOfValues>(pub SE);

impl<SE: SeedEvaluator> SeedChooser for SeedOnly<SE> {
    type UsedValues = UsedValueSet;
    
    type Core = SeedCore;

    #[inline(always)] fn core(&self) -> Self::Core { SeedCore }

    /*#[inline(always)] fn f_slice(primary_code: u64, slice_begin: usize, seed: u16, conf: &Conf) -> usize {
        slice_begin + conf.in_slice(primary_code, seed)
    }*/

    #[inline(always)]
    fn best_seed<C: Core>(&self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &C, bits_per_seed: u8, bucket_nr: usize, first_bucket_in_window: usize) -> u16 {
        let mut best_seed = 0;
        let mut best_value = SE::MAX;//usize::MAX;
        if keys.len() <= SMALL_BUCKET_LIMIT {
            best_seed_small(self, self.0.clone(), &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed, bucket_nr, first_bucket_in_window)
        } else {
            best_seed_big(self, self.0.clone(), &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed, bucket_nr, first_bucket_in_window)
        };
        if best_seed != 0 { // can assign seed to the bucket
            for key in keys {
                used_values.add(conf.f(*key, best_seed));
            }
        };
        best_seed
    }
    
    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> crate::phast::Weights {
        self.0.bucket_evaluator(bits_per_seed, slice_len)
    }
}

#[derive(Clone, Copy)]
pub struct SeedNoBumpCore;

impl SeedChooserCore for SeedNoBumpCore {
    const BUMPING: bool = false;
    const FIRST_SEED: u16 = 0;

    #[inline(always)] fn f<C: Core>(&self, primary_code: u64, seed: u16, conf: &C) -> usize {
        conf.f_nobump(primary_code, seed)
    }

    #[inline(always)] fn read(_input: &mut dyn io::Read) -> io::Result<Self> { Ok(Self) }
}

/// Choose best seed without shift component.
#[derive(Clone, Copy)]
pub struct SeedOnlyNoBump<SE: SeedEvaluator = ProdOfValues>(pub SE);

impl<SE: SeedEvaluator> SeedChooser for SeedOnlyNoBump<SE> {
    type UsedValues = UsedValueSet;

    type Core = SeedNoBumpCore;
    
    #[inline(always)] fn core(&self) -> Self::Core { SeedNoBumpCore }

    #[inline(always)] fn f<C: Core>(&self, primary_code: u64, seed: u16, conf: &C) -> usize {
        conf.f_nobump(primary_code, seed)
    }

    #[inline]
    fn best_seed<C: Core>(&self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &C, bits_per_seed: u8, bucket_nr: usize, first_bucket_in_window: usize) -> u16 {
        //let _: [(); Self::FIRST_SEED as usize] = [];
        let mut best_seed = u16::MAX;
        let mut best_value = SE::MAX;//ComparableF64(f64::MAX);//usize::MAX;
        if keys.len() <= SMALL_BUCKET_LIMIT {
            best_seed_small(self, self.0.clone(), &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed, bucket_nr, first_bucket_in_window)
        } else {
            best_seed_big(self, self.0.clone(), &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed, bucket_nr, first_bucket_in_window)
        };
        if best_seed != u16::MAX { // can assign seed to the bucket
            for key in keys {
                used_values.add(conf.f_nobump(*key, best_seed));
            }
        };
        best_seed
    }
    
    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> crate::phast::Weights {
        self.0.bucket_evaluator(bits_per_seed, slice_len)
    }
}


