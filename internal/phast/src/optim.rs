use ph::phast::{BucketToActivateEvaluator, KSeedEvaluator, UsedValueMultiSetU8};

#[derive(Default, Clone, Copy)]
#[repr(transparent)]
pub struct F(pub f64);

impl PartialEq for F {
    #[inline(always)] fn eq(&self, other: &Self) -> bool { self.cmp(other).is_eq() }
}

impl PartialOrd for F {
    #[inline(always)] fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}

impl Eq for F {}

impl Ord for F {
    #[inline(always)] fn cmp(&self, other: &Self) -> std::cmp::Ordering { self.0.total_cmp(&other.0) }
}

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
    type Value = F;

    const MIN: Self::Value = F(f64::MIN);

    fn eval(&self, bucket_nr: usize, bucket_size: usize) -> Self::Value {
        let sw = self.size_weights.get(bucket_size-1).copied()
            .unwrap_or_else(|| {
                let len = self.size_weights.len();
                let l = self.size_weights[len-1];
                let p = self.size_weights[len-2];
                l + (l-p) * (bucket_size - len) as f64
            });
        F(sw - 1024.0 * bucket_nr as f64)
    }
}


#[derive(Clone)]
pub struct SumOfWeightedValuesF(pub Box<[f64]>);

impl From<ph::phast::SumOfWeightedValues> for SumOfWeightedValuesF {
    fn from(value: ph::phast::SumOfWeightedValues) -> Self {
        Self(value.0.iter().map(|v| *v as f64).collect())
    }
}

impl KSeedEvaluator for SumOfWeightedValuesF {
        
    type Value = F;
    
    const MAX: Self::Value = F(f64::MAX);

    fn eval(&self, k: u8, values_used_by_seed: &[usize], used_values: &UsedValueMultiSetU8) -> Self::Value {
        let mut result = 0.0;
        for value in values_used_by_seed.iter().copied() {
            let free_values = (k - used_values[value]) as usize;
            result += (1024*value) as f64;
            if let Some(v) = self.0.get(free_values) { result += v; }
        }
        F(result)
    }
}