use std::str::FromStr;

use clap::{Parser, Subcommand, ValueEnum};
use ph::{phast::{Core, Generic, KSeedEvaluatorConf, Partial, SeedChooser, SeedChooserCore, SeedCore, SeedKCore, SeedOnly, SeedOnlyK, Turbo, bucket_size_normalization_multiplier}, seeds::BitsFast, utils::verify_partial_kphf};

use crate::{benchmark::{Result, benchmark}, function::{Function, PartialFunction}, optim::{Cost, CostFn, GenericProdOfValues, PerfectLog0Cost, PerfectLog1Cost, PerfectLogCost, SumOfLogValuesF, SumOfLogValuesF0, SumOfLogValuesF1, SumOfLogValuesFEval, SumOfLogValuesFW1, WGenericProdOfValues, WeightsCost, WeightsF}};

use optimize::{Minimizer, NelderMead, NelderMeadBuilder};
use ndarray::{Array, ArrayView1};
use rayon::{current_num_threads, iter::{IntoParallelIterator, ParallelIterator}};

use minuit2::MnSimplex;

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

    /// k-perfect PHast with logarithmic seed evaluation
    perfectlog,

    /// k-perfect PHast with logarithmic seed evaluation and first_weight=0
    perfectlog0,

    /// k-perfect PHast with logarithmic seed evaluation and first_weight=1
    perfectlog1,

    /// Optimize weights for selecting buckets by PHast
    optphast,

    /// Optimize weights for selecting buckets by PHast+ with wrapping
    optpluswrap {
        #[arg(default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=3))]
        multiplier: u8
    },

    /// Optimize weights for selecting buckets by PHast+
    optplus,

    /// Optimize seed evaluation in perfectlog
    optperfectlog,

    /// Optimize seed evaluation in perfectlog with first_weight=0
    optperfectlog0,

    /// Optimize seed evaluation in perfectlog with first_weight=1
    optperfectlog1,

    /// Optimize seed evaluation in perfectlog with free_values_weight=1
    optperfectlogf1,

    optgenprod,

    optwgenprod,

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
            Method::perfectlog => write!(f, "Perfect with: log(f(x) - minimum sum + value_shift) - free_values_weight * log(free(f(x)+free_shift))"),
            Method::perfectlog0 => write!(f, "Perfect with: log(f(x) - minimum in bucket + value_shift) - free_values_weight * log(free(f(x)+free_shift))"),
            Method::perfectlog1 => write!(f, "Perfect with: log(f(x) - minimum in window + value_shift) - free_values_weight * log(free(f(x)+free_shift))"),
            Method::optphast => write!(f, "Optimize PHast weights"),
            Method::optpluswrap { multiplier } => write!(f, "Optimize PHast+wrap {multiplier} weights"),
            Method::optplus => write!(f, "Optimize PHast+ weights"),
            Method::optperfectlog => write!(f, "Optimize seed evaluation in perfectlog"),
            Method::optperfectlog0 => write!(f, "Optimize seed evaluation in perfectlog with first_weight=0"),
            Method::optperfectlog1 => write!(f, "Optimize seed evaluation in perfectlog with first_weight=1"),
            Method::optperfectlogf1 => write!(f, "Optimize seed evaluation in perfectlog with free_values_weight=1"),
            Method::optgenprod => write!(f, "Optimize GenericProdOfValues"),
            Method::optwgenprod => write!(f, "Optimize WGenericProdOfValues"),
            Method::none => write!(f, "Do nothing"),
        }
    }
}

pub const CSV_HEADER: &'static str = "keys_num method k bits/seed bucket_size100 slice threads seed bits/key bumped_% range_overhead_% build_ns/key query_ns/key";

#[derive(Clone, Copy, PartialEq)]
pub enum BucketSize {
    Size100(u16),
    Turbo
}

impl FromStr for BucketSize {
    type Err = String;
    
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let sl = s.to_lowercase();
        Ok(if sl == "t" || sl == "turbo" {
            BucketSize::Turbo
        } else {
            BucketSize::Size100(s.parse()
                .map_err(|_| "Expected number or 't'".to_string())?)
        })
    }
}

impl std::fmt::Display for BucketSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BucketSize::Size100(v) => std::fmt::Display::fmt(&(*v as f64 / 100.0), f),
            BucketSize::Turbo => write!(f, "4 (turbo)"),
        }
    }
}

impl Into<u16> for BucketSize {
    fn into(self) -> u16 {
        match self {
            BucketSize::Size100(v) => v,
            BucketSize::Turbo => 400,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Optimizer {
    /// Nelder Mead
    #[clap(alias = "nm")]
    NelderMead,
    /// Particle Swarm
    #[clap(alias = "pso")]
    ParticleSwarm,
}

impl std::fmt::Display for Optimizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match *self {
                Optimizer::NelderMead => "Nelder Mead",
                Optimizer::ParticleSwarm => "Particle Swarm",
            }
        )
    }
}


#[derive(Parser)]
#[command(author="Piotr Beling", version, about, long_about = None)]
/// Minimal perfect hashing benchmark.
pub struct Conf {
    /// Method to run
    #[command(subcommand)]
    pub method: Method,

    /// Number of bits to store seed of each bucket (0 for turbo)
    #[arg(short='s', default_value_t = 8, value_parser = clap::value_parser!(u8).range(0..16))]
    pub bits_per_seed: u8,

    /// Expected number of keys per bucket multiplied by 100
    #[arg(short='b')]
    pub bucket_size: Option<BucketSize>,

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
    #[arg(short, default_value_t = 1, value_parser = clap::value_parser!(u16).range(1..))]
    pub k: u16,

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

    /// Numerical Optimization algorithm to use
    #[arg(short='o', long, value_enum, default_value_t = Optimizer::NelderMead)]
    pub optimizer: Optimizer,
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

    pub fn bucket_size(&self) -> BucketSize {
        self.bucket_size.unwrap_or_else(|| BucketSize::Size100(
            (ph::phast::bits_per_seed_to_100_bucket_size(self.bits_per_seed) as f64 * bucket_size_normalization_multiplier(self.k)) as u16
        ))
    }

    pub fn is_turbo(&self) -> bool {
        self.bucket_size == Some(BucketSize::Turbo)
    }

    pub fn keys_for_seed(&self, seed: u32) -> Box<[u64]> {
        butils::XorShift64(seed as u64).take(self.keys_num as usize).collect()
    }

    pub fn params<SS>(&self, seed_size: SS, bucket_size100: u16) -> Generic<SS> {
        Generic { seed_size, bucket_size100, preferred_slice_len: self.slice_len }
    }

    pub fn params_turbo<SS>(&self, seed_size: SS) -> Turbo<SS> {
        Turbo { seed_size, preferred_slice_len: self.slice_len }
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
            self.k, self.bits_per_seed, self.bucket_size(), self.slice_len, self.threads())
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

    fn optimizer<SC: SeedChooserCore>(&self, seed_chooser_core: SC) -> (NelderMead, ph::phast::GenericCore) {
        let bucket_size = self.bucket_size().into();
        let minimizer = NelderMeadBuilder::default()
            .maxiter(self.optimization_iters() as usize) 
            .ulps(128)
            .build()
            .unwrap();
        (minimizer, seed_chooser_core.conf_for_minimal(self.keys_num as usize, self.bits_per_seed, bucket_size, self.slice_len))
    }

    pub fn core<SC: SeedChooserCore>(&self, seed_chooser_core: SC) -> ph::phast::GenericCore {
        seed_chooser_core.conf_for_minimal(self.keys_num as usize, self.bits_per_seed, self.bucket_size().into(), self.slice_len)
    }

    pub const KEY_SETS_NUM: u32 = 96;

    pub fn par_eval<F: Fn(&mut [u64]) -> usize + Sync>(&self, x: &[f64], f: F) -> usize {
        (0..Self::KEY_SETS_NUM).into_par_iter().map(|i| {
            f(&mut self.keys_for_seed(200+i))
        }).sum()
    }

    fn par_f_eval<F: Fn(&mut [u64]) -> usize + Sync>(&self, x: &[f64], f: F) -> f64 {
        let unassigned_keys: usize = (0..Self::KEY_SETS_NUM).into_par_iter().map(|i| {
            f(&mut self.keys_for_seed(200+i))
        }).sum();
        print!("{unassigned_keys} {:.2}%", unassigned_keys as f64 * 100.0 / (Self::KEY_SETS_NUM as f64 * self.keys_num as f64));
        for v in x { print!(" {v:.0}") }
        println!();
        unassigned_keys as f64
    }

    pub fn optimize<CF: CostFn>(&self, cost: CF) {
        println!("{} optimization steps with {}", self.optimization_iters(), self.optimizer);
        let cost = Cost::new(self, cost);
        match self.optimizer {
            Optimizer::NelderMead => {
                let nm = NelderMeadBuilder::default()
                    .maxiter(self.optimization_iters() as usize) 
                    .ulps(128)
                    .build()
                    .unwrap();
                let ans = nm.minimize(|x: ArrayView1<f64>| {
                    cost.eval(x.as_slice().unwrap()) as f64
                }, Array::from_vec(cost.init()).view());
                println!("Optimal weights: {ans:.0}");
            },
            Optimizer::ParticleSwarm => {
                use argmin::{solver::particleswarm::ParticleSwarm};
                let pso: ParticleSwarm<Vec<f64>, f64, _> = ParticleSwarm::new(cost.bounds(), 10);
                let executor = argmin::core::Executor::new(cost, pso)/*.configure(|state|
                    state
                        // Set initial parameters (depending on the solver,
                        // this may be required)
                        .param(args)
                        // Set maximum iterations to 10
                        // (optional, set to `std::u64::MAX` if not provided)
                        .max_iters(10)
                        // Set target cost. The solver stops when this cost
                        // function value is reached (optional)
                        .target_cost(0.0)
                )*/;
                let res = executor.run().unwrap();
                println!("{}", res);
            }
        }
    }

    pub fn optimize_weights<SC: SeedChooser>(&self, seed_chooser: SC) {
        self.optimize(WeightsCost(seed_chooser));
    }

    pub fn optimize_perfectlog(&self) {
        self.optimize(PerfectLogCost)
        //let ans = &res.state().get_best_param().unwrap().position;
        //println!("Optimal parameters: free_values_weight: {:.5}, value_shift: {:.5}, free_shift: {:.5}", ans[2], ans[0], ans[1]);
    }

    pub fn optimize_perfectlog0(&self) {
        self.optimize(PerfectLog0Cost);
    }

    pub fn optimize_perfectlog1(&self) {
        self.optimize(PerfectLog1Cost);
    }

    pub fn optimize_perfectlogf1(&self) {
        let s = SumOfLogValuesFW1.for_k(self.k);
        let args = Array::from_vec(vec![s.value_shift, s.free_shift, s.first_weight]);
        let (minimizer, conf) = self.optimizer(SeedKCore(self.k));
        
        let ans = minimizer.minimize(|x: ArrayView1<f64>| {
            println!("value_shift: {:.5}, free_shift: {:.5}, first_weight: {:.5}", x[0], x[1], x[2]);
            let evaluator = SumOfLogValuesFEval { free_values_weight: 1.0, value_shift: x[0], free_shift: x[1], first_weight: x[2] };
            self.par_f_eval(x.as_slice().unwrap(), |keys| Partial::with_hashes_bps_conf_sc_u(keys, BitsFast(self.bits_per_seed),
                    conf, SeedOnlyK::new(self.k, evaluator)).1)
        }, args.view());
        println!("Optimal parameters: value_shift: {:.5}, free_shift: {:.5}, first_weight: {:.5}", ans[0], ans[1], ans[2]);
    }

    /*pub fn optimize_genericprod(&self) {
        let s = GenericProdOfValues { first_weight: 0.0, shift: 70.0 };
        let args = Array::from_vec(vec![s.first_weight, s.shift]);
        let (minimizer, conf) = self.optimizer(&SeedOnly(s));
        
        let ans = minimizer.minimize(|x: ArrayView1<f64>| {
            let evaluator = GenericProdOfValues { first_weight: x[0], shift: x[1] };
            println!("first_weight: {}, shift: {}", x[0], x[1]);
            self.par_f_eval(x, |keys| Partial::with_hashes_bps_conf_sc_u(keys, BitsFast(self.bits_per_seed),
                    conf, SeedOnly(evaluator)).1)
        }, args.view());
        println!("Optimal parameters: first_weight: {}, shift: {}", ans[0], ans[1]);
    }*/

    pub fn optimize_genericprod(&self) {
        let conf = SeedOnly(GenericProdOfValues { first_weight: 0.5, shift: 100.0 }).conf_for_minimal(self.keys_num as usize, self.bits_per_seed, self.bucket_size().into(), self.slice_len);
        let result = MnSimplex::new()
            .add_limited("first_weight", 0.5, 0.01, 0.0, 1.0)
            .add_lower_limited("shift", 100.0, 1.0, 1.0)
            .max_fcn(if self.iters == 0 { 100 } else {self.iters as usize})
            .minimize(&|x: &[f64]| {
                println!("first_weight: {}, shift: {}", x[0], x[1]);
                let evaluator = GenericProdOfValues { first_weight: x[0], shift: x[1] };
                self.par_f_eval(x, |keys| Partial::with_hashes_bps_conf_sc_u(keys, BitsFast(self.bits_per_seed),
                    conf, SeedOnly(evaluator)).1)
            });
        println!("{result}");
    }

    /*pub fn optimize_wgenericprod(&self) {
        let conf = SeedOnly(WGenericProdOfValues([145.0; 9])).conf_for_minimal(self.keys_num as usize, self.bits_per_seed, self.bucket_size().into(), self.slice_len);
        let result = MnSimplex::new()
            .add_lower_limited("1", 145.0, 10.0, 1.0)
            .add_lower_limited("2", 145.0, 10.0, 1.0)
            .add_lower_limited("3", 145.0, 10.0, 1.0)
            .add_lower_limited("4", 145.0, 10.0, 1.0)
            .add_lower_limited("5", 145.0, 10.0, 1.0)
            .add_lower_limited("6", 145.0, 10.0, 1.0)
            .add_lower_limited("7", 145.0, 10.0, 1.0)
            .add_lower_limited("8", 145.0, 10.0, 1.0)
            .add_lower_limited("9", 145.0, 10.0, 1.0)
            .max_fcn(if self.iters == 0 { 100 } else {self.iters as usize})
            .minimize(&|x: &[f64]| {
                let evaluator = WGenericProdOfValues(*x.as_array::<9>().unwrap());
                self.par_f_eval(x, |keys| Partial::with_hashes_bps_conf_sc_u(keys, BitsFast(self.bits_per_seed),
                    conf, SeedOnly(evaluator)).1)
            });
        println!("{result}");
    }*/

    pub fn optimize_wgenericprod(&self) {
        let (minimizer, conf) = self.optimizer(SeedCore);
        let args = Array::from_vec(vec![181.0, 177.0, 108.0, 80.0]);
        let ans = minimizer.minimize(|x: ArrayView1<f64>| {
            let slice = x.as_slice().unwrap();
            let evaluator = WGenericProdOfValues(*slice.as_array::<4>().unwrap());
            self.par_f_eval(slice, |keys| Partial::with_hashes_bps_conf_sc_u(keys, BitsFast(self.bits_per_seed),
                    conf, SeedOnly(evaluator)).1)
        }, args.view());
        println!("Optimal weights: {ans:.0}");
    }
}