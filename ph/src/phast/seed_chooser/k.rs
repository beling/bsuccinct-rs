use bitm::ceiling_div;

use crate::phast::{conf::Conf, cyclic::{GenericUsedValue, UsedValueMultiSetU8}};
use super::SeedChooser;

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
pub struct SumOfWeightedValues(pub [isize; 8]);

impl SumOfWeightedValues {
    pub fn new(k: u8) -> Self {
        Self(match k {  // TODO fix last value to be 0 and decrease degree of freedom when optimizing
            2 => [170171 +39231, -39231 +39231  , -39231, -39231, -39231, -39231, -39231, -39231],
            3 => [272469, 149612, -125313  , -125313, -125313, -125313, -125313, -125313],
            4 => [302489, 235381, 66592, -215829  , -215829, -215829, -215829, -215829],
            _ => [302069, 234257, 75490, -85613, -282732  , -282732, -282732, -282732]  // 5
        })
    }
}

impl KSeedEvaluator for SumOfWeightedValues {
        
    type Value = isize;
    
    const MAX: Self::Value = isize::MAX;

    fn eval(&self, k: u8, values_used_by_seed: &[usize], used_values: &UsedValueMultiSetU8) -> Self::Value {
        let mut result = 0;
        for value in values_used_by_seed.iter().copied() {
            let free_values = (k - used_values[value]) as usize;
            result += (1024*value) as isize + self.0.get(free_values).unwrap_or_else(|| unsafe{self.0.last().unwrap_unchecked()});
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