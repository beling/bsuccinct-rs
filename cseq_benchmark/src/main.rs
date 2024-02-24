#![doc = include_str!("../README.md")]

mod elias_fano;
mod bitm;
mod sucds;
mod succinct;
mod sux;
#[cfg(feature = "vers-vecs")] mod vers;

use std::{fs::{File, OpenOptions}, hint::black_box, num::{NonZeroU32, NonZeroU64}, ops::Range, time::Instant};
use std::io::Write;

use butils::{UnitPrefix, XorShift64};
use clap::{Parser, Subcommand, ValueEnum};

//#[allow(non_camel_case_types)]
//#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[derive(Subcommand)]
pub enum Structure {
    /// Elias-Fano from cseq crate
    EliasFano,
    /// Rank/Select on uncompressed bit vector using bitm crate
    BitmBV,
    /// Rank/Select on uncompressed bit vector using sucds crate
    SucdsBV,
    /// Rank/Select on uncompressed bit vector using Jacobson from succinct crate
    #[clap(visible_alias = "succ-jacobson")]
    SuccinctJacobson,
    /// Rank/Select on uncompressed bit vector using Rank9 from succinct crate
    #[clap(visible_aliases = ["succ-rank9", "succ-r9"])]
    SuccinctRank9,
    /// SelectFixed1 on uncompressed bit vector using sux crate
    #[clap(visible_aliases = ["sux-sf1", "sux-sel1"])]
    SuxSelectFixed1,
    /// SelectFixed2 on uncompressed bit vector using sux crate
    #[clap(visible_aliases = ["sux-sf2", "sux-sel2"])]
    SuxSelectFixed2,
    /// Rank/Select on uncompressed bit vector using vers crate
    #[cfg(feature = "vers-vecs")] Vers,
    /// Rank and select on bit vectors using all supported methods and crates
    BV
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Distribution {
    /// Items distributed uniformly across the universe.
    #[clap(alias = "u")]
    Uniform,
    /// 99% of the items on the last num indexes of the universe.
    #[clap(alias = "a")]
    Adversarial,
    /// Item density increases linearly with indexes.
    #[clap(aliases = ["l", "ld"])]
    LinearlyDensified,
}

impl std::fmt::Display for Distribution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match *self {
            Distribution::Uniform => "uniform",
            Distribution::Adversarial => "adversarial",
            Distribution::LinearlyDensified => "linearly densified",
        })
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None, infer_subcommands=true, infer_long_args=true)]
/// Compact sequences benchmark.
pub struct Conf {
    /// Structure to test
    #[command(subcommand)]
    pub structure: Structure,

    /// The number of items to use
    #[arg(short = 'n', long, default_value_t = 500_000_000)]
    pub num: usize,

    /// Item universe.
    #[arg(short = 'u', long, default_value_t = 1_000_000_000)]
    pub universe: usize,

    /// Distribution of items across the universe.
    #[arg(short='d', long, value_enum, default_value_t = Distribution::Uniform)]
    pub distribution: Distribution,

    /// Time (in seconds) of measuring and warming up the CPU cache before measuring
    #[arg(short='t', long, default_value_t = 5)]
    pub time: u16,

    /// Time (in seconds) of cooling (sleeeping) before measuring
    #[arg(short='c', long, default_value_t = 2)]
    pub cooling_time: u16,

    /// Whether to check the validity of built sequence
    #[arg(long, default_value_t = false)]
    pub verify: bool,

    /// Seed for (XorShift64) random number generator
    #[arg(short='s', long, default_value_t = NonZeroU64::new(1234).unwrap())]
    pub seed: NonZeroU64,

    // Number of pre-generated queries
    #[arg(short='q', long, default_value_t = NonZeroU32::new(1_000_000).unwrap())]
    pub queries: NonZeroU32,

    /// Save detailed results to CSV file(s)
    #[arg(short='f', long, default_value_t = false)]
    pub save_details: bool,
}

const INPUT_HEADER: &'static str = "universe,num,distribution";
const RANK_SELECT_HEADER: &'static str = "method,space_overhead,time_per_query";

struct Tester<'c> {
    conf: &'c Conf,
    number_of_ones: usize,
    rank_includes_current: bool
}

fn check<R: Into<Option<usize>>>(structure_name: &str, operation_name: &str, argument: usize, expected: usize, got: R) {
    if let Some(got) = got.into() {
        if got != expected {
            eprintln!("{structure_name}: {operation_name}({argument}) returned {got}, but should {expected}");
        }
    } else {
        eprintln!("{structure_name}: select({argument}) returned None, but should {expected}")
    }
}

impl<'c> Tester<'c> {
    #[inline(always)] pub fn raport_rank<R: Into<Option<usize>>, F>(&self, method_name: &str, space_overhead: f64, rank: F)
    where F: Fn(usize) -> R
    {
        print!("  rank:  space overhead {:.2}%", space_overhead);
        let time = self.conf.queries_measure(&self.conf.rand_queries(self.conf.universe), &rank).as_nanos();
        println!("  time/query {:.2}ns", time);
        self.conf.save_rank(method_name, space_overhead, time);
        self.verify_rank(method_name, rank);
    }

    fn verify_rank<R: Into<Option<usize>>, F>(&self, method_name: &str, rank: F) where F: Fn(usize) -> R {
        if self.conf.verify {
            //print!("   verification of rank answers... ");
            self.conf.data_foreach(|index, mut expected_rank, value| {
                if self.rank_includes_current && value { expected_rank += 1 }
                check(method_name, "rank", index, expected_rank, rank(index))
            });
            //println!("DONE");
        }
    }

    #[inline(always)] pub fn raport_select1<R: Into<Option<usize>>, F>(&self, method_name: &str, space_overhead: f64, select: F)
    where F: Fn(usize) -> R
    {
        if self.number_of_ones == 0 {
            //println!("skipping select1 test as there are no ones");
            return;
        }
        print!("  select1:");
        if space_overhead != 0.0 { print!("  space overhead {:.2}%", space_overhead); }
        let time = self.conf.queries_measure(&self.conf.rand_queries(self.number_of_ones), &select).as_nanos();
        println!("  time/query {:.2}ns", time);
        self.conf.save_select1(method_name, space_overhead, time);
        self.verify_select1(method_name, select);
    }

    fn verify_select1<R: Into<Option<usize>>, F>(&self, method_name: &str, select: F) where F: Fn(usize) -> R {
        if self.conf.verify {
            //print!("   verification of select1 answers... ");
            self.conf.data_foreach(|index, rank, value| if value {
                check(method_name, "select", rank, index, select(rank))
            });
            //println!("DONE");
        }
    }

    #[inline(always)] pub fn raport_select0<R: Into<Option<usize>>, F>(&self, method_name: &str, space_overhead: f64, select0: F)
    where F: Fn(usize) -> R
    {
        if self.conf.universe == self.number_of_ones {
            //println!("skipping select0 test as there are no zeros");
            return;
        }
        print!("  select0:");
        if space_overhead != 0.0 { print!("  space overhead {:.2}%", space_overhead); }
        let time = self.conf.queries_measure(
            &self.conf.rand_queries(self.conf.universe-self.number_of_ones),
            &select0).as_nanos();
        println!("  time/query {:.2}ns", time);
        self.conf.save_select0(method_name, space_overhead, time);
        self.verify_select0(method_name, select0);
    }

    fn verify_select0<R: Into<Option<usize>>, F>(&self, method_name: &str, select0: F) where F: Fn(usize) -> R {
        if self.conf.verify {
            //print!("   verification of select0 answers... ");
            self.conf.data_foreach(|index, rank1, value| if !value {
                let rank0 = index - rank1;
                check(method_name, "select0", rank0, index, select0(rank0))
            });
            //println!("DONE");
        }
    }
}

impl Conf {
    #[inline(always)] fn uniform_foreach<F: FnMut(usize, usize, bool)>(&self, mut f: F, gen: &mut XorShift64, total_ones: &mut usize, mut num: usize, universe: Range<usize>) {
        let mut remain_universe = universe.len();
        for i in universe {
            let included = gen.get() as usize % remain_universe < num;
            f(i, *total_ones, included);
            if included {
                *total_ones += 1;
                num -= 1;
            }
            remain_universe -= 1;
        }
    }

    #[inline(always)] fn data_foreach<F: FnMut(usize, usize, bool)>(&self, mut f: F) -> usize {
        let mut gen = self.rand_gen();
        let mut number_of_ones = 0;

        match self.distribution {
            Distribution::Uniform => self.uniform_foreach(f, &mut gen, &mut number_of_ones, self.num, 0..self.universe),
            Distribution::Adversarial => {
                let sparse_threshold = self.universe - self.num;
                self.uniform_foreach(&mut f, &mut gen, &mut number_of_ones, (self.num+50)/100, 0..sparse_threshold);
                let num = self.num - number_of_ones;
                self.uniform_foreach(f, &mut gen, &mut number_of_ones, num, sparse_threshold..self.universe);
            }
            Distribution::LinearlyDensified => {    // linear density increase
                let (reverse, num_dbl) = if self.num * 2 > self.universe {
                    (true, (self.universe - self.num)*2)
                } else {
                    (false, self.num*2)
                };
                for i in 0..self.universe {
                    /*let remain_universe = self.universe - i;
                    let remain_num = num - number_of_ones;
                    let included = (gen.get() as usize % remain_universe * (remain_universe-1) < 2 * remain_num * i) ^ reverse;*/
                    let j = if reverse { self.universe - i } else { i };
                    let included = (gen.get() as usize % self.universe * (self.universe-1) < num_dbl * j) ^ reverse;
                    f(i, number_of_ones, included);
                    number_of_ones += included as usize;
                }
            }
        }
        number_of_ones
    }

    fn rand_data<F: FnMut(usize, bool)>(&self, mut add: F) -> Tester {
        let number_of_ones = self.data_foreach(|index, _, v| add(index, v));
        println!(" input: number of bit ones is {} / {} ({:.2}%), {} distribution",
            number_of_ones, self.universe, percent_of(number_of_ones, self.universe), self.distribution);
        Tester { conf: self, number_of_ones, rank_includes_current: false }
    }

    fn file(&self, file_name: &str, extra_header: &str) -> Option<File> {
        if !self.save_details { return None; }
        let file_name = format!("{}.csv", file_name);
        let file_already_existed = std::path::Path::new(&file_name).exists();
        let mut file = OpenOptions::new().append(true).create(true).open(&file_name).unwrap();
        if !file_already_existed { writeln!(file, "{},{}", INPUT_HEADER, extra_header).unwrap(); }
        Some(file)
    }

    fn save_rank_or_select(&self, file_name: &str, method_name: &str, space_overhead: f64, time: f64) {
        if let Some(mut file) = self.file(file_name, RANK_SELECT_HEADER) {
            writeln!(file, "{},{},{},{},{},{}", self.universe, self.num, self.distribution, method_name, space_overhead, time).unwrap();
        }
    }
    
    pub fn save_rank(&self, method_name: &str, space_overhead: f64, time: f64) {
        self.save_rank_or_select("rank", method_name, space_overhead, time)
    }

    pub fn save_select1(&self, method_name: &str, space_overhead: f64, time: f64) {
        self.save_rank_or_select("select1", method_name, space_overhead, time)
    }

    pub fn save_select0(&self, method_name: &str, space_overhead: f64, time: f64) {
        self.save_rank_or_select("select0", method_name, space_overhead, time)
    }

    fn rand_gen(&self) -> XorShift64 { XorShift64(self.seed.get()) }

    fn rand_queries(&self, query_universe: usize) -> Box<[usize]> {
        self.rand_gen().take(self.queries.get() as usize).map(|v| v as usize % query_universe).collect()
    }

    #[inline(always)] fn measure<F>(&self, f: F) -> f64
     where F: Fn()
    {
        if self.cooling_time > 0 {
            std::thread::sleep(std::time::Duration::from_secs(self.cooling_time as u64));
        }
        let mut iters = 1;
        if self.time > 0 {
            let time = Instant::now();
            loop {
                f();
                if time.elapsed().as_secs() > self.time as u64 { break; }
                iters += 1;
            }
        }
        let start_moment = Instant::now();
        for _ in 0..iters { f(); }
        return start_moment.elapsed().as_secs_f64() / iters as f64
    }

    #[inline(always)] fn queries_measure<R, F>(&self, queries: &[usize], f: F) -> f64
    where F: Fn(usize) -> R
    {
        self.measure(|| for i in queries { black_box(f(*i)); }) / queries.len() as f64
    }

    /*#[inline(always)]pub fn raport_rank<R, F>(&self, method_name: &str, space_overhead: f64, f: F)
    where F: Fn(usize) -> R
    {
        print!("  rank:  space overhead {:.2}%", space_overhead);
        let time = self.queries_measure(&self.rand_queries(self.universe), f).as_nanos();
        println!("  time/query {:.2}ns", time);
        self.save_rank(method_name, space_overhead, time)
    }

    #[inline(always)]pub fn raport_select1<R, F>(&self, method_name: &str, space_overhead: f64, f: F)
    where F: Fn(usize) -> R
    {
        print!("  select1:");
        if space_overhead != 0.0 { print!("  space overhead {:.2}%", space_overhead); }
        let time = self.queries_measure(&self.rand_queries(self.num), f).as_nanos();
        println!("  time/query {:.2}ns", time);
        self.save_select1(method_name, space_overhead, time)
    }

    #[inline(always)]pub fn raport_select0<R, F>(&self, method_name: &str, space_overhead: f64, f: F)
    where F: Fn(usize) -> R
    {
        print!("  select0:");
        if space_overhead != 0.0 { print!("  space overhead {:.2}%", space_overhead); }
        let time = self.queries_measure(&self.rand_queries(self.universe-self.num), f).as_nanos();
        println!("  time/query {:.2}ns", time);
        self.save_select0(method_name, space_overhead, time)
    }*/

    /*#[inline(always)] fn sampling_measure<R, F>(&self, steps: StepBy<Range<usize>>, f: F) -> f64
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
    }*/
}

fn percent_of(overhead: usize, whole: usize) -> f64 { (overhead*100) as f64 / whole as f64 }
fn percent_of_diff(with_overhead: usize, whole: usize) -> f64 { percent_of(with_overhead-whole, whole) }

fn main() {
    let conf: Conf = Conf::parse();
    match conf.structure {
        Structure::EliasFano => elias_fano::benchmark(&conf),
        Structure::BitmBV => bitm::benchmark_rank_select(&conf),
        Structure::SucdsBV => sucds::benchmark_rank9_select(&conf),
        Structure::SuccinctJacobson => succinct::benchmark_jacobson(&conf),
        Structure::SuccinctRank9 => succinct::benchmark_rank9(&conf),
        Structure::SuxSelectFixed1 => sux::benchmark_select_fixed1(&conf),
        Structure::SuxSelectFixed2 => sux::benchmark_select_fixed2(&conf),
        #[cfg(feature = "vers-vecs")] Structure::Vers => vers::benchmark_rank_select(&conf),
        Structure::BV => {
            bitm::benchmark_rank_select(&conf);
            sucds::benchmark_rank9_select(&conf);
            succinct::benchmark_rank9(&conf);
            succinct::benchmark_jacobson(&conf);
            #[cfg(feature = "vers-vecs")] vers::benchmark_rank_select(&conf);
            sux::benchmark_select_fixed2(&conf);
            sux::benchmark_select_fixed1(&conf);
        },
    }
}