#![doc = include_str!("../README.md")]

use clap::{Parser, Subcommand};

use ph::{fmph::Bits8, phast::{bits_per_seed_to_100_bucket_size, Perfect, SeedChooser, SeedOnly, SeedOnlyK}, GetSize};
use rayon::current_num_threads;

type Hasher = ph::Seedable<fxhash::FxBuildHasher>;
//type StrHasher = ph::BuildDefaultSeededHasher;

#[allow(non_camel_case_types)]
//#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[derive(Subcommand)]
pub enum Method {
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
    fn bucket_size(&self) -> u16 {
        self.bucket_size.unwrap_or_else(|| ph::phast::bits_per_seed_to_100_bucket_size(self.bits_per_seed))
    }

    fn keys_for_seed(&self, seed: u64) -> Box<[u64]> {
        butils::XorShift64(seed).take(self.keys_num as usize).collect()
    }
}

struct Executor {
    conf: Conf,
    try_nr: u32
}

impl Executor {
    fn keys(&self) -> Box<[u64]> {
        self.conf.keys_for_seed(self.try_nr as u64)
    }

    fn run<F, B, L>(&self, build: B, lookup: L)
        where F: GetSize, B: FnOnce(&[u64]) -> F, L: Fn(&F, u64) -> usize
    {
        let keys = self.keys();
        let f = build(&keys);
        let mut max_value = 0;
        for key in keys {
            let v = lookup(&f, key);
            if v > max_value { max_value = v; }
        }
        println!("{} bits/key, max value = {max_value} = {}%n",
            (8*f.size_bytes()) as f64 / self.conf.keys_num as f64,
            max_value * 100 / self.conf.keys_num
        );
    }
}

impl From<Conf> for Executor {
    fn from(conf: Conf) -> Self {
        Self { conf, try_nr: 1 }
    }
}

fn perfect<SC: SeedChooser>(keys: &[u64], seed_chooser: SC) -> Perfect<Bits8, SC, Hasher>
{
    Perfect::with_slice_bps_bs_hash_sc(keys, Bits8::default(), bits_per_seed_to_100_bucket_size(8), Hasher::default(), seed_chooser)
}

fn main() {
    let executor: Executor = Conf::parse().into();
    if executor.conf.multiple_threads {
        println!("multi-threaded calculations use {} threads (to set by the RAYON_NUM_THREADS environment variable)", current_num_threads());
    }
    match executor.conf.method {
        Method::perfect => {
            if executor.conf.k == 1 {
                executor.run(|keys| perfect(&keys, SeedOnly), |f, key| f.get(&key));
            } else {
                executor.run(|keys| perfect(&keys, SeedOnlyK(executor.conf.k)),
                    |f, key| f.get(&key));
            }
        },
    };
}
