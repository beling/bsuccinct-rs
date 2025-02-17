
use std::{hash::Hash, time::Instant};

use cpu_time::ThreadTime;

use crate::{BenchmarkResult, BuildStats, Conf, SearchStats, Threads};

pub trait MPHFBuilder<K: Hash> {
    const CAN_DETECT_ABSENCE: bool = true;
    const BUILD_THREADS: Threads = Threads::Both;
    const BUILD_THREADS_DOES_NOT_CHANGE_SIZE: bool = true;

    type MPHF;
    fn new(&self, keys: &[K], use_multiple_threads: bool) -> Self::MPHF;
    fn value(mphf: &Self::MPHF, key: &K, levels: &mut u64) -> Option<u64>;
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

    /// Builds, tests, and returns MPHF.
    fn benchmark(&self, i: &(Vec<K>, Vec<K>), conf: &Conf) -> BenchmarkResult {
        let (h, build) = self.benchmark_build(&i.0, conf);
        let size_bytes = Self::mphf_size(&h);
        let bits_per_value = 8.0 * size_bytes as f64 / i.0.len() as f64;
        if conf.lookup_runs == 0 {
            return BenchmarkResult { included: SearchStats::nan(), absent: SearchStats::nan(), size_bytes, bits_per_value, build }
        }
        let included = SearchStats::new(&i.0, |k, s| Self::value(&h, k, s), conf.verify, conf.lookup_runs);
        assert_eq!(included.absences_found, 0.0, "MPHF does not assign the value for {}% keys of the input", included.absences_found*100.0);
        let absent = if conf.save_details && Self::CAN_DETECT_ABSENCE {
            SearchStats::new(&i.1, |k, s| Self::value(&h, k, s), false, conf.lookup_runs)
        } else {
            SearchStats::nan()
        };
        BenchmarkResult { included, absent, size_bytes, bits_per_value, build }
    }
}