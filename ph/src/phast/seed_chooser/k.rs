use core::f64;

use bitm::ceiling_div;

use crate::phast::{conf::Core, cyclic::{GenericUsedValue, UsedValueMultiSetU8}};
use super::SeedChooser;

/// Returns approximation of lower bound of space (in bits/key)
/// needed to represent minimal `k`-perfect function.
pub fn space_lower_bound(k: u8) -> f64 {
    match k {
        0|1 => 1.4426950408889634,  // TODO? 0 should panic
        2 => 0.9426950408889634,
        3 => 0.7193867070748593,
        _ => {
            const LOG2PI: f64 = 2.651496129472319;
            let k = k as f64;
            //let k2 = 2.0 * k;
            //log2(pi*k2)/k2 + 0.12/(k*k)
            0.5 * (LOG2PI + k.log2()) / k + 0.12/(k*k)
        }
    }
}

/// Returns the multiplier that allows obtaining a bucket size of `k`-perfect function from a bucket size of 1-perfect function.
pub fn bucket_size_normalization_multiplier(k: u8) -> f64 {
    let overhead = 0.08; //+ 0.25 / (k as f64 * k as f64);
    (space_lower_bound(1)+overhead) / (space_lower_bound(k)+overhead)
}

/*pub fn bucket_size_normalization_multiplier(k: u8) -> f64 {
    if k == 1 { return 1.0; }
    const LOG2PI: f64 = 2.651496129472319;
    let k = k as f64;
    //2.7941142836856487*k as f64/(LOG2PI+k.log2())
    2.0*k as f64/(LOG2PI+k.log2())
}*/

/// Evaluate (harness of) seed for k-perfect function.
/// Seed with the lowest value is used.
pub trait KSeedEvaluator: Clone + Sync {
    /// Type of evaluation value.
    type Value: PartialEq + PartialOrd + Ord;

    /// Precalculated data usable to evaluate each seed in the same bucket.
    type BucketData: Copy;

    /// Value grater than each value returned by `eval`.
    const MAX: Self::Value;

    /// Precalculates data usable to evaluate each seed in the same bucket.
    /// The result is passed to `eval` for each seed in the bucket.
    fn for_bucket<C: Core>(&self, bucket_nr: usize, core: &C) -> Self::BucketData;

    /// Evaluate (harness of) seed that used given `values`.
    fn eval(&self, k: u8, values_used_by_seed: &[usize], used_values: &UsedValueMultiSetU8, bucket_data: Self::BucketData) -> Self::Value;
}

#[derive(Clone)]
pub struct SumOfValues;

/// Evaluate seed using sum of values it takes.
impl KSeedEvaluator for SumOfValues {
    type Value = usize;
    
    const MAX: Self::Value = usize::MAX;

    type BucketData = ();

    #[inline]
    fn for_bucket<C: Core>(&self, _bucket_nr: usize, _core: &C) -> Self::BucketData {
        ()
    }

    #[inline]
    fn eval(&self, _k: u8, values_used_by_seed: &[usize], _used_values: &UsedValueMultiSetU8, _bucket_data: Self::BucketData) -> Self::Value {
        values_used_by_seed.iter().sum()
    }
    

}

#[derive(Clone)]
pub struct SumOfWeightedValues(pub [usize; 8]);

impl SumOfWeightedValues {
    pub fn new(k: u8) -> Self {
        Self(match k {
            2 => [240914, 0, 0, 0, 0, 0, 0, 0],
            3 => [378761, 208579, 0, 0, 0, 0, 0, 0],
            4 => [489347, 355545, 196741, 0, 0, 0, 0, 0],
            5 => [543147, 438544, 306812, 154314, 0, 0, 0, 0],
            6 => [588805, 498050, 388973, 259831, 130127, 0, 0, 0],
            7 => [592640, 518403, 429431, 317914, 200281, 81677, 0, 0],
            8 => [618809, 549890, 470877, 374067, 272131, 169017, 32274, 0],
            _ => [591953, 528254, 453509, 378310, 301994, 186614, 30642, 5928],
        })
    }
}

impl KSeedEvaluator for SumOfWeightedValues {
        
    type Value = usize;
    
    const MAX: Self::Value = usize::MAX;

    type BucketData = ();

    #[inline]
    fn for_bucket<C: Core>(&self, _bucket_nr: usize, _core: &C) -> Self::BucketData {
        ()
    }

    fn eval(&self, k: u8, values_used_by_seed: &[usize], used_values: &UsedValueMultiSetU8, _bucket_data: Self::BucketData) -> Self::Value {
        let mut result = 0;
        for value in values_used_by_seed.iter().copied() {
            let free_values = (k - used_values[value]) as usize;
            result += (1024*value) as usize;
            if let Some(v) = self.0.get(free_values) { result += v; };
        }
        result
    }
}


/// [`SeedChooser`] to build `k`-perfect functions.
/// `k` is given as a parameter of this chooser.
/// 
/// Should be used with [`Perfect`].
/// 
/// It chooses best seed with quite strong hasher, without shift component,
/// which should lead to quite small size, but long construction time.
#[derive(Clone, Copy)]
pub struct SeedOnlyK<SE> {
    pub seed_evaluator: SE,
    pub k: u8,
}

impl<SE: KSeedEvaluator> SeedOnlyK<SE> {
    pub fn new(k: u8, seed_evaluator: SE) -> Self {
        Self { seed_evaluator, k }
    }
}

#[inline(always)]
fn best_seed_k<SC: SeedChooser, SE: KSeedEvaluator, C: Core>(k: u8, seed_chooser: &SC, seed_evaluator: &SE, best_value: &mut SE::Value, best_seed: &mut u16, used_values: &mut UsedValueMultiSetU8, keys: &[u64], core: &C, seeds_num: u16, bucket_nr: usize) {
    //assert!(keys.len() <= SMALL_BUCKET_LIMIT);  // seems to speeds up a bit
    //let mut values_used_by_seed = arrayvec::ArrayVec::<_, SMALL_BUCKET_LIMIT>::new(); // Vec::with_capacity(keys.len());
    let mut values_used_by_seed = Vec::with_capacity(keys.len());
    let bucket_data = seed_evaluator.for_bucket(bucket_nr, core);
    for seed in SC::FIRST_SEED..seeds_num {    // seed=0 is special = no seed,
        values_used_by_seed.clear();
        for key in keys.iter().copied() {
            let value = seed_chooser.f(key, seed, core);
            if used_values[value] == k { break; }
            used_values.add(value);
            values_used_by_seed.push(value);
        }
        if values_used_by_seed.len() == keys.len() {
            let seed_value = seed_evaluator.eval(k, &values_used_by_seed, used_values, bucket_data);
            if seed_value < *best_value {
                *best_value = seed_value;
                *best_seed = seed;
            }
        }
        for v in &values_used_by_seed {
            used_values[*v] -= 1;
        }
    }
}



impl<SE: KSeedEvaluator> SeedChooser for SeedOnlyK<SE> {
    type UsedValues = UsedValueMultiSetU8;

    /// Returns maximum number of keys mapped to each output value; `k` of `k`-perfect function.
    #[inline(always)] fn k(&self) -> u8 { self.k }

    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys.
    #[inline(always)] fn minimal_output_range(&self, num_of_keys: usize) -> usize { ceiling_div(num_of_keys, self.k() as usize) }

    #[inline(always)] fn f<C: Core>(&self, primary_code: u64, seed: u16, core: &C) -> usize {
        core.f(primary_code, seed)
    }

    #[inline(always)]
    fn best_seed<C: Core>(&self, used_values: &mut Self::UsedValues, keys: &[u64], core: &C, bits_per_seed: u8, bucket_nr: usize) -> u16 {
        let mut best_seed = 0;
        let mut best_value = SE::MAX;
        best_seed_k(self.k, self, &self.seed_evaluator, &mut best_value, &mut best_seed, used_values, keys, core, 1<<bits_per_seed, bucket_nr);
        if best_seed != 0 { // can assign seed to the bucket
            for key in keys {               
                used_values.add(core.f(*key, best_seed));
            }
        };
        best_seed
    }
}

/// Wrapper over `f64` with compare operators.
#[derive(Default, Clone, Copy)]
#[repr(transparent)]
pub struct ComparableF64(pub f64);

impl PartialEq for ComparableF64 {
    #[inline(always)] fn eq(&self, other: &Self) -> bool { self.cmp(other).is_eq() }
}

impl PartialOrd for ComparableF64 {
    #[inline(always)] fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}

impl Eq for ComparableF64 {}

impl Ord for ComparableF64 {
    #[inline(always)] fn cmp(&self, other: &Self) -> std::cmp::Ordering { self.0.total_cmp(&other.0) }
}

/// Chooses seed that minimizes
/// sum_{x in bucket} log(f(x,seed) - minimum value in the bucket + value_shift) - free_values_weight * log(free(f(x,seed))+free_shift)
#[derive(Clone, Copy)]
pub struct SumOfLogValues {
    pub free_values_weight: f64,
    pub value_shift: usize,
    pub free_shift: usize
}

impl Default for SumOfLogValues {
    fn default() -> Self {
        Self { free_values_weight: 74.0, value_shift: 29, free_shift: 147 } // for k=2
        //Self { free_values_weight: 62, value_shift: 31, free_shift: 157 } // for k=3
        //Self { free_values_weight: 57, value_shift: 31, free_shift: 169 } // for k=4
        //Self { free_values_weight: 50, value_shift: 32, free_shift: 173 } // for k=5
        //Self { free_values_weight: 47, value_shift: 32, free_shift: 179 } // for k=6
        //Self { free_values_weight: 42, value_shift: 33, free_shift: 185 } // for k=7
        //Self { free_values_weight: 39, value_shift: 35, free_shift: 188 } // for k=8
        //Self { free_values_weight: 37, value_shift: 33, free_shift: 191 } // for k=9
        //Self { free_values_weight: 36, value_shift: 32, free_shift: 201 } // for k=10
        //Self { free_values_weight: 25, value_shift: 35, free_shift: 202 } // for k=16
        //Self { free_values_weight: 16, value_shift: 33, free_shift: 217 } // for k=32
        //Self { free_values_weight: 8, value_shift: 36, free_shift: 224 } // for k=64
    }
}

impl KSeedEvaluator for SumOfLogValues {
    type Value = ComparableF64;

    type BucketData = usize;

    const MAX: Self::Value = ComparableF64(f64::MAX);

    fn for_bucket<C: Core>(&self, bucket_nr: usize, core: &C) -> Self::BucketData {
        core.slice_begin_for_bucket(bucket_nr).wrapping_sub(self.value_shift)
    }

    fn eval(&self, k: u8, values_used_by_seed: &[usize], used_values: &UsedValueMultiSetU8, to_subtract_from_value: Self::BucketData) -> Self::Value {
        let mut result = 0.0;
        for value in values_used_by_seed.iter().copied() {
            let free_values = (self.free_shift + k as usize - used_values[value] as usize) as f64;
            result += (value.wrapping_sub(to_subtract_from_value) as f64).log2() - self.free_values_weight * free_values.log2();
        }
        ComparableF64(result)
    }
}