
use bitm::{BitAccess, BitVec};
use std::fmt::{Display, Formatter};
use std::hint::black_box;
use std::hash::Hash;
use cpu_time::ProcessTime;

/// Represents average (per value) lookup: level searched, times (seconds).
pub struct SearchStats {
    /// average number of level searched
    pub avg_deep: f64,
    /// average lookup time
    pub avg_lookup_time: f64,
    /// proportion of elements not found
    pub absences_found: f64
}

impl SearchStats {
    /// Lookups `h` for all keys in `input` and returns search statistics.
    /// If `verify` is `true`, checks if the MPHF `h` is valid for the given `input`.
    pub fn new<K: Hash, F: Fn(&K, &mut u64) -> Option<u64>>(input: &[K], h: F, verify: bool, lookup_runs: u32) -> Self {
        if input.is_empty() || lookup_runs == 0 { return Self::nan(); }
        let mut extra_levels_searched = 0u64;
        let mut not_found = 0usize;
        if verify {
            let mut seen = Box::<[u64]>::with_zeroed_bits(input.len());
            for v in input {
                if let Some(index) = h(v, &mut extra_levels_searched) {
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
                if black_box(h(v, &mut extra_levels_searched)).is_none() {
                    not_found += 1;
                }
            }
        }
        let start_process_moment = ProcessTime::now();
        for _ in 0..lookup_runs {
            let mut dump = 0;
            for v in input { black_box(h(v, &mut dump)); }
        }
        let seconds = start_process_moment.elapsed().as_secs_f64();
        let divider = input.len() as f64;
        Self {
            avg_deep: extra_levels_searched as f64 / divider,
            avg_lookup_time: seconds / (divider * lookup_runs as f64),
            absences_found: not_found as f64 / divider
        }
    }

    pub fn nan() -> Self {
        Self { avg_deep: f64::NAN, avg_lookup_time: f64::NAN, absences_found: f64::NAN }
    }
}


/// Building statistics
pub struct BuildStats {
    /// Construction time using a single thread in seconds
    pub time_st: f64,
    /// Construction time using multiple threads in seconds
    pub time_mt: f64
}

impl Display for BuildStats {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match (!self.time_st.is_nan(), !self.time_mt.is_nan()) {
            (true, false) => write!(f, "build time [ms] ST: {:.0}", self.time_st * 1_000.0),
            (false, true) => write!(f, "build time [ms] MT: {:.0}", self.time_mt * 1_000.0),
            (true, true) => write!(f, "build time [ms] ST, MT: {:.0}, {:.0}", self.time_st * 1_000.0, self.time_mt * 1_000.0),
            _ => write!(f, "build time is unknown")
        }
    }
}

/// All statistics/results.
pub struct BenchmarkResult {
    pub included: SearchStats,
    pub absent: SearchStats,
    pub size_bytes: usize,
    pub bits_per_value: f64,
    pub build: BuildStats
}

impl BenchmarkResult {
    pub fn all<'a>(&'a self) -> impl Display + 'a {
        struct All<'a>(&'a BenchmarkResult);
        impl<'a> Display for All<'a> {
            fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
                write!(f, "{} {} {} {} {} {} {} {} {}", self.0.size_bytes,
                       self.0.bits_per_value, self.0.included.avg_deep, self.0.included.avg_lookup_time,
                       self.0.build.time_st, self.0.build.time_mt,
                       self.0.absent.avg_lookup_time, self.0.absent.avg_lookup_time, self.0.absent.absences_found
                )
            }
        }
        All(self)
    }
}

impl Display for BenchmarkResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "size [bits/key]: {:.2}", self.bits_per_value)?;
        if !self.included.avg_lookup_time.is_nan() {
            write!(f, "\tlookup time [ns]: {:.0}", self.included.avg_lookup_time * 1_000_000_000.0)?;
        }
        write!(f, "\t{}", self.build)?;
        Ok(())
    }
}