//! Seed choosers and seed evaluators using only regular hashing, without shifting.

use std::io;

use crate::{fmph::SeedSize, phast::{ComparableF64, Core, SeedChooser, SeedChooserCore, Weights, cyclic::UsedValueSet, seed_chooser::SeedChooserConf}};

/// Evaluate (harness of) seed for (1-)perfect function.
/// Seed with the lowest value is used.
/// 
/// Also provides bucket evaluator suitable to use with `Self`.
pub trait SeedEvaluator: Copy + Sync {
    /// Type of evaluation value.
    type Value: PartialEq + PartialOrd + Ord;

    /// Value greater than each value returned by `eval`.
    const MAX: Self::Value;

    /// Precalculated data usable to evaluate each seed in the same bucket.
    type BucketData: Copy;

    /// Precalculates data usable to evaluate each seed in the same bucket.
    /// The result is passed to `eval` for each seed in the bucket.
    fn for_bucket<C: Core>(&self, bucket_nr: usize, first_bucket_in_window: usize, core: &C) -> Self::BucketData;

    /// Evaluate (harness of) seed that used given `values`.
    fn eval(&self, values_used_by_seed: &[usize], bucket_data: Self::BucketData) -> Self::Value;

    /// Returns bucket evaluator which compares buckets (for choosing the best one) and works well with `self` as `SeedEvaluator`.
    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {
        Weights::new(bits_per_seed, slice_len)
    }

    /// Returns slice length suitable to given `output_range`, `bits_per_seed` and `preferred_slice_len`.
    /// 
    /// Usually it returns `preferred_slice_len` (if its not `0`; `0` is for chooser-dependent default)
    /// or lower value for small `output_range`.
    fn slice_len(&self, output_range: usize, bits_per_seed: u8, preferred_slice_len: u16) -> u16;
}

/// `SeedEvaluator` which is based on product of values or sum of their logarithms.
#[derive(Clone, Copy)]
pub struct ProdOfValues;

impl SeedEvaluator for ProdOfValues {

    type Value = ComparableF64;

    const MAX: Self::Value = ComparableF64(f64::MAX);
        
    type BucketData = usize;
    
    fn for_bucket<C: Core>(&self, bucket_nr: usize, _first_bucket_in_window: usize, core: &C) -> Self::BucketData {
       core.slice_begin_for_bucket(bucket_nr).wrapping_sub(95)
    }

    fn eval(&self, values_used_by_seed: &[usize], to_extract: Self::BucketData) -> Self::Value {
        ComparableF64(values_used_by_seed.iter().map(|v| {    // simple sum gives 1.921
            //2048.0 * ((v - min) as f64).log2()    // 1.905
            v.wrapping_sub(to_extract) as f64    // 1.905 (0,2) 1.903 (10) 1.901 (20) 1.900 (30) 1.899 (40) 1.898 (50,60,80,100,120,150), 1.899 (200), 1.900 (250), 1.901 (300)
            //2048.0 * ((v - min + 5) as f64).sqrt()  // 1.902 (0,5,10), 1.903 (30,50), 1.905 (100)
        }).product())
    }

    #[cfg(not(feature = "W256"))]   // TODO optimize for small output_range
    fn slice_len(&self, output_range: usize, bits_per_seed: u8, preferred_slice_len: u16) -> u16 {  
        let max_res = match output_range {
            n @ 0..64 => (n/2+1).next_power_of_two() as u16,
            64..1300 => 64,
            1300..9500 => 128,
            9500..12000 => 256,
            12000..140000 => 512,
            _ if bits_per_seed < 4 => return if preferred_slice_len == 0 { 512 } else { preferred_slice_len },
            _ if bits_per_seed < 7 => return if preferred_slice_len == 0 { 1024 } else { preferred_slice_len },
            _ if bits_per_seed < 10 => return if preferred_slice_len == 0 { 2048 } else { preferred_slice_len },
            _ => return if preferred_slice_len == 0 { 4096 } else { preferred_slice_len }
            // TODO for S=12 8192 performs almost identical to 4096, check which is better in practice
        };
        if preferred_slice_len != 0 { max_res.min(preferred_slice_len) } else { max_res }
    }

    #[cfg(feature = "W256")]    // TODO optimize for small output_range
    fn slice_len(&self, output_range: usize, bits_per_seed: u8, preferred_slice_len: u16) -> u16 {
        let max_res = match output_range {
            n @ 0..64 => (n/2+1).next_power_of_two() as u16,
            64..1300 => 64,
            1300..9500 => 128,
            9500..12000 => 256,
            12000..140000 => 512,
            _ if bits_per_seed < 6 => return if preferred_slice_len == 0 { 512 } else { preferred_slice_len },
            _ if bits_per_seed < 9 => return if preferred_slice_len == 0 { 1024 } else { preferred_slice_len },
            _ => return if preferred_slice_len == 0 { 2048 } else { preferred_slice_len }
            // TODO for S=12 4096 performs almost identical to 2048, check which is better in practice
        };
        if preferred_slice_len != 0 { max_res.min(preferred_slice_len) } else { max_res }
    }
}

/// `SeedEvaluator` which is based on sum of values.
#[derive(Clone, Copy)]
pub struct SumOfValues;

impl SeedEvaluator for SumOfValues {
    type Value = usize;

    const MAX: Self::Value = usize::MAX;

    type BucketData = ();

    #[inline] fn for_bucket<C: Core>(&self, _bucket_nr: usize, _first_bucket_in_window: usize, _core: &C) -> Self::BucketData { }

    #[inline] fn eval(&self, values_used_by_seed: &[usize], _bucket_data: Self::BucketData) -> Self::Value {
        values_used_by_seed.iter().sum()
    }

    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {  // TODO old weights, found for WINDOW_SIZE=256
        Weights(if slice_len <= 256 {  // this is used only for small number of keys
            match (bits_per_seed, slice_len) {
                (..=4, ..=32) => [-258168, 486268, 500232, 507928, 513944, 519716, 524012], // 2.5
                (..=4, ..=64) => [-258044, 467460, 495284, 509868, 521244, 530112, 536764], // 2.5
                (..=4, ..=128) => [-256400, 429360, 484788, 513996, 534872, 549248, 561764], // 2.5
                (..=4, _) => [-293968, 346416, 454052, 512564, 552880, 581824, 605176],  // 2.5, 256
                (..=6, ..=32) => [-254584, 498516, 508000, 512676, 518484, 520732, 523924],  // 6, 3.2
                (..=6, ..=64) => [-255872, 480136, 500364, 511948, 520376, 526064, 530536],  // 6, 3.2
                (..=6, ..=128) => [-258556, 449136, 486728, 509464, 524800, 537440, 546436], // 6, 3.2
                (..=6, _) => [-291960, 388780, 462940, 508184, 537612, 561068, 577716], // 6, 3.2, 256
                (_, ..=32) => [-240136, 468228, 516180, 521120, 524312, 526432, 528440],   // 8, 4.3
                (8, ..=64) => [-247724, 493280, 508576, 517664, 523056, 528700, 531912],   // 8, 4.3
                (8, ..=128) => [-259412, 462060, 490952, 509652, 521120, 531576, 537344],    // 8, 4.3
                (8, _) => [-292668, 417452, 471324, 505256, 528904, 548288, 558952],    // 8, 4.3, 256
                (_, ..=64) => [-252100, 501432, 512112, 519856, 525268, 529068, 531268], // 10, 5.9, 64
                (_, ..=128) => [-258700, 474672, 498892, 512676, 522312, 531988, 535624],    // 10, 5.9, 128
                (_, _) => [-284276, 432616, 485864, 513864, 532212, 548104, 555284]  // 10, 5.9, 256
            }
        } else {    // for 512+
            match (bits_per_seed, slice_len) {
                (..=4, ..=512) => [-507876, 62744, 271980, 397716, 466844, 875820, 932300], // 2.5
                (..=4, ..=1024) => [-271376, 51768, 413248, 622416, 764960, 796420, 812840],  // 2.5
                (5, ..=512) => [-500684, 127632, 299080, 400260, 460460, 506916, 659512],    // 5, 2.9
                (5, ..=1024) => [-245436, 91672, 394928, 579880, 720448, 825984, 902220],    // 5, 2.9
                (6, ..=512) => [-271428, 197720, 364024, 454440, 524716, 556436, 1061164], // 3.2
                (6, ..=1024) => [-222664, 146528, 418284, 583492, 694576, 783288, 886308],   // 3.2
                (7, ..=512) => [-268400, 264880, 400720, 460204, 525576, 569152, 592808],   // 3.7
                (7, ..=1024) => [-202936, 196392, 429984, 573836, 677148, 757040, 816528],   // 3.7
                (8, ..=512) => [-246568, 340896, 451756, 516144, 563236, 601292, 622328], // 4.3
                (8, ..=1024) => [-200684, 237848, 439472, 567460, 654256, 724368, 771408],    // 4.3
                (..=8, _) => [-7912, 59744, 359048, 600448, 760476, 896852, 1372284], // 8, 4.3, 2048
                (9, ..=512) => [-242672, 347612, 468184, 528832, 562996, 598208, 613712], // 5.3
                (9, ..=1024) => [-234128, 245536, 469340, 585236, 656544, 717980, 748012],   // 5.3
                (9, _) => [-8112, 41836, 408788, 644412, 804796, 911868, 1416536],  // 9, 5.3, 2048
                (10, ..=512) => [-263568, 264812, 545444, 623180, 648380, 686508, 698864],    // 5.9
                (10, ..=1024) => [-260816, 269468, 477340, 582764, 652952, 717836, 742580],   // 5.9
                (10, _) => [-6732, 33288, 477032, 686716, 815320, 932852, 1283780], // 10, 5.9, 2048
                (_, ..=512) => [-259616, 271896, 564840, 616568, 650524, 686692, 698016],    // 11, 6.5, 512
                (_, ..=1024) => [-252000, 277984, 492788, 589096, 657884, 718708, 739640], // 11, 6.5, 1024
                (_, _) => [-6264, 34396, 464096, 741576, 852156, 949168, 998628] // 11, 6.5, 2048
            }
        })
    }

    fn slice_len(&self, output_range: usize, bits_per_seed: u8, preferred_slice_len: u16) -> u16 {  //TODO check
        let max_res = match output_range {
            n @ 0..64 => (n/2+1).next_power_of_two() as u16,
            64..1300 => 64,
            1300..9500 => 128,
            9500..12000 => 256,
            12000..140000 => 512,
            _ if bits_per_seed < 6 => return if preferred_slice_len == 0 { 512 } else { preferred_slice_len },
            _ if bits_per_seed < 12 => return if preferred_slice_len == 0 { 1024 } else { preferred_slice_len },   // for 11 2048 gives ~0.002 bit/key smaller size at cost of ~5% longer construction
            _ => return if preferred_slice_len == 0 { 2048 } else { preferred_slice_len }
        };
        if preferred_slice_len != 0 { max_res.min(preferred_slice_len) } else { max_res }
    }
}

/// Selects, among all feasible seeds, the one minimizing the value returned by `seed_evaluator`,
/// and returns it (through `best_seed`). Used for buckets with more keys than `SMALL_BUCKET_LIMIT`;
/// keys are then consumed in groups of four (which is SIMD friendly).
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn best_seed_big<SC: SeedChooser, SE: SeedEvaluator, C: Core>(seed_chooser: &SC, seed_evaluator: SE, best_value: &mut SE::Value, best_seed: &mut u16, used_values: &mut UsedValueSet, keys: &[u64], conf: &C, seeds_num: u16, bucket_nr: usize, first_bucket_in_window: usize) {
    let mut values_used_by_seed = Vec::with_capacity(keys.len());
    let simd_keys = keys.len() / 4 * 4;
    //assert!(simd_keys <= keys.len());
    let seed_eval_data = seed_evaluator.for_bucket(bucket_nr, first_bucket_in_window, conf);
    'outer: for seed in SC::Core::FIRST_SEED..seeds_num {    // seed=0 is special = no seed,
        values_used_by_seed.clear();
        for i in (0..simd_keys).step_by(4) {
            let values = [
                seed_chooser.f(keys[i], seed, conf),
                seed_chooser.f(keys[i+1], seed, conf),
                seed_chooser.f(keys[i+2], seed, conf),
                seed_chooser.f(keys[i+3], seed, conf),
            ];
            let contains = [
                used_values.contain(values[0]),
                used_values.contain(values[1]),
                used_values.contain(values[2]),
                used_values.contain(values[3]),
            ];
            if contains.iter().any(|b| *b) { continue 'outer; }
            //if contains[0] || contains[1] || contains[2] || contains[3] { continue 'outer; }
            values_used_by_seed.push(values[0]);
            values_used_by_seed.push(values[1]);
            values_used_by_seed.push(values[2]);
            values_used_by_seed.push(values[3]);
        }
        //assert!(keys.len() - simd_keys < 4);
        for i in simd_keys..keys.len() {
            let value = seed_chooser.f(keys[i], seed, conf);
            if used_values.contain(value) { continue 'outer; }
            values_used_by_seed.push(value);
        }
        let seed_value = seed_evaluator.eval(&values_used_by_seed, seed_eval_data);
        if seed_value < *best_value {
            values_used_by_seed.sort();
            if values_used_by_seed.windows(2).any(|v| v[0]==v[1]) {
                continue;
            }
            *best_value = seed_value;
            *best_seed = seed;
        }
    }
}

/// Selects, among all feasible seeds, the one minimizing the value returned by `seed_evaluator`,
/// and returns it (through `best_seed`). Used for buckets with at most `SMALL_BUCKET_LIMIT` keys.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn best_seed_small<SC: SeedChooser, SE: SeedEvaluator, C: Core>(seed_chooser: &SC, seed_evaluator: SE, best_value: &mut SE::Value, best_seed: &mut u16, used_values: &mut UsedValueSet, keys: &[u64], conf: &C, seeds_num: u16, bucket_nr: usize, first_bucket_in_window: usize) {
    assert!(keys.len() <= SMALL_BUCKET_LIMIT);  // seems to speeds up a bit
    let mut values_used_by_seed = arrayvec::ArrayVec::<_, SMALL_BUCKET_LIMIT>::new(); // Vec::with_capacity(keys.len());
    let seed_eval_data = seed_evaluator.for_bucket(bucket_nr, first_bucket_in_window, conf);
    'outer: for seed in SC::Core::FIRST_SEED..seeds_num {    // seed=0 is special = no seed,
        values_used_by_seed.clear();
        for key in keys.iter().copied() {
            let value = seed_chooser.f(key, seed, conf);
            if used_values.contain(value) { continue 'outer; }
            values_used_by_seed.push(value);
        }
        let seed_value = seed_evaluator.eval(&values_used_by_seed, seed_eval_data);
        if seed_value < *best_value {
            //for i in 1..values_used_by_seed.len() {
            //    for j in 0..i {
            for i in 0..values_used_by_seed.len() {
                for j in i+1..values_used_by_seed.len() {
                    if values_used_by_seed[i] == values_used_by_seed[j] {
                        continue 'outer;
                    }
                }
            }
            *best_value = seed_value;
            *best_seed = seed;
        }
    }
}

/// The largest bucket size for which the (simpler) `best_seed_small` is used
/// instead of `best_seed_big`.
const SMALL_BUCKET_LIMIT: usize = 8;

/// `SeedChooserCore` that passes all seed bits to hash function and do not use shifting.
/// It allows for bumping (one seed value is reserved for bumping).
#[derive(Clone, Copy)]
pub struct SeedOnlyCore;

impl SeedChooserCore for SeedOnlyCore {
    #[inline(always)] fn f<C: Core>(&self, primary_code: u64, seed: u16, conf: &C) -> usize {
        conf.f(primary_code, seed)
    }

    #[inline(always)]
    fn try_f<SS, C>(&self, seed_size: SS, seeds: &[SS::VecElement], primary_code: u64, conf: &C) -> Option<usize> where SS: SeedSize, C: Core {
        conf.try_f(seed_size, seeds, primary_code)
    }

    /// Read `Self` from the `input`.
    #[inline(always)] fn read(_input: &mut dyn io::Read) -> io::Result<Self> { Ok(Self) }
}

/// [`SeedChooser`] to build (1-)perfect functions.
/// 
/// Can be used with any function type: [`Function`](crate::phast::Function), [`Function2`](crate::phast::Function2), [`Perfect`](crate::phast::Perfect).
/// 
/// It chooses best seed with quite strong hasher (it passes all seed bits to hash function), without shift component,
/// which should lead to small size, but long construction time.
/// 
/// To compare seeds it uses `SE` as a `SeedEvaluator`.
#[derive(Clone, Copy)]
pub struct SeedOnly<SE: SeedEvaluator = ProdOfValues>(pub SE);

impl<SE: SeedEvaluator> SeedChooserConf for SeedOnly<SE> {

    type SeedChooser = Self;

    type BucketEvaluator = Weights;
    
    type Core = SeedOnlyCore;

    type UsedValues = UsedValueSet;

    #[inline(always)] fn core(&self) -> Self::Core { SeedOnlyCore }

    #[inline(always)] fn seed_chooser(&self, _bits_per_seed: u8, _slice_len: u16, _loading_factor_1000: u16) -> Self::SeedChooser {
        *self
    }

    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16, _loading_factor_1000: u16) -> Weights {
        self.0.bucket_evaluator(bits_per_seed, slice_len)
    }

    #[inline] fn empty_used_values(&self) -> Self::UsedValues { Default::default() }

    #[inline(always)] fn add_used(&self, used_values: &mut Self::UsedValues, value: usize) { used_values.add(value); }

    #[inline(always)] fn clear_used(&self, used_values: &mut Self::UsedValues, value: usize) { used_values.remove(value); }

    /// Returns the slice length recommended by the seed evaluator `SE`.
    #[inline(always)] fn slice_len(&self, output_range: usize, bits_per_seed: u8, preferred_slice_len: u16) -> u16 {
        self.0.slice_len(output_range, bits_per_seed, preferred_slice_len)
    }
}

impl<SE: SeedEvaluator> SeedChooser for SeedOnly<SE> {

    #[inline(always)]
    fn best_seed<C: Core>(&self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &C, bits_per_seed: u8, bucket_nr: usize, first_bucket_in_window: usize) -> u16 {
        let mut best_seed = 0;
        let mut best_value = SE::MAX;
        if keys.len() <= SMALL_BUCKET_LIMIT {
            best_seed_small(self, self.0, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed, bucket_nr, first_bucket_in_window)
        } else {
            best_seed_big(self, self.0, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed, bucket_nr, first_bucket_in_window)
        };
        if best_seed != 0 { // can assign seed to the bucket
            for key in keys {
                used_values.add(conf.f(*key, best_seed));
            }
        };
        best_seed
    }
}

/// `SeedChooserCore` that passes all seed bits to hash function and do not use shifting.
/// It does not allow for bumping (each seed value is a real seed).
#[derive(Clone, Copy)]
pub struct SeedNoBumpCore;

impl SeedChooserCore for SeedNoBumpCore {
    const BUMPING: bool = false;
    const FIRST_SEED: u16 = 0;

    #[inline(always)] fn f<C: Core>(&self, primary_code: u64, seed: u16, core: &C) -> usize {
        core.f_nobump(primary_code, seed)
    }

    #[inline(always)] fn read(_input: &mut dyn io::Read) -> io::Result<Self> { Ok(Self) }
}

/// `SeedChooser` that passes all seed bits to hash function and do not use shifting.
/// It does not allow for bumping (each seed value is a real seed).
#[derive(Clone, Copy)]
pub struct SeedOnlyNoBump<SE: SeedEvaluator = ProdOfValues>(pub SE);

impl<SE: SeedEvaluator> SeedChooserConf for SeedOnlyNoBump<SE> {

    type SeedChooser = Self;

    type BucketEvaluator = Weights;
    
    type Core = SeedNoBumpCore;

    type UsedValues = UsedValueSet;

    #[inline] fn empty_used_values(&self) -> Self::UsedValues { Default::default() }

    #[inline(always)] fn add_used(&self, used_values: &mut Self::UsedValues, value: usize) { used_values.add(value); }

    #[inline(always)] fn clear_used(&self, used_values: &mut Self::UsedValues, value: usize) { used_values.remove(value); }

    #[inline(always)] fn core(&self) -> Self::Core { SeedNoBumpCore }

    #[inline(always)] fn seed_chooser(&self, _bits_per_seed: u8, _slice_len: u16, _loading_factor_1000: u16) -> Self::SeedChooser {
        *self
    }

    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16, _loading_factor_1000: u16) -> Weights {
        self.0.bucket_evaluator(bits_per_seed, slice_len)
    }
}

impl<SE: SeedEvaluator> SeedChooser for SeedOnlyNoBump<SE> {
    /// Returns the seed with the lowest evaluation value, or `u16::MAX` if there is no feasible seed
    /// (i.e. the bucket cannot be assigned and, since bumping is not allowed,
    /// the construction must be retried with another hashing seed).
    #[inline]
    fn best_seed<C: Core>(&self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &C, bits_per_seed: u8, bucket_nr: usize, first_bucket_in_window: usize) -> u16 {
        let mut best_seed = u16::MAX;
        let mut best_value = SE::MAX;
        if keys.len() <= SMALL_BUCKET_LIMIT {
            best_seed_small(self, self.0, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed, bucket_nr, first_bucket_in_window)
        } else {
            best_seed_big(self, self.0, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed, bucket_nr, first_bucket_in_window)
        };
        if best_seed != u16::MAX { // can assign seed to the bucket
            for key in keys {
                used_values.add(conf.f_nobump(*key, best_seed));
            }
        };
        best_seed
    }
}


