//! [`PartialML`] – multi-level map-or-bump function based on PHast.

use dyn_size_of::GetSize;
use seedable_hash::{BuildDefaultSeededHasher, BuildSeededHasher};
use std::hash::Hash;

use crate::{phast::{Conf, SeedChooserConf, SeedChooserCore, SeedOnlyCore, conf::{Core, CoreConf}, function::{Level, SeedEx}, perfect::{build_level_from_slice_no_bitmap_mt, build_level_from_slice_no_bitmap_st, build_level_no_bitmap_mt, build_level_no_bitmap_st}}, seeds::SeedSize};

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
/// Each level hashes keys with its own seed (the level number).
/// Each level uses a disjoint part of the output range, so each key is assigned a value
/// by the first level that does not bump it; only the keys bumped at the last level are assigned `None`.
/// 
/// The behavior depends on the loading factor in the configuration ([`Conf::loading_factor_1000`] stores 1000 * loading factor)
/// and on the minimal output range, i.e. the output range of a minimal (perfect or k-perfect) function for the considered
/// number of keys (`number_of_keys / k` for a k-perfect function and `number_of_keys` for a perfect one;
/// see [`Partial::minimal_output_range`](crate::phast::Partial::minimal_output_range)).
/// The output range of the entire function ([`PartialML::output_range`]) is never below the minimal one.
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
pub struct PartialML<C, SS, SCC = SeedOnlyCore, S = BuildDefaultSeededHasher> where C: Core, SS: SeedSize {
    /// Seeds and core of the first level (constructed for all keys).
    level0: SeedEx<SS::VecElement, C>,
    /// Seeds, cores and shifts of the successive levels,
    /// constructed for the keys bumped at the previous levels.
    levels: Box<[Level<SS::VecElement, C>]>,
    /// Hasher used to hash keys; each level uses the level number as the seed.
    hasher: S,
    /// Core of the seed chooser used at evaluation time.
    seed_chooser: SCC,
    /// Seed size (number of bits per seed).
    seed_size: SS,
}

impl<C: Core, SS: SeedSize, SCC, S> GetSize for PartialML<C, SS, SCC, S> {
    fn size_bytes_dyn(&self) -> usize { self.level0.size_bytes_dyn() + self.levels.size_bytes_dyn() }
    fn size_bytes_content_dyn(&self) -> usize { self.level0.size_bytes_content_dyn() + self.levels.size_bytes_content_dyn() }
    const USES_DYN_MEM: bool = true;
}

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore, S> PartialML<C, SS, SCC, S> {
    /// Returns the number of levels of `self` (including the first one).
    #[inline] pub fn levels(&self) -> usize { self.levels.len() + 1 }

    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys,
    /// i.e. 1 + maximum value that minimal function can return.
    #[inline(always)] pub fn minimal_output_range(&self, num_of_keys: usize) -> usize {
        self.seed_chooser.minimal_output_range(num_of_keys)
    }

    /// Returns output range of `self`, i.e. 1 + maximum value that `self` can return
    /// (the total output range of all its levels).
    pub fn output_range(&self) -> usize {
        if let Some(last_level) = self.levels.last() {
            last_level.shift + last_level.seeds.core.output_range(self.seed_chooser, self.seed_size.into())
        } else {
            self.level0.core.output_range(self.seed_chooser, self.seed_size.into())
        }
    }
}
impl<C: Core, SS: SeedSize, SCC: SeedChooserCore, S: BuildSeededHasher> PartialML<C, SS, SCC, S> {
    /// Returns value assigned to the given `key` or `None`.
    /// 
    /// The returned value is in the range from `0` (inclusive) to [`PartialML::output_range`] (exclusive).
    /// `key` must come from the input key collection given during construction;
    /// for any other key the result is unspecified (it can be `None` or any value).
    #[inline(always)]
    pub fn get<K>(&self, key: &K) -> Option<usize> where K: Hash + ?Sized {
        let key_hash = self.hasher.hash_one(key, 0);
        // SAFETY: the bucket number returned by `bucket_for` is always within the level's seeds array.
        let seed = unsafe { self.level0.seed_for(self.seed_size, key_hash) };
        if seed != 0 { return Some(self.seed_chooser.f(key_hash, seed, &self.level0.core)); }
        for (level_nr, level) in self.levels.iter().enumerate() {
            let key_hash = self.hasher.hash_one(key, level_nr as u64 + 1);
            // SAFETY: the bucket number returned by `bucket_for` is always within the level's seeds array.
            let seed = unsafe { level.seeds.seed_for(self.seed_size, key_hash) };
            if seed != 0 { return Some(self.seed_chooser.f(key_hash, seed, &level.seeds.core) + level.shift); }
        }
        None
    }

    /// Common construction: builds the first level with `build_first` for all `num_of_keys` keys,
    /// giving it the `first_range` output range computed by this method,
    /// then repeatedly builds the next levels with `build_level` as long as there are bumped keys
    /// and a part of the output range not used yet by the previous levels.
    /// Returns the function and the number of keys without assigned values
    /// (the keys left in the vector built by `build_first` and the following `build_level` calls).
    #[inline]
    fn _new<K, BF, BL, CC, SC>(build_first: BF, build_level: BL, conf: Conf<SS, CC, S>, seed_chooser: &SC, num_of_keys: usize) -> (Self, usize)
        where BF: FnOnce(&Conf<SS, CC, S>, usize) -> (Vec<K>, SeedEx<SS::VecElement, C>),
            BL: Fn(&mut Vec<K>, usize, u64, &Conf<SS, CC, S>) -> SeedEx<SS::VecElement, C>,
            K: Hash, CC: CoreConf<Core = C>, SC: SeedChooserConf<Core = SCC>
    {
        let loading_factor_1000 = conf.loading_factor_1000;
        let minimal_range = seed_chooser.minimal_output_range(num_of_keys);
        // The output range of the entire function is never below the minimal one.
        let total_range = if loading_factor_1000 >= 1000 { minimal_range }
            else { seed_chooser.output_range(num_of_keys, loading_factor_1000) };
        // With a loading factor below 1, the first level gets the minimal output range (as for a loading factor of 1),
        // so that the keys bumped from it fit in the still unused part of the desired output range of the function.
        let first_range = if loading_factor_1000 > 1000 { seed_chooser.output_range(num_of_keys, loading_factor_1000) }
            else { minimal_range };

        let (mut keys, level0) = build_first(&conf, first_range);

        let mut levels = Vec::new();
        let mut shift = first_range;
        let mut remaining = total_range - first_range;  // part of the output range not used yet by the previous levels
        let mut level_nr = 1u64;
        while !keys.is_empty() && remaining > 0 {
            let range = if loading_factor_1000 > 1000 { seed_chooser.output_range(keys.len(), loading_factor_1000) }
                else { seed_chooser.minimal_output_range(keys.len()) };
            // The remainder of the output range is no longer split if it is smaller than
            // PARTIAL_ML_MINIMAL_LEVEL_THRESHOLD; it is then consumed in full by the last level.
            let last_level = range + PARTIAL_ML_MINIMAL_LEVEL_THRESHOLD > remaining;
            let range = if last_level { remaining } else { range };
            let seeds = build_level(&mut keys, range, level_nr, &conf);
            levels.push(Level { seeds, shift });
            shift += range;
            remaining -= range;
            level_nr += 1;
        }

        (Self {
            level0,
            levels: levels.into_boxed_slice(),
            hasher: conf.hasher,
            seed_chooser: seed_chooser.core(),
            seed_size: conf.seed_size,
        }, keys.len())
    }

    /// Constructs [`PartialML`] for given `keys` (given as a vector), configuration and seed chooser,
    /// using a single thread. Returns the function and the number of keys without assigned values
    /// (i.e. the keys bumped at the last level). `keys` cannot contain duplicates.
    pub fn with_vec_conf_sc_u<K, CC, SC>(mut keys: Vec<K>, conf: Conf<SS, CC, S>, seed_chooser: SC) -> (Self, usize)
        where K: Hash, CC: CoreConf<Core = C>, SC: SeedChooserConf<Core = SCC>
    {
        let num_of_keys = keys.len();
        Self::_new(|conf, first_range| {
            let level0 = build_level_no_bitmap_st(&mut keys, first_range, conf, seed_chooser.clone(), 0);
            (keys, level0)
        }, |keys, range, level_nr, conf| {
            build_level_no_bitmap_st(keys, range, conf, seed_chooser.clone(), level_nr)
        }, conf, &seed_chooser, num_of_keys)
    }

    /// Constructs [`PartialML`] for given `keys` (given as a vector), configuration and seed chooser,
    /// using multiple (given number of) threads. Returns the function and the number of keys
    /// without assigned values (i.e. the keys bumped at the last level). `keys` cannot contain duplicates.
    pub fn with_vec_conf_threads_sc_u<K, CC, SC>(mut keys: Vec<K>, conf: Conf<SS, CC, S>, threads_num: usize, seed_chooser: SC) -> (Self, usize)
        where K: Hash+Sync+Send, S: Sync, CC: CoreConf<Core = C>, SC: SeedChooserConf<Core = SCC>
    {
        if threads_num == 1 { return Self::with_vec_conf_sc_u(keys, conf, seed_chooser); }
        let num_of_keys = keys.len();
        Self::_new(|conf, first_range| {
            let level0 = build_level_no_bitmap_mt(&mut keys, first_range, conf, threads_num, seed_chooser.clone(), 0);
            (keys, level0)
        }, |keys, range, level_nr, conf| {
            build_level_no_bitmap_mt(keys, range, conf, threads_num, seed_chooser.clone(), level_nr)
        }, conf, &seed_chooser, num_of_keys)
    }

    /// Constructs [`PartialML`] for given `keys` (given as a slice), configuration and seed chooser,
    /// using a single thread. Returns the function and the number of keys without assigned values
    /// (i.e. the keys bumped at the last level). `keys` cannot contain duplicates.
    /// 
    /// # Example
    /// ```
    /// use ph::phast::{Conf, PartialML, ProdOfValues, SeedOnly};
    /// 
    /// let keys: Vec<u16> = (0..1000).collect();
    /// // The loading factor of 1 (the default) gives exactly one level with the minimal output range:
    /// let (f, unassigned) = PartialML::with_slice_conf_sc_u(&keys, Conf::generic8(400), SeedOnly(ProdOfValues));
    /// assert_eq!(f.levels(), 1);
    /// assert_eq!(f.output_range(), f.minimal_output_range(keys.len()));
    /// assert_eq!(unassigned, keys.iter().filter(|key| f.get(*key).is_none()).count());
    /// 
    /// // A loading factor greater than 1 splits the construction into more levels,
    /// // but the output range of the entire function is still the minimal one:
    /// let mut conf = Conf::generic8(400);
    /// conf.loading_factor_1000 = 1500;
    /// let (f, unassigned) = PartialML::with_slice_conf_threads_sc_u(&keys, conf, 4, SeedOnly(ProdOfValues));
    /// assert!(f.levels() > 1);
    /// assert_eq!(f.output_range(), f.minimal_output_range(keys.len()));
    /// assert_eq!(unassigned, keys.iter().filter(|key| f.get(*key).is_none()).count());
    /// ```
    pub fn with_slice_conf_sc_u<K, CC, SC>(keys: &[K], conf: Conf<SS, CC, S>, seed_chooser: SC) -> (Self, usize)
        where K: Hash+Clone, CC: CoreConf<Core = C>, SC: SeedChooserConf<Core = SCC>
    {
        Self::_new(|conf, first_range| {
            build_level_from_slice_no_bitmap_st(keys, first_range, conf, seed_chooser.clone(), 0)
        }, |keys, range, level_nr, conf| {
            build_level_no_bitmap_st(keys, range, conf, seed_chooser.clone(), level_nr)
        }, conf, &seed_chooser, keys.len())
    }

    /// Constructs [`PartialML`] for given `keys` (given as a slice), configuration and seed chooser,
    /// using multiple (given number of) threads. Returns the function and the number of keys
    /// without assigned values (i.e. the keys bumped at the last level). `keys` cannot contain duplicates.
    pub fn with_slice_conf_threads_sc_u<K, CC, SC>(keys: &[K], conf: Conf<SS, CC, S>, threads_num: usize, seed_chooser: SC) -> (Self, usize)
        where K: Hash+Sync+Send+Clone, S: Sync, CC: CoreConf<Core = C>, SC: SeedChooserConf<Core = SCC>
    {
        if threads_num == 1 { return Self::with_slice_conf_sc_u(keys, conf, seed_chooser); }
        Self::_new(|conf, first_range| {
            build_level_from_slice_no_bitmap_mt(keys, first_range, conf, threads_num, seed_chooser.clone(), 0)
        }, |keys, range, level_nr, conf| {
            build_level_no_bitmap_mt(keys, range, conf, threads_num, seed_chooser.clone(), level_nr)
        }, conf, &seed_chooser, keys.len())
    }
}


#[cfg(test)]
pub(crate) mod tests {
    use crate::phast::{Partial, ProdOfValues, SeedOnly, SeedOnlyK};
    use crate::utils::{verify_partial_kphf, verify_partial_phf};

    use super::*;

    /// Returns the number of `keys` that `f` assigns no value.
    fn unassigned_count<K, C, SS, SCC, S>(f: &PartialML<C, SS, SCC, S>, keys: &[K]) -> usize
        where K: Hash, C: Core, SS: SeedSize, SCC: SeedChooserCore, S: BuildSeededHasher
    {
        keys.iter().filter(|key| f.get(*key).is_none()).count()
    }

    #[test]
    fn test_small() {
        let input = [1u16, 2, 3, 4, 5];
        let (f, unassigned) = PartialML::with_slice_conf_sc_u(&input, Conf::generic8(400), SeedOnly(ProdOfValues));
        assert_eq!(f.levels(), 1);      // a loading factor of 1 builds a single level
        assert_eq!(f.output_range(), f.minimal_output_range(input.len()));
        verify_partial_phf(f.output_range(), &input, |key| f.get(key));
        assert_eq!(unassigned, unassigned_count(&f, &input));
        assert!(f.size_bytes_dyn() > 0);
    }

    #[test]
    fn test_medium() {
        let input: Box<[u16]> = (0..1000).collect();
        let (f, unassigned) = PartialML::with_slice_conf_sc_u(&input, Conf::generic8(400), SeedOnly(ProdOfValues));
        assert_eq!(f.levels(), 1);
        assert_eq!(f.output_range(), f.minimal_output_range(input.len()));
        verify_partial_phf(f.output_range(), &input[..], |key| f.get(key));
        assert_eq!(unassigned, unassigned_count(&f, &input));
    }

    #[test]
    fn test_lf_above_1() {
        let input: Box<[u16]> = (0..1000).collect();
        let mut conf = Conf::generic8(400);
        conf.loading_factor_1000 = 1500;
        let (f, unassigned) = PartialML::with_slice_conf_sc_u(&input, conf, SeedOnly(ProdOfValues));
        assert!(f.levels() > 1);    // the keys bumped at a level are mapped by the following levels
        assert_eq!(f.output_range(), f.minimal_output_range(input.len()));
        verify_partial_phf(f.output_range(), &input[..], |key| f.get(key));
        assert_eq!(unassigned, unassigned_count(&f, &input));
    }

    #[test]
    fn test_lf_below_1() {
        let input: Box<[u16]> = (0..1000).collect();
        let seed_chooser = SeedOnly(ProdOfValues);
        let desired_range = seed_chooser.output_range(input.len(), 950);
        let mut conf = Conf::generic8(400);
        conf.loading_factor_1000 = 950;
        let (f, unassigned) = PartialML::with_slice_conf_sc_u(&input, conf, seed_chooser);
        assert!(f.output_range() > f.minimal_output_range(input.len()));
        assert!(f.output_range() <= desired_range);     // the desired output range is not exceeded
        verify_partial_phf(f.output_range(), &input[..], |key| f.get(key));
        assert_eq!(unassigned, unassigned_count(&f, &input));
    }

    /// Checks the case where the remainder of the output range is still large enough
    /// to be split into more levels (so more than two levels are constructed).
    #[test]
    fn test_many_levels() {
        let input: Box<[u16]> = (0..20000).collect();
        let mut conf = Conf::generic8(400);
        conf.loading_factor_1000 = 3000;
        let (f, unassigned) = PartialML::with_slice_conf_sc_u(&input, conf, SeedOnly(ProdOfValues));
        assert!(f.levels() > 2);
        assert_eq!(f.output_range(), f.minimal_output_range(input.len()));
        verify_partial_phf(f.output_range(), &input[..], |key| f.get(key));
        assert_eq!(unassigned, unassigned_count(&f, &input));
    }

    #[test]
    fn test_k_perfect() {
        let input: Box<[u16]> = (0..1000).collect();
        for loading_factor_1000 in [1000, 1500] {
            let mut conf = Conf::generic8(400);
            conf.loading_factor_1000 = loading_factor_1000;
            let (f, unassigned) = PartialML::with_slice_conf_sc_u(&input, conf, SeedOnlyK::with_evaluator(3, ProdOfValues));
            if loading_factor_1000 > 1000 {
                assert_eq!(f.output_range(), f.minimal_output_range(input.len()));
            }
            verify_partial_kphf(3, f.output_range(), &input[..], |key| f.get(key));
            assert_eq!(unassigned, unassigned_count(&f, &input));
        }
    }

    #[test]
    fn test_turbo_core() {
        let input: Box<[u16]> = (0..1000).collect();
        let mut conf = Conf::turbo();
        conf.loading_factor_1000 = 1500;
        let (f, unassigned) = PartialML::with_slice_conf_sc_u(&input, conf, SeedOnly(ProdOfValues));
        assert_eq!(f.output_range(), f.minimal_output_range(input.len()));
        verify_partial_phf(f.output_range(), &input[..], |key| f.get(key));
        assert_eq!(unassigned, unassigned_count(&f, &input));
    }

    /// Checks that a single level built by `PartialML` (a loading factor of 1)
    /// is the same as the one built by [`Partial`] for the same hashes.
    #[test]
    fn test_vs_partial() {
        let input: Box<[u16]> = (0..1000).collect();
        // The default hasher is deterministic, so an independently built instance can be used
        // to prepare the hashes passed to `Partial` (note that `Partial` sorts them in place):
        let hasher = Conf::generic8(400).hasher;
        let mut hashes: Box<[u64]> = input.iter().map(|key| hasher.hash_one(key, 0)).collect();
        let (f, unassigned) = PartialML::with_slice_conf_sc_u(&input, Conf::generic8(400), SeedOnly(ProdOfValues));
        let (p, partial_unassigned) = Partial::with_hashes_conf_sc_u(&mut hashes, &Conf::generic8(400), SeedOnly(ProdOfValues));
        assert_eq!(unassigned, partial_unassigned);
        for key in input.iter() {
            assert_eq!(f.get(key), p.get_for_hash(hasher.hash_one(key, 0)));
        }
    }

    #[test]
    fn test_determinism() {
        let input: Box<[u16]> = (0..1000).collect();
        let mut conf1 = Conf::generic8(400);
        conf1.loading_factor_1000 = 1300;
        let mut conf2 = Conf::generic8(400);
        conf2.loading_factor_1000 = 1300;
        let (f1, unassigned1) = PartialML::with_slice_conf_sc_u(&input, conf1, SeedOnly(ProdOfValues));
        let (f2, unassigned2) = PartialML::with_slice_conf_sc_u(&input, conf2, SeedOnly(ProdOfValues));
        assert_eq!(unassigned1, unassigned2);
        assert_eq!(f1.levels(), f2.levels());
        for key in input.iter() { assert_eq!(f1.get(key), f2.get(key)); }
    }

    /// Checks that multi-threaded construction (of vectors and slices, for a loading factor
    /// greater than 1 and equal to 1) gives the same results as the single-threaded one.
    #[test]
    fn test_mt() {
        let input: Box<[u16]> = (0..1000).collect();
        let mut conf = Conf::generic8(400);
        conf.loading_factor_1000 = 1500;
        let (fs, unassigned_s) = PartialML::with_slice_conf_sc_u(&input, conf, SeedOnly(ProdOfValues));
        let mut conf = Conf::generic8(400);
        conf.loading_factor_1000 = 1500;
        let (fv, unassigned_v) = PartialML::with_vec_conf_threads_sc_u(input.to_vec(), conf, 4, SeedOnly(ProdOfValues));
        let mut conf = Conf::generic8(400);
        conf.loading_factor_1000 = 1500;
        let (fsl, unassigned_sl) = PartialML::with_slice_conf_threads_sc_u(&input, conf, 4, SeedOnly(ProdOfValues));
        assert_eq!(unassigned_s, unassigned_v);
        assert_eq!(unassigned_s, unassigned_sl);
        assert_eq!(fs.levels(), fv.levels());
        assert_eq!(fs.levels(), fsl.levels());
        for key in input.iter() {
            assert_eq!(fs.get(key), fv.get(key));
            assert_eq!(fs.get(key), fsl.get(key));
        }
        verify_partial_phf(fv.output_range(), &input[..], |key| fv.get(key));
        assert_eq!(unassigned_v, unassigned_count(&fv, &input));

        // A loading factor of 1 gives a single level, again with the same result as single-threaded:
        let (f1, unassigned_1) = PartialML::with_slice_conf_sc_u(&input, Conf::generic8(400), SeedOnly(ProdOfValues));
        let (fm, unassigned_m) = PartialML::with_vec_conf_threads_sc_u(input.to_vec(), Conf::generic8(400), 4, SeedOnly(ProdOfValues));
        assert_eq!(f1.levels(), fm.levels());
        assert_eq!(unassigned_1, unassigned_m);
        for key in input.iter() { assert_eq!(f1.get(key), fm.get(key)); }
    }

    #[test]
    fn test_empty() {
        let input: [u16; 0] = [];
        let (f, unassigned) = PartialML::with_slice_conf_sc_u(&input, Conf::generic8(400), SeedOnly(ProdOfValues));
        assert_eq!(f.levels(), 1);
        assert_eq!(f.output_range(), 0);
        assert_eq!(unassigned, 0);
    }

    /// TEMPORARY (to be removed): measures the construction time of `PartialML` and `Perfect`.
    #[test]
    #[ignore]
    fn measure_construction_time() {
        use std::hint::black_box;
        use std::time::Instant;
        use crate::phast::Perfect;

        let input: Box<[u64]> = (0..1_000_000u64).map(|i| i.wrapping_mul(0x9E37_79B9_7F4A_7C15)).collect();
        for loading_factor_1000 in [1000u16, 1500] {
            let mut best = f64::MAX;
            for _ in 0..3 {
                let mut conf = Conf::generic8(400);
                conf.loading_factor_1000 = loading_factor_1000;
                let start = Instant::now();
                let (f, unassigned) = PartialML::with_slice_conf_sc_u(&input, conf, SeedOnly(ProdOfValues));
                let elapsed = start.elapsed().as_secs_f64();
                black_box((&f, unassigned));
                best = best.min(elapsed);
            }
            println!("PartialML (lf = {loading_factor_1000}) build: {:.2} ms", best * 1000.0);
        }
        let mut best = f64::MAX;
        for _ in 0..3 {
            let start = Instant::now();
            let f = Perfect::with_slice_conf_sc(&input[..], Conf::generic8(400), SeedOnly(ProdOfValues));
            let elapsed = start.elapsed().as_secs_f64();
            black_box(&f);
            best = best.min(elapsed);
        }
        println!("Perfect (slice_st, lf = 1000) build: {:.2} ms", best * 1000.0);
    }
}
