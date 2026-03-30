use ph::phast::{BucketToActivateEvaluator, ComparableF64, Core, KSeedEvaluator, KSeedEvaluatorConf, SeedEvaluator, SumOfLogValues, UsedValueMultiSetU8};

/// Weights version that uses f64 and works well with numerical optimization.
pub struct WeightsF {
    pub size_weights: Box<[f64]>,
}

impl From<ph::phast::Weights> for WeightsF {
    fn from(value: ph::phast::Weights) -> Self {
        Self { size_weights: value.0.iter().map(|v| *v as f64).collect() }
    }
}

impl BucketToActivateEvaluator for &WeightsF {
    type Value = ComparableF64;

    const MIN: Self::Value = ComparableF64(f64::MIN);

    fn eval(&self, bucket_nr: usize, bucket_size: usize) -> Self::Value {
        let sw = self.size_weights.get(bucket_size-1).copied()
            .unwrap_or_else(|| {
                let len = self.size_weights.len();
                let l = self.size_weights[len-1];
                let p = self.size_weights[len-2];
                l + (l-p) * (bucket_size - len) as f64
            });
        ComparableF64(sw - 1024.0 * bucket_nr as f64)
    }
}


pub struct SumOfLogValuesF;

impl KSeedEvaluatorConf for SumOfLogValuesF {
    type KSeedEvaluator = SumOfLogValuesFEval;

    fn for_k(&self, k: u8) -> Self::KSeedEvaluator {
        let s = SumOfLogValues.for_k(k);
        SumOfLogValuesFEval {
            free_values_weight: s.free_values_weight, value_shift: s.value_shift as f64, free_shift: s.free_shift as f64
        }
    }
}

/// Chooses seed that minimizes
/// sum_{x in bucket} log(f(x,seed) - minimum value in the bucket + value_shift) - free_values_weight * log(freeSlots(f(x,seed)))
#[derive(Clone, Copy)]
pub struct SumOfLogValuesFEval {
    pub free_values_weight: f64,
    pub value_shift: f64,
    pub free_shift: f64
}

impl KSeedEvaluatorConf for SumOfLogValuesFEval {
    type KSeedEvaluator = Self;
    fn for_k(&self, _k: u8) -> Self { *self }
}

impl KSeedEvaluator for SumOfLogValuesFEval {
    type Value = ComparableF64;

    type BucketData = f64;

    const MAX: Self::Value = ComparableF64(f64::MAX);

    fn for_bucket<C: Core>(&self, bucket_nr: usize, _first_bucket_in_window: usize, core: &C) -> Self::BucketData {
        core.slice_begin_for_bucket(bucket_nr) as f64 - self.value_shift
    }

    fn eval(&self, k: u8, values_used_by_seed: &[usize], used_values: &UsedValueMultiSetU8, to_subtract_from_value: Self::BucketData) -> Self::Value {
        let mut result = 0.0;
        for value in values_used_by_seed.iter().copied() {
            let free_values = self.free_shift + k as f64 - used_values[value] as f64;
            result += (value as f64 - to_subtract_from_value).log2() - self.free_values_weight * free_values.log2();
        }
        ComparableF64(result)
    }
}


#[derive(Clone, Copy)]
pub struct GenericProdOfValues {
    pub first_weight: f64,
    pub shift: f64,
}   // first_weight: 0.10001434445381163, shift: 30.01843410730362 1.16%

impl SeedEvaluator for GenericProdOfValues {

    type Value = ComparableF64;

    const MAX: Self::Value = ComparableF64(f64::MAX);
        
    type BucketData = f64;
    
    fn for_bucket<C: Core>(&self, bucket_nr: usize, first_bucket_in_window: usize, core: &C) -> Self::BucketData {
       core.slice_begin_for_bucket(bucket_nr) as f64 * (1.0-self.first_weight) +
       core.slice_begin_for_bucket(first_bucket_in_window) as f64 * self.first_weight
        - self.shift
    }

    fn eval(&self, values_used_by_seed: &[usize], to_extract: Self::BucketData) -> Self::Value {
        ComparableF64(values_used_by_seed.iter().map(|v| {    // simple sume gives 1.921
            (*v as f64) - to_extract
        }).product())
    }
}