use ph::phast::{BucketToActivateEvaluator, ComparableF64, KSeedEvaluator, SumOfLogValues, UsedValueMultiSetU8};

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



/// Chooses seed that minimizes
/// sum_{x in bucket} log(f(x,seed) - minimum value in the bucket + value_shift) - free_values_weight * log(freeSlots(f(x,seed)))
#[derive(Clone, Copy)]
pub struct SumOfLogValuesF {
    pub free_values_weight: f64,
    pub value_shift: f64,
    pub free_shift: f64
}

impl Default for SumOfLogValuesF {
    fn default() -> Self {
        let s = SumOfLogValues::default();
        Self { free_values_weight: s.free_values_weight, value_shift: s.value_shift as f64, free_shift: s.free_shift as f64 }
    }
}

impl KSeedEvaluator for SumOfLogValuesF {
    type Value = ComparableF64;

    type BucketData = f64;

    const MAX: Self::Value = ComparableF64(f64::MAX);

    fn for_bucket<C: ph::phast::Core>(&self, bucket_nr: usize, core: &C) -> Self::BucketData {
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