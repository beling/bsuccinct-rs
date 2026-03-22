use ph::phast::{BucketToActivateEvaluator, KSeedEvaluator, UsedValueMultiSetU8, ComparableF64};

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


#[derive(Clone)]
pub struct SumOfWeightedValuesF(pub Box<[f64]>);

impl From<ph::phast::SumOfWeightedValues> for SumOfWeightedValuesF {
    fn from(value: ph::phast::SumOfWeightedValues) -> Self {
        Self(value.0.iter().map(|v| *v as f64).collect())
    }
}

impl KSeedEvaluator for SumOfWeightedValuesF {
        
    type Value = ComparableF64;
    
    const MAX: Self::Value = ComparableF64(f64::MAX);

    type BucketData = ();

    #[inline]
    fn for_bucket<C: ph::phast::Core>(&self, _bucket_nr: usize, _core: &C) -> Self::BucketData {
        ()
    }

    fn eval(&self, k: u8, values_used_by_seed: &[usize], used_values: &UsedValueMultiSetU8, _bucket_data: Self::BucketData) -> Self::Value {
        let mut result = 0.0;
        for value in values_used_by_seed.iter().copied() {
            let free_values = (k - used_values[value]) as usize;
            result += (1024*value) as f64;
            if let Some(v) = self.0.get(free_values) { result += v; }
        }
        ComparableF64(result)
    }
}