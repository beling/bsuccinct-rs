use core::f64;
use std::io;

use binout::{AsIs, Serializer};
use bitm::ceiling_div;

use crate::phast::{ComparableF64, SeedChooserCore, conf::Core, cyclic::{GenericUsedValue, UsedValueMultiSetU16}, space_lower_bound};
use super::SeedChooser;

/// Returns the multiplier that allows obtaining a bucket size of `k`-perfect function from a bucket size of 1-perfect function.
pub fn bucket_size_normalization_multiplier(k: u16) -> f64 {
    let overhead = 0.05; //+ 0.25 / (k as f64 * k as f64);
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
    fn for_bucket<C: Core>(&self, bucket_nr: usize, first_bucket_in_window: usize, core: &C) -> Self::BucketData;

    /// Evaluate (harness of) seed that used given `values`.
    fn eval(&self, k: u16, values_used_by_seed: &[usize], used_values: &UsedValueMultiSetU16, bucket_data: Self::BucketData) -> Self::Value;
}

pub trait KSeedEvaluatorConf {
    /// Type of evaluator.
    type KSeedEvaluator: KSeedEvaluator;

    /// Returns evaluator for given `k`.
    fn for_k(&self, k: u16) -> Self::KSeedEvaluator;
}

#[derive(Clone)]
pub struct SumOfValues;

/// Evaluate seed using sum of values it takes.
impl KSeedEvaluator for SumOfValues {
    type Value = usize;
    
    const MAX: Self::Value = usize::MAX;

    type BucketData = ();

    #[inline]
    fn for_bucket<C: Core>(&self, _first_bucket_in_window: usize, _bucket_nr: usize, _core: &C) -> Self::BucketData {
        ()
    }

    #[inline]
    fn eval(&self, _k: u16, values_used_by_seed: &[usize], _used_values: &UsedValueMultiSetU16, _bucket_data: Self::BucketData) -> Self::Value {
        values_used_by_seed.iter().sum()
    }
}

impl KSeedEvaluatorConf for SumOfValues {
    type KSeedEvaluator = Self;
    #[inline] fn for_k(&self, _k: u16) -> Self::KSeedEvaluator { SumOfValues }
}

#[derive(Clone, Copy)]
pub struct SumOfLogValues;

impl KSeedEvaluatorConf for SumOfLogValues {
    type KSeedEvaluator = SumOfLogValuesEvaluator;

    fn for_k(&self, k: u16) -> Self::KSeedEvaluator {
        match k {
            2=>SumOfLogValuesEvaluator { free_values_weight: 74.0, value_shift: 29, free_shift: 147 }, // for k=2   0.91%
            3=>SumOfLogValuesEvaluator { free_values_weight: 62.0, value_shift: 31, free_shift: 157 }, // for k=3   0.89%
            4=>SumOfLogValuesEvaluator { free_values_weight: 57.0, value_shift: 31, free_shift: 169 }, // for k=4   0.91%
            5=>SumOfLogValuesEvaluator { free_values_weight: 50.0, value_shift: 32, free_shift: 173 }, // for k=5   0.91%
            6=>SumOfLogValuesEvaluator { free_values_weight: 47.0, value_shift: 32, free_shift: 179 }, // for k=6   0.89%
            7=>SumOfLogValuesEvaluator { free_values_weight: 42.0, value_shift: 33, free_shift: 185 }, // for k=7
            8=>SumOfLogValuesEvaluator { free_values_weight: 39.0, value_shift: 35, free_shift: 188 }, // for k=8
            9=>SumOfLogValuesEvaluator { free_values_weight: 37.0, value_shift: 33, free_shift: 191 }, // for k=9
            10=>SumOfLogValuesEvaluator { free_values_weight: 36.0, value_shift: 32, free_shift: 201 }, // for k=10   0.75%
            11..32=>SumOfLogValuesEvaluator { free_values_weight: 25.0, value_shift: 35, free_shift: 202 }, // for k=16   0.69%
            32..64=>SumOfLogValuesEvaluator { free_values_weight: 16.0, value_shift: 33, free_shift: 217 }, // for k=32   0.77%
            64..128=>SumOfLogValuesEvaluator { free_values_weight: 8.0, value_shift: 36, free_shift: 224 }, // for k=64
            _=>SumOfLogValuesEvaluator { free_values_weight: 5.0, value_shift: 40, free_shift: 265 },   // for k=128
        }
    }
}

/// Chooses seed that minimizes
/// sum_{x in bucket} log(f(x,seed) - minimum value in the bucket + value_shift) - free_values_weight * log(free(f(x,seed))+free_shift)
#[derive(Clone, Copy)]
pub struct SumOfLogValuesEvaluator {
    pub free_values_weight: f64,
    pub value_shift: usize,
    pub free_shift: usize
}

impl KSeedEvaluator for SumOfLogValuesEvaluator {
    type Value = ComparableF64;

    type BucketData = usize;

    const MAX: Self::Value = ComparableF64(f64::MAX);

    fn for_bucket<C: Core>(&self, bucket_nr: usize, _first_bucket_in_window: usize, core: &C) -> Self::BucketData {
        core.slice_begin_for_bucket(bucket_nr).wrapping_sub(self.value_shift)
    }

    fn eval(&self, k: u16, values_used_by_seed: &[usize], used_values: &UsedValueMultiSetU16, to_subtract_from_value: Self::BucketData) -> Self::Value {
        let mut result = 0.0;
        for value in values_used_by_seed.iter().copied() {
            let free_values = (self.free_shift + k as usize - used_values[value] as usize) as f64;
            result += (value.wrapping_sub(to_subtract_from_value) as f64).log2() - self.free_values_weight * free_values.log2();
        }
        ComparableF64(result)
    }
}

#[derive(Clone, Copy)]
pub struct SeedKCore(pub u16);

impl SeedChooserCore for SeedKCore {
    
    #[inline(always)] fn k(&self) -> u16 { self.0 }

    #[inline(always)] fn f<C: Core>(&self, primary_code: u64, seed: u16, core: &C) -> usize {
        core.f(primary_code, seed)
    }

    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys.
    #[inline(always)] fn minimal_output_range(&self, num_of_keys: usize) -> usize { ceiling_div(num_of_keys, self.0 as usize) }

    /// Writes `self` to the `output`.
    fn write(&self, output: &mut dyn io::Write) -> io::Result<()> { 
        AsIs::write(output, self.0)
    }

    /// Returns number of bytes which `write` will write.
    fn write_bytes(&self) -> usize { AsIs::size(self.0) }

    /// Read `Self` from the `input`.
    fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Ok(Self(AsIs::read(input)?)) 
    }
}


/// [`SeedChooser`] to build `k`-perfect functions.
/// `k` is given as a parameter of this chooser.
/// 
/// Should be used with [`KFunction`] or [`Perfect`].
/// 
/// It chooses best seed with quite strong hasher, without shift component,
/// which should lead to quite small size, but long construction time.
#[derive(Clone, Copy)]
pub struct SeedOnlyK<SE = SumOfLogValuesEvaluator> {
    pub seed_evaluator: SE,
    pub core: SeedKCore,
}

impl SeedOnlyK<SumOfLogValuesEvaluator> {
    pub fn new(k: u16) -> Self {
        Self::with_evaluator(k, SumOfLogValues)
    }
}

impl<SE: KSeedEvaluator> SeedOnlyK<SE> {
    pub fn with_evaluator<SEC: KSeedEvaluatorConf<KSeedEvaluator=SE>>(k: u16, seed_evaluator_conf: SEC) -> Self {
        Self { seed_evaluator: seed_evaluator_conf.for_k(k), core: SeedKCore(k) }
    }
}

#[inline(always)]
fn best_seed_k<SC: SeedChooser, SE: KSeedEvaluator, C: Core>(k: u16, seed_chooser: &SC, seed_evaluator: &SE, best_value: &mut SE::Value, best_seed: &mut u16, used_values: &mut UsedValueMultiSetU16, keys: &[u64], core: &C, seeds_num: u16, bucket_nr: usize, first_bucket_in_window: usize) {
    //assert!(keys.len() <= SMALL_BUCKET_LIMIT);  // seems to speeds up a bit
    //let mut values_used_by_seed = arrayvec::ArrayVec::<_, SMALL_BUCKET_LIMIT>::new(); // Vec::with_capacity(keys.len());
    let mut values_used_by_seed = Vec::with_capacity(keys.len());
    let bucket_data = seed_evaluator.for_bucket(bucket_nr, first_bucket_in_window, core);
    for seed in SC::Core::FIRST_SEED..seeds_num {    // seed=0 is special = no seed,
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
    type UsedValues = UsedValueMultiSetU16;

    type Core = SeedKCore;
    
    #[inline(always)] fn core(&self) -> Self::Core { self.core }

    #[inline(always)]
    fn best_seed<C: Core>(&self, used_values: &mut Self::UsedValues, keys: &[u64], core: &C, bits_per_seed: u8, bucket_nr: usize, first_bucket_in_window: usize) -> u16 {
        let mut best_seed = 0;
        let mut best_value = SE::MAX;
        best_seed_k(self.k(), self, &self.seed_evaluator, &mut best_value, &mut best_seed, used_values, keys, core, 1<<bits_per_seed, bucket_nr, first_bucket_in_window);
        if best_seed != 0 { // can assign seed to the bucket
            for key in keys {               
                used_values.add(core.f(*key, best_seed));
            }
        };
        best_seed
    }
}

