use clap::{Parser, Subcommand};
use ph::{phast::{Partial, SeedChooser}, seeds::BitsFast};

use crate::{benchmark::{benchmark, Result}, function::{Function, PartialFunction}, optim::WeightsF};

use optimize::{Minimizer, NelderMeadBuilder};
use ndarray::{Array, ArrayView1};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

#[allow(non_camel_case_types)]
//#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[derive(Subcommand, Clone, Copy)]
pub enum Method {
    /// PHast
    phast,

    /// PHast
    phast2,

    /// PHast+ with wrapping
    pluswrap {
        #[arg(default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=3))]
        multiplier: u8
    },

    /// PHast+ with wrapping and building last level with regular PHast
    pluswrap2 {
        #[arg(default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=3))]
        multiplier: u8
    },

    /// k-perfect PHast
    perfect,

    /// Optimize weights in PHast
    optphast,

    /// Optimize weights for PHast+ with wrapping
    optpluswrap,
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

    /// Number of iterations done by optimization commands (ignored by the rest)
    #[arg(default_value_t = 50)]
    iters: u16
}

impl Conf {
    pub fn minimum_range(&self) -> u32 {
        self.keys_num.div_ceil(self.k as u32)
    }

    pub fn bucket_size_100(&self) -> u16 {
        self.bucket_size.unwrap_or_else(|| ph::phast::bits_per_seed_to_100_bucket_size(self.bits_per_seed))
    }

    pub fn keys_for_seed(&self, seed: u32) -> Box<[u64]> {
        butils::XorShift64(seed as u64).take(self.keys_num as usize).collect()
    }

    pub fn run<F, B>(&self, build: B)
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

    pub fn runp<F, B>(&self, build: B)
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

    pub fn optimize_weights<SC: SeedChooser + Sync>(&self, seed_chooser: SC) {
        let bucket_size = self.bucket_size_100();
        let minimizer = NelderMeadBuilder::default()
            .maxiter(self.iters as usize) 
            .build()
            .unwrap();
        let conf = seed_chooser.conf_for_minimal(self.keys_num as usize, self.bits_per_seed, bucket_size);
        let args = Array::from_vec(WeightsF::new(self.bits_per_seed, conf.slice_len()).size_weights.into_vec());

        let ans = minimizer.minimize(|x: ArrayView1<f64>| {
            let evaluator = WeightsF{ size_weights: x.as_slice().unwrap().try_into().unwrap() };

            let key_sets_num: u32 = 96;
            let unassigned_keys: usize = (0..key_sets_num).into_par_iter().map(|i| {
                let mut keys = self.keys_for_seed(200+i);
                Partial::with_hashes_bps_conf_sc_be_u(&mut keys, BitsFast(self.bits_per_seed),
                    conf,
                    seed_chooser, &evaluator).1
            }).sum();
            println!("{unassigned_keys} {:.2}% {x:.0}", unassigned_keys as f64 * 100.0 / (key_sets_num as f64 * self.keys_num as f64));
            unassigned_keys as f64
        }, args.view());
        println!("Optimal weights: {ans:.0}");
    }
}