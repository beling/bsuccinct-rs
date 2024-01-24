#![doc = include_str!("../README.md")]

mod elias_fano;
mod bitm;
mod sucds;

use std::{hint::black_box, iter::StepBy, num::NonZeroU64, ops::Range, time::Instant};

use butils::XorShift64;
use clap::{Parser, Subcommand};

trait UnitPrefix {
    fn micros(self) -> f64;
    fn nanos(self) -> f64;
    fn picos(self) -> f64;
}

impl UnitPrefix for f64 {
    #[inline(always)] fn micros(self) -> f64 { self * 1_000_000.0 }
    #[inline(always)] fn nanos(self) -> f64 { self * 1_000_000_000.0 }
    #[inline(always)] fn picos(self) -> f64 { self * 1_000_000_000_000.0 }
}

//#[allow(non_camel_case_types)]
//#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[derive(Subcommand)]
pub enum Structure {
    /// Elias-Fano
    EliasFano,
    /// Uncompressed bit vector from bitm library
    BitmBV,
    /// Uncompressed bit vector from sucds library
    SucdsBV,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
/// Compact sequences benchmark.
pub struct Conf {
    /// Structure to test
    #[command(subcommand)]
    pub structure: Structure,

    /// The number of items to use
    #[arg(short = 'n', long, default_value_t = 1024*1024*1024/2)]
    pub num: usize,

    /// Item universe.
    #[arg(short = 'u', long, default_value_t = 1024*1024*1024)]
    pub universe: usize,

    /// Whether to warm up the CPU cache before measuring
    #[arg(short='w', long, default_value_t = true)]
    pub warm: bool,

    /// Whether to check the validity of built sequence
    #[arg(short='v', long, default_value_t = true)]
    pub verify: bool,

    /// Seed for (XorShift64) rundom number generator
    #[arg(short='s', long, default_value_t = NonZeroU64::new(1234).unwrap())]
    pub seed: NonZeroU64,
}

impl Conf {
    fn rand_gen(&self) -> XorShift64 { XorShift64(self.seed.get()) }

    #[inline(always)] fn measure<F>(&self, f: F) -> f64
     where F: Fn()
    {
        let mut iters = 1;
        if self.warm {
            let time = Instant::now();
            loop {
                f();
                if time.elapsed().as_millis() > 3000 { break; }
                iters += 1;
            }
        }
        let start_moment = Instant::now();
        for _ in 0..iters { f(); }
        return start_moment.elapsed().as_secs_f64() / iters as f64
    }

    #[inline(always)] fn sampling_measure<R, F>(&self, steps: StepBy<Range<usize>>, f: F) -> f64
    where F: Fn(usize) -> R
    {
        self.measure(|| for i in steps.clone() { black_box(f(i)); }) / steps.len() as f64
    }

    #[inline(always)] fn num_sampling_measure<R, F>(&self, steps_num: usize, f: F) -> f64
    where F: Fn(usize) -> R
    {
        self.sampling_measure((0..self.num).step_by((self.num / steps_num).max(1)), f)
    }

    #[inline(always)] fn num_complement_sampling_measure<R, F>(&self, steps_num: usize, f: F) -> f64
    where F: Fn(usize) -> R
    {
        let complement = self.universe - self.num;
        self.sampling_measure((0..complement).step_by((complement / steps_num).max(1)), f)
    }

    #[inline(always)] fn universe_sampling_measure<R, F>(&self, steps_num: usize, f: F) -> f64
    where F: Fn(usize) -> R
    {
        self.sampling_measure((0..self.universe).step_by((self.universe / steps_num).max(1)), f)
    }
}

fn main() {
    let conf: Conf = Conf::parse();
    match conf.structure {
        Structure::EliasFano => elias_fano::benchmark(&conf),
        Structure::BitmBV => bitm::benchmark_rank_select(&conf),
        Structure::SucdsBV => sucds::benchmark_rank9_select(&conf),
    }
}