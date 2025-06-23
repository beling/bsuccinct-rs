#![doc = include_str!("../README.md")]

mod function;

use clap::{Parser, Subcommand};

use ph::{seeds::{Bits8, BitsFast}, phast::{SeedOnly, SeedOnlyK}};
use rayon::current_num_threads;

use crate::function::{perfect, phast, Function};

#[allow(non_camel_case_types)]
//#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[derive(Subcommand, Clone, Copy)]
pub enum Method {
    // PHast
    phast,

    // k-perfect PHast
    perfect,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
/// Minimal perfect hashing benchmark.
pub struct Conf {
    /// Method to run
    #[command(subcommand)]
    pub method: Method,

    /// Number of bits to store seed of each bucket
    #[arg(short='s', default_value_t = 8, value_parser = clap::value_parser!(u8).range(1..16))]
    pub bits_per_seed: u8,

    /// Expected number of keys per bucket multipled by 100
    #[arg(short='b')]
    pub bucket_size: Option<u16>,

    /// Number of times to perform the lookup test
    #[arg(short='l', long, default_value_t = 1)]
    pub lookup_runs: u32,

    /// Number of times to perform the construction
    #[arg(short='t', long, default_value_t = 1, value_parser = clap::value_parser!(u64).range(1..))]
    pub build_runs: u64,

    /// Whether to check the validity of built MPHFs
    #[arg(short='v', long, default_value_t = false)]
    pub verify: bool,

    /// The number of random keys to use
    #[arg(short='n', long, default_value_t = 1_000_000)]
    pub keys_num: usize,

    /// Whether to use multiple threads
    #[arg(short='j', long, default_value_t = false)]
    pub multiple_threads: bool,

    /// k for k-Perfect function
    #[arg(short, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..))]
    pub k: u8,

    /// Cooling time before measuring construction or query time, in milliseconds
    #[arg(short='c', long, default_value_t = 200)]
    pub cooling: u16,

    /// Build and test only the first level
    #[arg(short='1', long, default_value_t = false)]
    pub first: bool,
}

impl Conf {
    fn bucket_size_100(&self) -> u16 {
        self.bucket_size.unwrap_or_else(|| ph::phast::bits_per_seed_to_100_bucket_size(self.bits_per_seed))
    }

    fn keys_for_seed(&self, seed: u64) -> Box<[u64]> {
        butils::XorShift64(seed).take(self.keys_num as usize).collect()
    }

    fn run<F, B>(&self, build: B)
        where F: Function, B: Fn(&[u64]) -> F
    {
        for try_nr in 1..=self.build_runs {
            if self.build_runs > 1 { print!("{try_nr}: "); }
            let keys = self.keys_for_seed(try_nr);
            let f = build(&keys);
            let mut max_value = 0;
            for key in keys {
                let v = f.get(key);
                if let Some(v) = v {
                    if v > max_value { max_value = v; }
                }
            }
            let range = f.output_range();
            let minimal = f.minimal_output_range(self.keys_num);
            print!("{:.3} bits/key, output range = {range} = {:.1}% over the minimum",
                (8*f.size_bytes()) as f64 / self.keys_num as f64,
                (range - minimal) as f64 * 100.0 / minimal as f64
            );
            if max_value+1 != range {
                print!(", real range = {}", max_value+1)
            }
            println!()
        }
    }
}

fn main() {
    let conf = Conf::parse();
    let threads_num = if conf.multiple_threads { current_num_threads() } else { 1 };
    let bucket_size = conf.bucket_size_100();
    println!("n={} k={} bits/seed={} lambda={:.2} threads={} first={}", conf.keys_num, conf.k,
        conf.bits_per_seed, bucket_size as f64/100 as f64, threads_num, conf.first);
    match (conf.method, conf.first, conf.k, conf.bits_per_seed) {
        (Method::phast, false, 1, 8) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, Bits8, SeedOnly)),
        (Method::phast, false, 1, b) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, BitsFast(b), SeedOnly)),
        (Method::perfect, false, 1, 8) =>
            conf.run(|keys| perfect(&keys, bucket_size, threads_num, Bits8, SeedOnly)),
        (Method::perfect, false, 1, b) =>
            conf.run(|keys| perfect(&keys, bucket_size, threads_num, BitsFast(b), SeedOnly)),
        (Method::perfect, false, k, 8) =>
            conf.run(|keys| perfect(&keys, bucket_size, threads_num, Bits8, SeedOnlyK(k))),
        (Method::perfect, false, k, b) =>
            conf.run(|keys| perfect(&keys, bucket_size, threads_num, BitsFast(b), SeedOnlyK(k))),
        (Method::perfect, true, k, 8) => {

        }
        _ => eprintln!("Unsupported configuration")
    };
}
