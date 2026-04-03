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


pub struct SumOfLogValuesF0;

impl KSeedEvaluatorConf for SumOfLogValuesF0 {
    type KSeedEvaluator = SumOfLogValuesFEval;

    fn for_k(&self, k: u8) -> Self::KSeedEvaluator {
        let s = SumOfLogValues.for_k(k);
        SumOfLogValuesFEval {
            free_values_weight: s.free_values_weight, value_shift: s.value_shift as f64, free_shift: s.free_shift as f64,
            first_weight: 0.0, 
        }
    }
}

/// Chooses seed that minimizes
/// sum_{x in bucket} log(f(x,seed) - minimum value in the bucket + value_shift) - free_values_weight * log(freeSlots(f(x,seed)))
pub struct SumOfLogValuesF;

impl KSeedEvaluatorConf for SumOfLogValuesF {
    type KSeedEvaluator = SumOfLogValuesFEval;

    fn for_k(&self, k: u8) -> Self::KSeedEvaluator {
        match k {
            //..=2 => free_values_weight: 109, value_shift: 39, free_shift: 233, first_weight: 0.036    // 0.91%
            ..=2 => SumOfLogValuesFEval { free_values_weight: 53.0, value_shift: 14.0, free_shift: 119.0, first_weight: 0.092 },  // 0.91%
            3 => SumOfLogValuesFEval { free_values_weight: 38.0, value_shift: 12.0, free_shift: 102.0, first_weight: 0.145 },  // 0.87%
            4 => SumOfLogValuesFEval { free_values_weight: 22.0, value_shift: 11.0, free_shift: 78.0, first_weight: 0.510 },     // 0.87%
            5 => SumOfLogValuesFEval { free_values_weight: 20.0, value_shift: 11.0, free_shift: 78.0, first_weight: 0.513 }, // 0.82%
            6..10 => SumOfLogValuesFEval { free_values_weight: 19.0, value_shift: 11.0, free_shift: 81.0, first_weight: 0.512 }, //  0.76%
            10..16 => SumOfLogValuesFEval { free_values_weight: 15.0, value_shift: 11.0, free_shift: 91.0, first_weight: 0.517 },  //  0.58%
            16..32 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 59.0, free_shift: 12.0, first_weight: 1.383 }, //  0.43% // DO MORE ITERATIONS
            32.. => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 69.0, free_shift: 9.0, first_weight: 0.897 }, //  0.49%    // DO MORE ITERATIONS
        }
    }
}

/// Chooses seed that minimizes
/// sum_{x in bucket} log(f(x,seed) - minimum value in the window + value_shift) - free_values_weight * log(freeSlots(f(x,seed)))
pub struct SumOfLogValuesF1;

impl KSeedEvaluatorConf for SumOfLogValuesF1 {
    type KSeedEvaluator = SumOfLogValuesFEval;

    fn for_k(&self, k: u8) -> Self::KSeedEvaluator {
        let s = SumOfLogValues.for_k(k);
        // 2 => free_values_weight: 43.422, value_shift: 55.238, free_shift: 184.280    // 1.08%
        SumOfLogValuesFEval {
            free_values_weight: s.free_values_weight, value_shift: s.value_shift as f64 + 20.0, free_shift: s.free_shift as f64,
            first_weight: 1.0, 
        }
    }
}


/// Chooses seed that minimizes
/// sum_{x in bucket} log(f(x,seed) - first_weight*minimum value in the window - (1-first_weight)*minimum value in the bucket + value_shift) - free_values_weight * log(freeSlots(f(x,seed)))
#[derive(Clone, Copy)]
pub struct SumOfLogValuesFEval {
    pub first_weight: f64,
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

    fn for_bucket<C: Core>(&self, bucket_nr: usize, first_bucket_in_window: usize, core: &C) -> Self::BucketData {
       core.slice_begin_for_bucket(bucket_nr) as f64 * (1.0-self.first_weight) +
       core.slice_begin_for_bucket(first_bucket_in_window) as f64 * self.first_weight
        - self.value_shift
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
}   // first_weight: 1.098765e-5, shift: 145 1.16%

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


#[derive(Clone, Copy)]
pub struct WGenericProdOfValues(pub [f64; 4]);

impl SeedEvaluator for WGenericProdOfValues {

    type Value = ComparableF64;

    const MAX: Self::Value = ComparableF64(f64::MAX);
        
    type BucketData = f64;
    
    fn for_bucket<C: Core>(&self, bucket_nr: usize, _first_bucket_in_window: usize, core: &C) -> Self::BucketData {
       core.slice_begin_for_bucket(bucket_nr) as f64
    }

    fn eval(&self, values_used_by_seed: &[usize], bucket_first: Self::BucketData) -> Self::Value {
        let to_extract = bucket_first - self.0.get(if values_used_by_seed.len() >= 3 { values_used_by_seed.len()-2} else {0}).unwrap_or_else(|| self.0.last().unwrap());
        ComparableF64(values_used_by_seed.iter().map(|v| {
            (*v as f64) - to_extract
        }).product())
    }
}