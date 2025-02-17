
use std::{hash::Hash, hint::black_box, time::Instant};

use bitm::{BitAccess, BitVec};
use cpu_time::{ProcessTime, ThreadTime};

use crate::{BenchmarkResult, BuildStats, Conf, SearchStats, Threads};

pub trait MPHFBuilder<K: Hash> {
    const CAN_DETECT_ABSENCE: bool = true;
    const BUILD_THREADS: Threads = Threads::Both;
    const BUILD_THREADS_DOES_NOT_CHANGE_SIZE: bool = true;

    type MPHF;
    type Value;
    fn new(&self, keys: &[K], use_multiple_threads: bool) -> Self::MPHF;
    fn value_ex(mphf: &Self::MPHF, key: &K, levels: &mut u64) -> Option<u64>;
    fn value(mphf: &Self::MPHF, key: &K) -> Self::Value;
    fn mphf_size(mphf: &Self::MPHF) -> usize;

    /// Builds the MPHF and measure the CPU thread time of building. Returns: the MPHF, the time measured.
    fn benchmark_build_st(&self, keys: &[K], repeats: u32) -> (Self::MPHF, BuildStats) {
        let start_moment = ThreadTime::now();   // ProcessTime?
        for _ in 1..repeats { self.new(keys, false); }
        let h = self.new(keys, false);
        let build_time_seconds = start_moment.elapsed().as_secs_f64();
        (h, BuildStats { time_st: build_time_seconds / repeats as f64, time_mt: f64::NAN } )
    }

    /// Builds the MPHF and measure the CPU thread time of building. Returns: the MPHF, the time measured.
    fn benchmark_build_mt(&self, keys: &[K], repeats: u32) -> (Self::MPHF, BuildStats) {
        let start_moment =  Instant::now();
        for _ in 1..repeats { self.new(keys, true); }
        let h = self.new(keys, true);
        let build_time_seconds = start_moment.elapsed().as_secs_f64();
        (h, BuildStats { time_st: f64::NAN, time_mt: build_time_seconds / repeats as f64 } )
    }

    /// Builds MPHF and measure the time of building. Returns: MPHF, and either single-thread and multiple-thread time of building, and one NaN.
    fn benchmark_build(&self, keys: &[K], conf: &Conf) -> (Self::MPHF, BuildStats) {
        match (Self::BUILD_THREADS, conf.threads) {
            (Threads::Single, _) | (Threads::Both, Threads::Single) => self.benchmark_build_st(keys, conf.build_runs),
            (Threads::Multi, _) | (Threads::Both, Threads::Multi) => self.benchmark_build_mt(keys, conf.build_runs),
            _ => {  // (Both, Both) pair
                let (mphf, result_st) = self.benchmark_build_st(keys, conf.build_runs);
                let size_st = Self::BUILD_THREADS_DOES_NOT_CHANGE_SIZE.then(|| Self::mphf_size(&mphf));
                drop(mphf);
                let (mphf, mut result_mt) = self.benchmark_build_mt(keys, conf.build_runs);
                if let Some(size_st) = size_st {
                    let size_mt = Self::mphf_size(&mphf);
                    if size_st != size_mt {
                        eprintln!("WARNING: ST/MT differ in sizes, {} != {}", size_st, size_mt);
                    }
                }
                result_mt.time_st = result_st.time_st;
                (mphf, result_mt)
            }
        }
    }

    /// Lookups for all keys in `input` and returns search statistics.
    /// If `verify` is `true`, checks if the `mphf` is valid for the given `input`.
    fn benchmark_lookup(&self, mphf: &Self::MPHF, input: &[K], verify: bool, lookup_runs: u32) -> SearchStats {
        if input.is_empty() || lookup_runs == 0 { return SearchStats::nan(); }
        let mut extra_levels_searched = 0u64;
        let mut not_found = 0usize;
        if verify {
            let mut seen = Box::<[u64]>::with_zeroed_bits(input.len());
            for v in input {
                if let Some(index) = Self::value_ex(mphf, v, &mut extra_levels_searched) {
                    let index = index as usize;
                    assert!(index < input.len(), "MPHF assigns too large value {}>{}.", index, input.len());
                    assert!(!seen.get_bit(index), "MPHF assigns the same value to two keys of input.");
                    seen.set_bit(index);
                } else {
                    not_found += 1;
                }
            }
        } else {
            for v in input {
                if Self::value_ex(mphf, v, &mut extra_levels_searched).is_none() {
                    not_found += 1;
                }
            }
        }
        let start_process_moment = ProcessTime::now();
        for _ in 0..lookup_runs {
            let mut dump = 0;
            for v in input { black_box(Self::value_ex(mphf, v, &mut dump)); }
        }
        let seconds = start_process_moment.elapsed().as_secs_f64();
        let divider = input.len() as f64;
        SearchStats {
            avg_deep: extra_levels_searched as f64 / divider,
            avg_lookup_time: seconds / (divider * lookup_runs as f64),
            absences_found: not_found as f64 / divider
        }
    }

    /// Builds, tests, and returns MPHF.
    fn benchmark(&self, i: &(Vec<K>, Vec<K>), conf: &Conf) -> BenchmarkResult {
        let (h, build) = self.benchmark_build(&i.0, conf);
        let size_bytes = Self::mphf_size(&h);
        let bits_per_value = 8.0 * size_bytes as f64 / i.0.len() as f64;
        if conf.lookup_runs == 0 {
            return BenchmarkResult { included: SearchStats::nan(), absent: SearchStats::nan(), size_bytes, bits_per_value, build }
        }
        let included = self.benchmark_lookup(&h, &i.0, conf.verify, conf.lookup_runs);
        assert_eq!(included.absences_found, 0.0, "MPHF does not assign the value for {}% keys of the input", included.absences_found*100.0);
        let absent = if conf.save_details && Self::CAN_DETECT_ABSENCE {
            self.benchmark_lookup(&h, &i.1, false, conf.lookup_runs)
        } else {
            SearchStats::nan()
        };
        BenchmarkResult { included, absent, size_bytes, bits_per_value, build }
    }
}