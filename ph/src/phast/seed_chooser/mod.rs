//! Seed choosers that determine particular PHast variants.

mod utils;
pub use utils::{ComparableF64, ProdCmp, space_lower_bound, perfect_output_range};

mod k;
use std::io;

mod seed;
pub use seed::{SeedOnly, SeedOnlyCore, SeedEvaluator, SeedOnlyNoBump, SeedNoBumpCore, ProdOfValues, SumOfValues};

pub use k::{SeedOnlyK, SeedOnlyKCore, KSeedEvaluator, KSeedEvaluatorConf, ProdOfValuesKEval, bucket_size_normalization_multiplier};

mod shift;
pub use shift::{ShiftOnly, ShiftCore};

mod shift_wrap;
pub use shift_wrap::{ShiftOnlyWrapped, ShiftWrappedCore, ShiftSeedWrapped, ShiftSeedCore};

mod shift_wrap_prod;
pub use shift_wrap_prod::ShiftOnlyProdWrapped;

use crate::{fmph::SeedSize, phast::{BucketEvaluator, Placement, conf::{Core, CoreConf, GenericCore}}};



/// Part of seed chooser stored in the function and needed for evaluation, without stuff needed only for constructing.
pub trait SeedChooserCore: Copy {
    /// Specifies whether bumping is allowed.
    const BUMPING: bool = true;

    /// The lowest seed that does not indicate bumping.
    const FIRST_SEED: u16 = if Self::BUMPING { 1 } else { 0 };

    /// Size of last level of Function2. Important when `extra_shift()>0` (i.e. for `ShiftOnly`).
    const FUNCTION2_THRESHOLD: usize = 4096;

    /// Returns function value for given primary code and seed.
    fn f<C: Core>(&self, primary_code: u64, seed: u16, core: &C) -> usize;

    #[inline(always)]
    fn try_f<SS, C>(&self, seed_size: SS, seeds: &[SS::VecElement], primary_code: u64, core: &C) -> Option<usize> where SS: SeedSize, C: Core {
        let seed = unsafe { seed_size.get_seed(seeds, core.bucket_for(primary_code)) };
        (seed != 0).then(|| self.f(primary_code, seed, core))
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

    /// Writes `self` to the `output`.
    fn write(&self, _output: &mut dyn io::Write) -> io::Result<()> { Ok(()) }

    /// Returns number of bytes which `write` will write.
    fn write_bytes(&self) -> usize { 0 }

    /// Read `Self` from the `input`.
    fn read(input: &mut dyn io::Read) -> io::Result<Self>;
}

/// Provides seed evaluator (that chooses best seed in the bucket) and
/// bucket evaluator (which compares buckets, for choosing the best one).
/// It affects the trade-off between size and evaluation and construction time.
pub trait SeedChooserConf: Clone + Sync {

    type SeedChooser: SeedChooser<Core = Self::Core>;

    type BucketEvaluator: BucketEvaluator;

    type Core: SeedChooserCore;

    type UsedValues: Send;

    fn core(&self) -> Self::Core;

    fn seed_chooser(&self, bits_per_seed: u8, slice_len: u16) -> Self::SeedChooser;

    /// Returns bucket evaluator which compares buckets (for choosing the best one).
    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Self::BucketEvaluator;

    #[inline] fn evaluators(&self, bits_per_seed: u8, slice_len: u16) -> (Self::BucketEvaluator, Self::SeedChooser) {
        (self.bucket_evaluator(bits_per_seed, slice_len), self.seed_chooser(bits_per_seed, slice_len))
    }

    /// Returns maximum number of keys mapped to each output value; `k` of `k`-perfect function.
    #[inline(always)] fn k(&self) -> u16 { self.core().k() }  

    fn empty_used_values(&self) -> Self::UsedValues;

    fn add_used(&self, used_values: &mut Self::UsedValues, value: usize);

    fn clear_used(&self, used_values: &mut Self::UsedValues, value: usize);

    /// Returns slice length suitable to given `output_range`, `bits_per_seed` and `preferred_slice_len`.
    /// 
    /// Usually it returns `preferred_slice_len` (if its not `0`; `0` is for chooser-dependent default)
    /// or lower value for small `output_range`.
    fn slice_len(&self, output_range: usize, bits_per_seed: u8, preferred_slice_len: u16) -> u16 {
        let max_res = match output_range.saturating_sub(self.extra_shift(bits_per_seed) as usize) {
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

    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys.
    #[inline(always)] fn minimal_output_range(&self, num_of_keys: usize) -> usize { self.core().minimal_output_range(num_of_keys) }

    /// Returns output range of (perfect or k-perfect) function for given number of keys and 1000*loading factor.
    #[inline(always)] fn output_range(&self, number_of_keys: usize, loading_factor_1000: u16) -> usize {
        self.core().output_range(number_of_keys, loading_factor_1000)
    }

    /// How much the chooser can add to value over slice length.
    #[inline(always)] fn extra_shift(&self, bits_per_seed: u8) -> u16 { self.core().extra_shift(bits_per_seed) }

    fn generic_f_core<P: Placement>(&self, output_range: usize, num_of_keys: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> GenericCore<P> {
        GenericCore::new(output_range, num_of_keys, bucket_size_100, self.slice_len(output_range, bits_per_seed, preferred_slice_len), self.extra_shift(bits_per_seed))
    }

    #[inline(always)] fn minimal_generic_f_core<P: Placement>(&self, num_of_keys: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> GenericCore<P> {
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

    /// Returns function value for given primary code and seed.
    #[inline(always)]
    fn f<C: Core>(&self, primary_code: u64, seed: u16, conf: &C) -> usize {
        self.core().f::<C>(primary_code, seed, conf)
    }

    #[inline(always)]
    fn try_f<SS, C>(&self, seed_size: SS, seeds: &[SS::VecElement], primary_code: u64, conf: &C) -> Option<usize> where SS: SeedSize, C: Core {
        self.core().try_f::<SS, C>(seed_size, seeds, primary_code, conf)
    }
}

/// Choose best seed in the bucket.
/// It affects the trade-off between size and evaluation and construction time.
pub trait SeedChooser: SeedChooserConf {

    /// Returns best seed to store in seeds array or `u16::MAX` if `NO_BUMPING` is `true` and there is no feasible seed.
    fn best_seed<C: Core>(&self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &C, bits_per_seed: u8, bucket_nr: usize, first_bucket_in_window: usize) -> u16;
}

// This implementation makes possible to give non-default bucket evaluator when [`SeedChooserConf`] is required.
impl<SC: SeedChooserConf, BE: BucketEvaluator> SeedChooserConf for (SC, BE) {
    type SeedChooser = SC::SeedChooser;

    type BucketEvaluator = BE;

    type Core = SC::Core;

    type UsedValues = SC::UsedValues;

    #[inline(always)] fn core(&self) -> Self::Core {
        self.0.core()
    }

    #[inline(always)] fn bucket_evaluator(&self, _bits_per_seed: u8, _slice_len: u16) -> Self::BucketEvaluator {
        self.1.clone()
    }

    #[inline(always)] fn seed_chooser(&self, bits_per_seed: u8, slice_len: u16) -> Self::SeedChooser {
        self.0.seed_chooser(bits_per_seed, slice_len)
    }

    #[inline(always)] fn empty_used_values(&self) -> Self::UsedValues {
        self.0.empty_used_values()
    }

    #[inline(always)] fn add_used(&self, used_values: &mut Self::UsedValues, value: usize) {
        self.0.add_used(used_values, value);
    }

    #[inline(always)] fn clear_used(&self, used_values: &mut Self::UsedValues, value: usize) {
        self.0.clear_used(used_values, value);
    }
}