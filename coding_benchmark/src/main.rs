#![doc = include_str!("../README.md")]
mod minimum_redundancy;
mod huffman_compress;
mod constriction;

use std::num::{NonZeroU16, NonZeroU8};
use std::{hint::black_box, time::Instant};

use butils::{UnitPrefix, XorShift64};
//use butils::XorShift64;
use clap::{Parser, Subcommand};

use rand::prelude::*;
use rand::distributions::WeightedIndex;
use rand_pcg::Pcg64Mcg;

//#[allow(non_camel_case_types)]
//#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[derive(Subcommand)]
pub enum Coding {
    /// Huffman coding implementation from minimum_redundancy (generic)
    MinimumRedundancy,
    /// Huffman coding implementation from minimum_redundancy with u8 specific improvements
    MinimumRedundancyU8,
    /// Huffman coding implementation from huffman-compress
    HuffmanCompress,
    /// Huffman coding implementation from constriction
    Constriction
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
/// Coding benchmark.
pub struct Conf {
    /// Coder to test
    #[command(subcommand)]
    pub coding: Coding,

    /// Length of the test text
    #[arg(short = 'l', long, default_value_t = 1024*1024)]
    pub len: usize,

    /// Number of different symbols in the test text
    #[arg(short = 's', long, default_value_t = NonZeroU8::new(255).unwrap())]
    pub symbols: NonZeroU8,

    /// The upper end of the range of relative frequencies of the drawn text symbols (the lower end is 1)
    #[arg(short = 'r', long, default_value_t = NonZeroU16::new(100).unwrap())]
    pub range: NonZeroU16,

    /// Time (in seconds) of measuring and warming up the CPU cache before measuring
    #[arg(short='t', long, default_value_t = 3)]
    pub time: u16,

    /// Whether to check the validity
    #[arg(short='v', long, default_value_t = false)]
    pub verify: bool,

    /// Seed for random number generators
    #[arg(short='s', long, default_value_t = 1234)]
    pub seed: u64,
    //pub seed: NonZeroU64,
}

impl Conf {
    //fn rand_gen(&self) -> XorShift64 { XorShift64(self.seed.get()) }

    /// Returns pseudo-random text for testing.
    fn text(&self) -> Box<[u8]> {
        let r = self.range.get() as u64;
        let weights: Vec<_> = XorShift64(self.seed).take(self.symbols.get() as usize).map(|v| (v % r) as u16).collect();
        let dist = WeightedIndex::new(&weights).unwrap();
        let rng = Pcg64Mcg::seed_from_u64(self.seed);
        dist.sample_iter(rng).map(|v| v as u8).take(self.len).collect()
    }

    #[inline(always)] fn measure<R, F>(&self, mut f: F) -> f64
     where F: FnMut() -> R
    {
        let mut iters = 1;
        if self.time > 0 {
            let time = Instant::now();
            loop {
                black_box(f());
                if time.elapsed().as_secs() > self.time as u64 { break; }
                iters += 1;
            }
        }
        let start_moment = Instant::now();
        for _ in 0..iters { black_box(f()); }
        return start_moment.elapsed().as_secs_f64() / iters as f64
    }

    fn print_speed(&self, label: &str, sec: f64) {
        print!("{}:   ", label);
        if self.len >= 512 * 1024 {
            print!("{:.0} µs   ", sec.as_micros());
        } else {
            print!("{:.0} ns   ", sec.as_nanos());
        }
        let mb = self.len as f64 / (1024 * 1024) as f64;
        println!("{:.0} mb/sec", mb / sec);
    }
}

fn compare_texts(original: &[u8], decoded: &[u8]) {
    if original.len() == decoded.len() {
        for (i, (e, g)) in original.iter().zip(decoded).enumerate() {
            if e != g {
                println!("FAIL: decoded text at index {} has {}, while the original has {}", i, g, e);
                return;
            }
        }
    } else {
        println!("FAIL: decoded text has length {} different from original {}", decoded.len(), original.len());
    }
    println!("DONE")
}

fn main() {
    let conf: Conf = Conf::parse();
    match conf.coding {
        Coding::MinimumRedundancy => minimum_redundancy::benchmark(&conf),
        Coding::MinimumRedundancyU8 => minimum_redundancy::benchmark_u8(&conf),
        Coding::HuffmanCompress => huffman_compress::benchmark(&conf),
        Coding::Constriction => constriction::benchmark(&conf),
    }
}