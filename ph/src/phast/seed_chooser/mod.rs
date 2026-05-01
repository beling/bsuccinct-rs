mod utils;
pub use utils::{ComparableF64, ProdCmp, space_lower_bound, perfect_output_range};

mod k;
use std::io;

mod seed;
pub use seed::{SeedOnly, SeedCore, SeedOnlyNoBump, SeedNoBumpCore, ProdOfValues, SumOfValues};

pub use k::{SeedOnlyK, SeedKCore, KSeedEvaluator, KSeedEvaluatorConf, ProdOfValuesKEval, bucket_size_normalization_multiplier};

mod shift;
pub use shift::{ShiftOnly, ShiftCore};

mod shift_wrap;
pub use shift_wrap::{ShiftOnlyWrapped, ShiftWrappedCore, ShiftSeedWrapped, ShiftSeedCore};

use crate::{fmph::SeedSize, phast::{Weights, conf::{Core, CoreConf}, cyclic::GenericUsedValue}};

use super::conf::GenericCore;



/// Part of seed chooser stored in the function, without stuff needed only for constructing.
pub trait SeedChooserCore: Copy {
    /// Specifies whether bumping is allowed.
    const BUMPING: bool = true;

    /// The lowest seed that does not indicate bumping.
    const FIRST_SEED: u16 = if Self::BUMPING { 1 } else { 0 };

    /// Size of last level of Function2. Important when `extra_shift()>0` (i.e. for `ShiftOnly`).
    const FUNCTION2_THRESHOLD: usize = 4096;

    /// Returns function value for given primary code and seed.
    fn f<C: Core>(&self, primary_code: u64, seed: u16, conf: &C) -> usize;

    #[inline(always)]
    fn try_f<SS, C>(&self, seed_size: SS, seeds: &[SS::VecElement], primary_code: u64, conf: &C) -> Option<usize> where SS: SeedSize, C: Core {
        let seed = unsafe { seed_size.get_seed(seeds, conf.bucket_for(primary_code)) };
        (seed != 0).then(|| self.f(primary_code, seed, conf))
    }

    /// How much the chooser can add to value over slice length.
    #[inline(always)] fn extra_shift(&self, _bits_per_seed: u8) -> u16 { 0 }

    /// Returns maximum number of keys mapped to each output value; `k` of `k`-perfect function.
    #[inline(always)] fn k(&self) -> u16 { 1 }

    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys.
    #[inline(always)] fn minimal_output_range(&self, num_of_keys: usize) -> usize { num_of_keys }

    /// Returns output range of (perfect or k-perfect) function for given number of keys and 1000*loading factor.
    #[inline(always)] fn output_range(&self, number_of_keys: usize, loading_factor_1000: u16) -> usize {
        self.minimal_output_range(perfect_output_range(number_of_keys, loading_factor_1000))
    }

    /// Returns slice length suitable to given `output_range`, `bits_per_seed` and `preferred_slice_len`.
    /// 
    /// Usually it returns `preferred_slice_len` (if its not `0`; `0` is for chooser-dependent default)
    /// or lower value for small `output_range`.
    fn slice_len(&self, output_range: usize, bits_per_seed: u8, preferred_slice_len: u16) -> u16 {
        match output_range.saturating_sub(self.extra_shift(bits_per_seed) as usize) {
            n @ 0..64 => (n/2+1).next_power_of_two() as u16,
            64..1300 => 64,
            1300..9500 => 128,
            9500..12000 => 256,
            12000..140000 => 512,
            _ if bits_per_seed < 6 => if preferred_slice_len == 0 { 512 } else { preferred_slice_len },
            _ if bits_per_seed < 12 => if preferred_slice_len == 0 { 1024 } else { preferred_slice_len },   // for 11 2048 gives ~0.002 bit/key smaller size at cost of ~5% longer construction
            _ => if preferred_slice_len == 0 { 2048 } else { preferred_slice_len }
        }
    }

    fn generic_f_core(&self, output_range: usize, num_of_keys: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> GenericCore {
        GenericCore::new(output_range, num_of_keys, bucket_size_100, self.slice_len(output_range, bits_per_seed, preferred_slice_len), self.extra_shift(bits_per_seed))
    }

    #[inline(always)] fn minimal_generic_f_core(&self, num_of_keys: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> GenericCore {
        self.generic_f_core(self.minimal_output_range(num_of_keys), num_of_keys, bits_per_seed, bucket_size_100, preferred_slice_len)
    }

    #[inline(always)] fn f_core<CC: CoreConf>(&self, output_range: usize, num_of_keys: usize, core: &CC, bits_per_seed: u8) -> CC::Core {
        core.core(output_range, num_of_keys, self.slice_len(output_range, bits_per_seed, core.preferred_slice_len()), self.extra_shift(bits_per_seed))
    }

    #[inline(always)] fn minimal_f_core<CC: CoreConf>(&self, num_of_keys: usize, core: &CC, bits_per_seed: u8) -> CC::Core {
        self.f_core(self.minimal_output_range(num_of_keys), num_of_keys, core, bits_per_seed)
    }

    #[inline(always)] fn f_core_lf<CC: CoreConf>(&self, num_of_keys: usize, loading_factor_1000: u16, core: &CC, bits_per_seed: u8) -> CC::Core {
        self.f_core(self.output_range(num_of_keys, loading_factor_1000), num_of_keys, core, bits_per_seed)
    }

    /// Writes `self` to the `output`.
    fn write(&self, _output: &mut dyn io::Write) -> io::Result<()> { Ok(()) }

    /// Returns number of bytes which `write` will write.
    fn write_bytes(&self) -> usize { 0 }

    /// Read `Self` from the `input`.
    fn read(input: &mut dyn io::Read) -> io::Result<Self>;
}


/// Choose best seed in bucket. It affects the trade-off between size and evaluation and construction time.
pub trait SeedChooser: Clone + Sync {

    type Core: SeedChooserCore;

    fn core(&self) -> Self::Core;

    type UsedValues: GenericUsedValue;

    /// Returns maximum number of keys mapped to each output value; `k` of `k`-perfect function.
    #[inline(always)] fn k(&self) -> u16 { self.core().k() }

    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys.
    #[inline(always)] fn minimal_output_range(&self, num_of_keys: usize) -> usize { self.core().minimal_output_range(num_of_keys) }

    /// Returns output range of (perfect or k-perfect) function for given number of keys and 1000*loading factor.
    #[inline(always)] fn output_range(&self, number_of_keys: usize, loading_factor_1000: u16) -> usize {
        self.core().output_range(number_of_keys, loading_factor_1000)
    }

    #[inline] fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {
        Weights::new(bits_per_seed, slice_len)
    }

    /// How much the chooser can add to value over slice length.
    #[inline(always)] fn extra_shift(&self, bits_per_seed: u8) -> u16 { self.core().extra_shift(bits_per_seed) }

    /*#[inline(always)] fn slice_len(&self, output_range: usize, bits_per_seed: u8, preferred_slice_len: u16) -> u16 {
        self.core().slice_len(output_range, bits_per_seed, preferred_slice_len)
    }*/

/*     fn conf(&self, output_range: usize, input_size: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> Conf {
        let max_shift = self.extra_shift(bits_per_seed);
        let slice_len = slice_len(output_range.saturating_sub(max_shift as usize), bits_per_seed.into(), preferred_slice_len);
        Conf::new(output_range, input_size, bucket_size_100, slice_len, max_shift)
    }

    #[inline(always)] fn conf_for_minimal(&self, num_of_keys: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> Conf {
        self.conf(self.minimal_output_range(num_of_keys), num_of_keys, bits_per_seed, bucket_size_100, preferred_slice_len)
    }

    #[inline(always)] fn conf_for_minimal_p<SS: Copy+Into<u8>>(&self, num_of_keys: usize, params: &Params<SS>) -> Conf {
        self.conf_for_minimal(num_of_keys, params.seed_size.into(), params.bucket_size100, params.preferred_slice_len)
    } */

    fn generic_f_core(&self, output_range: usize, num_of_keys: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> GenericCore {
        self.core().generic_f_core(output_range, num_of_keys, bits_per_seed, bucket_size_100, preferred_slice_len)
    }

    #[inline(always)] fn minimal_generic_f_core(&self, num_of_keys: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> GenericCore {
        self.core().minimal_generic_f_core(num_of_keys, bits_per_seed, bucket_size_100, preferred_slice_len)
    }

    #[inline(always)] fn f_core<CC: CoreConf>(&self, output_range: usize, num_of_keys: usize, core: &CC, bits_per_seed: u8) -> CC::Core {
        self.core().f_core::<CC>(output_range, num_of_keys, core, bits_per_seed)
    }

    #[inline(always)] fn minimal_f_core<CC: CoreConf>(&self, num_of_keys: usize, core: &CC, bits_per_seed: u8) -> CC::Core {
        self.core().minimal_f_core::<CC>(num_of_keys, core, bits_per_seed)
    }

    #[inline(always)] fn f_core_lf<CC: CoreConf>(&self, num_of_keys: usize, loading_factor_1000: u16, core: &CC, bits_per_seed: u8) -> CC::Core {
        self.core().f_core_lf::<CC>(num_of_keys, loading_factor_1000, core, bits_per_seed)
    }

    /// Returns function value for given primary code and seed.
    #[inline(always)]
    fn f<C: Core>(&self, primary_code: u64, seed: u16, conf: &C) -> usize {
        self.core().f::<C>(primary_code, seed, conf)
    }

    #[inline(always)]
    fn try_f<SS, C>(&self, seed_size: SS, seeds: &[SS::VecElement], primary_code: u64, conf: &C) -> Option<usize> where SS: SeedSize, C: Core {
        self.core().try_f::<SS, C>(seed_size, seeds, primary_code, conf)
    }
    
    /// Returns best seed to store in seeds array or `u16::MAX` if `NO_BUMPING` is `true` and there is no feasible seed.
    fn best_seed<C: Core>(&self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &C, bits_per_seed: u8, bucket_nr: usize, first_bucket_in_window: usize) -> u16;
}


/// Evaluate (harness of) seed for (1-)perfect function.
/// Seed with the lowest value is used.
pub trait SeedEvaluator: Clone + Sync {
    /// Type of evaluation value.
    type Value: PartialEq + PartialOrd + Ord;

    /// Value grater than each value returned by `eval`.
    const MAX: Self::Value;

    /// Precalculated data usable to evaluate each seed in the same bucket.
    type BucketData: Copy;

    /// Precalculates data usable to evaluate each seed in the same bucket.
    /// The result is passed to `eval` for each seed in the bucket.
    fn for_bucket<C: Core>(&self, bucket_nr: usize, first_bucket_in_window: usize, core: &C) -> Self::BucketData;

    /// Evaluate (harness of) seed that used given `values`.
    fn eval(&self, values_used_by_seed: &[usize], bucket_data: Self::BucketData) -> Self::Value;

    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {
        Weights::new(bits_per_seed, slice_len)
    }
}
