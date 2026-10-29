#![doc = include_str!("../README.md")]

use std::time::{Duration, Instant};

use ph::GetSize;

use crate::kptrhash::{KptrHash, Mode, space_lower_bound};

mod kptrhash;

#[derive(Default)]
pub struct Result {
    /// Total size
    pub size_bytes: usize,

    /// Total building time
    pub build_time: Duration,

    /// Total query time
    pub evaluation_time: Duration,
}

pub fn benchmark<R, F: FnOnce() -> R>(f: F) -> (R, Duration) {
    let start_moment = Instant::now();
    let r = f();
    let time = start_moment.elapsed();
    (r, time)
}

pub fn keys(keys_num: usize, seed: u32) -> Box<[u32]> {
    butils::XorShift32(seed).take(keys_num as usize).collect()
}

//const ALPHAS: [f32; 6] = [0.7, 0.8, 0.9, 0.95, 0.98, 0.99];
//const FACTORS: [f32; 7] = [2.5, 2.25, 2.0, 1.75, 1.5, 1.25, 1.0];
//const QUERY: bool = true;

fn bench_kptrhash<const MODE: u8, const K: usize>(alpha: f32, factor: f32, keys: &[u32]) {
    let n = keys.len();
    let lb = space_lower_bound(K, alpha);
    let target_bits_per_key = lb * factor;
    // Construction
    let t0 = std::time::Instant::now();
    let kphf = KptrHash::<MODE, K>::new(alpha, target_bits_per_key, &keys).unwrap();
    let build_ns: f32 = t0.elapsed().as_nanos() as f32 / n as f32;

    // Space
    let actual_bits_per_key = kphf.bits_used() as f32 / n as f32;
    let bumped_frac = kphf.num_bumped() as f32 / n as f32;
    let actual_alpha = n as f32 / (kphf.num_bins() as f32 * K as f32);
   
    let start = std::time::Instant::now();
    for key in keys {
        std::hint::black_box(kphf.get(*key));
    }

    let loop_ns = start.elapsed().as_nanos() as f32 / n as f32;

    let start = std::time::Instant::now();
    let lookahead = 32;
    for i in 0..keys.len().saturating_sub(lookahead) {
        kphf.prefetch(keys[i + lookahead]);
        std::hint::black_box(kphf.get(keys[i]));
    }
    let throughput_ns = start.elapsed().as_nanos() as f32 / n as f32;

    println!("{K} kPtrHash{MODE} {factor} {actual_bits_per_key} {actual_alpha} {bumped_frac} {build_ns} {loop_ns} {throughput_ns}");
}

fn bench_kphast(k: u16, alpha: f32, bucket_size100: u32, keys: &[u32]) {
    let n = keys.len();

    let t0 = std::time::Instant::now();
    let f = ph::phast::KFunction::from_slice_st(k, keys);
    let build_ns: f32 = t0.elapsed().as_nanos() as f32 / n as f32;
   
    let start = std::time::Instant::now();
    for key in keys {
        std::hint::black_box(f.get(key));
    }

    let loop_ns = start.elapsed().as_nanos() as f32 / n as f32;

    let start = std::time::Instant::now();
    let lookahead = 32;
    for i in 0..keys.len().saturating_sub(lookahead) {
        f.prefetch(&keys[i + lookahead]);
        std::hint::black_box(f.get(&keys[i]));
    }
    let throughput_ns = start.elapsed().as_nanos() as f32 / n as f32;
    let actual_bits_per_key = (f.size_bytes() * 8) as f32 / n as f32;
    let actual_alpha = n as f32 / (f.output_range() as f32 * k as f32);
    println!("{k} kPHast {bucket_size100} {actual_bits_per_key} {actual_alpha} {build_ns} {loop_ns} {throughput_ns}");
}

fn main() {
    let keys = keys(10_000_000, 1234);
    
    bench_kptrhash::<{ Mode::SortBump as u8 }, 4>(0.99, 1.5, &keys);
    bench_kphast(4, 0.99, 400, &keys);
}