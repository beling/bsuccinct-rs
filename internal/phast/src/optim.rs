use std::{cell::RefCell, usize};

use ph::{phast::{BucketToActivateEvaluator, ComparableF64, Core, KSeedEvaluator, KSeedEvaluatorConf, Partial, SeedChooser, SeedEvaluator, SeedOnly, SeedOnlyK, SumOfLogValues, UsedValueMultiSetU16}, seeds::BitsFast};

use crate::conf::Conf;

pub enum Constrain {
    Weak(f64),
    Strong(f64),
}

pub trait CostFn {
    fn eval(&self, conf: &Conf, x: &[f64]) -> usize;
    fn init(&self, conf: &Conf) -> Vec<f64>;
    fn params(&self, conf: &Conf) -> Vec<(&str, Constrain, Constrain, usize)> {
        self.init(conf).iter().map(|v| (
            "",
            Constrain::Weak(if *v > 0.0 { (v-1.0).max(0.0) } else { v-1.0 }),
            Constrain::Weak(v + 1.0),
            1
        )).collect()
    }
    fn bounds(&self, conf: &Conf) -> (Vec<f64>, Vec<f64>) {
        let s = self.init(conf);
        (
            s.iter().map(|v| if *v > 0.0 { (v-1.0).max(0.0) } else { v-1.0 }).collect(),
            s.iter().map(|v| v + 1.0).collect(),
        )
    }
    //fn names(&self, x: &[f64]) -> Vec<(&str, usize)>
    fn print(&self, x: &[f64]) {
        for v in x { print!(" {v:.0}") }
    }
}

pub struct Cost<'c, CF: CostFn> {
    pub conf: &'c Conf,
    pub cost: CF,
    pub best_cost: RefCell<usize>
}

impl<'c, CF: CostFn> Cost<'c, CF> {
    #[inline] pub fn new(conf: &'c Conf, cost: CF) -> Self { Self { conf, cost, best_cost: RefCell::new(usize::MAX) } }
    pub fn eval(&self, x: &[f64]) -> usize {
        let v = self.cost.eval(self.conf, x);
        print!("{v} {:.2}% ", v as f64 * 100.0 / (Conf::KEY_SETS_NUM as f64 * self.conf.keys_num as f64));
        self.print(x);
        if v < *self.best_cost.borrow() {
            print!(" (best)");
            *self.best_cost.borrow_mut() = v;
        }
        println!();
        v
    }
    #[inline] pub fn init(&self) -> Vec<f64> { self.cost.init(self.conf) }
    #[inline] pub fn bounds(&self) -> (Vec<f64>, Vec<f64>) { self.cost.bounds(&self.conf) }
    #[inline] pub fn print(&self, x: &[f64]) { self.cost.print(x); }
}

impl<'c, CF: CostFn> argmin::core::CostFunction for Cost<'c, CF> {
    type Param = Vec<f64>;
    type Output = f64;

    #[inline] fn cost(&self, param: &Self::Param) -> Result<Self::Output, argmin::core::Error> {
        Ok(self.eval(param) as f64)
    }
}

pub struct WeightsCost<SC: SeedChooser>(pub SC);

impl<SC: SeedChooser> CostFn for WeightsCost<SC> {
    fn eval(&self, conf: &Conf, x: &[f64]) -> usize {
        conf.par_eval(x, |keys| Partial::with_hashes_bps_conf_sc_be_u(keys, BitsFast(conf.bits_per_seed),
                    conf.core(self.0.core()),
                    self.0.clone(), &WeightsF{ size_weights: x.into() }).1)
    }

    fn init(&self, conf: &Conf) -> Vec<f64> {
        WeightsF::from(self.0.bucket_evaluator(conf.bits_per_seed, conf.core(self.0.core()).slice_len())).size_weights.into()
    }
}


pub struct PerfectLogCost;

impl CostFn for PerfectLogCost {
    fn eval(&self, conf: &Conf, x: &[f64]) -> usize {
        let e = SumOfLogValuesFEval { free_values_weight: x[2], value_shift: x[0], free_shift: x[1], first_weight: x[3] };
        let s = SeedOnlyK::new(conf.k, e);
        conf.par_eval(x, |keys| Partial::with_hashes_bps_conf_sc_u(keys, BitsFast(conf.bits_per_seed),
            conf.core(s.core()), s).1)
    }

    fn init(&self, conf: &Conf) -> Vec<f64> {
        let s = SumOfLogValuesF.for_k(conf.k);
        vec![s.value_shift, s.free_shift, s.free_values_weight, s.first_weight]
    }

    fn bounds(&self, _conf: &Conf) -> (Vec<f64>, Vec<f64>) {
        (vec![0.00001, 1.0, 0.5, 0.0],
         vec![0.01, 10.0, 2.0, 1.0])
    }

    fn print(&self, x: &[f64]) {
        print!("free_values_weight: {:.5}, value_shift: {:.5}, free_shift: {:.5}, first_weight: {:.5}", x[2], x[0], x[1], x[3]);
    }
}


pub struct PerfectLog0Cost;

impl CostFn for PerfectLog0Cost {
    fn eval(&self, conf: &Conf, x: &[f64]) -> usize {
        let e = SumOfLogValuesFEval { free_values_weight: x[2], value_shift: x[0], free_shift: x[1], first_weight: 0.0 };
        let s = SeedOnlyK::new(conf.k, e);
        conf.par_eval(x, |keys| Partial::with_hashes_bps_conf_sc_u(keys, BitsFast(conf.bits_per_seed),
            conf.core(s.core()), s).1)
    }

    fn init(&self, conf: &Conf) -> Vec<f64> {
        let s = SumOfLogValuesF0.for_k(conf.k);
        vec![s.value_shift, s.free_shift, s.free_values_weight]
    }

    fn bounds(&self, _conf: &Conf) -> (Vec<f64>, Vec<f64>) {
        (vec![0.00001, 1.0, 0.5],
         vec![0.01, 10.0, 2.0])
    }

    fn print(&self, x: &[f64]) {
        print!("free_values_weight: {:.5}, value_shift: {:.5}, free_shift: {:.5}", x[2], x[0], x[1]);
    }
}


pub struct PerfectLog1Cost;

impl CostFn for PerfectLog1Cost {
    fn eval(&self, conf: &Conf, x: &[f64]) -> usize {
        let e = SumOfLogValuesFEval { free_values_weight: x[2], value_shift: x[0], free_shift: x[1], first_weight: 1.0 };
        let s = SeedOnlyK::new(conf.k, e);
        conf.par_eval(x, |keys| Partial::with_hashes_bps_conf_sc_u(keys, BitsFast(conf.bits_per_seed),
            conf.core(s.core()), s).1)
    }

    fn init(&self, conf: &Conf) -> Vec<f64> {
        let s = SumOfLogValuesF1.for_k(conf.k);
        vec![s.value_shift, s.free_shift, s.free_values_weight]
    }

    fn bounds(&self, _conf: &Conf) -> (Vec<f64>, Vec<f64>) {
        (vec![0.00001, 1.0, 0.5],
         vec![0.01, 10.0, 2.0])
    }

    fn print(&self, x: &[f64]) {
        print!("free_values_weight: {:.5}, value_shift: {:.5}, free_shift: {:.5}", x[2], x[0], x[1]);
    }
}

pub struct PerfectLogFW1Cost;

impl CostFn for PerfectLogFW1Cost {
    fn eval(&self, conf: &Conf, x: &[f64]) -> usize {
        let e = SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: x[0], free_shift: x[1], first_weight: x[2] };
        let s = SeedOnlyK::new(conf.k, e);
        conf.par_eval(x, |keys| Partial::with_hashes_bps_conf_sc_u(keys, BitsFast(conf.bits_per_seed),
            conf.core(s.core()), s).1)
    }

    fn init(&self, conf: &Conf) -> Vec<f64> {
        let s = SumOfLogValuesFW1.for_k(conf.k);
        vec![s.value_shift, s.free_shift, s.first_weight]
    }

    fn bounds(&self, _conf: &Conf) -> (Vec<f64>, Vec<f64>) {
        (vec![0.00001, 1.0, 0.5],
         vec![0.01, 10.0, 2.0])
    }

    fn print(&self, x: &[f64]) {
        println!("value_shift: {:.5}, free_shift: {:.5}, first_weight: {:.5}", x[0], x[1], x[2]);
    }
}


struct ProdOfValuesCost;

impl CostFn for ProdOfValuesCost {
    fn eval(&self, conf: &Conf, x: &[f64]) -> usize {
        let s = SeedOnly(GenericProdOfValues { first_weight: x[0], shift: x[1] });
        conf.par_eval(x, |keys| Partial::with_hashes_bps_conf_sc_u(keys, BitsFast(conf.bits_per_seed),
            conf.core(s.core()), s).1)
    }

    fn init(&self, conf: &Conf) -> Vec<f64> {
        todo!()
    }
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

    fn for_k(&self, k: u16) -> Self::KSeedEvaluator {
        let s = SumOfLogValues.for_k(k);
        SumOfLogValuesFEval {
            free_values_weight: s.free_values_weight, value_shift: s.value_shift as f64, free_shift: s.free_shift as f64,
            first_weight: 0.0, 
        }
    }
}

/// Chooses seed that minimizes
/// sum_{x in bucket} log(f(x,seed) - minimum value in the bucket + value_shift) - free_values_weight * log(freeSlots(f(x,seed)))
/// where minimum value in the bucket = first_weight * minimal value in window  +  (1-first_weight) * minimal value in bucket
pub struct SumOfLogValuesF;

impl KSeedEvaluatorConf for SumOfLogValuesF {
    type KSeedEvaluator = SumOfLogValuesFEval;

    fn for_k(&self, k: u16) -> Self::KSeedEvaluator {
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
            80..100 => SumOfLogValuesFEval { free_values_weight: 0.87878, value_shift: 0.00331, free_shift: 4.10116, first_weight: 0.67052 }, // 0.55%
            100..128 => SumOfLogValuesFEval { free_values_weight: 0.86586, value_shift: 0.00344, free_shift: 4.10146, first_weight: 0.66337 },  // 0.60%
            128..200 => SumOfLogValuesFEval { free_values_weight: 0.84560, value_shift: 0.00332, free_shift: 4.22102, first_weight: 0.69698 },  // 0.68%
            200..256 => SumOfLogValuesFEval { free_values_weight: 0.83602, value_shift: 0.00353, free_shift: 4.34861, first_weight: 0.66096 }, // 0.94%
            256..300 => SumOfLogValuesFEval { free_values_weight: 0.87853, value_shift: 0.00384, free_shift: 5.43240, first_weight: 0.60892 },  // 1.16%
            300..400 => SumOfLogValuesFEval { free_values_weight: 0.94034, value_shift: 0.00279, free_shift: 8.08843, first_weight: 0.61947 }, // 1.34%
            400..500 => SumOfLogValuesFEval { free_values_weight: 0.86279, value_shift: 0.00392, free_shift: 5.79904, first_weight: 0.58109 }, // 1.79%
            500..1000 => SumOfLogValuesFEval { free_values_weight: 0.86415, value_shift: 0.00393, free_shift: 5.81884, first_weight: 0.59772 }, // 2.23%
            1000..1024 => SumOfLogValuesFEval { free_values_weight: 0.88145, value_shift: 0.00397, free_shift: 5.73186, first_weight: 0.60419 }, // 2.25%
            1024.. => SumOfLogValuesFEval { free_values_weight: 0.88038, value_shift: 0.00399, free_shift: 5.96197, first_weight: 0.59852 } // 2.24%
        }
    }
}

/// Chooses seed that minimizes
/// sum_{x in bucket} log(f(x,seed) - minimum value in the window + value_shift) - free_values_weight * log(freeSlots(f(x,seed)))
pub struct SumOfLogValuesF1;

impl KSeedEvaluatorConf for SumOfLogValuesF1 {
    type KSeedEvaluator = SumOfLogValuesFEval;

    fn for_k(&self, k: u16) -> Self::KSeedEvaluator {
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

pub struct SumOfLogValuesFW1;

impl KSeedEvaluatorConf for SumOfLogValuesFW1 {
    type KSeedEvaluator = SumOfLogValuesFEval;

    fn for_k(&self, k: u16) -> Self::KSeedEvaluator {
        match k {
            ..=2 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00440, free_shift: 1.67344, first_weight: 0.12821 }, // 1.02%
            3 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00353, free_shift: 1.79754, first_weight: 0.21056 }, // 1.08%
            4 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00414, free_shift: 2.05039, first_weight: 0.42381 }, // 1.09%
            5 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00362, free_shift: 2.41147, first_weight: 0.63314 }, // 1.05%
            6 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00322, free_shift: 2.64235, first_weight: 0.76291 }, // 0.97%
            7 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00316, free_shift: 2.86673, first_weight: 0.76727 }, // 0.89%
            8 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00305, free_shift: 2.93630, first_weight: 0.73511 }, // 0.81%
            9 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00334, free_shift: 3.00825, first_weight: 0.71309 }, // 0.73%
            10 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00340, free_shift: 3.23864, first_weight: 0.73775 }, // 0.68%
            11 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00326, free_shift: 3.31397, first_weight: 0.71208 }, // 0.63%
            12 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00305, free_shift: 3.35685, first_weight: 0.68939 }, // 0.60%
            13 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00306, free_shift: 3.49506, first_weight: 0.70382 }, // 0.57%
            14 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00317, free_shift: 3.49727, first_weight: 0.67751 }, // 0.56%
            100 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00352, free_shift: 5.16385, first_weight: 0.43017 }, // 0.61%
            200 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00386, free_shift: 5.53550, first_weight: 0.37559 }, // 0.96%
            300 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00292, free_shift: 8.95345, first_weight: 0.52976 }, // 1.35%
            400 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00431, free_shift: 7.25800, first_weight: 0.35377 }, // 1.78%
            500 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00432, free_shift: 7.79703, first_weight: 0.31048 }, // 2.22%
            1000 => SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: 0.00460, free_shift: 6.56534, first_weight: 0.34167 }, // 2.23%
            _ => {
                SumOfLogValuesFEval { free_values_weight: 1.0, ..SumOfLogValuesF.for_k(k) }
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
    fn for_k(&self, _k: u16) -> Self { *self }
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

    fn eval(&self, k: u16, values_used_by_seed: &[usize], used_values: &UsedValueMultiSetU16, to_subtract_from_value: Self::BucketData) -> Self::Value {
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