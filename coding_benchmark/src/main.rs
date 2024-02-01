#![doc = include_str!("../README.md")]
mod minimum_redundancy;
mod huffman_compress;

use std::num::{NonZeroU16, NonZeroU8};
use std::{hint::black_box, time::Instant};

use butils::XorShift64;
//use butils::XorShift64;
use clap::{Parser, Subcommand};

use rand::prelude::*;
use rand::distributions::WeightedIndex;
use rand_pcg::Pcg64Mcg;

//#[allow(non_camel_case_types)]
//#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[derive(Subcommand)]
pub enum Coding {
    /// Huffman coding implementation from minimum_redundancy
    MinimumRedundancy,
    /// Huffman coding implementation from huffman-compress
    HuffmanCompress,
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

    /// Disable warming up the CPU cache before measuring
    #[arg(short='w', long, default_value_t = false)]
    pub no_warm: bool,

    /// Whether to check the validity
    #[arg(short='v', long, default_value_t = false)]
    pub verify: bool,

    /// Seed for (XorShift64) rundom number generator
    #[arg(short='s', long, default_value_t = 1234)]
    pub seed: u64,
    //pub seed: NonZeroU64,
}

impl Conf {
    //fn rand_gen(&self) -> XorShift64 { XorShift64(self.seed.get()) }

    fn rand_text(&self) -> Box<[u8]> {
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
        if !self.no_warm {
            let time = Instant::now();
            loop {
                black_box(f());
                if time.elapsed().as_millis() > 3000 { break; }
                iters += 1;
            }
        }
        let start_moment = Instant::now();
        for _ in 0..iters { black_box(f()); }
        return start_moment.elapsed().as_secs_f64() / iters as f64
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
        Coding::HuffmanCompress => huffman_compress::benchmark(&conf),
    }
}