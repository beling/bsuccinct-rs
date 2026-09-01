//! [`SeedOnlyK`] – the seed chooser for k-perfect functions.

use std::io;

use binout::{AsIs, Serializer};
use bitm::ceiling_div;

use crate::phast::{ProdCmp, ProdOfValues, SeedChooserCore, SeedEvaluator, SumOfValues, Weights, conf::Core, cyclic::FreeValueMultiSetU16, seed_chooser::SeedChooserConf, space_lower_bound};
use super::SeedChooser;

/// Returns the multiplier that allows obtaining a bucket size of `k`-perfect function from a bucket size of 1-perfect function.
pub fn bucket_size_normalization_multiplier(k: u16) -> f64 {
    //let overhead = 0.05; //+ 0.25 / (k as f64 * k as f64);
    //(space_lower_bound(1)+overhead) / (space_lower_bound(k)+overhead)
    space_lower_bound(1) / space_lower_bound(k)
}

/// Configuration and factory of `KSeedEvaluator`.
/// 
/// It is implemented for structures that are `KSeedEvaluator`
/// (in which case they contain specific parameters and the factories return `self`)
/// as well as those that can only construct `KSeedEvaluator`
/// (in which case the parameters are selected automatically).
pub trait KSeedEvaluatorConf: Clone + Sync {
    /// Type of evaluator.
    type KSeedEvaluator: KSeedEvaluator;

    /// Returns evaluator for given `k`.
    fn seed_evaluator_k(&self, k: u16, bits_per_seed: u8, slice_len: u16) -> Self::KSeedEvaluator;
 
    /// Returns bucket evaluator that works well with seed evaluator returned by `seed_evaluator_k`.
    fn bucket_evaluator_k(&self, k: u16, bits_per_seed: u8, slice_len: u16) -> Weights;
}

/// Evaluate (harness of) seed for k-perfect function.
/// Seed with the lowest value is used.
pub trait KSeedEvaluator: KSeedEvaluatorConf<KSeedEvaluator=Self> {
    /// Type of evaluation value.
    type Value: PartialEq + PartialOrd + Ord;

    /// Precalculated data usable to evaluate each seed in the same bucket.
    type BucketData: Copy;

    /// Value greater than each value returned by `eval`.
    const MAX: Self::Value;

    /// Precalculates data usable to evaluate each seed in the same bucket.
    /// The result is passed to `eval` for each seed in the bucket.
    fn for_bucket<C: Core>(&self, bucket_nr: usize, first_bucket_in_window: usize, core: &C) -> Self::BucketData;

    /// Evaluate (harness of) seed that used given `values`.
    fn eval_and_remove(&self, k: u16, values_used_by_seed: &[usize], free_values: &mut FreeValueMultiSetU16, bucket_data: Self::BucketData) -> Self::Value;
}

/// Evaluate seed using sum of values it takes.
impl KSeedEvaluator for SumOfValues {
    type Value = usize;
    
    const MAX: Self::Value = usize::MAX;

    type BucketData = ();

    #[inline]
    fn for_bucket<C: Core>(&self, _bucket_nr: usize, _first_bucket_in_window: usize, _core: &C) -> Self::BucketData {}

    #[inline]
    fn eval_and_remove(&self, _k: u16, values_used_by_seed: &[usize], free_values: &mut FreeValueMultiSetU16, _bucket_data: Self::BucketData) -> Self::Value {
        let mut result = 0;
        for value in values_used_by_seed {
            result += *value;
            free_values[*value] += 1;
        }
        result
    }
}

impl KSeedEvaluatorConf for SumOfValues {
    type KSeedEvaluator = Self;
    #[inline] fn seed_evaluator_k(&self, _k: u16, _bits_per_seed: u8, _slice_len: u16) -> Self::KSeedEvaluator { SumOfValues }
    #[inline] fn bucket_evaluator_k(&self, _k: u16, bits_per_seed: u8, slice_len: u16) -> Weights {
        SumOfValues.bucket_evaluator(bits_per_seed, slice_len)
    }
}



/// For given `k`, `bits_per_seed` and `slice_len`, returns the rows of the tuning table
/// that bracket `k`: the closest row below or at `k`, and – if `k` lays between two rows –
/// the next row (otherwise `None`).
fn prod_for(k: u16, bits_per_seed: u8, slice_len: u16) -> (&'static (u16, [i32; 7], ProdOfValuesKEval), Option<&'static (u16, [i32; 7], ProdOfValuesKEval)>) {
    let values = match (bits_per_seed, slice_len) {
        /*(_, ..=512) => PROD_S8_L512.as_ref(),
        (_, ..=1024) => PROD_S8_L1024.as_ref(),
        (_, ..=2048) => PROD_S8_L2048.as_ref(),
        (_, ..=4096) => PROD_S8_L4096.as_ref(),
        (_, _) => PROD_S8_L8192.as_ref(),*/

        (_, ..=512) => PROD_S8_L512.as_ref(),
        (_, _) => PROD_S8_L1024.as_ref()
    };
    for (i, row) in values.iter().enumerate() {
        if row.0 >= k {
            return if row.0 == k || i == 0 { (row, None) } else { (&values[i-1], Some(row)) };
        }
    }
    (unsafe{values.last().unwrap_unchecked()}, None)
}
/// How close k is to nk for pk <= k <= nk; 1.0 if k==nk and 0.0 if k==pk.
#[inline] fn next_weight(pk: u16, k: u16, nk: u16) -> f64 { (k-pk) as f64/(nk-pk) as f64 }

/// Interpolates (linearly, `nw` being the weight of the `n` row)
/// two tuning-table rows `p` and `n` into a single row.
fn combine(p: &[i32; 7], n: &[i32; 7], nw: f64) -> [i32; 7] {
    let pw = 1.0 - nw;
    std::array::from_fn(|i| ((p[i] as f64 * pw) + (n[i] as f64 * nw)).round() as i32)
}

impl KSeedEvaluatorConf for ProdOfValues {
    type KSeedEvaluator = ProdOfValuesKEval;

    fn seed_evaluator_k(&self, k: u16, bits_per_seed: u8, slice_len: u16) -> Self::KSeedEvaluator {       
        let (pk, nk) = prod_for(k, bits_per_seed, slice_len);
        if let Some(nk) = nk {
            pk.2.combine(&nk.2, next_weight(pk.0, k, nk.0) /*(k-*pk) as f64/(*nk-*pk) as f64*/)
        } else { pk.2 }
    }

    fn bucket_evaluator_k(&self, k: u16, bits_per_seed: u8, slice_len: u16) -> Weights {
        let (pk, nk) = prod_for(k, bits_per_seed, slice_len);
        Weights(if let Some(nk) = nk {
            combine(&pk.1, &nk.1, next_weight(pk.0, k, nk.0))
        } else { pk.1 })
    }
}

/// Chooses seed that minimizes
/// sum_{x in bucket} log(f(x,seed) - first_weight*minimum value in the window - (1-first_weight)*minimum value in the bucket + value_shift) - free_values_weight * log(freeSlots(f(x,seed)))
#[derive(Clone, Copy)]
pub struct ProdOfValuesKEval {
    /// The weight given to the minimum value in the already-processed window
    /// (as opposed to the minimum value in the bucket).
    pub first_weight: f64,
    /// A shift added to each value.
    pub value_shift: f64,
    /// A shift added to number of free slots.
    pub free_shift: f64
}

impl ProdOfValuesKEval {
    #[inline] pub fn combine(&self, other: &Self, other_weight: f64) -> Self {
        let self_weight = 1.0 - other_weight;
        Self { first_weight: self_weight * self.first_weight + other_weight * other.first_weight,
               value_shift: self_weight * self.value_shift + other_weight * other.value_shift,
               free_shift: self_weight * self.free_shift + other_weight * other.free_shift}
    }
}

impl KSeedEvaluatorConf for ProdOfValuesKEval {
    type KSeedEvaluator = Self;
    fn seed_evaluator_k(&self, _k: u16, _bits_per_seed: u8, _slice_len: u16) -> Self { 
        //let mut r= *self; r.free_shift += k as f64; r 
        *self
    }
    fn bucket_evaluator_k(&self, k: u16, bits_per_seed: u8, slice_len: u16) -> Weights {
        ProdOfValues.bucket_evaluator_k(k, bits_per_seed, slice_len)
    }
}

impl KSeedEvaluator for ProdOfValuesKEval {
    type Value = ProdCmp;
    const MAX: Self::Value = ProdCmp::MAX;

    type BucketData = f64;   

    fn for_bucket<C: Core>(&self, bucket_nr: usize, first_bucket_in_window: usize, core: &C) -> Self::BucketData {
       core.slice_begin_for_bucket(bucket_nr) as f64 * (1.0-self.first_weight) +
       core.slice_begin_for_bucket(first_bucket_in_window) as f64 * self.first_weight
        - self.value_shift
    }

    fn eval_and_remove(&self, _k: u16, values_used_by_seed: &[usize], free_values: &mut FreeValueMultiSetU16, to_subtract_from_value: Self::BucketData) -> Self::Value {
        let mut result = ProdCmp::default();
        for value in values_used_by_seed.iter().copied() {
            result *= (value as f64 - to_subtract_from_value) / (self.free_shift + free_values[value] as f64);
            free_values[value] += 1;
        }
        result
    }
}

/// [`SeedChooserCore`] of `k`-perfect functions: it passes all seed bits to the hash function
/// and does not use shifting; the value of `k` is stored in the function structure.
#[derive(Clone, Copy)]
pub struct SeedOnlyKCore(pub u16);

impl SeedChooserCore for SeedOnlyKCore {
    
    #[inline(always)] fn k(&self) -> u16 { self.0 }

    #[inline(always)] fn f<C: Core>(&self, primary_code: u64, seed: u16, core: &C) -> usize {
        core.f(primary_code, seed)
    }

    #[inline(always)] fn minimal_output_range(&self, num_of_keys: usize) -> usize { ceiling_div(num_of_keys, self.0 as usize) }

    fn write(&self, output: &mut dyn io::Write) -> io::Result<()> { 
        AsIs::write(output, self.0)
    }

    fn write_bytes(&self) -> usize { AsIs::size(self.0) }

    fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Ok(Self(AsIs::read(input)?)) 
    }
}


/// [`SeedChooserConf`] (and [`SeedChooser`] if `SE` is [`KSeedEvaluator`]) to build `k`-perfect functions.
/// `k` is given as a parameter of this chooser.
/// 
/// Should be used with [`KFunction`](crate::phast::KFunction) or [`Perfect`](crate::phast::Perfect).
/// 
/// It chooses best seed with quite strong hasher, without shift component,
/// which should lead to quite small size, but long construction time.
#[derive(Clone, Copy)]
pub struct SeedOnlyK<SE: KSeedEvaluatorConf = ProdOfValuesKEval> {
    /// Seed evaluator used to compare the seeds of a bucket
    /// (a concrete evaluator obtained from `SE` for the actual parameters).
    pub seed_evaluator: SE,
    /// [`SeedChooserCore`] of the chooser, storing the value of `k`.
    pub core: SeedOnlyKCore,
}

impl<SE: KSeedEvaluatorConf> SeedChooserConf for SeedOnlyK<SE> {

    type SeedChooser = SeedOnlyK<SE::KSeedEvaluator>;

    type BucketEvaluator = Weights;

    type Core = SeedOnlyKCore;

    type UsedValues = FreeValueMultiSetU16;

    #[inline] fn empty_used_values(&self) -> Self::UsedValues {
        Self::UsedValues::filled_with(self.k())
    }

    #[inline(always)] fn add_used(&self, free_values: &mut Self::UsedValues, value: usize) {
        free_values[value] -= 1;
    }
    
    #[inline(always)] fn clear_used(&self, free_values: &mut Self::UsedValues, value: usize) {
        free_values[value] = self.k();
    }

    #[inline(always)] fn core(&self) -> Self::Core { self.core }
    
    #[inline(always)] fn seed_chooser(&self, bits_per_seed: u8, slice_len: u16) -> Self::SeedChooser {
        SeedOnlyK::<SE::KSeedEvaluator> {
            seed_evaluator: self.seed_evaluator.seed_evaluator_k(self.k(), bits_per_seed, slice_len),
            core: self.core,
        }
    }

    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {
        self.seed_evaluator.bucket_evaluator_k(self.k(), bits_per_seed, slice_len)
    }
}

impl SeedOnlyK<ProdOfValues> {
    /// Constructs the chooser for given `k`, using [`ProdOfValues`] as the seed evaluator.
    pub fn new(k: u16) -> Self {
        Self::with_evaluator(k, ProdOfValues)
    }
}

impl<SE: KSeedEvaluatorConf> SeedOnlyK<SE> {
    /// Constructs the chooser for given `k` and `seed_evaluator`.
    pub fn with_evaluator(k: u16, seed_evaluator: SE) -> Self {
        Self { seed_evaluator, core: SeedOnlyKCore(k) }
    }
}

/// Selects, among all feasible seeds, the one minimizing the value returned by `seed_evaluator`
/// and returns it (through `best_seed`). A seed is feasible if none of the values it assigns
/// exhausts the remaining free slots; the slots taken by a candidate seed are temporarily
/// decremented in `free_values` (and restored if the seed turns out not to be the best).
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn best_seed_k<SC: SeedChooser, SE: KSeedEvaluator, C: Core>(k: u16, seed_chooser: &SC, seed_evaluator: &SE, best_value: &mut SE::Value, best_seed: &mut u16, free_values: &mut FreeValueMultiSetU16, keys: &[u64], core: &C, seeds_num: u16, bucket_nr: usize, first_bucket_in_window: usize) {
    let mut values_used_by_seed = Vec::with_capacity(keys.len());
    let bucket_data = seed_evaluator.for_bucket(bucket_nr, first_bucket_in_window, core);
    'outer: for seed in SC::Core::FIRST_SEED..seeds_num {    // seed=0 is special = no seed,
        values_used_by_seed.clear();
        for key in keys.iter().copied() {
            let value = seed_chooser.f(key, seed, core);
            if free_values[value] == 0 {
                for v in &values_used_by_seed { free_values[*v] += 1; }
                continue 'outer;
            }
            free_values[value] -= 1;
            values_used_by_seed.push(value);
        }
        unsafe{std::hint::assert_unchecked(values_used_by_seed.len() == keys.len());}   // this speeds up the code!
        let seed_value = seed_evaluator.eval_and_remove(k, &values_used_by_seed, free_values, bucket_data);
        if seed_value < *best_value {
            *best_value = seed_value;
            *best_seed = seed;
        }
    }
}


impl<SE: KSeedEvaluator> SeedChooser for SeedOnlyK<SE> {
   
    /// Returns the seed with the lowest evaluation value, or `0` (which indicates bumping)
    /// if there is no feasible seed.
    #[inline(always)]
    fn best_seed<C: Core>(&self, free_values: &mut Self::UsedValues, keys: &[u64], core: &C, bits_per_seed: u8, bucket_nr: usize, first_bucket_in_window: usize) -> u16 {
        let mut best_seed = 0;
        let mut best_value = SE::MAX;
        best_seed_k(self.k(), self, &self.seed_evaluator, &mut best_value, &mut best_seed, free_values, keys, core, 1<<bits_per_seed, bucket_nr, first_bucket_in_window);
        if best_seed != 0 { // can assign seed to the bucket
            for key in keys {
                free_values[core.f(*key, best_seed)] -= 1;
            }
        };
        best_seed
    }
}



type P=ProdOfValuesKEval;
const PROD_S8_L512: [(u16, [i32; 7], ProdOfValuesKEval); 14] = [   // for W=512
    (2, [0, 76473, 93538, 100036, 111405, 124459, 127558], P{value_shift: 0.00968, free_shift: 1.94309, first_weight: 0.28871}), // 1.28% for λ=6.88
    (3, [0, 43209, 47718, 47849, 56764, 62883, 65002], P{value_shift: 0.009677, free_shift: 1.317506, first_weight: 0.310870}), // 1.18% for λ=9.02
    (4, [0, 124831, 125295, 125906, 134811, 144356, 145403], P{value_shift: 0.00668, free_shift: 1.29751, first_weight: 0.27419}), // 1.12% for λ=11.02
    (6, [0, 149127, 160970, 171446, 177056, 182860, 183392], P{value_shift: 0.005145, free_shift: 1.194641, first_weight: 0.584520}), //0.99% for λ=14.76
    (8, [0, 163258, 202756, 216115, 219743, 223466, 223810], P{value_shift: 0.00364, free_shift: 1.21311, first_weight: 0.85403}), // 0.92% for λ=18.28
    (10, [0, 167476, 202441, 212104, 213999, 215955, 216104], P{value_shift: 0.003798, free_shift: 1.172512, first_weight: 0.871241}), // 0.88% for λ=21.64
    (12, [0, 169779, 209752, 213158, 213291, 213433, 213442], P{value_shift: 0.003807, free_shift: 1.141909, first_weight: 0.857620}), // 0.86% for λ=24.90
    (16, [0, 170107, 206229, 210576, 210830, 211146, 211180], P{value_shift: 0.00365, free_shift: 1.12751, first_weight: 0.87378}), // 0.85% for λ=31.16
    (20, [0, 172926, 194515, 207725, 209558, 211778, 211993], P{value_shift: 0.003568, free_shift: 1.125351, first_weight: 0.905329}), // 0.85% for λ=37.17
    (24, [0, 179037, 198479, 210605, 212759, 215360, 215624], P{value_shift: 0.003623, free_shift: 1.118238, first_weight: 0.914403}), // 0.83% for λ=43.00
    (32, [0, 179706, 199221, 210865, 213144, 215884, 216163], P{value_shift: 0.003632, free_shift: 1.128020, first_weight: 0.916687}), // 0.79% for λ=54.24
    (48, [0, 179533, 198080, 208200, 210344, 212994, 213256], P{value_shift: 0.003635, free_shift: 1.173609, first_weight: 0.925055}), // 0.77% for λ=75.62
    (64, [0, 181531, 198187, 207728, 209818, 212437, 212699], P{value_shift: 0.003652, free_shift: 1.229707, first_weight: 0.913129}), // 0.79% for λ=96.01
    (100, [0, 188236, 199627, 208338, 209812, 211581, 211790], P{value_shift: 0.003562, free_shift: 1.285305, first_weight: 0.862204}), // 0.91% for λ=139.64
];
#[cfg(not(feature = "W256"))]
const PROD_S8_L1024: [(u16, [i32; 7], ProdOfValuesKEval); 33] = [   // for W=512
    (2, [0, 124853, 186485, 217947, 238678, 260990, 266209], P{value_shift: 0.005040, free_shift: 1.543640, first_weight: 0.101565}), // 1.15% for λ=6.88
    //(3, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00456, free_shift: 1.67791, first_weight: 0.13456}), // O1.03%
    (4, [0, 133201, 188435, 207660, 220157, 228500, 230630], P{value_shift: 0.004811, free_shift: 1.312647, first_weight: 0.303816}), // 1.15% for λ=11.02
    //(5, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00396, free_shift: 2.35814, first_weight: 0.62478 }), // O1.04%
    (6, [0, 164475, 244036, 270054, 271239, 272210, 272747], P{value_shift: 0.003458, free_shift: 1.157448, first_weight: 0.813080}), // 1.05% for λ=14.76
    (7, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00352, free_shift: 2.79442, first_weight: 0.74567 }), // O0.88%
    (8, [0, 132557, 183504, 217346, 240456, 257524, 268553], P{value_shift: 0.00324, free_shift: 2.79127, first_weight: 0.76304}), // 1.56% for λ=18.28
    (9, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00344, free_shift: 3.05469, first_weight: 0.72960 }), // O0.73%
    (10, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00357, free_shift: 3.23864, first_weight: 0.73775 }), // O0.67%
    (11, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00326, free_shift: 3.31397, first_weight: 0.71208 }), // O0.63%
    (12, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00305, free_shift: 3.35685, first_weight: 0.68939 }), // O0.60%
    (13, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00306, free_shift: 3.49506, first_weight: 0.70382 }), // O0.57%
    (14, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00317, free_shift: 3.49727, first_weight: 0.67751 }), // O0.56%
    (15, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00305, free_shift: 3.54152, first_weight: 0.66301 }), // O0.55%
    (16, [0, 132238, 192126, 228640, 246369, 263743, 270676], P{value_shift: 0.003030, free_shift: 3.179457, first_weight: 0.781803}), // 1.15% for λ=31.16
    (32, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00297, free_shift: 4.53498, first_weight: 0.62771 }), // O0.64%
    (50, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00269, free_shift: 5.52251, first_weight: 0.60593 }), // O0.76%
    (64, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00180, free_shift: 6.20170, first_weight: 0.61391 }), // O0.84%
    (100, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00352, free_shift: 5.16385, first_weight: 0.43017 }), //O0.61%
    (128, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00294, free_shift: 6.66377, first_weight: 0.55960 }), // O0.69%
    (200, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00386, free_shift: 5.53550, first_weight: 0.37559 }), // O0.96%
    (256, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00648, free_shift: 10.29860, first_weight: 0.66279 }), // O1.16%
    (300, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00292, free_shift: 8.95345, first_weight: 0.52976 }), // O1.35%
    (400, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00431, free_shift: 7.25800, first_weight: 0.35377 }), // O1.78%
    (500, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00432, free_shift: 7.79703, first_weight: 0.31048 }), // O2.22%
    (512, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00523, free_shift: 7.70449, first_weight: 0.28980 }), // O2.27%
    (750, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00520, free_shift: 7.10482, first_weight: 0.31508 }), // O3.27%
    (1000, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00460, free_shift: 6.56534, first_weight: 0.34167 }), // O2.23%
    (1024, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00416, free_shift: 6.90059, first_weight: 0.45755 }), // O2.28%
    (1500, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00412, free_shift: 6.96199, first_weight: 0.48575 }), // O1.68%
    (2000, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00440, free_shift: 7.16988, first_weight: 0.44569 }), // O2.24%
    (3000, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00446, free_shift: 7.39262, first_weight: 0.45036 }), // O3.27%
    (4000, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00423, free_shift: 7.45927, first_weight: 0.38573 }), // O4.33%
    (5000, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.60851, free_shift: 25.95862, first_weight: 0.27874 }), // O5.25%
    (10000, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 1.30708, free_shift: 189.36282, first_weight: 1.00000 }), // O5.22%
];
//const PROD_S8_L2048: [(u16, [i32; 7], ProdOfValuesKEval); 0] = [   // for W=512
    //(8, [0, 140681, 188411, 228234, 228489, 228727, 228748], P{value_shift: 0.00316, free_shift: 1.15326, first_weight: 0.82834}), // 1.16% for λ=18.28
//];
//const PROD_S8_L4096: [(u16, [i32; 7], ProdOfValuesKEval); 0] = [
    //(2, [0, 2247, 165284, 270422, 346622, 407348, 446489], P{value_shift: 0.00831, free_shift: 0.61121, first_weight: 0.60566}), //R0.86%
    //(4, [0, 226, 34010, 34417, 374541, 477304, 496400], P{value_shift: 0.00657, free_shift: 0.98817, first_weight: 0.06933}), //R0.71%
    //(8, [0, 21680, 607272, 943854, 1019752, 1036883, 1167619], P{value_shift: 0.02297, free_shift: 0.64340, first_weight: 0.80836}), //R0.84%
//];
//const PROD_S8_L8192: [(u16, [i32; 7], ProdOfValuesKEval); 0] = [
    //(4, [0, 1872, 5387, 42264, 163703, 316315, 366479], P{value_shift: 0.04430, free_shift: 0.12747, first_weight: 0.11532}), //R0.83%
    //(8, [0, 5580, 20700, 195106, 297919, 313269, 412413], P{value_shift: 0.02860, free_shift: 0.10610, first_weight: 0.47308}), //R1.03%
//];


#[cfg(feature = "W256")]
const VALUES: [(u16, [i32; 7], ProdOfValuesKEval); 35] = [
    (2, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00471, free_shift: 1.65874, first_weight: 0.12367 }), // 1.01%
    (3, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00381, free_shift: 1.80743, first_weight: 0.20054 }), // 1.07%
    (4, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00426, free_shift: 2.07610, first_weight: 0.41340 }), // 1.08%
    (5, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00396, free_shift: 2.35814, first_weight: 0.62478 }), // 1.04%
    (6, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00351, free_shift: 2.64993, first_weight: 0.74814 }), // 0.96%
    (7, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00352, free_shift: 2.79442, first_weight: 0.74567 }), // 0.88%
    (8, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00320, free_shift: 2.91581, first_weight: 0.73696 }), // 0.80%
    (9, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00344, free_shift: 3.05469, first_weight: 0.72960 }), // 0.73%
    (10, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00357, free_shift: 3.23864, first_weight: 0.73775 }), // 0.67%
    (11, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00326, free_shift: 3.31397, first_weight: 0.71208 }), // 0.63%
    (12, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00305, free_shift: 3.35685, first_weight: 0.68939 }), // 0.60%
    (13, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00306, free_shift: 3.49506, first_weight: 0.70382 }), // 0.57%
    (14, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00317, free_shift: 3.49727, first_weight: 0.67751 }), // 0.56%
    (15, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00305, free_shift: 3.54152, first_weight: 0.66301 }), // 0.55%
    (16, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00312, free_shift: 3.66667, first_weight: 0.68020 }), // 0.54%
    (32, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00297, free_shift: 4.53498, first_weight: 0.62771 }), // 0.64%
    (50, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00269, free_shift: 5.52251, first_weight: 0.60593 }), // 0.76%
    (64, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00180, free_shift: 6.20170, first_weight: 0.61391 }), // 0.84%
    (100, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00352, free_shift: 5.16385, first_weight: 0.43017 }), // 0.61%
    (128, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00294, free_shift: 6.66377, first_weight: 0.55960 }), // 0.69%
    (200, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00386, free_shift: 5.53550, first_weight: 0.37559 }), // 0.96%
    (256, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00648, free_shift: 10.29860, first_weight: 0.66279 }), // 1.16%
    (300, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00292, free_shift: 8.95345, first_weight: 0.52976 }), // 1.35%
    (400, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00431, free_shift: 7.25800, first_weight: 0.35377 }), // 1.78%
    (500, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00432, free_shift: 7.79703, first_weight: 0.31048 }), // 2.22%
    (512, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00523, free_shift: 7.70449, first_weight: 0.28980 }), // 2.27%
    (750, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00520, free_shift: 7.10482, first_weight: 0.31508 }), // 3.27%
    (1000, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00460, free_shift: 6.56534, first_weight: 0.34167 }), // 2.23%
    (1024, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00416, free_shift: 6.90059, first_weight: 0.45755 }), // 2.28%
    (1500, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00412, free_shift: 6.96199, first_weight: 0.48575 }), // 1.68%
    (2000, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00440, free_shift: 7.16988, first_weight: 0.44569 }), // 2.24%
    (3000, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00446, free_shift: 7.39262, first_weight: 0.45036 }), // 3.27%
    (4000, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.00423, free_shift: 7.45927, first_weight: 0.38573 }), // 4.33%
    (5000, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 0.60851, free_shift: 25.95862, first_weight: 0.27874 }), // 5.25%
    (10000, [0, 106597, 160801, 189894, 211279, 228324, 238991], P{value_shift: 1.30708, free_shift: 189.36282, first_weight: 1.00000 }), // 5.22%
];