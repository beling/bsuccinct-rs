use bitm::ceiling_div;

use crate::phast::{conf::Conf, cyclic::{GenericUsedValue, UsedValueMultiSetU8}};
use super::SeedChooser;

pub fn bucket_size_normalization_multiplier(k: u8) -> f64 {
    if k == 1 { return 1.0; }
    const LOG2PI: f64 = 2.651496129472319;
    let k = k as f64;
    //2.7941142836856487*k as f64/(LOG2PI+k.log2())
    2.0*k as f64/(LOG2PI+k.log2())
}

pub trait KSeedEvaluator: Clone + Sync {
    /// Type of evaluation value.
    type Value: PartialEq + PartialOrd + Ord;

    /// Value grater than each value returned by `eval`.
    const MAX: Self::Value;

    fn eval(&self, k: u8, values_used_by_seed: &[usize], used_values: &UsedValueMultiSetU8) -> Self::Value;
}

#[derive(Clone)]
pub struct SumOfValues;

impl KSeedEvaluator for SumOfValues {
    type Value = usize;
    
    const MAX: Self::Value = usize::MAX;

    #[inline]
    fn eval(&self, _k: u8, values_used_by_seed: &[usize], _used_values: &UsedValueMultiSetU8) -> Self::Value {
        values_used_by_seed.iter().sum()
    }
}

#[derive(Clone)]
pub struct SumOfWeightedValues(pub [usize; 8]);

impl SumOfWeightedValues {
    pub fn new(k: u8) -> Self {
        Self(match k {
            2 => [210779, 0, 0, 0, 0, 0, 0, 0],
            3 => [381745, 253826, 0, 0, 0, 0, 0, 0],
            4 => [474823, 373126, 199065, 0, 0, 0, 0, 0],
            5 => [563361, 482534, 350180, 202469, 0, 0, 0, 0],
            6 => [579020, 499022, 391923, 267822, 123465, 0, 0, 0],
            7 => [588003, 517257, 426899, 296792, 200651, 79243, 0, 0],
            8 => [581006, 512411, 433954, 349743, 252462, 141124, 23163, 0],
            _ => [587671, 505121, 438750, 358642, 266301, 173888, 25186, 8572],
        })
    }
}

impl KSeedEvaluator for SumOfWeightedValues {
        
    type Value = usize;
    
    const MAX: Self::Value = usize::MAX;

    fn eval(&self, k: u8, values_used_by_seed: &[usize], used_values: &UsedValueMultiSetU8) -> Self::Value {
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
fn best_seed_k<SC: SeedChooser, SE: KSeedEvaluator>(k: u8, seed_chooser: &SC, seed_evaluator: &SE, best_value: &mut SE::Value, best_seed: &mut u16, used_values: &mut UsedValueMultiSetU8, keys: &[u64], conf: &Conf, seeds_num: u16) {
    //assert!(keys.len() <= SMALL_BUCKET_LIMIT);  // seems to speeds up a bit
    //let mut values_used_by_seed = arrayvec::ArrayVec::<_, SMALL_BUCKET_LIMIT>::new(); // Vec::with_capacity(keys.len());
    let mut values_used_by_seed = Vec::with_capacity(keys.len());
    for seed in SC::FIRST_SEED..seeds_num {    // seed=0 is special = no seed,
        values_used_by_seed.clear();
        for key in keys.iter().copied() {
            let value = seed_chooser.f(key, seed, conf);
            if used_values[value] == k { break; }
            used_values.add(value);
            values_used_by_seed.push(value);
        }
        if values_used_by_seed.len() == keys.len() {
            let seed_value = seed_evaluator.eval(k, &values_used_by_seed, used_values);
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

    #[inline(always)] fn f(&self, primary_code: u64, seed: u16, conf: &Conf) -> usize {
        conf.f(primary_code, seed)
    }

    #[inline(always)]
    fn best_seed(&self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &Conf, bits_per_seed: u8) -> u16 {
        let mut best_seed = 0;
        let mut best_value = SE::MAX;
        best_seed_k(self.k, self, &self.seed_evaluator, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed);
        if best_seed != 0 { // can assign seed to the bucket
            for key in keys {               
                used_values.add(conf.f(*key, best_seed));
            }
        };
        best_seed
    }
}