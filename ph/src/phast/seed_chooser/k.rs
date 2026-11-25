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

    /// Returns evaluator for given `k`, `bits_per_seed`, `slice_len` and `loading_factor_1000` (1000× desired loading factor).
    fn seed_evaluator_k(&self, k: u16, bits_per_seed: u8, slice_len: u16, loading_factor_1000: u16) -> Self::KSeedEvaluator;
 
    /// Returns bucket evaluator that works well with seed evaluator returned by `seed_evaluator_k`.
    fn bucket_evaluator_k(&self, k: u16, bits_per_seed: u8, slice_len: u16, loading_factor_1000: u16) -> Weights;
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
    #[inline] fn seed_evaluator_k(&self, _k: u16, _bits_per_seed: u8, _slice_len: u16, _loading_factor_1000: u16) -> Self::KSeedEvaluator { SumOfValues }
    #[inline] fn bucket_evaluator_k(&self, _k: u16, bits_per_seed: u8, slice_len: u16, _loading_factor_1000: u16) -> Weights {
        SumOfValues.bucket_evaluator(bits_per_seed, slice_len)
    }
}



/// For given `k`, `bits_per_seed` and `slice_len`, returns the rows of the tuning table
/// that bracket `k`: the closest row below or at `k`, and – if `k` lays between two rows –
/// the next row (otherwise `None`).
fn prod_for(k: u16, bits_per_seed: u8, slice_len: u16, loading_factor_1000: u16) -> (&'static (u16, [i32; 7], ProdOfValuesKEval), Option<&'static (u16, [i32; 7], ProdOfValuesKEval)>) {
    let values = match (bits_per_seed, slice_len, loading_factor_1000) {
        (..=2, ..=128, _) => PROD_S2_L128.as_ref(),
        (..=2, ..=256, _) => PROD_S2_L256.as_ref(),
        (3, ..=128, _) => PROD_S3_L128.as_ref(),
        (3, ..=256, _) => PROD_S3_L256.as_ref(),
        (4, ..=128, _) => PROD_S4_L128.as_ref(),
        (4, ..=256, _) => PROD_S4_L256.as_ref(),
        (5, ..=128, _) => PROD_S5_L128.as_ref(),
        (5, ..=256, _) => PROD_S5_L256.as_ref(),
        (6, ..=256, ..851) => PROD_S6_L256_A81.as_ref(),
        (6, ..=256, 851..951) => PROD_S6_L256_A91.as_ref(),
        (6, ..=512, ..851) => PROD_S6_L512_A81.as_ref(),
        (6, ..=512, 851..951) => PROD_S6_L512_A91.as_ref(),
        (6, ..=1024, ..851) => PROD_S6_L1024_A81.as_ref(),
        (6, ..=1024, 851..951) => PROD_S6_L1024_A91.as_ref(),
        (6, _, ..851) => PROD_S6_L2048_A81.as_ref(),
        (6, _, 851..951) => PROD_S6_L2048_A91.as_ref(),
        (6, ..=128, _) => PROD_S6_L128.as_ref(),
        (6, ..=256, _) => PROD_S6_L256.as_ref(),
        (..=6, ..=512, _) => PROD_S6_L512.as_ref(),
        (..=6, _, _) => PROD_S6_L1024.as_ref(),
        (7, ..=128, _) => PROD_S7_L128.as_ref(),
        (7, ..=256, _) => PROD_S7_L256.as_ref(),
        (7, ..=512, _) => PROD_S7_L512.as_ref(),
        (7, _, _) => PROD_S7_L1024.as_ref(),
        (8, ..=128, ..851) => PROD_S8_L128_A81.as_ref(),
        (8, ..=256, ..851) => PROD_S8_L256_A81.as_ref(),
        (8, ..=512, ..851) => PROD_S8_L512_A81.as_ref(),
        (8, ..=256, 851..951) => PROD_S8_L256_A91.as_ref(),
        (8, ..=512, 851..951) => PROD_S8_L512_A91.as_ref(),
        (8, ..=1024, ..851) => PROD_S8_L1024_A81.as_ref(),
        (8, ..=2048, ..851) => PROD_S8_L2048_A81.as_ref(),
        (8, _, ..851) => PROD_S8_L4096_A81.as_ref(),
        (8, ..=1024, 851..951) => PROD_S8_L1024_A91.as_ref(),
        (8, ..=32, _) => PROD_S8_L32.as_ref(),
        (8, ..=64, _) => PROD_S8_L64.as_ref(),
        (8, ..=128, _) => PROD_S8_L128.as_ref(),
        (8, ..=256, _) => PROD_S8_L256.as_ref(),
        (8, ..=512, _) => PROD_S8_L512.as_ref(),
        (8, ..=1024, _) => PROD_S8_L1024.as_ref(),
        (8, _, _) => PROD_S8_L2048.as_ref(),
        //(9.., ..=128, _) => PROD_S9_L128.as_ref(),
        (9, ..=256, _) => PROD_S9_L256.as_ref(),
        (9, ..=512, _) => PROD_S9_L512.as_ref(),
        (9, ..=1024, _) => PROD_S9_L1024.as_ref(),
        (9, _, _) => PROD_S9_L2048.as_ref(),
        (_, _, _) => PROD_S10_L256.as_ref()
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

    fn seed_evaluator_k(&self, k: u16, bits_per_seed: u8, slice_len: u16, loading_factor_1000: u16) -> Self::KSeedEvaluator {       
        let (pk, nk) = prod_for(k, bits_per_seed, slice_len, loading_factor_1000);
        if let Some(nk) = nk {
            pk.2.combine(&nk.2, next_weight(pk.0, k, nk.0) /*(k-*pk) as f64/(*nk-*pk) as f64*/)
        } else { pk.2 }
    }

    fn bucket_evaluator_k(&self, k: u16, bits_per_seed: u8, slice_len: u16, loading_factor_1000: u16) -> Weights {
        let (pk, nk) = prod_for(k, bits_per_seed, slice_len, loading_factor_1000);
        Weights(if let Some(nk) = nk {
            combine(&pk.1, &nk.1, next_weight(pk.0, k, nk.0))
        } else { pk.1 })
    }
}

/// Chooses seed that minimizes
/// sum_{x in bucket} log(f(x,seed) - window_weight*minimum value in the window - (1-window_weight)*minimum value in the bucket + value_shift) - free_values_weight * log(freeSlots(f(x,seed)))
#[derive(Clone, Copy)]
pub struct ProdOfValuesKEval {
    /// The weight given to the minimum value in the already-processed window
    /// (as opposed to the minimum value in the bucket).
    pub window_weight: f64,
    /// A shift added to each value.
    pub value_shift: f64,
    /// A shift added to number of free slots.
    pub free_shift: f64
}

impl ProdOfValuesKEval {
    #[inline] pub fn combine(&self, other: &Self, other_weight: f64) -> Self {
        let self_weight = 1.0 - other_weight;
        Self { window_weight: self_weight * self.window_weight + other_weight * other.window_weight,
               value_shift: self_weight * self.value_shift + other_weight * other.value_shift,
               free_shift: self_weight * self.free_shift + other_weight * other.free_shift}
    }
}

impl KSeedEvaluatorConf for ProdOfValuesKEval {
    type KSeedEvaluator = Self;
    fn seed_evaluator_k(&self, _k: u16, _bits_per_seed: u8, _slice_len: u16, _loading_factor_1000: u16) -> Self { 
        //let mut r= *self; r.free_shift += k as f64; r 
        *self
    }
    fn bucket_evaluator_k(&self, k: u16, bits_per_seed: u8, slice_len: u16, loading_factor_1000: u16) -> Weights {
        ProdOfValues.bucket_evaluator_k(k, bits_per_seed, slice_len, loading_factor_1000)
    }
}

impl KSeedEvaluator for ProdOfValuesKEval {
    type Value = ProdCmp;
    const MAX: Self::Value = ProdCmp::MAX;

    type BucketData = f64;   

    fn for_bucket<C: Core>(&self, bucket_nr: usize, first_bucket_in_window: usize, core: &C) -> Self::BucketData {
       core.slice_begin_for_bucket(bucket_nr) as f64 * (1.0-self.window_weight) +
       core.slice_begin_for_bucket(first_bucket_in_window) as f64 * self.window_weight
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
    
    #[inline(always)] fn seed_chooser(&self, bits_per_seed: u8, slice_len: u16, loading_factor_1000: u16) -> Self::SeedChooser {
        SeedOnlyK::<SE::KSeedEvaluator> {
            seed_evaluator: self.seed_evaluator.seed_evaluator_k(self.k(), bits_per_seed, slice_len, loading_factor_1000),
            core: self.core,
        }
    }

    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16, loading_factor_1000: u16) -> Weights {
        self.seed_evaluator.bucket_evaluator_k(self.k(), bits_per_seed, slice_len, loading_factor_1000)
    }

    fn slice_len(&self, output_range: usize, bits_per_seed: u8, preferred_slice_len: u16) -> u16 {
        let max_res = match (bits_per_seed, self.k(), output_range.saturating_sub(self.extra_shift(bits_per_seed) as usize)) {
            (_, _, n @ 0..64) => (n/2+1).next_power_of_two() as u16,
            (_, _, 64..1300) => 64,
            (_, _, 1300..9500) => 128,
            _ if preferred_slice_len != 0 => preferred_slice_len,

            (..=5, _, _) => 256,

            (..=6, ..=4, _) => 1024,    // TODO check exact k border
            (..=6, ..=12, _) => 512,    // TODO check exact k border
            (..=6, _, _) => 256,

            (7, ..=5, _) => 1024,   // TODO check exact k border
            (7, ..=12, _) => 512,   // TODO check exact k border
            (7, _, _) => 256,

            (8, ..=2, _) => 2048,   // TODO check exact k border
            (8, ..=5, _) => 1024,   // TODO check exact k border
            (8, ..=12, _) => 512,   // TODO check exact k border
            (8, _, _) => 256,

            (9, ..=2, _) => 2048,   // TODO check exact k border
            (9, ..=3, _) => 1024,   // TODO check exact k border
            (9, ..=8, _) => 512,   // TODO check exact k border
            (9, _, _) => 256,

            (_, ..=2, _) => 2048,   // TODO check exact k border
            (_, ..=3, _) => 1024,   // TODO check exact k border
            (_, ..=8, _) => 512,   // TODO check exact k border
            (_, _, _) => 256,
        };
        if preferred_slice_len != 0 { max_res.min(preferred_slice_len) } else { max_res }
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
const PROD_S2_L128: [(u16, [i32; 7], ProdOfValuesKEval); 3] = [   // for W=512 50M keys
    (10, [0, 64688, 70048, 76232, 317420, 368056, 396996], P{value_shift: 0.005121, free_shift: 2.533247, window_weight: 0.571104}), // 10.88% for 1.6 λ=7.69 TODO moreit
    (100, [0, 64408, 71692, 82100, 306916, 380348, 389972], P{value_shift: 0.005049, free_shift: 2.344172, window_weight: 0.692045}), // 4.60% for 1.6 λ=55.85 100M
    (1000, [0, 64552, 69456, 76080, 309128, 386592, 389752], P{value_shift: 0.005220, free_shift: 2.573802, window_weight: 0.544201}) // 2.30% for 1.6 λ=365.88 100M
];
const PROD_S2_L256: [(u16, [i32; 7], ProdOfValuesKEval); 3] = [   // for W=512 50M keys
    (10, [0, 11032, 14184, 14216, 84236, 107388, 179000], P{value_shift: 0.008705, free_shift: 1.760856, window_weight: 0.998068}), // 11.04% for 1.6 λ=7.69
    (100, [0, 28908, 29948, 30104, 45028, 51476, 66968], P{value_shift: 0.008489, free_shift: 3.326709, window_weight: 0.825169}), // 4.45% for 1.6 λ=49.65 100M
    (1000, [0, 17480, 18364, 20648, 27124, 29872, 33528], P{value_shift: 0.007692, free_shift: 3.461758, window_weight: 0.734415}) // 2.19% for 1.6 λ=365.88 100M
];
const PROD_S3_L128: [(u16, [i32; 7], ProdOfValuesKEval); 3] = [   // for W=512 50M keys
    (10, [0, 123796, 133960, 134028, 222728, 248664, 262096], P{value_shift: 0.006407, free_shift: 1.852792, window_weight: 0.544414}), // 4.63% for 1.8 λ=8.65
    (100, [0, 589100, 628300, 737152, 775180, 833620, 837312], P{value_shift: 0.005361, free_shift: 2.155627, window_weight: 0.877523}), // 1.83% for 1.8 λ=55.85 100M
    (1000, [0, 639084, 671532, 801232, 821980, 850416, 851592], P{value_shift: 0.005511, free_shift: 1.722081, window_weight: 0.950049}) // 1.33% for 1.8 λ=411.62 100M TODO moreit
];
const PROD_S3_L256: [(u16, [i32; 7], ProdOfValuesKEval); 3] = [   // for W=512 50M keys
    (10, [0, 51324, 52128, 60596, 91960, 93556, 130408], P{value_shift: 0.009163, free_shift: 1.808156, window_weight: 0.882559}), // 4.57% for 1.8 λ=8.65
    (100, [0, 717580, 748188, 838576, 865008, 910204, 915688], P{value_shift: 0.005281, free_shift: 2.190872, window_weight: 0.925006}), // 1.63% for 1.8 λ=55.85 100M
    (1000, [0, 878116, 930900, 943840, 961288, 989388, 991032], P{value_shift: 0.006121, free_shift: 1.462251, window_weight: 0.751971}) // 1.09% for 1.8 λ=411.62 100M
];
const PROD_S4_L128: [(u16, [i32; 7], ProdOfValuesKEval); 3] = [   // for W=512 40M keys
    (10, [0, 198620, 212004, 213900, 245684, 246284, 255168], P{value_shift: 0.005268, free_shift: 1.706588, window_weight: 0.533021}), // 2.46% for 2.2 λ=10.58 TODO moreit
    (100, [0, 618712, 652408, 765792, 789436, 822432, 824192], P{value_shift: 0.005340, free_shift: 1.776102, window_weight: 0.956766}), // 0.99% for 2.2 λ=68.27 100M
    (1000, [0, 617220, 658548, 787484, 805820, 831760, 832508], P{value_shift: 0.005449, free_shift: 1.732168, window_weight: 0.952813}) // 1.02% for 2.2 λ=503.09 100M TODO moreit
];
const PROD_S4_L256: [(u16, [i32; 7], ProdOfValuesKEval); 3] = [   // for W=512 40M keys
    (10, [0, 40044, 51596, 52248, 74584, 86456, 103080], P{value_shift: 0.008509, free_shift: 1.931638, window_weight: 0.942058}), // 2.32% for 2.2 λ=10.58
    (100, [0, 708660, 773296, 872740, 887092, 912436, 915372], P{value_shift: 0.005271, free_shift: 2.145369, window_weight: 0.935827}), // 0.80% for 2.2 λ=68.27 100M
    (1000, [0, 838980, 904684, 907328, 923204, 949448, 950436], P{value_shift: 0.005819, free_shift: 1.458893, window_weight: 0.742259}) // 0.78% for 2.2 λ=503.09 100M
];
const PROD_S5_L128: [(u16, [i32; 7], ProdOfValuesKEval); 3] = [   // for W=512 20M keys
    (10, [0, 504632, 508232, 511288, 520792, 520872, 524108], P{value_shift: 0.004527, free_shift: 1.444326, window_weight: 0.726941}), // 1.07% for 2.5 λ=12.02
    (100, [0, 608064, 641424, 764004, 783160, 810564, 811392], P{value_shift: 0.005259, free_shift: 1.771463, window_weight: 0.959031}), // 0.30% for 2.4 λ=74.47 100M TODO moreit
    (1000, [0, 610360, 640648, 770048, 788968, 815964, 816552], P{value_shift: 0.005305, free_shift: 1.736112, window_weight: 0.954771}) // 0.75% for 2.4 λ=548.83 100M
];
const PROD_S5_L256: [(u16, [i32; 7], ProdOfValuesKEval); 3] = [   // for W=512 20M keys
    (10, [0, 72216, 72592, 72888, 85184, 87476, 93616], P{value_shift: 0.006306, free_shift: 1.509232, window_weight: 0.655954}), // 0.90% for 2.5 λ=12.02
    (100, [0, 715592, 794172, 838744, 854380, 878004, 879052], P{value_shift: 0.005140, free_shift: 1.706599, window_weight: 0.942970}), // 0.25% for 2.4 λ=74.47 100M
    (1000, [0, 710548, 776444, 816320, 831220, 853652, 854200], P{value_shift: 0.005349, free_shift: 1.711154, window_weight: 0.917018}), // 0.54% for 2.4 λ=548.83 100M
];
//S5_L512:
//10 (0.91% for 2.5): 0, 69916, 71414, 73883, 80921, 84423, 86831  value_shift: 0.003853, free_shift: 1.616938, window_weight: 0.866771
const PROD_S6_L128: [(u16, [i32; 7], ProdOfValuesKEval); 5] = [   // for W=512 16M keys
    (10, [0, 512112, 530688, 554360, 567900, 576904, 578600], P{value_shift: 0.004160, free_shift: 1.367330, window_weight: 0.844929}), // 0.84% for 3.1 λ=14.91
    (16, [0, 694292, 855628, 900640, 915272, 929684, 930752], P{value_shift: 0.004107, free_shift: 1.376566, window_weight: 0.838948}), // 0.68% for 3.1 λ=21.46
    (32, [0, 675016, 812400, 876988, 891096, 907300, 908140], P{value_shift: 0.004362, free_shift: 1.464215, window_weight: 0.861123}), // 0.51% for 3.1 λ=37.37
    (100, [0, 590128, 629880, 757528, 775864, 802400, 802920], P{value_shift: 0.005213, free_shift: 1.783900, window_weight: 0.953582}), // 0.30% for 3.0 λ=93.09 100M
    (1000, [0, 589732, 625032, 754748, 772528, 797892, 798300], P{value_shift: 0.005302, free_shift: 1.811741, window_weight: 0.952016}), // 0.73% for 3.0 λ=686.04 100M
];
const PROD_S6_L256: [(u16, [i32; 7], ProdOfValuesKEval); 7] = [   // for W=512 16M keys
    (6, [0, 159720, 165732, 166280, 179900, 180540, 186992], P{value_shift: 0.005627, free_shift: 1.530868, window_weight: 0.768346}), // 0.94% for 3.1 λ=10.17
    (8, [0, 167776, 172912, 175528, 190136, 196592, 199736], P{value_shift: 0.005545, free_shift: 1.350731, window_weight: 0.789078}), // 0.81% for 3.1 λ=12.59
    (10, [0, 673012, 782588, 790000, 805640, 820644, 822812], P{value_shift: 0.004284, free_shift: 1.308742, window_weight: 0.854566}), // 0.74% for 3.1 λ=14.91
    (16, [0, 664864, 807656, 844304, 856392, 868836, 870036], P{value_shift: 0.004008, free_shift: 1.303037, window_weight: 0.912569}), // 0.60% for 3.1 λ=21.46
    (32, [0, 656316, 784448, 820056, 832036, 845440, 846476], P{value_shift: 0.004284, free_shift: 1.395468, window_weight: 0.961233}), // 0.45% for 3.1 λ=37.37
    (100, [0, 712216, 786716, 840984, 851184, 867060, 867752], P{value_shift: 0.005150, free_shift: 1.719539, window_weight: 0.931391}), // 0.22% for 3.0 λ=93.09 100M
    (1000, [0, 712964, 773164, 822152, 831648, 846016, 846408], P{value_shift: 0.005307, free_shift: 1.713287, window_weight: 0.906782}), // 0.53% for 3.0 λ=686.04 100M
];
const PROD_S6_L512: [(u16, [i32; 7], ProdOfValuesKEval); 6] = [   // for W=512 16M keys
    (2, [0, 255532, 455740, 550424, 598896, 647848, 673728], P{value_shift: 0.017016, free_shift: 1.588817, window_weight: 0.167693}), // 1.43% for 3.2 λ=4.89
    (4, [0, 151444, 151904, 164168, 237744, 286108, 298748], P{value_shift: 0.008497, free_shift: 1.508332, window_weight: 0.297361}), // 1.28% for 3.2 λ=7.83
    (6, [0, 232480, 239704, 241528, 296708, 307148, 315184], P{value_shift: 0.006723, free_shift: 1.342556, window_weight: 0.269844}), // 0.88% for 3.1 λ=10.17
    (8, [0, 210684, 211484, 211948, 223892, 227504, 231692], P{value_shift: 0.004438, free_shift: 1.332068, window_weight: 0.823758}), // 0.79% for 3.1 λ=12.59
    (10, [0, 649232, 753604, 779028, 795544, 811072, 813168], P{value_shift: 0.003758, free_shift: 1.241938, window_weight: 0.887839}), // 0.73% for 3.1 λ=14.91
    (100, [0, 744128, 805040, 821764, 826864, 833220, 834052], P{value_shift: 0.003703, free_shift: 1.334236, window_weight: 0.863755}), // 0.23% for 3.0 λ=93.09 100M
];
const PROD_S6_L1024: [(u16, [i32; 7], ProdOfValuesKEval); 2] = [   // for W=512 16M keys
    (2, [0, 934992, 1263864, 1435288, 1538140, 1628088, 1675028], P{value_shift: 0.003863, free_shift: 1.486350, window_weight: 0.079398}),  // 1.31% for 3.2 λ=4.89
    (4, [0, 280664, 294180, 301892, 441092, 563960, 584812], P{value_shift: 0.007904, free_shift: 1.470643, window_weight: 0.229561}),  // 1.27% for 3.2 λ=7.83
];
/*const PROD_S7_L64: [(u16, [i32; 7], ProdOfValuesKEval); 11] = [   // for W=512 15M keys
    (2, [0, 45736, 62352, 73252, 78912, 86740, 89868], P{value_shift: 0.014315, free_shift: 1.751797, window_weight: 0.278672})  // 2.30% for 3.7 λ=5.66
];*/
const PROD_S7_L128: [(u16, [i32; 7], ProdOfValuesKEval); 13] = [   // for W=512 15M keys S8 params work excellent for k=16, 32; and OK+ for k=48, 8, 4, 2, weak for k=64, 100
    (2, [0, 70164, 93104, 124268, 132544, 143632, 150396], P{value_shift: 0.012291, free_shift: 1.788075, window_weight: 0.318863}), // 1.55% for 3.7 λ=5.66
    (3, [0, 22536, 27556, 36252, 50904, 60296, 65072], P{value_shift: 0.018230, free_shift: 1.566620, window_weight: 0.539185}), // 1.30% for 3.7 λ=7.42
    (4, [0, 108324, 109576, 109624, 120724, 122152, 126116], P{value_shift: 0.008558, free_shift: 1.485461, window_weight: 0.760050}), // 1.19% for 3.7 λ=9.06
    (5, [0, 98568, 103340, 104444, 124240, 124304, 127604], P{value_shift: 0.008258, free_shift: 1.680940, window_weight: 0.738092}), // 1.05% for 3.7 λ=10.63
    (6, [0, 198456, 204072, 204208, 217124, 218268, 220332], P{value_shift: 0.007186, free_shift: 1.454377, window_weight: 0.806906}), // 0.95% for 3.7 λ=12.13
    (8, [0, 454160, 466484, 474724, 484496, 490272, 491684], P{value_shift: 0.004061, free_shift: 1.383542, window_weight: 0.815084}), // 0.82% for 3.7 λ=15.03
    (10, [0, 508744, 547980, 568308, 577976, 584624, 585664], P{value_shift: 0.004077, free_shift: 1.379174, window_weight: 0.828238}), // 0.73% for 3.7 λ=17.80
    (16, [0, 650844, 801316, 857656, 867680, 877804, 878520], P{value_shift: 0.004117, free_shift: 1.382515, window_weight: 0.828996}),  // 0.61% for 3.7 λ=25.62
    (32, [0, 603448, 717416, 797092, 823272, 853832, 854296], P{value_shift: 0.004476, free_shift: 1.529438, window_weight: 0.910847}), // 0.37% for 3.6 λ=43.39 35M
    (48, [0, 616476, 684048, 782004, 805996, 833196, 833616], P{value_shift: 0.004718, free_shift: 1.613400, window_weight: 0.902932}), // 0.32% for 3.6 λ=60.49 50M
    (64, [0, 612188, 665188, 774836, 792008, 812216, 812600], P{value_shift: 0.004819, free_shift: 1.688580, window_weight: 0.922953}), // 0.29% for 3.6 λ=76.80 70M
    (100, [0, 574748, 619488, 749584, 766460, 790580, 790900], P{value_shift: 0.005121, free_shift: 1.822026, window_weight: 0.934671}), // 0.26% for 3.6 λ=111.71 100M
    (1000, [0, 607036, 641296, 778188, 795180, 819900, 820204], P{value_shift: 0.005657, free_shift: 1.721928, window_weight: 0.871389}), // 0.68% for 3.6 λ=823.25 100M
];
const PROD_S7_L256: [(u16, [i32; 7], ProdOfValuesKEval); 13] = [   // for W=512 15M keys S8 params work excellent for k=32, 16, 8, 4; and OK+ for k=48, 2, OK k=64
    (2, [0, 230780, 230896, 230932, 244700, 246628, 250272], P{value_shift: 0.001943, free_shift: 1.965737, window_weight: 0.954503}), // 2.05% for 3.7 λ=5.66
    (3, [0, 230760, 230892, 230920, 244676, 246600, 250244], P{value_shift: 0.001991, free_shift: 1.924222, window_weight: 0.954355}), // 1.49% for 3.7 λ=7.42
    (4, [0, 230740, 230884, 230908, 244648, 246568, 250216], P{value_shift: 0.002039, free_shift: 1.701494, window_weight: 0.954207}), // 1.11% for 3.7 λ=9.06
    (5, [0, 250308, 254872, 261548, 292260, 293356, 297384], P{value_shift: 0.004076, free_shift: 1.290662, window_weight: 0.206234}), // 0.85% for 3.7 λ=10.63
    (6, [0, 340564, 341868, 348780, 367236, 367332, 370308], P{value_shift: 0.004932, free_shift: 1.269037, window_weight: 0.264518}), // 0.78% for 3.7 λ=12.13
    (8, [0, 661996, 775984, 786428, 798588, 810264, 811752], P{value_shift: 0.004170, free_shift: 1.272015, window_weight: 0.862649}), // 0.69% for 3.7 λ=15.03
    (10, [0, 649564, 770052, 788520, 799728, 810760, 811900], P{value_shift: 0.004254, free_shift: 1.272361, window_weight: 0.872906}), // 0.62% for 3.7 λ=17.80
    (16, [0, 672040, 817312, 867144, 875312, 883772, 884468], P{value_shift: 0.004016, free_shift: 1.275868, window_weight: 0.895949}),  // 0.53% for 3.7 λ=25.62
    (32, [0, 641800, 726824, 781408, 791780, 802408, 803024], P{value_shift: 0.004503, free_shift: 1.387469, window_weight: 0.878155}), // 0.30% for 3.6 λ=43.39 35M
    (48, [0, 644504, 787220, 818768, 828808, 840220, 840744], P{value_shift: 0.004743, free_shift: 1.456763, window_weight: 0.896127}), // 0.26% for 3.6 λ=60.49 50M
    (64, [0, 693012, 819164, 874720, 884196, 895120, 895588], P{value_shift: 0.004575, free_shift: 1.494077, window_weight: 0.899156}), // 0.23% for 3.6 λ=76.80 70M
    (100, [0, 697392, 769172, 828492, 836400, 848636, 849160], P{value_shift: 0.005116, free_shift: 1.720470, window_weight: 0.922365}), // 0.20% for 3.6 λ=111.71 100M
    (1000, [0, 696940, 763624, 823600, 829840, 839480, 839752], P{value_shift: 0.005183, free_shift: 1.756795, window_weight: 0.912032}), // 0.49% for 3.6 λ=823.25 100M
];
const PROD_S7_L512: [(u16, [i32; 7], ProdOfValuesKEval); 14] = [   // for W=512 15M keys  S8 params work excellent for k=64, 48, 32, 16, 8; and OK for k=4, 2
    (2, [0, 214936, 363352, 427120, 484612, 519944, 538540], P{value_shift: 0.015365, free_shift: 1.708053, window_weight: 0.152936}), // 0.89% for 3.7 λ=5.66
    (3, [0, 234516, 314624, 349196, 417500, 462044, 474612], P{value_shift: 0.015128, free_shift: 1.493484, window_weight: 0.130663}), // 0.86% for 3.7 λ=7.42
    (4, [0, 440204, 464708, 470184, 511764, 549516, 559920], P{value_shift: 0.006757, free_shift: 1.596317, window_weight: 0.261390}), // 0.84% for 3.7 λ=9.06
    (5, [0, 307292, 311048, 311820, 344180, 358168, 363652], P{value_shift: 0.005116, free_shift: 1.382913, window_weight: 0.410752}),  // 0.77% for 3.7 λ=10.63
    (6, [0, 405396, 435540, 444904, 471680, 485412, 489640], P{value_shift: 0.004703, free_shift: 1.356749, window_weight: 0.568669}), // 0.72% for 3.7 λ=12.13
    (7, [0, 191476, 206964, 209780, 224844, 227496, 230276], P{value_shift: 0.005019, free_shift: 1.277399, window_weight: 0.667103}), // 0.67% for 3.7 λ=13.60
    (8, [0, 609428, 747680, 777688, 792392, 806836, 808264], P{value_shift: 0.003710, free_shift: 1.230614, window_weight: 0.873711}), // 0.67% for 3.7 λ=15.03
    (9, [0, 625628, 755104, 768240, 780412, 792112, 793208], P{value_shift: 0.003667, free_shift: 1.225008, window_weight: 0.888079}), // 0.62% for 3.7 λ=16.42
    (10, [0, 649924, 768772, 792584, 803968, 815260, 816332], P{value_shift: 0.003703, free_shift: 1.218465, window_weight: 0.875453}), // 0.61% for 3.7 λ=17.80
    (16, [0, 672536, 815668, 834452, 835660, 837180, 837332], P{value_shift: 0.003667, free_shift: 1.182518, window_weight: 0.876810}),  // 0.59% for 3.7 λ=25.62
    (32, [0, 721088, 799512, 841944, 851148, 861704, 862784], P{value_shift: 0.003654, free_shift: 1.134473, window_weight: 0.914314}), // 0.61% for 3.7 λ=44.60
    (48, [0, 711096, 785048, 821216, 829824, 840396, 841452], P{value_shift: 0.003662, free_shift: 1.209650, window_weight: 0.933524}), // 0.63% for 3.7 λ=62.17
    (64, [0, 738308, 802536, 842972, 851416, 861968, 863012], P{value_shift: 0.003648, free_shift: 1.232832, window_weight: 0.915246}), // 0.68% for 3.7 λ=78.94
    (100, [0, 764552, 824220, 844020, 850776, 858880, 859800], P{value_shift: 0.003571, free_shift: 1.288549, window_weight: 0.864340}), // 0.83% for 3.7 λ=114.82
];
const PROD_S7_L1024: [(u16, [i32; 7], ProdOfValuesKEval); 6] = [    // for W=512 15M keys
    (2, [0, 776460, 1051616, 1194464, 1280000, 1361752, 1397700], P{value_shift: 0.003744, free_shift: 1.613469, window_weight: 0.083098}), // 0.75% for 3.7 λ=5.66
    (3, [0, 423860, 532116, 615536, 727584, 812872, 833736], P{value_shift: 0.007560, free_shift: 1.495320, window_weight: 0.101414}), // 0.80% for 3.7 λ=7.42
    (4, [0, 337156, 343940, 351264, 431532, 500884, 515972], P{value_shift: 0.007127, free_shift: 1.433053, window_weight: 0.232233}), // 0.78% for 3.7 λ=9.06
    (6, [0, 437728, 567832, 595096, 616556, 631104, 637308], P{value_shift: 0.005361, free_shift: 1.322542, window_weight: 0.513551}), // 0.73% for 3.7 λ=12.13
    (8, [0, 502292, 698644, 815344, 836076, 854544, 856672], P{value_shift: 0.003374, free_shift: 1.226815, window_weight: 0.843214}), // 0.69% for 3.7 λ=15.03
    (10, [0, 527484, 725748, 848760, 853320, 857592, 857716], P{value_shift: 0.003292, free_shift: 1.199457, window_weight: 0.844621}), // 0.67% for 3.7 λ=17.80
];

const PROD_S8_L32: [(u16, [i32; 7], ProdOfValuesKEval); 2] = [   // for W=512 10M keys
    (500, [0, 519380, 542504, 672436, 683712, 701040, 701204], P{value_shift: 0.005704, free_shift: 2.072629, window_weight: 0.917840}),  // moreit 1.49% for 4.3 λ=533.97
    (1000, [0, 564580, 583960, 701488, 713864, 732308, 732584], P{value_shift: 0.005780, free_shift: 2.083312, window_weight: 0.841375}), // moreit 1.34% for 4.3 λ=655.35
];
const PROD_S8_L64: [(u16, [i32; 7], ProdOfValuesKEval); 4] = [   // for W=512 10M keys
    (64, [0, 497452, 568388, 694464, 706212, 722780, 722892], P{value_shift: 0.005643, free_shift: 1.984586, window_weight: 0.871503}), // 0.49% for 4.3 λ=91.74
    (100, [0, 514288, 536064, 664924, 676004, 692632, 692752], P{value_shift: 0.005566, free_shift: 2.037221, window_weight: 0.914972}),   // 0.50% for 4.3 λ=133.44
    (500, [0, 516472, 548092, 672604, 683876, 700740, 700948], P{value_shift: 0.005649, free_shift: 2.043347, window_weight: 0.928031}), // 1.09% for 4.3 λ=533.97
    (1000, [0, 516896, 560732, 667944, 679204, 696052, 696268], P{value_shift: 0.005881, free_shift: 1.988958, window_weight: 0.937083}), // 0.99% for 4.3 λ=655.35
    // W=1024, k=1000: 0.99%  0, 130889, 141308, 169391, 172222, 176429, 176482  value_shift: 0.006105, free_shift: 1.994368, window_weight: 0.935901
];
const PROD_S8_L128: [(u16, [i32; 7], ProdOfValuesKEval); 17] = [   // for W=512 10M keys
    (2, [0, 27172, 47036, 60444, 72348, 84164, 88620], P{value_shift: 0.013562, free_shift: 1.844853, window_weight: 0.600869}), // 1.43% for 4.3 λ=6.58
    (3, [0, 59240, 60052, 66164, 76400, 79060, 83064], P{value_shift: 0.008849, free_shift: 1.500312, window_weight: 0.637867}), // 1.19% for 4.3 λ=8.62
    (4, [0, 109340, 109388, 109900, 116440, 117348, 119700], P{value_shift: 0.008588, free_shift: 1.461835, window_weight: 0.755033}),   // 1.05% for 4.3 λ=10.53
    (5, [0, 318920, 322720, 323588, 330824, 331372, 333196], P{value_shift: 0.004317, free_shift: 1.422497, window_weight: 0.939602}), // 0.96% for 4.3 λ=12.35
    (6, [0, 461528, 481340, 490624, 497992, 501408, 502800], P{value_shift: 0.004301, free_shift: 1.399241, window_weight: 0.813602}), // 0.85% for 4.3 λ=14.10
    (8, [0, 651396, 793784, 844992, 857980, 870464, 871532], P{value_shift: 0.004086, free_shift: 1.389829, window_weight: 0.824165}), // 0.74% for 4.3 λ=17.47
    (10, [0, 678492, 826056, 868240, 879236, 890064, 890884], P{value_shift: 0.004067, free_shift: 1.375535, window_weight: 0.806195}), // 0.67% for 4.3 λ=20.68
    (16, [0, 647872, 789048, 853800, 861668, 869640, 870160], P{value_shift: 0.004056, free_shift: 1.396727, window_weight: 0.817199}),  // 0.55% for 4.3 λ=29.77
    (20, [0, 652892, 778496, 856780, 863440, 870208, 870632], P{value_shift: 0.004192, free_shift: 1.432003, window_weight: 0.835094}), // 0.50% for 4.3 λ=35.52
    (24, [0, 651720, 758720, 854516, 860028, 865728, 866044], P{value_shift: 0.004318, free_shift: 1.361641, window_weight: 0.855886}),  // 0.47% for 4.3 λ=41.09
    (32, [0, 626152, 724592, 820120, 827828, 836264, 836564], P{value_shift: 0.004403, free_shift: 1.443351, window_weight: 0.869070}),  // 0.43% for 4,3 λ=51.83
    (40, [0, 625580, 716604, 816984, 824576, 832968, 833224], P{value_shift: 0.004449, free_shift: 1.510469, window_weight: 0.869724}),  // 0.40% for 4.3 λ=62.19
    (48, [0, 626180, 696836, 808712, 817880, 828068, 828348], P{value_shift: 0.004499, free_shift: 1.564914, window_weight: 0.879130}),  // 0.39% for 4.3 λ=72.26
    (64, [0, 610816, 665448, 781868, 797528, 815256, 815488], P{value_shift: 0.004627, free_shift: 1.696177, window_weight: 0.903999}), // 0.26% for 4.2 λ=89.60 70M
    (100, [0, 573532, 596296, 748156, 764608, 786052, 786292], P{value_shift: 0.004977, free_shift: 1.857305, window_weight: 0.868128}), // 0.38% for 4.3 λ=133.44
    (500, [0, 562976, 590652, 735988, 750656, 770268, 770476], P{value_shift: 0.005116, free_shift: 1.913994, window_weight: 0.859051}), // 0.97% for 4.3 λ=533.97
    (1000, [0, 574452, 609000, 755020, 762140, 771700, 771740], P{value_shift: 0.005362, free_shift: 1.934122, window_weight: 0.865017}), // 1.25% for 4.3 λ=655.35
    // W=1024, k=1000: 1.25%  0, 143722, 152365, 188897, 190678, 193070, 193080  value_shift: 0.005362, free_shift: 1.934127, window_weight: 0.885969
];
const PROD_S8_L256: [(u16, [i32; 7], ProdOfValuesKEval); 18] = [   // for W=512 10M keys
    (2, [0, 230800, 230924, 230936, 244684, 246608, 250252], P{value_shift: 0.001943, free_shift: 1.966776, window_weight: 0.954382}), // 1.64% for 4.3 λ=6.58
    (3, [0, 230768, 230920, 230928, 244668, 246584, 250232], P{value_shift: 0.001988, free_shift: 1.786913, window_weight: 0.954204}), // 1.13% for 4.3 λ=8.62
    (4, [0, 230740, 230892, 230912, 244644, 246564, 250208], P{value_shift: 0.001942, free_shift: 1.701445, window_weight: 0.954089}), // 0.86% for 4.3 λ=10.53
    (5, [0, 206172, 212828, 213568, 223808, 225540, 228440], P{value_shift: 0.003041, free_shift: 1.412530, window_weight: 0.961096}), // 0.73% for 4.3 λ=12.35
    (6, [0, 380216, 408460, 424100, 433732, 438172, 440032], P{value_shift: 0.003301, free_shift: 1.326613, window_weight: 0.918017}), // 0.67% for 4.3 λ=14.10
    (8, [0, 666788, 774488, 825768, 837264, 848660, 849696], P{value_shift: 0.004140, free_shift: 1.255893, window_weight: 0.875665}), // 0.59% for 4.3 λ=17.47
    (9, [0, 666852, 796552, 831380, 841352, 851180, 852044], P{value_shift: 0.004204, free_shift: 1.256947, window_weight: 0.871172}), // 0.56% for 4.3 λ=19.09
    //(10, [0, 668732, 802596, 842756, 851436, 860372, 861068], P{value_shift: 0.003928, free_shift: 1.181003, window_weight: 0.881245}), // 0.83% for 4.5 λ=21.64; 0.54% for 4.3 λ=20.68
    (10, [0, 662160, 787616, 836808, 845332, 853708, 854404], P{value_shift: 0.004054, free_shift: 1.257445, window_weight: 0.866225}), // 0.54% for 4.3 λ=20.68; 0.83% for 4.5 λ=21.64
    (12, [0, 664408, 798388, 848476, 857012, 865572, 866276], P{value_shift: 0.004230, free_shift: 1.262666, window_weight: 0.869147}), // 0.51% for 4.3 λ=23.79
    (14, [0, 645452, 806436, 845504, 852916, 860696, 861308], P{value_shift: 0.004126, free_shift: 1.294066, window_weight: 0.887341}), // 0.49% for 4.3 λ=26.82
    (16, [0, 649572, 789072, 845544, 852716, 860436, 861044], P{value_shift: 0.004013, free_shift: 1.275321, window_weight: 0.871179}), // 0.45% for 4.3 λ=29.77 20M
    (24, [0, 657020, 813608, 857892, 864560, 871600, 872064], P{value_shift: 0.004278, free_shift: 1.300274, window_weight: 0.854534}), // 0.43% for 4.3 λ=41.09
    (32, [0, 677052, 814032, 866684, 873532, 881044, 881500], P{value_shift: 0.004380, free_shift: 1.335272, window_weight: 0.873470}), // 0.41% for 4.3 λ=51.83
    (40, [0, 679864, 826956, 881512, 889028, 897696, 898172], P{value_shift: 0.004527, free_shift: 1.371665, window_weight: 0.877259}), // 0.40% for 4.3 λ=62.19
    (48, [0, 662644, 804156, 849796, 856852, 864924, 865336], P{value_shift: 0.004588, free_shift: 1.396611, window_weight: 0.891372}), // 0.40% for 4.3 λ=72.26
    (64, [0, 686456, 767180, 844692, 850692, 857616, 857920], P{value_shift: 0.004615, free_shift: 1.471455, window_weight: 0.886417}), // 0.20% for 4.2 λ=89.60 70M
    (100, [0, 662016, 730964, 783168, 787876, 794888, 795152], P{value_shift: 0.005175, free_shift: 1.717621, window_weight: 0.921655}), // 0.14% for 4.1 λ=127.23 100M keys
    (1000, [0, 620560, 682208, 723532, 732956, 746168, 746404], P{value_shift: 0.005581, free_shift: 1.894030, window_weight: 0.716864}), // 0.42% for 4.0 λ=914.72 100M
];
const PROD_S8_L512: [(u16, [i32; 7], ProdOfValuesKEval); 16] = [   // for W=512 10M keys
    (2, [0, 305892, 374152, 400144, 445620, 497836, 510232], P{value_shift: 0.00968, free_shift: 1.94309, window_weight: 0.28871}), // 1.28% for 4.5 λ=6.88
    (3, [0, 172836, 190872, 191396, 227056, 251532, 260008], P{value_shift: 0.009677, free_shift: 1.317506, window_weight: 0.310870}), // 1.18% for 4.5 λ=9.02
    (4, [0, 499324, 501180, 503624, 539244, 577424, 581612], P{value_shift: 0.00668, free_shift: 1.29751, window_weight: 0.27419}), // 1.12% for 4.5 λ=11.02
    (5, [0, 238856, 242864, 243384, 253440, 258756, 262436], P{value_shift: 0.007691, free_shift: 1.253225, window_weight: 0.348958}), // 0.63% for 4.3 λ=12.35
    (6, [0, 596508, 643880, 685784, 708224, 731440, 733568], P{value_shift: 0.005145, free_shift: 1.194641, window_weight: 0.584520}), //0.99% for 4.5 λ=14.76
    (7, [0, 619132, 724996, 749996, 766640, 784024, 785428], P{value_shift: 0.004508, free_shift: 1.234478, window_weight: 0.692905}), //0.57% for 4.3 λ=15.81
    (8, [0, 653032, 811024, 864460, 878972, 893864, 895240], P{value_shift: 0.00364, free_shift: 1.21311, window_weight: 0.85403}), // 0.92% for 4.5 λ=18.28
    (10, [0, 669904, 809764, 848416, 855996, 863820, 864416], P{value_shift: 0.003798, free_shift: 1.172512, window_weight: 0.871241}), // 0.88% for 4.5 λ=21.64
    (12, [0, 679116, 839008, 852632, 853164, 853732, 853768], P{value_shift: 0.003807, free_shift: 1.141909, window_weight: 0.857620}), // 0.86% for 4.5 λ=24.90
    (16, [0, 680428, 824916, 842304, 843320, 844584, 844720], P{value_shift: 0.00365, free_shift: 1.12751, window_weight: 0.87378}), // 0.85% for 4.5 λ=31.16
    (20, [0, 691704, 778060, 830900, 838232, 847112, 847972], P{value_shift: 0.003568, free_shift: 1.125351, window_weight: 0.905329}), // 0.85% for 4.5 λ=37.17
    (24, [0, 716148, 793916, 842420, 851036, 861440, 862496], P{value_shift: 0.003623, free_shift: 1.118238, window_weight: 0.914403}), // 0.83% for 4.5 λ=43.00
    (32, [0, 718824, 796884, 843460, 852576, 863536, 864652], P{value_shift: 0.003632, free_shift: 1.128020, window_weight: 0.916687}), // 0.79% for 4.5 λ=54.24
    (48, [0, 718132, 792320, 832800, 841376, 851976, 853024], P{value_shift: 0.003635, free_shift: 1.173609, window_weight: 0.925055}), // 0.77% for 4.5 λ=75.62
    (64, [0, 726124, 792748, 830912, 839272, 849748, 850796], P{value_shift: 0.003652, free_shift: 1.229707, window_weight: 0.913129}), // 0.79% for 4.5 λ=96.01
    (100, [0, 752944, 798508, 833352, 839248, 846324, 847160], P{value_shift: 0.003562, free_shift: 1.285305, window_weight: 0.862204}), // 0.91% for 4.5 λ=139.64
];
const PROD_S8_L1024: [(u16, [i32; 7], ProdOfValuesKEval); 8] = [   // for W=512 10M keys
    (2, [0, 499412, 745940, 871788, 954712, 1043960, 1064836], P{value_shift: 0.005040, free_shift: 1.543640, window_weight: 0.101565}), // 1.15% for 4.5 λ=6.88
    (3, [0, 296492, 337172, 395728, 506076, 575324, 587808], P{value_shift: 0.006193, free_shift: 1.334455, window_weight: 0.160041}), // 1.15% for 4.5 λ=9.02
    (4, [0, 532804, 753740, 830640, 880628, 914000, 922520], P{value_shift: 0.004811, free_shift: 1.312647, window_weight: 0.303816}), // 1.15% for 4.5 λ=11.02
    (5, [0, 576176, 805636, 858660, 884056, 900348, 904724], P{value_shift: 0.004140, free_shift: 1.232151, window_weight: 0.559441}), // 1.09% for 4.5 λ=12.92
    (6, [0, 657900, 976144, 1080216, 1084956, 1088840, 1090988], P{value_shift: 0.003458, free_shift: 1.157448, window_weight: 0.813080}), // 1.05% for 4.5 λ=14.76
    (7, [0, 509600, 656592, 766564, 789164, 811972, 814120], P{value_shift: 0.004036, free_shift: 1.244399, window_weight: 0.696917}), // 0.70% for 4.3 λ=15.81
    //(8, [0, 530228, 734016, 869384, 961824, 1030096, 1074212], P{value_shift: 0.00324, free_shift: 2.79127, window_weight: 0.76304}), // 1.56% for 4.5 λ=18.28
    //(10, [0, 522204, 695824, 834356, 917732, 986336, 1025832], P{value_shift: 0.003607, free_shift: 3.014842, window_weight: 0.799166}), // 1.42% for 4.5 λ=21.64
    (16, [0, 528952, 768504, 914560, 985476, 1054972, 1082704], P{value_shift: 0.003030, free_shift: 3.179457, window_weight: 0.781803}), // 1.15% for 4.5 λ=31.16
    (32, [0, 533180, 771512, 870428, 932852, 989976, 1004388], P{value_shift: 0.002571, free_shift: 3.173833, window_weight: 0.733757}), // 0.94% for 4.5 λ=54.24
];
const PROD_S8_L2048: [(u16, [i32; 7], ProdOfValuesKEval); 6] = [   // for W=512 10M keys
    (2, [0, 444884, 669176, 809208, 991988, 1178388, 1213848], P{value_shift: 0.005305, free_shift: 1.383475, window_weight: 0.110148}),   // 1.13% for 4.5 λ=6.88
    (3, [0, 224468, 228008, 239020, 431328, 597088, 619736], P{value_shift: 0.006703, free_shift: 1.359536, window_weight: 0.160694}), // 0.90% for 4.4 λ=8.82
    (4, [0, 324804, 383320, 387324, 496376, 599672, 612172], P{value_shift: 0.006220, free_shift: 1.326950, window_weight: 0.274944}), // 0.90% for 4.4 λ=10.77
    (5, [0, 361880, 450152, 474160, 541520, 607100, 613300], P{value_shift: 0.005613, free_shift: 1.226365, window_weight: 0.427567}), // 0.90% for 4.4 λ=12.64
    (6, [0, 437748, 557692, 623668, 667996, 712312, 716556], P{value_shift: 0.004785, free_shift: 1.239422, window_weight: 0.566020}), // 0.90% for 4.4 λ=14.43
    (8, [0, 538320, 749052, 900604, 901692, 902736, 902820], P{value_shift: 0.003281, free_shift: 1.226912, window_weight: 0.826709}), // 0.72% for 4.3 λ=17.47
];
//const PROD_S8_L4096: [(u16, [i32; 7], ProdOfValuesKEval); 3] = [   // for W=512 10M keys
//  (2, [0, 532424, 575440, 803208, 1146040, 1546784, 1613432], P{value_shift: 0.003347, free_shift: 1.333880, window_weight: 0.137510}),   // 1.17% for 4.5 λ=6.88
//  (3, [0, 344368, 353564, 371920, 596164, 1013220, 1054312], P{value_shift: 0.006365, free_shift: 1.316446, window_weight: 0.116298})   // 0.95% for 4.4 λ=8.82
//]
const PROD_S9_L256: [(u16, [i32; 7], ProdOfValuesKEval); 7] = [   // for W=512 10M keys
//    (2, [0, 492544, 750048, 846568, 928144, 1013824, 1030476], P{value_shift: 0.005129, free_shift: 1.522596, window_weight: 0.098544}), // 0.96% for 5.1 λ=7.80
//    (3, [0, 426048, 448848, 469780, 509908, 575720, 584064], P{value_shift: 0.005462, free_shift: 1.224635, window_weight: 0.150164}), // 0.95% for 5.1 λ=10.22
    (4, [0, 477052, 633304, 634628, 666896, 683948, 686592], P{value_shift: 0.005576, free_shift: 1.364103, window_weight: 0.209758}), // 1.04% for 5.1 λ=12.49
    (8, [0, 623324, 921796, 975904, 978488, 981120, 981824], P{value_shift: 0.003131, free_shift: 1.215869, window_weight: 0.878701}), // 0.77% for 5.1 λ=20.72
    (10, [0, 574272, 776940, 814828, 829576, 845112, 845716], P{value_shift: 0.003554, free_shift: 1.183646, window_weight: 0.674511}), // 0.72% for 5.1 λ=24.53
    (16, [0, 830700, 989132, 1209940, 1210996, 1211760, 1212296], P{value_shift: 0.003046, free_shift: 1.273976, window_weight: 0.933178}), // 0.61% for 5.1 λ=35.31 20M
    (32, [0, 437896, 931992, 1219772, 1222088, 1224768, 1225292], P{value_shift: 0.003605, free_shift: 1.352787, window_weight: 0.869508}), // 0.47% for 5.1 λ=61.48 40M
    (100, [0, 435544, 852848, 1321284, 1322904, 1325788, 1326200], P{value_shift: 0.005933, free_shift: 1.937995, window_weight: 0.996255}), // 0.28% for 5.1 λ=158.26 100M
    (1000, [0, 406548, 868960, 1156044, 1157112, 1158448, 1158632], P{value_shift: 0.003906, free_shift: 1.484429, window_weight: 0.828027}), // 0.54% for 5.0 λ=1143.40 100M
];
const PROD_S9_L512: [(u16, [i32; 7], ProdOfValuesKEval); 5] = [   // for W=512 10M keys
    (2, [0, 184660, 199508, 235588, 291996, 320436, 330812], P{value_shift: 0.011864, free_shift: 1.492050, window_weight: 0.226916}), // 1.06% for 5.1 λ=7.80
    (4, [0, 381220, 401284, 435616, 448928, 455928, 459212], P{value_shift: 0.005678, free_shift: 1.174598, window_weight: 0.202620}), // 0.93% for 5.1 λ=12.49
    (8, [0, 636372, 953988, 993984, 994412, 994868, 994988], P{value_shift: 0.003216, free_shift: 1.169975, window_weight: 0.889471}), // 0.77% for 5.1 λ=20.72
    (10, [0, 589972, 787412, 826124, 832168, 839088, 839176], P{value_shift: 0.003577, free_shift: 1.160926, window_weight: 0.659485}), // 0.75% for 5.1 λ=24.53
    //(16, [0, 529752, 781728, 906356, 948392, 996396, 1006564], P{value_shift: 0.002970, free_shift: 2.763143, window_weight: 0.805190}), // 0.92% for 5.1 λ=35.31 20M
    (32, [0, 549180, 816544, 913564, 960964, 1011572, 1016648], P{value_shift: 0.002834, free_shift: 2.653888, window_weight: 0.762301}), // 0.58% for 5.1 λ=61.48 40M
];
const PROD_S9_L1024: [(u16, [i32; 7], ProdOfValuesKEval); 8] = [   // for W=512 10M keys
    (2, [0, 492544, 750048, 846568, 928144, 1013824, 1030476], P{value_shift: 0.005129, free_shift: 1.522596, window_weight: 0.098544}), // 0.96% for 5.1 λ=7.80
    (3, [0, 426048, 448848, 469780, 509908, 575720, 584064], P{value_shift: 0.005462, free_shift: 1.224635, window_weight: 0.150164}), // 0.95% for 5.1 λ=10.22
    (4, [0, 553668, 778628, 814304, 853596, 882660, 886972], P{value_shift: 0.004996, free_shift: 1.199566, window_weight: 0.291018}), // 0.94% for 5.1 λ=12.49
    (6, [0, 607924, 864948, 901264, 911192, 918196, 919240], P{value_shift: 0.004112, free_shift: 1.152632, window_weight: 0.585186}), // 0.88% for 5.1 λ=16.73
    (8, [0, 636268, 953840, 993832, 994260, 994716, 994836], P{value_shift: 0.003129, free_shift: 1.169975, window_weight: 0.878484}), // 0.87% for 5.1 λ=20.72
    (9, [0, 609844, 875764, 914856, 918624, 922728, 922844], P{value_shift: 0.003413, free_shift: 1.165285, window_weight: 0.776700}), // 0.88% for 5.1 λ=22.64
    (10, [0, 575124, 790084, 829712, 836616, 844188, 844316], P{value_shift: 0.003506, free_shift: 1.186065, window_weight: 0.651800}), // 0.90% for 5.1 λ=24.53
    (16, [0, 546828, 759384, 907404, 974416, 1035992, 1058104], P{value_shift: 0.003044, free_shift: 3.105925, window_weight: 0.785002}), // 0.99% for 5.1 λ=35.31 20M
];
const PROD_S9_L2048: [(u16, [i32; 7], ProdOfValuesKEval); 8] = [   // for W=512 10M keys
    (2, [0, 445712, 671664, 795996, 964036, 1133604, 1162472], P{value_shift: 0.005353, free_shift: 1.417937, window_weight: 0.109232}), // 0.93% for 5.1 λ=7.80
    (3, [0, 386940, 487868, 567764, 686920, 804076, 818544], P{value_shift: 0.006377, free_shift: 1.311951, window_weight: 0.191832}), // 0.98% for 5.1 λ=10.22
    (4, [0, 331652, 334580, 398896, 477736, 545232, 551528], P{value_shift: 0.007072, free_shift: 1.169105, window_weight: 0.291385}), // 0.97% for 5.1 λ=12.49
    (6, [0, 449008, 598556, 681268, 715008, 745528, 745700], P{value_shift: 0.004912, free_shift: 1.144739, window_weight: 0.568293}), // 0.98% for 5.1 λ=16.73
    (8, [0, 572516, 838396, 948696, 951404, 953980, 954140], P{value_shift: 0.003076, free_shift: 1.172570, window_weight: 0.808650}), // 1.04% for 5.1 λ=20.72
    (9, [0, 575136, 791404, 928136, 929868, 931492, 931596], P{value_shift: 0.003244, free_shift: 1.187008, window_weight: 0.821200}), // 1.08% for 5.1 λ=22.64
    (10, [0, 537488, 757840, 902704, 904092, 905396, 905504], P{value_shift: 0.003278, free_shift: 1.216569, window_weight: 0.830729}), // 1.12% for 5.1 λ=24.53
    (16, [0, 547052, 767944, 898072, 898860, 899564, 899620], P{value_shift: 0.003326, free_shift: 1.153749, window_weight: 0.847241}), // 1.20% for 5.1 λ=35.31 20M
];
const PROD_S10_L256: [(u16, [i32; 7], ProdOfValuesKEval); 3] = [   // for W=512 10M keys
  //(10, [0, 546092, 752496, 767748, 786380, 806684, 806688], P{value_shift: 0.003575, free_shift: 1.224974, window_weight: 0.670027}), // 0.63% for 5.7 λ=27.42
  (10, [0, 555220, 762372, 785240, 801396, 819252, 819708], P{value_shift: 0.003558, free_shift: 1.217001, window_weight: 0.690244}), // 0.75% for 5.8 λ=27.90
//  (100, [0, 411684, 753628, 1176448, 1177356, 1178524, 1178712], P{value_shift: 0.004607, free_shift: 1.696350, window_weight: 0.721485}), // 0.21% for 5.6 λ=173.78
  (100, [0, 433488, 845264, 1313072, 1314552, 1317272, 1317568], P{value_shift: 0.005969, free_shift: 1.991858, window_weight: 0.992794}), // 0.24% for 5.7 λ=176.88 100M
  (1000, [0, 263644, 417048, 644420, 645104, 646344, 646496], P{value_shift: 0.005414, free_shift: 2.100069, window_weight: 0.835282}), // 0.54s% for 5.7 λ=1303.48 100M
];
//const PROD_S10_L512: [(u16, [i32; 7], ProdOfValuesKEval); 1] = [   // for W=512 10M keys
//    (10, [0, 596956, 800052, 829588, 837820, 847084, 847212], P{value_shift: 0.003702, free_shift: 1.175097, window_weight: 0.632076}), // 0.65% for 5.7 λ=27.42
//];

// S4_L256_A82
// 4: 0, 14992, 15142, 17105, 22422, 26729, 28817  value_shift: 0.008692, free_shift: 0.860894, window_weight: 0.968914 // 2.56% for 4.8 λ=11.75
// 16: 0, 32023, 33253, 33916, 38621, 42429, 42949  value_shift: 0.009720, free_shift: 0.207154, window_weight: 0.993932 // 0.60% for 8.7 λ=60.24

// S4_L256_A92
// 4: 0, 10529, 16106, 19477, 32838, 46774, 50771  value_shift: 0.011395, free_shift: 1.349613, window_weight: 0.645572 // 1.72% for 3.0 λ=7.34
// 16: 0, 29923, 30471, 33561, 38143, 42599, 44311  value_shift: 0.007899, free_shift: 1.498430, window_weight: 0.993910 // 1.87% for 4.7 λ=32.54

const PROD_S6_L256_A81: [(u16, [i32; 7], ProdOfValuesKEval); 2] = [   // for W=512 10M keys, alpha=0.81
    (4, [0, 189100, 194496, 207744, 216280, 216900, 220612], P{value_shift: 0.005506, free_shift: 0.851065, window_weight: 0.922497}), // 0.65% for 7.5 λ=18.37 TODO more iter, larger bucket
    //(16, [0, 1096244, 1133568, 1148792, 1152520, 1155848, 1156544], P{value_shift: 0.006314, free_shift: 0.131816, window_weight: 0.981104}) // 0.97% for 16.80 λ=116.34 30M
    (16, [0, 1097356, 1134273, 1149368, 1153076, 1156381, 1157074], P{value_shift: 0.006613, free_shift: 0.131881, window_weight: 0.981242}), // 1.18% for 17.2 λ=119.11 30M
];

const PROD_S6_L512_A81: [(u16, [i32; 7], ProdOfValuesKEval); 1] = [   // for W=512 10M keys, alpha=0.81
    (16, [0, 111196, 136177, 139463, 250785, 313273, 314867], P{value_shift: 0.002441, free_shift: 0.641869, window_weight: 0.999812}), // 0.98% for 17.2 λ=119.11 30M
];

const PROD_S6_L1024_A81: [(u16, [i32; 7], ProdOfValuesKEval); 2] = [   // for W=512 10M keys, alpha=0.81    TODO more iters
    (4, [0, 134504, 139044, 144324, 314108, 389984, 402576], P{value_shift: 0.002538, free_shift: 0.735067, window_weight: 0.776704}), // 1.27% for 8.3 λ=20.33
    (16, [0, 160809, 161763, 169043, 374512, 460618, 463724], P{value_shift: 0.002689, free_shift: 0.613560, window_weight: 0.999098}), // 0.52% for 17.2 λ=119.11 30M
];

const PROD_S6_L2048_A81: [(u16, [i32; 7], ProdOfValuesKEval); 1] = [   // for W=512 10M keys, alpha=0.81
    (16, [0, 312012, 330336, 344827, 573445, 647231, 650858], P{value_shift: 0.009049, free_shift: 0.072425, window_weight: 0.229956}), // 0.61% for 17.2 λ=119.11 30M
];

const PROD_S8_L128_A81: [(u16, [i32; 7], ProdOfValuesKEval); 1] = [   // for W=512 10M keys, alpha=0.81
    (16, [0, 978168, 981892, 983276, 988320, 993200, 993396], P{value_shift: 0.005032, free_shift: 0.138797, window_weight: 0.988141}) // 2.55% for 24.70 λ=171.05 30M
];

const PROD_S8_L256_A81: [(u16, [i32; 7], ProdOfValuesKEval); 1] = [   // for W=512 10M keys, alpha=0.81
    //(4, [0, 230816, 230924, 230964, 244680, 246600, 250236], P{value_shift: 0.001991, free_shift: 1.700801, window_weight: 0.977667}), // 1.88% for 10.90 λ=26.70
    //(16, [0, 977120, 981580, 982636, 987784, 992624, 992968], P{value_shift: 0.004916, free_shift: 0.138733, window_weight: 0.978550}) // 1.35% for 24.60 λ=170.35
    (16, [0, 983203, 987691, 988753, 993934, 998804, 999150], P{value_shift: 0.005039, free_shift: 0.138733, window_weight: 0.978550}) // 1.44% for 24.80 λ=171.74
];

const PROD_S8_L512_A81: [(u16, [i32; 7], ProdOfValuesKEval); 1] = [   // for W=512 10M keys, alpha=0.81
    (16, [0, 759169, 983418, 988465, 1014557, 1014572, 1014895], P{value_shift: 0.003607, free_shift: 0.152776, window_weight: 0.330437}) // 1.13% for 24.80 λ=171.74 30M
];

const PROD_S8_L1024_A81: [(u16, [i32; 7], ProdOfValuesKEval); 2] = [   // for W=512 10M keys, alpha=0.81
    (4, [0, 751196, 937284, 955712, 989592, 1006728, 1013204], P{value_shift: 0.004738, free_shift: 0.507916, window_weight: 0.372584}), // 1.72% for 11.9 λ=29.15
    (16, [0, 645186, 718456, 1407008, 1443894, 1526059, 1527492], P{value_shift: 0.003168, free_shift: 0.125485, window_weight: 0.997560}) // 1.30% for 26.70 λ=184.90 30M
    //(16, [0, 1179347, 1667368, 1819722, 1885769, 1948556, 1949751], P{value_shift: 0.005202, free_shift: 0.075938, window_weight: 0.511066}) // 0.69% for 24.80 λ=171.74 30M
];

const PROD_S8_L2048_A81: [(u16, [i32; 7], ProdOfValuesKEval); 1] = [   // for W=512 10M keys, alpha=0.81
    (16, [0, 736576, 809047, 1002064, 1049920, 1075434, 1078364], P{value_shift: 0.003503, free_shift: 0.214369, window_weight: 0.954944}) // 1.05% for 26.70 λ=184.90 30M
    //(16, [0, 594624, 681774, 906664, 960492, 1004433, 1007948], P{value_shift: 0.003131, free_shift: 0.791033, window_weight: 0.997128}) // 0.58% for 24.80 λ=171.74 30M
];

const PROD_S8_L4096_A81: [(u16, [i32; 7], ProdOfValuesKEval); 1] = [   // for W=512 10M keys, alpha=0.81
    (16, [0, 858582, 1218266, 1359062, 1400615, 1434085, 1439760], P{value_shift: 0.003556, free_shift: 0.235519, window_weight: 0.989666}) // 0.90% for 26.70 λ=184.90 30M
    //(16, [0, 840839, 1222120, 1350258, 1390145, 1421015, 1426314], P{value_shift: 0.003297, free_shift: 0.230105, window_weight: 0.983330}) // 0.08% for 24.80 λ=171.74 30M
];

//const PROD_S8_L8192_A81: [(u16, [i32; 7], ProdOfValuesKEval); 1] = [   // for W=512 10M keys, alpha=0.81
    //(16, [0, 828540, 1190472, 1330950, 1413486, 1479479, 1490571], P{value_shift: 0.003456, free_shift: 0.245763, window_weight: 0.974089}) // 0.97% for 26.70 λ=184.90 30M
//];

const PROD_S6_L256_A91: [(u16, [i32; 7], ProdOfValuesKEval); 1] = [   // for W=512 30M keys, alpha=0.91
    (16, [0, 789624, 807572, 849524, 872436, 891548, 893616], P{value_shift: 0.004169, free_shift: 0.737674, window_weight: 0.974546}) // 1.23% for 8.2 λ=56.78 30M
];

const PROD_S6_L512_A91: [(u16, [i32; 7], ProdOfValuesKEval); 2] = [   // for W=512 10M keys, alpha=0.91
    (4, [0, 145184, 152448, 160592, 217408, 275276, 288748], P{value_shift: 0.009293, free_shift: 0.849006, window_weight: 0.365712}), // 1.41% for 5.35 λ=13.10 
    (16, [0, 200180, 202952, 213692, 230432, 240360, 245020], P{value_shift: 0.007540, free_shift: 0.499739, window_weight: 0.540330}) // 1.58% for 8.6 λ=59.55 30M
];

const PROD_S6_L1024_A91: [(u16, [i32; 7], ProdOfValuesKEval); 2] = [   // for W=512 10M keys, alpha=0.91
    (4, [0, 173336, 195188, 199292, 296536, 551924, 578024], P{value_shift: 0.007306, free_shift: 1.167883, window_weight: 0.482356}), // 1.22% for 5.35 λ=13.10 
    (16, [0, 202756, 221572, 285268, 371396, 587996, 597572], P{value_shift: 0.007055, free_shift: 0.945071, window_weight: 0.796202}) // 1.31% for 8.6 λ=59.55 30M
];

const PROD_S6_L2048_A91: [(u16, [i32; 7], ProdOfValuesKEval); 1] = [   // for W=512 10M keys, alpha=0.91
    (16, [0, 1144, 91080, 123324, 164840, 195532, 216432], P{value_shift: 0.014504, free_shift: 0.842729, window_weight: 0.522973}) // 1.39% for 8.6 λ=59.55 30M
];

const PROD_S8_L256_A91: [(u16, [i32; 7], ProdOfValuesKEval); 1] = [   // for W=512 30M keys, alpha=0.91
    (16, [0, 789824, 845084, 872448, 892208, 909212, 910404], P{value_shift: 0.004412, free_shift: 0.655868, window_weight: 0.986539}) // 0.92% for 11.2 λ=77.56 30M
    //(16, [0, 784967, 846372, 878101, 902173, 922282, 923666], P{value_shift: 0.004490, free_shift: 0.665282, window_weight: 0.981891}) // 1.66% for 12.0 λ=83.10 30M
];

const PROD_S8_L512_A91: [(u16, [i32; 7], ProdOfValuesKEval); 1] = [   // for W=512 30M keys, alpha=0.91
    (16, [0, 23930, 27973, 105262, 108790, 178866, 181612], P{value_shift: 0.000696, free_shift: 0.629757, window_weight: 0.831636}) // 1.33% for 12.0 λ=83.10 30M
];

const PROD_S8_L1024_A91: [(u16, [i32; 7], ProdOfValuesKEval); 2] = [   // for W=512 10M keys, alpha=0.91
    //(4, [0, 660504, 893572, 950976, 1030980, 1072984, 1089008], P{value_shift: 0.004720, free_shift: 0.792409, window_weight: 0.350899}) // 0.85% for 7.4 λ=18.12
    (4, [0, 307516, 491000, 595268, 617620, 637228, 651676], P{value_shift: 0.003430, free_shift: 1.122674, window_weight: 0.976634}), // 1.04% for 7.5 λ=18.37
    (16, [0, 6753, 110451, 2210204, 2210437, 2343961, 2348108], P{value_shift: 0.004431, free_shift: 0.410764, window_weight: 0.859839}) // 1.41% for 12.0 λ=83.10 30M
];

//S=8 k=16 12.0 λ=83.10 30M
//L=2048: 1.54%  0, 882611, 887325, 1306550, 1377890, 1404247, 1416673  value_shift: 0.003875, free_shift: 0.286924, window_weight: 0.364786
//L=4096: 1.48%  0, 18845, 1717215, 2155956, 2235833, 2344335, 2369368  value_shift: 0.007334, free_shift: 0.400328, window_weight: 0.371568
//L=8192: 2.18%  0, 133892, 138134, 226826, 318630, 319382, 354491  value_shift: 0.003344, free_shift: 0.591322, window_weight: 0.990399