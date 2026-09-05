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
        (_, ..=128) => PROD_S8_L128.as_ref(),
        (_, ..=256) => PROD_S8_L256.as_ref(),
        (_, ..=512) => PROD_S8_L512.as_ref(),
        (_, ..=1024) => PROD_S8_L1024.as_ref(),
        _ => PROD_S8_L2048.as_ref()
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
const PROD_S8_L128: [(u16, [i32; 7], ProdOfValuesKEval); 10] = [   // for W=512
    (2, [0, 85619, 89969, 91596, 99920, 102373, 104194], P{value_shift: 0.003797, free_shift: 1.776096, first_weight: 0.892008}), // moreit 3.23% for 4.3 λ=6.58
    (4, [0, 109935, 110271, 110493, 112836, 113793, 114420], P{value_shift: 0.003997, free_shift: 1.496077, first_weight: 0.925504}),   // 1.25% for 4.3 λ=10.53
    (8, [0, 162849, 198446, 211248, 214495, 217616, 217883], P{value_shift: 0.004086, free_shift: 1.389829, first_weight: 0.824165}), // 0.74% for 4.3 λ=17.47
    (10, [0, 169623, 206514, 217060, 219809, 222516, 222721], P{value_shift: 0.004067, free_shift: 1.375535, first_weight: 0.806195}), // 0.67% for 4.3 λ=20.68
    (16, [0, 161968, 197262, 213450, 215417, 217410, 217540], P{value_shift: 0.004056, free_shift: 1.396727, first_weight: 0.817199}),  // 0.55% for 4.3 λ=29.77
    (20, [0, 163223, 194624, 214195, 215860, 217552, 217658], P{value_shift: 0.004192, free_shift: 1.432003, first_weight: 0.835094}), // 0.50% for 4.3 λ=35.52
    (24, [0, 162930, 189680, 213629, 215007, 216432, 216511], P{value_shift: 0.004318, free_shift: 1.361641, first_weight: 0.855886}),  // 0.47% for 4.3 λ=41.09
    (32, [0, 156538, 181148, 205030, 206957, 209066, 209141], P{value_shift: 0.004403, free_shift: 1.443351, first_weight: 0.869070}),  // 0.43% for 4,3 λ=51.83
    (40, [0, 156395, 179151, 204246, 206144, 208242, 208306], P{value_shift: 0.004449, free_shift: 1.510469, first_weight: 0.869724}),  // 0.40% for 4.3 λ=62.19
    (48, [0, 156545, 174209, 202178, 204470, 207017, 207087], P{value_shift: 0.004499, free_shift: 1.564914, first_weight: 0.879130}),  // 0.39% for 4.3 λ=72.26
];
const PROD_S8_L256: [(u16, [i32; 7], ProdOfValuesKEval); 13] = [   // for W=512
    (2, [0, 57696, 57734, 57739, 61173, 61653, 62564], P{value_shift: 0.001943, free_shift: 1.785186, first_weight: 0.954276}), // 1.70% for 4.3 λ=6.58
    (4, [0, 57685, 57723, 57728, 61161, 61641, 62552], P{value_shift: 0.001942, free_shift: 1.701445, first_weight: 0.954089}), // 0.86% for 4.3 λ=10.53
    (8, [0, 166697, 193622, 206442, 209316, 212165, 212424], P{value_shift: 0.004140, free_shift: 1.255893, first_weight: 0.875665}), // 0.59% for 4.3 λ=17.47
    (9, [0, 166713, 199138, 207845, 210338, 212795, 213011], P{value_shift: 0.004204, free_shift: 1.256947, first_weight: 0.871172}), // 0.56% for 4.3 λ=19.09
    //(10, [0, 167183, 200649, 210689, 212859, 215093, 215267], P{value_shift: 0.003928, free_shift: 1.181003, first_weight: 0.881245}), // 0.83% for 4.5 λ=21.64; 0.54% for 4.3 λ=20.68
    (10, [0, 165540, 196904, 209202, 211333, 213427, 213601], P{value_shift: 0.004054, free_shift: 1.257445, first_weight: 0.866225}), // 0.54% for 4.3 λ=20.68; 0.83% for 4.5 λ=21.64
    (12, [0, 166102, 199597, 212119, 214253, 216393, 216569], P{value_shift: 0.004230, free_shift: 1.262666, first_weight: 0.869147}), // 0.51% for 4.3 λ=23.79
    (14, [0, 161363, 201609, 211376, 213229, 215174, 215327], P{value_shift: 0.004126, free_shift: 1.294066, first_weight: 0.887341}), // 0.49% for 4.3 λ=26.82
    (16, [0, 166001, 204192, 217297, 219365, 221520, 221696], P{value_shift: 0.004013, free_shift: 1.275321, first_weight: 0.871179}), // 0.69% for 4.5 λ=31.16
        // (16, [0, 167260, 206471, 218590, 220487, 222475, 222594], P{value_shift: 0.004032, free_shift: 1.284303, first_weight: 0.871493}), // 0.47% for 4.3 λ=29.77
    (20, [0, 166009, 204200, 217299, 219367, 221522, 221698], P{value_shift: 0.004188, free_shift: 1.275331, first_weight: 0.871113}), // 0.68% for 4.5 λ=37.17
    (24, [0, 164255, 203402, 214473, 216140, 217900, 218016], P{value_shift: 0.004278, free_shift: 1.300274, first_weight: 0.854534}), // 0.43% for 4.3 λ=41.09
    (32, [0, 169263, 203508, 216671, 218383, 220261, 220375], P{value_shift: 0.004380, free_shift: 1.335272, first_weight: 0.873470}), // 0.41% for 4.3 λ=51.83
    (40, [0, 169966, 206739, 220378, 222257, 224424, 224543], P{value_shift: 0.004527, free_shift: 1.371665, first_weight: 0.877259}), // 0.40% for 4.3 λ=62.19 (better than L=128)
    (48, [0, 165661, 201039, 212449, 214213, 216231, 216334], P{value_shift: 0.004588, free_shift: 1.396611, first_weight: 0.891372}), // 0.40% for 4.3 λ=72.26
];
const PROD_S8_L512: [(u16, [i32; 7], ProdOfValuesKEval); 16] = [   // for W=512
    (2, [0, 76473, 93538, 100036, 111405, 124459, 127558], P{value_shift: 0.00968, free_shift: 1.94309, first_weight: 0.28871}), // 1.28% for 4.5 λ=6.88
    (3, [0, 43209, 47718, 47849, 56764, 62883, 65002], P{value_shift: 0.009677, free_shift: 1.317506, first_weight: 0.310870}), // 1.18% for 4.5 λ=9.02
    (4, [0, 124831, 125295, 125906, 134811, 144356, 145403], P{value_shift: 0.00668, free_shift: 1.29751, first_weight: 0.27419}), // 1.12% for 4.5 λ=11.02
    (5, [0, 59714, 60716, 60846, 63360, 64689, 65609], P{value_shift: 0.007691, free_shift: 1.253225, first_weight: 0.348958}), // 0.63% for 4.3 λ=12.35
    (6, [0, 149127, 160970, 171446, 177056, 182860, 183392], P{value_shift: 0.005145, free_shift: 1.194641, first_weight: 0.584520}), //0.99% for 4.5 λ=14.76
    (7, [0, 154783, 181249, 187499, 191660, 196006, 196357], P{value_shift: 0.004508, free_shift: 1.234478, first_weight: 0.692905}), //0.57% for 4.3 λ=15.81
    (8, [0, 163258, 202756, 216115, 219743, 223466, 223810], P{value_shift: 0.00364, free_shift: 1.21311, first_weight: 0.85403}), // 0.92% for 4.5 λ=18.28
    (10, [0, 167476, 202441, 212104, 213999, 215955, 216104], P{value_shift: 0.003798, free_shift: 1.172512, first_weight: 0.871241}), // 0.88% for 4.5 λ=21.64
    (12, [0, 169779, 209752, 213158, 213291, 213433, 213442], P{value_shift: 0.003807, free_shift: 1.141909, first_weight: 0.857620}), // 0.86% for 4.5 λ=24.90
    (16, [0, 170107, 206229, 210576, 210830, 211146, 211180], P{value_shift: 0.00365, free_shift: 1.12751, first_weight: 0.87378}), // 0.85% for 4.5 λ=31.16
    (20, [0, 172926, 194515, 207725, 209558, 211778, 211993], P{value_shift: 0.003568, free_shift: 1.125351, first_weight: 0.905329}), // 0.85% for 4.5 λ=37.17
    (24, [0, 179037, 198479, 210605, 212759, 215360, 215624], P{value_shift: 0.003623, free_shift: 1.118238, first_weight: 0.914403}), // 0.83% for 4.5 λ=43.00
    (32, [0, 179706, 199221, 210865, 213144, 215884, 216163], P{value_shift: 0.003632, free_shift: 1.128020, first_weight: 0.916687}), // 0.79% for 4.5 λ=54.24
    (48, [0, 179533, 198080, 208200, 210344, 212994, 213256], P{value_shift: 0.003635, free_shift: 1.173609, first_weight: 0.925055}), // 0.77% for 4.5 λ=75.62
    (64, [0, 181531, 198187, 207728, 209818, 212437, 212699], P{value_shift: 0.003652, free_shift: 1.229707, first_weight: 0.913129}), // 0.79% for 4.5 λ=96.01
    (100, [0, 188236, 199627, 208338, 209812, 211581, 211790], P{value_shift: 0.003562, free_shift: 1.285305, first_weight: 0.862204}), // 0.91% for 4.5 λ=139.64
];
const PROD_S8_L1024: [(u16, [i32; 7], ProdOfValuesKEval); 6] = [   // for W=512
    (2, [0, 124853, 186485, 217947, 238678, 260990, 266209], P{value_shift: 0.005040, free_shift: 1.543640, first_weight: 0.101565}), // 1.15% for 4.5 λ=6.88
    (3, [0, 74123, 84293, 98932, 126519, 143831, 146952], P{value_shift: 0.006193, free_shift: 1.334455, first_weight: 0.160041}), // 1.15% for 4.5 λ=9.02
    (4, [0, 133201, 188435, 207660, 220157, 228500, 230630], P{value_shift: 0.004811, free_shift: 1.312647, first_weight: 0.303816}), // 1.15% for 4.5 λ=11.02
    //(5, [0, 128493, 183625, 213859, 234965, 251661, 261834], P{value_shift: 0.00396, free_shift: 2.35814, first_weight: 0.62478 }), // O1.04%
    (6, [0, 164475, 244036, 270054, 271239, 272210, 272747], P{value_shift: 0.003458, free_shift: 1.157448, first_weight: 0.813080}), // 1.05% for 4.5 λ=14.76
    //(8, [0, 132557, 183504, 217346, 240456, 257524, 268553], P{value_shift: 0.00324, free_shift: 2.79127, first_weight: 0.76304}), // 1.56% for 4.5 λ=18.28
    //(10, [0, 130551, 173956, 208589, 229433, 246584, 256458], P{value_shift: 0.003607, free_shift: 3.014842, first_weight: 0.799166}), // 1.42% for 4.5 λ=21.64
    (16, [0, 132238, 192126, 228640, 246369, 263743, 270676], P{value_shift: 0.003030, free_shift: 3.179457, first_weight: 0.781803}), // 1.15% for 4.5 λ=31.16
    (32, [0, 133295, 192878, 217607, 233213, 247494, 251097], P{value_shift: 0.002571, free_shift: 3.173833, first_weight: 0.733757}), // 0.94% for 4.5 λ=54.24
];
const PROD_S8_L2048: [(u16, [i32; 7], ProdOfValuesKEval); 3] = [   // for W=512
    (2, [0, 111221, 167294, 202302, 247997, 294597, 303462], P{value_shift: 0.005305, free_shift: 1.383475, first_weight: 0.110148}),   // 1.13% for 4.5 λ=6.88
    (3, [0, 56117, 57002, 59755, 107832, 149272, 154934], P{value_shift: 0.006703, free_shift: 1.359536, first_weight: 0.160694}), // 0.90% for 4.4 λ=8.82
    (8, [0, 140681, 188411, 228234, 228489, 228727, 228748], P{value_shift: 0.00316, free_shift: 1.15326, first_weight: 0.82834}), // 1.16% for 4.5 λ=18.28
];