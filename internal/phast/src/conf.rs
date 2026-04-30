use std::str::FromStr;

use clap::{Parser, Subcommand, ValueEnum};
use ph::{phast::{Generic, SeedChooser, SeedChooserCore, Turbo, bucket_size_normalization_multiplier}, utils::verify_partial_kphf};

use crate::{benchmark::{Result, benchmark}, function::{Function, PartialFunction}, optim::{Cost, CostFn, PerfectLog0Cost, PerfectLog1Cost, PerfectLogCost, PerfectProdKCost, ProdOfValuesCost, WGenericProdOfValues, WeightsCost}};

use optimize::{Minimizer, NelderMeadBuilder};
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
    optprod,

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
            Method::optprod => write!(f, "Optimize seed evaluation in ProdOfValues"),
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
    /// Bounded Nelder Mead
    #[clap(alias = "bnm")]
    BoundedNelderMead,
    /// Particle Swarm
    #[clap(alias = "pso")]
    ParticleSwarm,
}

impl std::fmt::Display for Optimizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match *self {
                Optimizer::NelderMead => "Nelder Mead",
                Optimizer::BoundedNelderMead => "Bounded Nelder Mead",
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

    /// Desired loading factor * 1000
    #[arg(short='a', long, default_value_t = 1000, value_parser = clap::value_parser!(u16).range(1..=1000))]
    pub alpha: u16
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

    pub fn params<SS>(&self, seed_size: SS, bucket_size100: u16) -> ph::phast::Conf<SS, Generic> {
        ph::phast::Conf {
            seed_size,
            core_conf: Generic { bucket_size100, preferred_slice_len: self.slice_len },
            hasher: Default::default(),
            loading_factor_1000: self.alpha
        }
    }

    pub fn params_turbo<SS>(&self, seed_size: SS) -> ph::phast::Conf<SS, Turbo> {
        ph::phast::Conf {
            seed_size,
            core_conf: Turbo { preferred_slice_len: self.slice_len },
            hasher: Default::default(),
            loading_factor_1000: self.alpha
        }
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

    pub fn core<SC: SeedChooserCore>(&self, seed_chooser_core: SC) -> ph::phast::GenericCore {
        seed_chooser_core.minimal_generic_f_core(self.keys_num as usize, self.bits_per_seed, self.bucket_size().into(), self.slice_len)
    }

    pub const KEY_SETS_NUM: u32 = 96;

    pub fn par_eval<F: Fn(&mut [u64]) -> usize + Sync>(&self, f: F) -> usize {
        (0..Self::KEY_SETS_NUM).into_par_iter().map(|i| {
            f(&mut self.keys_for_seed(200+i))
        }).sum()
    }

    pub fn optimize<CF: CostFn>(&self, cost: CF) {
        println!("{} optimization steps with {}", self.optimization_iters(), self.optimizer);
        let mut cost = Cost::new(self, cost);
        match self.optimizer {
            Optimizer::NelderMead => {
                let nm = NelderMeadBuilder::default()
                    .maxiter(self.optimization_iters() as usize) 
                    .ulps(128)
                    .build()
                    .unwrap();
                nm.minimize(
                    |x: ArrayView1<f64>| cost.eval(x.as_slice().unwrap()) as f64,
                    Array::from_vec(cost.init()).view());
                //cost.print(ans.as_slice().unwrap());
            },
            Optimizer::ParticleSwarm => {
                use argmin::{solver::particleswarm::ParticleSwarm};
                let pso: ParticleSwarm<Vec<f64>, f64, _> = ParticleSwarm::new(cost.bounds(), 40);
                let executor = argmin::core::Executor::new(cost, pso).configure(|state|
                    state
                        // Set initial parameters (depending on the solver,
                        // this may be required)
                        //.param(args)
                        // Set maximum iterations to 10
                        // (optional, set to `std::u64::MAX` if not provided)
                        .max_iters(self.optimization_iters() as u64)
                        // Set target cost. The solver stops when this cost
                        // function value is reached (optional)
                        //.target_cost(0.0)
                );
                let mut res = executor.run().unwrap();
                println!("{}", res);
                cost = res.problem.take_problem().unwrap();
            },
            Optimizer::BoundedNelderMead => {
                let mut mn = MnSimplex::new().max_fcn(self.optimization_iters() as usize);
                for (index, (value, (mut name, lo, up, prec))) in cost.init().into_iter().zip(cost.params()).enumerate() {
                    let error = 0.1f64.powi(prec as i32);
                    let name_buf;
                    if name.is_empty() { name_buf = index.to_string(); name = &name_buf; }
                    mn = match (lo, up) {
                        (crate::optim::Constrain::Weak(_), crate::optim::Constrain::Weak(_)) => mn.add(name, value, error),
                        (crate::optim::Constrain::Weak(_), crate::optim::Constrain::Strong(upper)) => mn.add_upper_limited(name, value, error, upper),
                        (crate::optim::Constrain::Strong(lower), crate::optim::Constrain::Weak(_)) => mn.add_lower_limited(name, value, error, lower),
                        (crate::optim::Constrain::Strong(lower), crate::optim::Constrain::Strong(upper)) => mn.add_limited(name, value, error, lower, upper),
                    }
                }
                print!("{}", mn.minimize(&|x: &[f64]| cost.eval(x) as f64));
            }
        }
        cost.print_best();
    }

    pub fn optimize_weights<SC: SeedChooser>(&self, seed_chooser: SC) {
        self.optimize(WeightsCost(seed_chooser));
    }

    pub fn optimize_perfectlog(&self) {
        self.optimize(PerfectLogCost)
    }

    pub fn optimize_perfectlog0(&self) {
        self.optimize(PerfectLog0Cost);
    }

    pub fn optimize_perfectlog1(&self) {
        self.optimize(PerfectLog1Cost);
    }

    pub fn optimize_kprod(&self) {
        self.optimize(PerfectProdKCost);
    }

    pub fn optimize_genericprod(&self) {
        self.optimize(ProdOfValuesCost)
    }

    pub fn optimize_wgenericprod(&self) {
        self.optimize(WGenericProdOfValues([181.0, 177.0, 108.0, 80.0]));
    }
}