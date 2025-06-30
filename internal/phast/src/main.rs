#![doc = include_str!("../README.md")]

mod function;
use crate::function::{Function, PartialFunction};

mod perfect;
use crate::perfect::perfect;

mod phast;
use crate::phast::{phast, phast2};

mod partial;
use crate::partial::partial;

mod benchmark;
use crate::benchmark::{benchmark, Result};


use clap::{Parser, Subcommand};

use ph::phast::ShiftOnlyWrapped;
use ph::{seeds::{Bits8, BitsFast}, phast::{SeedOnly, SeedOnlyK}};
use rayon::current_num_threads;



#[allow(non_camel_case_types)]
//#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[derive(Subcommand, Clone, Copy)]
pub enum Method {
    // PHast
    phast,

    // PHast
    phast2,

    // PHast+ with wrapping
    pluswrap {
        #[arg(default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=3))]
        multiplier: u8
    },

    // PHast+ with wrapping and building last level with regular PHast
    pluswrap2 {
        #[arg(default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=3))]
        multiplier: u8
    },

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
    pub keys_num: u32,

    /// Whether to use multiple threads
    #[arg(short='j', long, default_value_t = false)]
    pub multiple_threads: bool,

    /// k for k-Perfect function
    #[arg(short, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..))]
    pub k: u8,

    /// Cooling time before measuring construction or query time, in milliseconds
    #[arg(short='c', long, default_value_t = 200)]
    pub cooling: u16,

    /// Whether to build only one level
    #[arg(short='1', long, default_value_t = false)]
    pub one: bool,
}

impl Conf {
    fn minimum_range(&self) -> u32 {
        self.keys_num.div_ceil(self.k as u32)
    }

    fn bucket_size_100(&self) -> u16 {
        self.bucket_size.unwrap_or_else(|| ph::phast::bits_per_seed_to_100_bucket_size(self.bits_per_seed))
    }

    fn keys_for_seed(&self, seed: u32) -> Box<[u64]> {
        butils::XorShift64(seed as u64).take(self.keys_num as usize).collect()
    }

    fn run<F, B>(&self, build: B)
        where F: Function, B: Fn(&[u64]) -> F
    {
        let mut total = Result::default();
        for try_nr in 1..=self.build_runs {
            let keys = self.keys_for_seed(try_nr);
            let (f, build_time) = benchmark(|| build(&keys));
            let evaluation_time = if self.lookup_runs > 0 {
                benchmark(|| for _ in 0..self.lookup_runs { f.get_all(&keys) }).1
            } else { Default::default() };
            let result = Result {
                size_bytes: f.size_bytes(),
                build_time,
                evaluation_time,
                bumped_keys: 0,
                range: f.output_range()
            };
            result.print_try(try_nr, self);
            total += result;
        }
        total.print_avg(self);
    }

    fn runp<F, B>(&self, build: B)
        where F: PartialFunction, B: Fn(&[u64]) -> F
    {
        let mut total = Result::default();
        for try_nr in 1..=self.build_runs {
            let keys = self.keys_for_seed(try_nr);
            let (f, build_time) = benchmark(|| build(&keys));
            let evaluation_time = if self.lookup_runs > 0 {
                benchmark(|| for _ in 0..self.lookup_runs { f.get_all(&keys) }).1
            } else { Default::default() };
            //let mut max_value = 0;
            let mut assigned_keys = 0;
            for key in keys {
                let v = f.get(key);
                if let Some(_) = v {
                    assigned_keys += 1;
                    //if v > max_value { max_value = v; }
                }
            }
            let result = Result {
                size_bytes: f.size_bytes(),
                build_time,
                evaluation_time,
                bumped_keys: self.keys_num as usize - assigned_keys,
                range: f.output_range(),
            };
            result.print_try(try_nr, self);
            /*let range = f.output_range();
            if max_value+1 != range {
                print!(", real range = {}", max_value+1)
            }*/
            total += result;
        }
        total.print_avg(self);
    }
}

fn main() {
    let conf = Conf::parse();
    let threads_num = if conf.multiple_threads { current_num_threads() } else { 1 };
    let bucket_size = conf.bucket_size_100();
    println!("n={} k={} bits/seed={} lambda={:.2} threads={threads_num}", conf.keys_num, conf.k,
        conf.bits_per_seed, bucket_size as f64/100 as f64);
    match (conf.method, conf.k, conf.bits_per_seed, conf.one) {
        (Method::phast, 1, 8, false) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, Bits8, SeedOnly)),
        (Method::phast, 1, b, false) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, BitsFast(b), SeedOnly)),

        (Method::phast2, 1, 8, false) =>
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, Bits8, SeedOnly)),
        (Method::phast2, 1, b, false) =>
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, BitsFast(b), SeedOnly)),

        (Method::perfect, 1, 8, false) =>
            conf.run(|keys| perfect(&keys, bucket_size, threads_num, Bits8, SeedOnly)),
        (Method::perfect, 1, b, false) =>
            conf.run(|keys| perfect(&keys, bucket_size, threads_num, BitsFast(b), SeedOnly)),
        (Method::perfect, k, 8, false) =>
            conf.run(|keys| perfect(&keys, bucket_size, threads_num, Bits8, SeedOnlyK(k))),
        (Method::perfect, k, b, false) =>
            conf.run(|keys| perfect(&keys, bucket_size, threads_num, BitsFast(b), SeedOnlyK(k))),

        (Method::phast|Method::phast2|Method::perfect, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, Bits8, SeedOnly)),
        (Method::phast|Method::phast2|Method::perfect, 1, b, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, BitsFast(b), SeedOnly)),
        (Method::phast|Method::phast2|Method::perfect, k, 8, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, Bits8, SeedOnlyK(k))),
        (Method::phast|Method::phast2|Method::perfect, k, b, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, BitsFast(b), SeedOnlyK(k))),

        (Method::pluswrap { multiplier: 1 }, 1, 8, false) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }, 1, 8, false) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }, 1, 8, false) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap { multiplier: 1 }, 1, b, false) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }, 1, b, false) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }, 1, b, false) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<3>)),

        (Method::pluswrap2 { multiplier: 1 }, 1, 8, false) =>
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap2 { multiplier: 2 }, 1, 8, false) =>
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap2 { multiplier: 3 }, 1, 8, false) =>
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap2 { multiplier: 1 }, 1, b, false) => 
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<1>)),
        (Method::pluswrap2 { multiplier: 2 }, 1, b, false) => 
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<2>)),
        (Method::pluswrap2 { multiplier: 3 }, 1, b, false) => 
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<3>)),

        (Method::pluswrap { multiplier: 1 } | Method::pluswrap2 { multiplier: 1 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 } | Method::pluswrap2 { multiplier: 2 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 } | Method::pluswrap2 { multiplier: 3 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap { multiplier: 1 }| Method::pluswrap2 { multiplier: 1 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }| Method::pluswrap2 { multiplier: 2 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }| Method::pluswrap2 { multiplier: 3 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<3>)),

        _ => eprintln!("Unsupported configuration.")
    };
}
