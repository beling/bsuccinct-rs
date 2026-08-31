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
    
    fn for_bucket<C: Core>(&self, _bucket_nr: usize, _first_bucket_in_window: usize, core: &C) -> Self::BucketData {
       core.slice_begin_for_bucket(_bucket_nr).wrapping_sub(95)
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
                (..=4, ..=32) => [-64542, 121567, 125058, 126982, 128486, 129929, 131003], // 2.5
                (..=4, ..=64) => [-64511, 116865, 123821, 127467, 130311, 132528, 134191], // 2.5
                (..=4, ..=128) => [-64100, 107340, 121197, 128499, 133718, 137312, 140441], // 2.5
                (..=4, _) => [-73492, 86604, 113513, 128141, 138220, 145456, 151294],  // 2.5, 256
                (..=6, ..=32) => [-63646, 124629, 127000, 128169, 129621, 130183, 130981],  // 6, 3.2
                (..=6, ..=64) => [-63968, 120034, 125091, 127987, 130094, 131516, 132634],  // 6, 3.2
                (..=6, ..=128) => [-64639, 112284, 121682, 127366, 131200, 134360, 136609], // 6, 3.2
                (..=6, _) => [-72990, 97195, 115735, 127046, 134403, 140267, 144429], // 6, 3.2, 256
                (_, ..=32) => [-60034, 117057, 129045, 130280, 131078, 131608, 132110],   // 8, 4.3
                (8, ..=64) => [-61931, 123320, 127144, 129416, 130764, 132175, 132978],   // 8, 4.3
                (8, ..=128) => [-64853, 115515, 122738, 127413, 130280, 132894, 134336],    // 8, 4.3
                (8, _) => [-73167, 104363, 117831, 126314, 132226, 137072, 139738],    // 8, 4.3, 256
                (_, ..=64) => [-63025, 125358, 128028, 129964, 131317, 132267, 132817], // 10, 5.9, 64
                (_, ..=128) => [-64675, 118668, 124723, 128169, 130578, 132997, 133906],    // 10, 5.9, 128
                (_, _) => [-71069, 108154, 121466, 128466, 133053, 137026, 138821]  // 10, 5.9, 256
            }
        } else {    // for 512+
            match (bits_per_seed, slice_len) {
                (..=4, ..=512) => [-126969, 15686, 67995, 99429, 116711, 218955, 233075], // 2.5
                (..=4, ..=1024) => [-67844, 12942, 103312, 155604, 191240, 199105, 203210],  // 2.5
                (5, ..=512) => [-125171, 31908, 74770, 100065, 115115, 126729, 164878],    // 5, 2.9
                (5, ..=1024) => [-61359, 22918, 98732, 144970, 180112, 206496, 225555],    // 5, 2.9
                (6, ..=512) => [-67857, 49430, 91006, 113610, 131179, 139109, 265291], // 3.2
                (6, ..=1024) => [-55666, 36632, 104571, 145873, 173644, 195822, 221577],   // 3.2
                (7, ..=512) => [-67100, 66220, 100180, 115051, 131394, 142288, 148202],   // 3.7
                (7, ..=1024) => [-50734, 49098, 107496, 143459, 169287, 189260, 204132],   // 3.7
                (8, ..=512) => [-61642, 85224, 112939, 129036, 140809, 150323, 155582], // 4.3
                (8, ..=1024) => [-50171, 59462, 109868, 141865, 163564, 181092, 192852],    // 4.3
                (..=8, _) => [-1978, 14936, 89762, 150112, 190119, 224213, 343071], // 8, 4.3, 2048
                (9, ..=512) => [-60668, 86903, 117046, 132208, 140749, 149552, 153428], // 5.3
                (9, ..=1024) => [-58532, 61384, 117335, 146309, 164136, 179495, 187003],   // 5.3
                (9, _) => [-2028, 10459, 102197, 161103, 201199, 227967, 354134],  // 9, 5.3, 2048
                (10, ..=512) => [-65892, 66203, 136361, 155795, 162095, 171627, 174716],    // 5.9
                (10, ..=1024) => [-65204, 67367, 119335, 145691, 163238, 179459, 185645],   // 5.9
                (10, _) => [-1683, 8322, 119258, 171679, 203830, 233213, 320945], // 10, 5.9, 2048
                (_, ..=512) => [-64904, 67974, 141210, 154142, 162631, 171673, 174504],    // 11, 6.5, 512
                (_, ..=1024) => [-63000, 69496, 123197, 147274, 164471, 179677, 184910], // 11, 6.5, 1024
                (_, _) => [-1566, 8599, 116024, 185394, 213039, 237292, 249657] // 11, 6.5, 2048
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

    #[inline(always)] fn seed_chooser(&self, _bits_per_seed: u8, _slice_len: u16) -> Self::SeedChooser {
        *self
    }

    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {
        self.0.bucket_evaluator(bits_per_seed, slice_len)
    }

    #[inline] fn empty_used_values(&self) -> Self::UsedValues { Default::default() }

    #[inline(always)] fn add_used(&self, used_values: &mut Self::UsedValues, value: usize) { used_values.add(value); }

    #[inline(always)] fn clear_used(&self, used_values: &mut Self::UsedValues, value: usize) { used_values.remove(value); }

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

    #[inline(always)] fn seed_chooser(&self, _bits_per_seed: u8, _slice_len: u16) -> Self::SeedChooser {
        *self
    }

    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {
        self.0.bucket_evaluator(bits_per_seed, slice_len)
    }
}

impl<SE: SeedEvaluator> SeedChooser for SeedOnlyNoBump<SE> {
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


