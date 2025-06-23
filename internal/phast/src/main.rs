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
    #[arg(short='t', long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
    pub build_runs: u32,

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
}

impl Conf {
    fn bucket_size_100(&self) -> u16 {
        self.bucket_size.unwrap_or_else(|| ph::phast::bits_per_seed_to_100_bucket_size(self.bits_per_seed))
    }

    fn keys_for_seed(&self, seed: u64) -> Box<[u64]> {
        butils::XorShift64(seed).take(self.keys_num as usize).collect()
    }
}

struct Executor {
    conf: Conf,
    try_nr: u32,
    threads_num: usize
}

impl Executor {
    fn keys(&self) -> Box<[u64]> {
        self.conf.keys_for_seed(self.try_nr as u64)
    }

    fn run<F, B>(&self, build: B)
        where F: Function, B: FnOnce(&[u64]) -> F
    {
        let keys = self.keys();
        let f = build(&keys);
        let mut max_value = 0;
        for key in keys {
            let v = f.get(key);
            if let Some(v) = v {
                if v > max_value { max_value = v; }
            }
        }
        let range = f.output_range();
        let minimal = f.minimal_output_range(self.conf.keys_num);
        print!("{:.3} bits/key, output range = {range} = {:.1}% over the minimum",
            (8*f.size_bytes()) as f64 / self.conf.keys_num as f64,
            (range - minimal) as f64 * 100.0 / minimal as f64
        );
        if max_value+1 != range {
            print!(", real range = {}", max_value+1)
        }
        println!()
    }

    #[inline] fn bucket_size_100(&self) -> u16 { self.conf.bucket_size_100() }
}

impl From<Conf> for Executor {
    fn from(conf: Conf) -> Self {
        let threads_num = if conf.multiple_threads { current_num_threads() } else { 1 };
        Self { conf, try_nr: 1, threads_num }
    }
}

fn main() {
    let executor: Executor = Conf::parse().into();
    let bucket_size = executor.bucket_size_100();
    println!("n={} k={} bits/seed={} lambda={:.2} threads={}", executor.conf.keys_num, executor.conf.k,
        executor.conf.bits_per_seed, bucket_size as f64/100 as f64, executor.threads_num);
    match (executor.conf.method, executor.conf.k) {
        (Method::phast, 1) => match executor.conf.bits_per_seed {
                8 => executor.run(|keys| phast(&keys, bucket_size, executor.threads_num, Bits8, SeedOnly)),
                //4 => executor.run(|keys| phast(&keys, bucket_size, executor.threads_num, TwoToPowerBitsStatic::<2>, SeedOnly)),
                b => executor.run(|keys| phast(&keys, bucket_size, executor.threads_num, BitsFast(b), SeedOnly)),
        },
        (Method::perfect, 1) => match executor.conf.bits_per_seed {
                    8 => executor.run(|keys| perfect(&keys, bucket_size, executor.threads_num, Bits8, SeedOnly)),
                    //4 => executor.run(|keys| perfect(&keys, bucket_size, executor.threads_num, TwoToPowerBitsStatic::<2>, SeedOnly)),
                    b => executor.run(|keys| perfect(&keys, bucket_size, executor.threads_num, BitsFast(b), SeedOnly)),
        },
        (Method::perfect, k) => {
                let sc = SeedOnlyK(k);
                match executor.conf.bits_per_seed {
                    8 => executor.run(|keys| perfect(&keys, bucket_size, executor.threads_num, Bits8, sc)),
                    //4 => executor.run(|keys| perfect(&keys, bucket_size, executor.threads_num, TwoToPowerBitsStatic::<2>, sc)),
                    b => executor.run(|keys| perfect(&keys, bucket_size, executor.threads_num, BitsFast(b), sc)),
                };
        },
        _ => eprintln!("Unsupported configuration")
    };
}
