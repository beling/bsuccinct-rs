
use std::{hash::Hash, hint::black_box, time::Instant};

use bitm::{BitAccess, BitVec};
use cpu_time::{ProcessTime, ThreadTime};

use crate::{BenchmarkResult, BuildStats, Conf, SearchStats, Threads};

#[inline(never)]
fn warn_size_diff(size_st: usize, size_mt: usize) {
    if size_st != size_mt {
        eprintln!("WARNING: ST/MT differ in sizes, {} != {}", size_st, size_mt);
    }
}

#[inline(never)]
fn check_collision(seen: &mut [u64], input_len: usize, index: usize) {
    assert!(index < input_len, "MPHF assigns too large value {}>{}.", index, input_len);
    assert!(!seen.get_bit(index), "MPHF assigns the same value {index} to two keys of input.");
    seen.set_bit(index);
}

#[inline(never)]
fn ensure_no_absences(absences_found: f64) {
    assert_eq!(absences_found, 0.0, "MPHF does not assign the value for {}% keys of the input", absences_found*100.0);
}

pub trait TypeToQuery {
    type ToHash<'s>:  ?Sized + Hash + 's where Self: 's;
    fn to_query_type<'s>(&'s self) -> &'s Self::ToHash::<'s>;
}

impl TypeToQuery for u32 {
    type ToHash<'s> = u32;
    #[inline(always)] fn to_query_type<'s>(&'s self) -> &'s Self::ToHash::<'s> { self }
}

impl TypeToQuery for u64 {
    type ToHash<'s> = u64;
    #[inline(always)] fn to_query_type<'s>(&'s self) -> &'s Self::ToHash::<'s> { self }
}

impl TypeToQuery for Box<[u8]> {
    type ToHash<'s> = [u8];
    #[inline(always)] fn to_query_type<'s>(&'s self) -> &'s Self::ToHash::<'s> { self.as_ref() }
}

pub trait MPHFBuilder<K: Hash + TypeToQuery> {
    const CAN_DETECT_ABSENCE: bool = true;
    const BUILD_THREADS: Threads = Threads::Both;
    const BUILD_THREADS_DOES_NOT_CHANGE_SIZE: bool = true;

    type MPHF;
    type Value;
    fn new(&self, keys: &[K], use_multiple_threads: bool) -> Self::MPHF;
    fn value_ex(mphf: &Self::MPHF, key: &K, levels: &mut usize) -> Option<u64>;
    fn value(mphf: &Self::MPHF, key: &K) -> Self::Value;
    fn mphf_size(mphf: &Self::MPHF) -> usize;
}

/// Builds the MPHF and measure the CPU thread time of building. Returns: the MPHF, the time measured.
fn benchmark_build_st<K: Hash + TypeToQuery, B: MPHFBuilder<K>>(b: &B, keys: &[K], conf: &Conf) -> (B::MPHF, BuildStats) {
    std::thread::sleep(std::time::Duration::from_millis(conf.cooling as u64));
    let start_moment = ThreadTime::now();   // ProcessTime?
    for _ in 1..conf.build_runs { b.new(keys, false); }
    let h = b.new(keys, false);
    let build_time_seconds = start_moment.elapsed().as_secs_f64();
    (h, BuildStats { time_st: build_time_seconds / conf.build_runs as f64, time_mt: f64::NAN } )
}

/// Builds the MPHF and measure the CPU thread time of building. Returns: the MPHF, the time measured.
fn benchmark_build_mt<K: Hash + TypeToQuery, B: MPHFBuilder<K>>(b: &B, keys: &[K], conf: &Conf) -> (B::MPHF, BuildStats) {
    std::thread::sleep(std::time::Duration::from_millis(conf.cooling as u64));
    let start_moment =  Instant::now();
    for _ in 1..conf.build_runs { b.new(keys, true); }
    let h = b.new(keys, true);
    let build_time_seconds = start_moment.elapsed().as_secs_f64();
    (h, BuildStats { time_st: f64::NAN, time_mt: build_time_seconds / conf.build_runs as f64 } )
}

/// Builds MPHF and measure the time of building. Returns: MPHF, and either single-thread and multiple-thread time of building, and one NaN.
fn benchmark_build<K: Hash + TypeToQuery, B: MPHFBuilder<K>>(b: &B, keys: &[K], conf: &Conf) -> (B::MPHF, BuildStats) {
    match (B::BUILD_THREADS, conf.threads) {
        (Threads::Single, _) | (Threads::Both, Threads::Single) => benchmark_build_st(b, keys, conf),
        (Threads::Multi, _) | (Threads::Both, Threads::Multi) => benchmark_build_mt(b, keys, conf),
        _ => {  // (Both, Both) pair
            let (mphf, result_st) = benchmark_build_st(b, keys, conf);
            let size_st = B::BUILD_THREADS_DOES_NOT_CHANGE_SIZE.then(|| B::mphf_size(&mphf));
            drop(mphf);
            let (mphf, mut result_mt) = benchmark_build_mt(b, keys, conf);
            if let Some(size_st) = size_st { warn_size_diff(size_st, B::mphf_size(&mphf)); }
            result_mt.time_st = result_st.time_st;
            (mphf, result_mt)
        }
    }
}

/// Lookups for all keys in `input` and returns search statistics.
/// If `verify` is `true`, checks if the `mphf` is valid for the given `input`.
fn benchmark_lookup<K: Hash + TypeToQuery, B: MPHFBuilder<K>>(mphf: &B::MPHF, input: &[K], verify: bool, conf: &Conf) -> SearchStats {
    if input.is_empty() || conf.lookup_runs == 0 { return SearchStats::nan(); }
    let mut extra_levels_searched = 0;
    let mut not_found = 0usize;
    if verify {
        let mut seen = Box::<[u64]>::with_zeroed_bits(input.len());
        for v in input {
            if let Some(index) = B::value_ex(mphf, v, &mut extra_levels_searched) {
                check_collision(&mut seen, input.len(), index as usize);
            } else {
                not_found += 1;
            }
        }
    } else {
        for v in input {
            if B::value_ex(mphf, v, &mut extra_levels_searched).is_none() {
                not_found += 1;
            }
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(conf.cooling as u64));
    let start_process_moment = ProcessTime::now();
    for _ in 0..conf.lookup_runs {
        for v in input { black_box(B::value(mphf, v)); }
    }
    let seconds = start_process_moment.elapsed().as_secs_f64();
    let divider = input.len() as f64;
    SearchStats {
        avg_deep: extra_levels_searched as f64 / divider,
        avg_lookup_time: seconds / (divider * conf.lookup_runs as f64),
        absences_found: not_found as f64 / divider
    }
}

/// Builds, tests, and returns MPHF.
pub fn benchmark<K: Hash + TypeToQuery, B: MPHFBuilder<K>>(b: B, i: &(Vec<K>, Vec<K>), conf: &Conf) -> BenchmarkResult {
    let (h, build) = benchmark_build(&b, &i.0, conf);
    let size_bytes = B::mphf_size(&h);
    let bits_per_value = 8.0 * size_bytes as f64 / i.0.len() as f64;
    if conf.lookup_runs == 0 {
        return BenchmarkResult { included: SearchStats::nan(), absent: SearchStats::nan(), size_bytes, bits_per_value, build }
    }
    let included = benchmark_lookup::<K, B>(&h, &i.0, conf.verify, conf);
    ensure_no_absences(included.absences_found);
    let absent = if conf.save_details && B::CAN_DETECT_ABSENCE {
        benchmark_lookup::<K, B>(&h, &i.1, false, conf)
    } else {
        SearchStats::nan()
    };
    BenchmarkResult { included, absent, size_bytes, bits_per_value, build }
}