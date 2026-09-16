use dyn_size_of::GetSize;
use voracious_radix_sort::RadixSort;

use crate::{phast::{Conf, SeedChooser, SeedChooserConf, SeedChooserCore, SeedOnlyCore, builder::{BuildConf, build_mt, build_st}, conf::{Core, CoreConf}, evaluator::BucketEvaluator, function::SeedEx}, seeds::SeedSize};
use std::hash::{BuildHasher, Hash, RandomState};

const MINIMAL_LEVEL_THRESHOLD: usize = 4096;

/// Map-or-bump function that assigns different numbers to some keys and `None` to other.
/// It is similar to [`Partial`] but usually smaller and slower to evaluate.
/// It supports each loading factor, also grater than 1,
/// but never builds a function with an output range below the minimum.
/// 
/// It constructs multiple levels. The first level is for all keys,
/// the second for those bumped at the first level,
/// the third for those bumped at the second level, and so on.
/// 
/// The behavior depends on the loading factor in the configuration.
/// 
/// If the loading factor is less than 1, then the output range of the entire function is greater than the minimal one.
/// Each level is constructed with the minimal output range (as for a loading factor of 1),
/// except for the last one, which may be smaller (so as not to exceed the desired output range of the entire function).
/// 
/// If the loading factor is 1, exactly one level (with minimal output range) is constructed.
/// In this case, we recommend using [`Partial`] instead (for simpler representation of the same).
/// 
/// If the loading factor is grater than 1, then the output range of the entire function is equal to the minimal one.
/// Every level except, at most, the last one is constructed with the loading factor given in configuration
/// (with output range below minimal). If the remaining output range falls below the `MINIMAL_LEVEL_THRESHOLD`,
/// it is fully consumed by the last level.
/// 
/// Can be used with any seed chooser (which specify a particular PHast variant):
/// [`ShiftOnlyWrapped`](crate::phast::ShiftOnlyWrapped), [`ShiftSeedWrapped`](crate::phast::ShiftSeedWrapped),
/// [`SeedOnly`](crate::phast::SeedOnly), [`SeedOnlyK`](crate::phast::SeedOnlyK).
/// 
/// See:
/// Piotr Beling, Peter Sanders, *PHast - Perfect Hashing made fast*, 2025, <https://arxiv.org/abs/2504.17918>
pub struct PartialML<C, SS, SCC = SeedOnlyCore, S = RandomState> where C: Core, SS: SeedSize {
    /// Seeds and core of the levels.
    seeds: Vec<SeedEx<SS::VecElement, C>>,
    /// Hasher used to hash keys.
    hasher: S,
    /// Core of the seed chooser used at evaluation time.
    seed_chooser: SCC,
    /// Seed size (number of bits per seed).
    seed_size: SS,
}