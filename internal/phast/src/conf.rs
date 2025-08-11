use clap::{Parser, Subcommand};
use ph::{phast::{bucket_size_normalization_multiplier, Params, Partial, SeedChooser, SeedOnlyK, SumOfWeightedValues}, seeds::BitsFast, utils::verify_partial_kphf};

use crate::{benchmark::{benchmark, Result}, function::{Function, PartialFunction}, optim::{SumOfWeightedValuesF, WeightsF}};

use optimize::{Minimizer, NelderMead, NelderMeadBuilder};
use ndarray::{Array, ArrayView1};
use rayon::{current_num_threads, iter::{IntoParallelIterator, ParallelIterator}};

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
        #[arg(default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=7))]
        multiplier: u8
    },

    /// PHast+ with wrapping and building last level using regular PHast
    pluswrap2 {
        #[arg(default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=7))]
        multiplier: u8
    },

    /// PHast+ with building last level using regular PHast
    plus,

    /// k-perfect PHast
    perfect,

    /// Optimize weights for selecting buckets by PHast
    optphast,

    /// Optimize weights for selecting buckets by PHast+ with wrapping
    optpluswrap {
        #[arg(default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=3))]
        multiplier: u8
    },

    /// Optimize weights for selecting buckets by PHast+
    optplus,

    /// Optimize score weights for k-perfect PHast
    optscore,

    /// Do nothing
    none
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Method::phast => write!(f, "PHast"),
            Method::phast2 => write!(f, "PHast2"),
            Method::pluswrap { multiplier } => write!(f, "PHast+wrap {multiplier}"),
            Method::pluswrap2 { multiplier } => write!(f, "PHast2+wrap {multiplier}"),
            Method::plus => write!(f, "PHast+"),
            Method::perfect => write!(f, "Perfect"),
            Method::optphast => write!(f, "Optimize PHast weights"),
            Method::optpluswrap { multiplier } => write!(f, "Optimize PHast+wrap {multiplier} weights"),
            Method::optplus => write!(f, "Optimize PHast+ weights"),
            Method::optscore => write!(f, "Optimize score weights for k-perfect PHast"),
            Method::none => write!(f, "Do nothing"),
        }
    }
}

pub const CSV_HEADER: &'static str = "keys_num method k bits/seed bucket_size100 slice threads seed bits/key bumped_% range_overhead_% build_ns/key query_ns/key";

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

    /// Number of times to perform evaluation (over all keys) test
    #[arg(short='e', long, default_value_t = 1)]
    pub evaluations: u32,

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

    /// Number of iterations done by optimization (50 if 0) commands or number of times to perform the construction (1 if 0)
    #[arg(short='i', long, default_value_t = 0)]
    pub iters: u32,

    /// Slice length or 0 for auto
    #[arg(short='l', long, default_value_t = 0)]
    pub slice_len: u16,

    /// Print output in CSV format
    #[arg(long, default_value_t = false)]
    pub csv: bool,

    /// Print CSV header
    #[arg(long, default_value_t = false)]
    pub head: bool,

    /// Print less, only average.
    #[arg(long, default_value_t = false)]
    pub less: bool,

    /// Use weighted scores in k-perfect
    #[arg(short='w', long, default_value_t = false)]
    pub weights: bool
}

impl Conf {
    pub fn optimization_iters(&self) -> u32 {
        if self.iters == 0 { 50 } else { self.iters }
    }

    pub fn tries(&self) -> u32 {
        if self.iters == 0 { 1 } else { self.iters }
    }

    pub fn many_tries(&self) -> bool {
        self.iters > 1
    }

    pub fn minimum_range(&self) -> u32 {
        self.keys_num.div_ceil(self.k as u32)
    }

    pub fn bucket_size_100(&self) -> u16 {
        let bs = self.bucket_size.unwrap_or_else(|| ph::phast::bits_per_seed_to_100_bucket_size(self.bits_per_seed));
        (bs as f64 * bucket_size_normalization_multiplier(self.k)) as u16
    }

    pub fn keys_for_seed(&self, seed: u32) -> Box<[u64]> {
        butils::XorShift64(seed as u64).take(self.keys_num as usize).collect()
    }

    pub fn params<SS>(&self, seed_size: SS) -> Params<SS> {
        Params { seed_size, bucket_size100: self.bucket_size_100(), preferred_slice_len: self.slice_len }
    }

    pub fn threads(&self) -> usize { if self.multiple_threads { current_num_threads() } else { 1 } }

    /// Whether the configuration supports CSV output
    pub fn support_csv(&self) -> bool {
        match self.method {
            Method::optphast|Method::optplus|Method::optpluswrap { multiplier: _ } => false,
            _ => true
        }
    }

    pub fn print_csv(&self) {
        print!("{} {} {} {} {} {} {}",
            self.keys_num, self.method.to_string().replace(" ", "_"),
            self.k, self.bits_per_seed, self.bucket_size_100(), self.slice_len, self.threads())
    }

    pub fn run<F, B>(&self, build: B)
        where F: Function, B: Fn(&[u64]) -> F
    {
        let mut total = Result::default();
        for try_nr in 1..=self.tries() {
            let keys = self.keys_for_seed(try_nr);
            let (f, build_time) = benchmark(|| build(&keys));
            if self.verify {
                verify_partial_kphf(self.k, f.output_range(), &keys, |key| Some(f.get(**key)));
            }
            let evaluation_time = if self.evaluations > 0 {
                benchmark(|| for _ in 0..self.evaluations { f.get_all(&keys) }).1
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
        for try_nr in 1..=self.tries() {
            let keys = self.keys_for_seed(try_nr);
            let (f, build_time) = benchmark(|| build(&keys));
            if self.verify {
                verify_partial_kphf(self.k, f.output_range(), &keys, |key| f.get(**key));
            }
            let evaluation_time = if self.evaluations > 0 {
                benchmark(|| for _ in 0..self.evaluations { f.get_all(&keys) }).1
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
            /*let range = f.output_range();
            if max_value+1 != range {
                print!(", real range = {}", max_value+1)
            }*/
            result.print_try(try_nr, self);
            total += result;
        }
        total.print_avg(self);
    }

    pub fn optimizer<SC: SeedChooser>(&self, seed_chooser: &SC) -> (NelderMead, ph::phast::Conf) {
        let bucket_size = self.bucket_size_100();
        let minimizer = NelderMeadBuilder::default()
            .maxiter(self.optimization_iters() as usize) 
            .build()
            .unwrap();
        (minimizer, seed_chooser.conf_for_minimal(self.keys_num as usize, self.bits_per_seed, bucket_size, self.slice_len))
    }

    pub fn optimize_weights<SC: SeedChooser + Sync>(&self, seed_chooser: SC) {
        let (minimizer, conf) = self.optimizer(&seed_chooser);
        let args = Array::from_vec(WeightsF::from(seed_chooser.bucket_evaluator(self.bits_per_seed, conf.slice_len())).size_weights.into_vec());

        let ans = minimizer.minimize(|x: ArrayView1<f64>| {
            let evaluator = WeightsF{ size_weights: x.as_slice().unwrap().try_into().unwrap() };

            let key_sets_num: u32 = 96;
            let unassigned_keys: usize = (0..key_sets_num).into_par_iter().map(|i| {
                let mut keys = self.keys_for_seed(200+i);
                Partial::with_hashes_bps_conf_sc_be_u(&mut keys, BitsFast(self.bits_per_seed),
                    conf,
                    seed_chooser.clone(), &evaluator).1
            }).sum();
            println!("{unassigned_keys} {:.2}% {x:.0}", unassigned_keys as f64 * 100.0 / (key_sets_num as f64 * self.keys_num as f64));
            unassigned_keys as f64
        }, args.view());
        println!("Optimal weights: {ans:.0}");
    }

    pub fn optimize_score(&self) {
        let (minimizer, conf) = self.optimizer(&SeedOnlyK::new(self.k, SumOfWeightedValues::new(self.k)));
        let args = Array::from_vec(SumOfWeightedValuesF::from(SumOfWeightedValues::new(self.k)).0[..8.min(self.k as usize - 1)].to_owned());

        let ans = minimizer.minimize(|x: ArrayView1<f64>| {
            let evaluator = SumOfWeightedValuesF(x.to_vec().into_boxed_slice());

            let key_sets_num: u32 = 96;
            let unassigned_keys: usize = (0..key_sets_num).into_par_iter().map(|i| {
                let mut keys = self.keys_for_seed(200+i);
                Partial::with_hashes_bps_conf_sc_u(&mut keys, BitsFast(self.bits_per_seed),
                    conf,
                    SeedOnlyK::new(self.k, evaluator.clone())).1
            }).sum();
            println!("{unassigned_keys} {:.2}% {x:.0}", unassigned_keys as f64 * 100.0 / (key_sets_num as f64 * self.keys_num as f64));
            unassigned_keys as f64
        }, args.view());
        println!("Optimal score weights: {ans:.0}");
    }
}