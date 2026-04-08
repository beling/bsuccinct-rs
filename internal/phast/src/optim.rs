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
            ..=2 => SumOfLogValuesFEval { free_values_weight: 1.75456, value_shift: 0.00370, free_shift: 3.15671, first_weight: 0.10135 },  // for2 1.01%
            3 => SumOfLogValuesFEval { free_values_weight: 1.53878, value_shift: 0.00325, free_shift: 3.09695, first_weight: 0.16796 }, // 1.08%
                // or: free_values_weight: 1.67798, value_shift: 0.00376, free_shift: 3.28776, first_weight: 0.11358
            4 => SumOfLogValuesFEval { free_values_weight: 1.22552, value_shift: 0.00385, free_shift: 2.68325, first_weight: 0.36487 }, // 1.09%
                // or: free_values_weight: 0.86019, value_shift: 0.00126, free_shift: 1.54111, first_weight: 0.32114
            5 => SumOfLogValuesFEval { free_values_weight: 1.16738, value_shift: 0.00341, free_shift: 2.85818, first_weight: 0.57022 }, // 1.05%
                // or (1.04%): free_values_weight: 0.95556, value_shift: -0.00787, free_shift: 2.09489, first_weight: 0.54287
            6 => SumOfLogValuesFEval { free_values_weight: 1.07811, value_shift: 0.00306, free_shift: 3.01460, first_weight: 0.74649 }, // 0.97%
            7 => SumOfLogValuesFEval { free_values_weight: 1.05410, value_shift: 0.00314, free_shift: 3.02227, first_weight: 0.72770 }, // 0.89%
            8 => SumOfLogValuesFEval { free_values_weight: 1.03750, value_shift: 0.00307, free_shift: 3.16688, first_weight: 0.73473 }, // 0.81%
            9 => SumOfLogValuesFEval { free_values_weight: 1.05199, value_shift: 0.00307, free_shift: 3.31693, first_weight: 0.71391 }, // 0.74%
            10 => SumOfLogValuesFEval { free_values_weight: 1.02577, value_shift: 0.00305, free_shift: 3.32123, first_weight: 0.71673 }, // 0.68%
            11 => SumOfLogValuesFEval { free_values_weight: 1.00964, value_shift: 0.00307, free_shift: 3.34136, first_weight: 0.70944 }, // 0.63%
            12 => SumOfLogValuesFEval { free_values_weight: 0.97919, value_shift: 0.00305, free_shift: 3.25671, first_weight: 0.70222 }, // 0.60%
            13 => SumOfLogValuesFEval { free_values_weight: 1.00290, value_shift: 0.00296, free_shift: 3.49979, first_weight: 0.70233 }, // 0.57%
            14 => SumOfLogValuesFEval { free_values_weight: 1.00255, value_shift: 0.00303, free_shift: 3.55407, first_weight: 0.68480 }, // 0.56%
            15 => SumOfLogValuesFEval { free_values_weight: 0.99834, value_shift: 0.00304, free_shift: 3.54822, first_weight: 0.67896 }, // 0.55%
            16..32 => SumOfLogValuesFEval { free_values_weight: 0.99529, value_shift: 0.00308, free_shift: 3.62006, first_weight: 0.68640 }, // 0.54%
            32..48 => SumOfLogValuesFEval { free_values_weight: 0.92816, value_shift: 0.00307, free_shift: 3.91187, first_weight: 0.66890 }, // 0.63%
            48..64 => SumOfLogValuesFEval { free_values_weight: 0.90327, value_shift: 0.00330, free_shift: 4.21686, first_weight: 0.64461 }, // 0.74%
            64..80 => SumOfLogValuesFEval { free_values_weight: 0.86516, value_shift: 0.00324, free_shift: 4.15503, first_weight: 0.66520 }, // 0.84%
            80.. => SumOfLogValuesFEval { free_values_weight: 0.87878, value_shift: 0.00331, free_shift: 4.10116, first_weight: 0.67052 } // 0.55%
        }
    }
}

/// Chooses seed that minimizes
/// sum_{x in bucket} log(f(x,seed) - minimum value in the window + value_shift) - free_values_weight * log(freeSlots(f(x,seed)))
pub struct SumOfLogValuesF1;

impl KSeedEvaluatorConf for SumOfLogValuesF1 {
    type KSeedEvaluator = SumOfLogValuesFEval;

    fn for_k(&self, k: u8) -> Self::KSeedEvaluator {
        match k {
            ..=2 => SumOfLogValuesFEval { free_values_weight: 43.422, value_shift: 55.238, free_shift: 184.280, first_weight: 1.0 },// 1.08%
            3 => SumOfLogValuesFEval { free_values_weight: 42.541, value_shift: 54.427, free_shift: 185.614, first_weight: 1.0 },// 1.02%
            4 => SumOfLogValuesFEval { free_values_weight: 19.648, value_shift: 11.240, free_shift: 84.452, first_weight: 1.0 },    //0.92%
            5 => SumOfLogValuesFEval { free_values_weight: 17.982, value_shift: 11.444, free_shift: 84.680, first_weight: 1.0 },    //0.85%
            16 => SumOfLogValuesFEval { free_values_weight: 1.464, value_shift: 0.0, free_shift: 9.789, first_weight: 1.0 },    //0.38%
                // free_values_weight: 1.794, value_shift: -77.768, free_shift: 9.832 // 0.35%
            _ => {
                SumOfLogValuesFEval { first_weight: 1.0, ..SumOfLogValuesF.for_k(k) }
            }
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