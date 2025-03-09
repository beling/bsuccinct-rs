use std::{fmt::{Display, Formatter}, fs::{File, OpenOptions}, io::Write};

use crate::{Conf, KeySource};

const BENCHMARK_HEADER: &'static str = "size_bytes bits_per_value avg_deep avg_lookup_time build_time_st build_time_mt absent_avg_deep absent_avg_lookup_time absences_found";

pub fn file(method_name: &str, conf: &Conf, i_lens0: usize, i_lens1: usize, extra_header: &str) -> Option<File> {
    if !conf.save_details { return None; }
    let ks_name = match conf.key_source {
        KeySource::xs32 => "32",
        KeySource::xs64 => "64",
        KeySource::stdin|KeySource::stdinz => "str",
    };
    let file_name = format!("{}_{}_{}_{}.csv", method_name, ks_name, i_lens0, i_lens1);
    let file_already_existed = std::path::Path::new(&file_name).exists();
    let mut file = OpenOptions::new().append(true).create(true).open(&file_name).unwrap();
    if !file_already_existed { writeln!(file, "{} {}", extra_header, BENCHMARK_HEADER).unwrap(); }
    Some(file)
}

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
    #[inline(never)]
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
    #[inline(never)]
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
    #[inline(never)]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "size [bits/key]: {:.2}", self.bits_per_value)?;
        if !self.included.avg_lookup_time.is_nan() {
            write!(f, "\tlookup time [ns]: {:.1}", self.included.avg_lookup_time * 1_000_000_000.0)?;
        }
        write!(f, "\t{}", self.build)?;
        Ok(())
    }
}

#[inline(never)]
pub fn print_input_stats(setname: &str, strings: &[Box<[u8]>]){
    if strings.len() == 0 {
        println!("{} is empty", setname);
    } else {
        println!("{} has {} strings with an average length of {:.1} bytes", setname, strings.len(), strings.iter().map(|s| s.len()).sum::<usize>() as f64 / strings.len() as f64)
    }
}
