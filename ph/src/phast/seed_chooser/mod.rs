mod k;
pub use k::{SeedOnlyK, KSeedEvaluator, SumOfValues, SumOfLogValues, bucket_size_normalization_multiplier, space_lower_bound, ComparableF64};

mod shift;
pub use shift::{ShiftOnly};

mod shift_wrap;
pub use shift_wrap::{ShiftOnlyWrapped, ShiftSeedWrapped};

use crate::{fmph::SeedSize, phast::{Weights, conf::{Core, Conf}, cyclic::{GenericUsedValue, UsedValueSet}}};

use super::conf::GenericCore;

/// Returns slice length for regular PHast.
pub(crate) fn slice_len(output_without_shift_range: usize, bits_per_seed: u8, preferred_slice_len: u16) -> u16 {
    match output_without_shift_range {
        n @ 0..64 => (n/2+1).next_power_of_two() as u16,
        64..1300 => 64,
        1300..9500 => 128,
        9500..12000 => 256,
        12000..140000 => 512,
        _ if bits_per_seed < 6 => if preferred_slice_len == 0 { 512 } else { preferred_slice_len },
        _ if bits_per_seed < 12 => if preferred_slice_len == 0 { 1024 } else { preferred_slice_len },   // for 11 2048 gives ~0.002 bit/key smaller size at cost of ~5% longer construction
        _ => if preferred_slice_len == 0 { 2048 } else { preferred_slice_len }
    }
}

/// Choose best seed in bucket. It affects the trade-off between size and evaluation and construction time.
pub trait SeedChooser: Clone + Sync {
    /// Specifies whether bumping is allowed.
    const BUMPING: bool = true;

    /// The lowest seed that does not indicate bumping.
    const FIRST_SEED: u16 = if Self::BUMPING { 1 } else { 0 };

    /// Size of last level of Function2. Important when `extra_shift()>0` (i.e. for `ShiftOnly`).
    const FUNCTION2_THRESHOLD: usize = 4096;

    type UsedValues: GenericUsedValue;

    /// Returns maximum number of keys mapped to each output value; `k` of `k`-perfect function.
    #[inline(always)] fn k(&self) -> u8 { 1 }

    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys.
    #[inline(always)] fn minimal_output_range(&self, num_of_keys: usize) -> usize { num_of_keys }

    #[inline] fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {
        Weights::new(bits_per_seed, slice_len)
    }

    /// How much the chooser can add to value over slice length.
    #[inline(always)] fn extra_shift(&self, _bits_per_seed: u8) -> u16 { 0 }

    #[inline(always)] fn slice_len(&self, output_range: usize, bits_per_seed: u8, preferred_slice_len: u16) -> u16 {
        slice_len(output_range.saturating_sub(self.extra_shift(bits_per_seed) as usize), bits_per_seed, preferred_slice_len)
    }

/*     fn conf(&self, output_range: usize, input_size: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> Conf {
        let max_shift = self.extra_shift(bits_per_seed);
        let slice_len = slice_len(output_range.saturating_sub(max_shift as usize), bits_per_seed.into(), preferred_slice_len);
        Conf::new(output_range, input_size, bucket_size_100, slice_len, max_shift)
    }

    #[inline(always)] fn conf_for_minimal(&self, num_of_keys: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> Conf {
        self.conf(self.minimal_output_range(num_of_keys), num_of_keys, bits_per_seed, bucket_size_100, preferred_slice_len)
    }

    #[inline(always)] fn conf_for_minimal_p<SS: Copy+Into<u8>>(&self, num_of_keys: usize, params: &Params<SS>) -> Conf {
        self.conf_for_minimal(num_of_keys, params.seed_size.into(), params.bucket_size100, params.preferred_slice_len)
    } */

    fn conf(&self, output_range: usize, num_of_keys: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> GenericCore {
        GenericCore::new(output_range, num_of_keys, bucket_size_100, self.slice_len(output_range, bits_per_seed, preferred_slice_len), self.extra_shift(bits_per_seed))
    }

    #[inline(always)] fn conf_for_minimal(&self, num_of_keys: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> GenericCore {
        self.conf(self.minimal_output_range(num_of_keys), num_of_keys, bits_per_seed, bucket_size_100, preferred_slice_len)
    }

    #[inline(always)] fn conf_p<P: Conf>(&self, output_range: usize, num_of_keys: usize, params: &P) -> P::Core {
        let bits_per_seed = params.bits_per_seed();
        params.conf(output_range, num_of_keys, self.slice_len(output_range, bits_per_seed, params.preferred_slice_len()), self.extra_shift(bits_per_seed))
    }

    #[inline(always)] fn conf_for_minimal_p<P: Conf>(&self, num_of_keys: usize, params: &P) -> P::Core {
        self.conf_p(self.minimal_output_range(num_of_keys), num_of_keys, params)
    }

    /// Returns function value for given primary code and seed.
    fn f<C: Core>(&self, primary_code: u64, seed: u16, conf: &C) -> usize;

    #[inline(always)]
    fn try_f<SS, C>(&self, seed_size: SS, seeds: &[SS::VecElement], primary_code: u64, conf: &C) -> Option<usize> where SS: SeedSize, C: Core {
        let seed = unsafe { seed_size.get_seed(seeds, conf.bucket_for(primary_code)) };
        (seed != 0).then(|| self.f(primary_code, seed, conf))
    }
    
    /// Returns best seed to store in seeds array or `u16::MAX` if `NO_BUMPING` is `true` and there is no feasible seed.
    fn best_seed<C: Core>(&self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &C, bits_per_seed: u8, bucket_nr: usize) -> u16;
}

struct SeedEvaluator {
    min_bucket_value_minus_100: usize
}

impl SeedEvaluator {
    pub fn new<C: Core>(bucket_nr: usize, core: &C) -> Self {
        Self { min_bucket_value_minus_100: core.slice_begin_for_bucket(bucket_nr).wrapping_sub(100) }
    }

    pub fn eval(&self, values_used_by_seed: &[usize]) -> usize {
        values_used_by_seed.iter().map(|v| {    // simple sume gives 1.921
            //2048.0 * ((v - min) as f64).log2()    // 1.905
            4096.0 * (v.wrapping_sub(self.min_bucket_value_minus_100) as f64).log2()    // 1.905 (0,2) 1.903 (10) 1.901 (20) 1.900 (30) 1.899 (40) 1.898 (50,60,80,100,120,150), 1.899 (200), 1.900 (250), 1.901 (300)
            //2048.0 * ((v - min + 5) as f64).sqrt()  // 1.902 (0,5,10), 1.903 (30,50), 1.905 (100)
        }).sum::<f64>() as usize
    }
}

#[inline(always)]
fn best_seed_big<SC: SeedChooser, C: Core>(seed_chooser: &SC, best_value: &mut usize, best_seed: &mut u16, used_values: &mut UsedValueSet, keys: &[u64], conf: &C, seeds_num: u16, bucket_nr: usize) {
    let mut values_used_by_seed = Vec::with_capacity(keys.len());
    let simd_keys = keys.len() / 4 * 4;
    //assert!(simd_keys <= keys.len());
    let seed_eval = SeedEvaluator::new(bucket_nr, conf);
    'outer: for seed in SC::FIRST_SEED..seeds_num {    // seed=0 is special = no seed,
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
        let seed_value = seed_eval.eval(&values_used_by_seed);
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
fn best_seed_small<SC: SeedChooser, C: Core>(seed_chooser: &SC, best_value: &mut usize, best_seed: &mut u16, used_values: &mut UsedValueSet, keys: &[u64], conf: &C, seeds_num: u16, bucket_nr: usize) {
    assert!(keys.len() <= SMALL_BUCKET_LIMIT);  // seems to speeds up a bit
    let mut values_used_by_seed = arrayvec::ArrayVec::<_, SMALL_BUCKET_LIMIT>::new(); // Vec::with_capacity(keys.len());
    let seed_eval = SeedEvaluator::new(bucket_nr, conf);
    'outer: for seed in SC::FIRST_SEED..seeds_num {    // seed=0 is special = no seed,
        values_used_by_seed.clear();
        for key in keys.iter().copied() {
            let value = seed_chooser.f(key, seed, conf);
            if used_values.contain(value) { continue 'outer; }
            values_used_by_seed.push(value);
        }
        let seed_value = seed_eval.eval(&values_used_by_seed);
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

/// [`SeedChooser`] to build (1-)perfect functions.
/// 
/// Can be used with any function type: [`Function`], [`Function2`], [`Perfect`].
/// 
/// It chooses best seed with quite strong hasher, without shift component,
/// which should lead to small size, but long construction time.
#[derive(Clone, Copy)]
pub struct SeedOnly;

impl SeedChooser for SeedOnly {
    type UsedValues = UsedValueSet;
    
    #[inline(always)] fn f<C: Core>(&self, primary_code: u64, seed: u16, conf: &C) -> usize {
        conf.f(primary_code, seed)
    }

    #[inline(always)]
    fn try_f<SS, C>(&self, seed_size: SS, seeds: &[SS::VecElement], primary_code: u64, conf: &C) -> Option<usize> where SS: SeedSize, C: Core {
        conf.try_f(seed_size, seeds, primary_code)
    }

    /*#[inline(always)] fn f_slice(primary_code: u64, slice_begin: usize, seed: u16, conf: &Conf) -> usize {
        slice_begin + conf.in_slice(primary_code, seed)
    }*/

    #[inline(always)]
    fn best_seed<C: Core>(&self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &C, bits_per_seed: u8, bucket_nr: usize) -> u16 {
        let mut best_seed = 0;
        let mut best_value = usize::MAX;
        if keys.len() <= SMALL_BUCKET_LIMIT {
            best_seed_small(self, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed, bucket_nr)
        } else {
            best_seed_big(self, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed, bucket_nr)
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
#[derive(Clone, Copy)]
pub struct SeedOnlyNoBump;

impl SeedChooser for SeedOnlyNoBump {
    const BUMPING: bool = false;
    const FIRST_SEED: u16 = 0;

    type UsedValues = UsedValueSet;

    #[inline(always)] fn f<C: Core>(&self, primary_code: u64, seed: u16, conf: &C) -> usize {
        conf.f_nobump(primary_code, seed)
    }

    #[inline]
    fn best_seed<C: Core>(&self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &C, bits_per_seed: u8, bucket_nr: usize) -> u16 {
        //let _: [(); Self::FIRST_SEED as usize] = [];
        let mut best_seed = u16::MAX;
        let mut best_value = usize::MAX;
        if keys.len() <= SMALL_BUCKET_LIMIT {
            best_seed_small(self, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed, bucket_nr)
        } else {
            best_seed_big(self, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed, bucket_nr)
        };
        if best_seed != u16::MAX { // can assign seed to the bucket
            for key in keys {
                used_values.add(conf.f_nobump(*key, best_seed));
            }
        };
        best_seed
    }
}


