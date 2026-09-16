//! [`PartialML`] – multi-level map-or-bump function based on PHast.

use dyn_size_of::GetSize;
use voracious_radix_sort::RadixSort;

use crate::{phast::{Conf, SeedChooser, SeedChooserConf, SeedChooserCore, SeedOnlyCore, builder::{BuildConf, build_mt, build_st}, conf::{Core, CoreConf}, evaluator::BucketEvaluator, function::{Level, SeedEx}}, seeds::SeedSize};
use std::hash::{BuildHasher, Hash, RandomState};

/// Minimum size of the part of the minimal output range that is left for the last level of [`PartialML`]:
/// the successive levels are constructed as long as the part of the minimal output range not used yet
/// by the previous levels is at least this large; otherwise it is consumed in full by the last level.
pub const PARTIAL_ML_MINIMAL_LEVEL_THRESHOLD: usize = 4096;

/// Map-or-bump function that assigns different numbers to some keys and `None` to other.
/// It is similar to [`Partial`](crate::phast::Partial), but usually uses less memory (see [`GetSize`](crate::GetSize))
/// and is slower to evaluate (a lookup may have to check more than one level).
/// It supports each loading factor, also greater than 1.
/// 
/// It constructs multiple levels. The first level is for all keys,
/// the second for those bumped at the first level,
/// the third for those bumped at the second level, and so on.
/// Each level uses a disjoint part of the output range, so each key is assigned a value
/// by the first level that does not bump it; only the keys bumped at the last level are assigned `None`.
/// 
/// The behavior depends on the loading factor in the configuration ([`Conf::loading_factor_1000`] stores 1000 * loading factor)
/// and on the minimal output range, i.e. the output range of a minimal (perfect or k-perfect) function for the considered
/// number of keys (`number_of_keys / k` for a k-perfect function and `number_of_keys` for a perfect one;
/// see [`Partial::minimal_output_range`](crate::phast::Partial::minimal_output_range)).
/// The output range of the entire function (`output_range()`) is never below the minimal one.
/// 
/// If the loading factor is less than 1, then the output range of the entire function is greater than the minimal one.
/// Each level is constructed with the minimal output range for the keys it handles (as for a loading factor of 1),
/// except for the last one, which may be smaller (so as not to exceed the desired output range
/// of the entire function, i.e. about `number_of_keys / loading factor / k`).
/// 
/// If the loading factor is 1, exactly one level (with minimal output range) is constructed.
/// In this case, we recommend using [`Partial`](crate::phast::Partial) instead (for simpler representation of the same).
/// 
/// If the loading factor is greater than 1, then the output range of the entire function is equal to the minimal one.
/// Every level except, at most, the last one is constructed with the loading factor given in configuration,
/// so its output range is below the minimal one for the keys it handles.
/// If the part of the minimal output range not used yet by the previous levels falls below [`PARTIAL_ML_MINIMAL_LEVEL_THRESHOLD`],
/// it is fully consumed by the last level.
/// 
/// Can be used with any seed chooser (which specifies a particular PHast variant):
/// [`ShiftOnlyWrapped`](crate::phast::ShiftOnlyWrapped), [`ShiftSeedWrapped`](crate::phast::ShiftSeedWrapped),
/// [`SeedOnly`](crate::phast::SeedOnly), [`SeedOnlyK`](crate::phast::SeedOnlyK).
/// 
/// See:
/// Piotr Beling, Peter Sanders, *PHast - Perfect Hashing made fast*, 2025, <https://arxiv.org/abs/2504.17918>
pub struct PartialML<C, SS, SCC = SeedOnlyCore, S = RandomState> where C: Core, SS: SeedSize {
    /// Seeds and core of the first level (constructed for all keys).
    level0: SeedEx<SS::VecElement, C>,
    /// Seeds, cores and shifts of the successive levels,
    /// constructed for the keys bumped at the previous levels.
    levels: Box<[Level<SS::VecElement, C>]>,
    /// Hasher used to hash keys.
    hasher: S,
    /// Core of the seed chooser used at evaluation time.
    seed_chooser: SCC,
    /// Seed size (number of bits per seed).
    seed_size: SS,
}