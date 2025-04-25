#![doc = include_str!("../README.md")]

#[cfg(feature = "cmph-sys")] mod cmph;
use builder::TypeToQuery;
#[cfg(feature = "cmph-sys")] use cmph::chd_benchmark;

mod builder;
pub use builder::MPHFBuilder;

mod stats;
use ph::phast::compressed_array::CompactFast;
use ph::phast::{bits_per_seed_to_100_bucket_size, DefaultCompressedArray};
pub use stats::{SearchStats, BuildStats, BenchmarkResult, file, print_input_stats};

mod inout;
use inout::{gen_data, RandomStrings, RawLines};

#[cfg(feature = "fmph")] mod fmph;
#[cfg(feature = "fmph")] use fmph::{fmph_benchmark, fmphgo_benchmark_all, fmphgo_run, FMPHGOBuildParams, FMPHGO_HEADER};

mod phast;
use phast::phast_benchmark;

#[cfg(feature = "ptr_hash")] mod ptrhash;
#[cfg(feature = "ptr_hash")] use ptrhash::ptrhash_benchmark;

use butils::{XorShift32, XorShift64};
use clap::{Parser, ValueEnum, Subcommand, Args};

use std::hash::Hash;
use std::fmt::Debug;
use rayon::current_num_threads;

type IntHasher = ph::Seedable<fxhash::FxBuildHasher>;
type StrHasher = ph::BuildDefaultSeededHasher;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum KeyAccess {
    /// Random-access, read-only access to the keys is allowed. The algorithm stores 8-bit indices of the remaining keys.
    Indices8,
    #[cfg(feature = "fmph-key-access")]
    /// Random-access, read-only access to the keys is allowed. The algorithm stores 16-bit indices of the remaining keys.
    Indices16,
    #[cfg(feature = "fmph-key-access")]
    /// Vector of keys can be modified. The method stores remaining keys, and removes the rest from the vector.
    Copy
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Threads {
    /// Single thread
    Single = 1,
    /// Multiple threads
    Multi = 2,
    /// Single and multiple threads too
    Both = 2 | 1
}

#[cfg(feature = "fmph")]
#[allow(non_camel_case_types)]
#[derive(Args)]
pub struct FMPHConf {
    /// Relative level size as percent of number of keys, equals to *100γ*.
    #[arg(short='l', long)]
    pub level_size: Option<u16>,
    /// FMPH caches 64-bit hashes of keys when their number (at the constructed level) is below this threshold
    #[arg(short='c', long, default_value_t = usize::MAX)]
    pub cache_threshold: usize,
    /// How FMPH can access keys.
    #[arg(value_enum, short='a', long, default_value_t = KeyAccess::Indices8)]
    pub key_access: KeyAccess,
}


#[allow(non_camel_case_types)]
#[derive(Args)]
pub struct FMPHGOConf {
    /// Number of bits to store seed of each group, *s*
    #[arg(short='s', long, value_parser = clap::value_parser!(u8).range(1..16))]
    pub bits_per_group_seed: Option<u8>,
    /// The size of each group, *b*
    #[arg(short='b', long, value_parser = clap::value_parser!(u8).range(1..63))]
    pub group_size: Option<u8>,
    /// Relative level size as percent of number of keys, equals to *100γ*
    #[arg(short='l', long)]
    pub level_size: Option<u16>,
    /// FMPHGO caches 64-bit hashes of keys when their number (at the constructed level) is below this threshold
    #[arg(short='c', long, default_value_t = usize::MAX)]
    pub cache_threshold: usize,
    /// How FMPHGO can access keys
    #[arg(value_enum, short='a', long, default_value_t = KeyAccess::Indices8)]
    pub key_access: KeyAccess,
}

#[derive(Args)]
pub struct PHastConf {
    /// Number of bits to store seed of each bucket
    #[arg(default_value_t = 8, value_parser = clap::value_parser!(u8).range(1..16))]
    pub bits_per_seed: u8,

    /// Expected number of keys per bucket multipled by 100
    #[arg()]
    pub bucket_size: Option<u16>,

    /// Test with Elias-Fano encoder of array that makes PHast minimal
    #[arg(short='e', long="ef", default_value_t = false)]
    pub elias_fano: bool,

    /// Test with Compact encoder of array that makes PHast minimal
    #[arg(short='c', long, default_value_t = false)]
    pub compact: bool
}

impl PHastConf {
    fn bucket_size(&self) -> u16 {
        self.bucket_size.unwrap_or_else(|| bits_per_seed_to_100_bucket_size(self.bits_per_seed))
    }

    /// should elias fano be tested
    fn elias_fano(&self) -> bool {
        self.elias_fano || !self.compact
    }
}

#[allow(non_camel_case_types)]
//#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[derive(Subcommand)]
pub enum Method {
    // Most methods
    //Most,
    #[cfg(feature = "fmph")]
    /// FMPHGO with all settings
    FMPHGO_all,
    #[cfg(feature = "fmph")]
    /// FMPHGO with selected settings
    FMPHGO(FMPHGOConf),
    #[cfg(feature = "fmph")]
    /// FMPH
    FMPH(FMPHConf),
    /// PHast
    phast(PHastConf),
    #[cfg(feature = "boomphf")]
    /// boomphf
    Boomphf {
        /// Relative level size as percent of number of keys, equals to *100γ*
        #[arg(short='l', long)]
        level_size: Option<u16>
    },
    /// CHD
    #[cfg(feature = "cmph-sys")] CHD {
        /// The average number of keys per bucket. By default tests all lambdas from 1 to 6
        #[arg(short='l', long, value_parser = clap::value_parser!(u8).range(1..32))]
        lambda: Option<u8>
    },
    #[cfg(feature = "ptr_hash")]
    /// PtrHash
    PtrHash {
        /// Configuration: 0 = compact, 1 = default, 2 = fast
        #[arg(default_value_t = 1, value_parser = clap::value_parser!(u8).range(0..=2))]
        speed: u8
    },
    /// No method is tested
    None
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum KeySource {
    /// Generate 32 bit keys with xor-shift 32
    xs32,
    /// Generate 64 bit keys with xor-shift 64
    xs64,
    /// Standard input, separated by newlines (0xA or 0xD, 0xA bytes)
    stdin,
    /// Standard input, zero-separated
    stdinz,
    /// Random strings, each of length in [10, 50)
    randstr
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
/// Minimal perfect hashing benchmark.
pub struct Conf {
    /// Method to run
    #[command(subcommand)]
    pub method: Method,

    /// Number of times to perform the lookup test
    #[arg(short='l', long, default_value_t = 1)]
    pub lookup_runs: u32,

    /// Number of times to perform the construction
    #[arg(short='b', long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
    pub build_runs: u32,

    /// Whether to check the validity of built MPHFs
    #[arg(short='v', long, default_value_t = false)]
    pub verify: bool,

    #[arg(short='s', long, value_enum, default_value_t = KeySource::stdin)]
    pub key_source: KeySource,

    /// The number of random keys to use or maximum number of keys to read from stdin
    #[arg(short='n', long)]
    pub keys_num: Option<usize>,

    /// Number of foreign keys (to generate or read) used to test the frequency of detection of non-contained keys
    #[arg(short='f', long, default_value_t = 0)]
    pub foreign_keys_num: usize,

    /// Whether to build MPHF using single or multiple threads, or try both. Ignored by the methods that do not support building with single or multiple threads
    #[arg(short='t', long, value_enum, default_value_t = Threads::Both)]
    pub threads: Threads,

    /// Save detailed results to CSV-like (but space separated) file
    #[arg(short='d', long, default_value_t = false)]
    pub save_details: bool,

    /// Seed of random number generator.
    #[arg(long, default_value_t = 1234, value_parser = clap::value_parser!(u64).range(1..))]
    pub seed: u64
}

#[cfg(feature = "cmph-sys")] trait CanBeKey: Hash + Sync + Send + Clone + Debug + Default + cmph::CMPHSource + TypeToQuery {}
#[cfg(feature = "cmph-sys")] impl<T: Hash + Sync + Send + Clone + Debug + Default + cmph::CMPHSource + TypeToQuery> CanBeKey for T {}

#[cfg(not(feature = "cmph-sys"))] trait CanBeKey: Hash + Sync + Send + Clone + Debug + Default + TypeToQuery {}
#[cfg(not(feature = "cmph-sys"))] impl<T: Hash + Sync + Send + Clone + Debug + Default + TypeToQuery> CanBeKey for T {}

fn run<K: CanBeKey>(conf: &Conf, i: &(Vec<K>, Vec<K>)) {
    match conf.method {
        #[cfg(feature = "fmph")] Method::FMPHGO_all =>
            fmphgo_benchmark_all(file("FMPHGO_all", &conf, i.0.len(), i.1.len(), FMPHGO_HEADER),
                &i, &conf, KeyAccess::Indices8),
        #[cfg(feature = "fmph")] Method::FMPHGO(ref fmphgo_conf) => {
            let mut file = file("FMPHGO", &conf, i.0.len(), i.1.len(), FMPHGO_HEADER);
            println!("FMPHGO hash caching threshold={}: s b gamma results...", fmphgo_conf.cache_threshold);
            let mut p = FMPHGOBuildParams {
                relative_level_size: fmphgo_conf.level_size.unwrap_or(0),
                cache_threshold: fmphgo_conf.cache_threshold,
                key_access: fmphgo_conf.key_access,
            };
            match (fmphgo_conf.bits_per_group_seed, fmphgo_conf.group_size) {
                (None, None) => {
                    for (bits_per_group_seed, bits_per_group) in [(1, 8), (2, 16), (4, 16), (8, 32)] {
                        fmphgo_run(&mut file, i, conf, bits_per_group_seed, bits_per_group, &mut p);
                    }
                },
                (Some(bits_per_group_seed), Some(bits_per_group)) => fmphgo_run(&mut file, i, conf, bits_per_group_seed, bits_per_group, &mut p),
                (Some(1), None) | (None, Some(8)) => fmphgo_run(&mut file, i, conf, 1, 8, &mut p),
                (Some(2), None) => fmphgo_run(&mut file, i, conf, 2, 16, &mut p),
                (Some(4), None) => fmphgo_run(&mut file, i, conf, 4, 16, &mut p),
                (None, Some(16)) => {
                    fmphgo_run(&mut file, i, conf, 2, 16, &mut p);
                    fmphgo_run(&mut file, i, conf, 4, 16, &mut p);
                }
                (Some(8), None) | (None, Some(32)) => fmphgo_run(&mut file, i, conf, 8, 32, &mut p),
                _ => eprintln!("Cannot deduce for which pairs of (bits per group seed, group size) calculate.")
            }
        }
        #[cfg(feature = "fmph")] Method::FMPH(ref fmph_conf) => {
            match conf.key_source {
                KeySource::xs32 | KeySource::xs64 => fmph_benchmark(i, conf, fmph_conf.level_size, Some((IntHasher::default(), fmph_conf))),
                _ => fmph_benchmark(i, conf, fmph_conf.level_size, Some((StrHasher::default(), fmph_conf)))
            }
        },
        Method::phast(ref phast_conf) => {
            println!("PHast {} {}: encoder results...", phast_conf.bits_per_seed, phast_conf.bucket_size());
            let mut csv_file = file("phast", &conf, i.0.len(), i.1.len(), "bits_per_seed bucket_size100 encoder");
            if phast_conf.elias_fano() {
                phast_benchmark::<DefaultCompressedArray, _>(&mut csv_file, i, conf, phast_conf, "EF");
            }
            if phast_conf.compact {
                phast_benchmark::<CompactFast, _>(&mut csv_file, i, conf, phast_conf, "C");
            }
        },
        #[cfg(feature = "boomphf")]
        Method::Boomphf{level_size} => {
            match conf.key_source {
                KeySource::xs32 | KeySource::xs64 => fmph_benchmark::<IntHasher, _>(i, conf, level_size, None),
                _ => fmph_benchmark::<StrHasher, _>(i, conf, level_size, None)
            }
        }
        #[cfg(feature = "cmph-sys")] Method::CHD{lambda} => {
            /*if conf.key_source == KeySource::stdin || conf.key_source == KeySource::stdinz {
                eprintln!("Benchmarking CHD with keys from stdin is not supported.")
            } else {*/
                println!("CHD: lambda results...");
                let mut csv = file("CHD", &conf, i.0.len(), i.1.len(), "lambda");
                if let Some(lambda) = lambda {
                    chd_benchmark(&mut csv, i, conf, lambda);
                } else {
                    for lambda in 1..=6 { chd_benchmark(&mut csv, i, conf, lambda); }
                }
            //}
        }
        #[cfg(feature = "ptr_hash")] Method::PtrHash{ speed } => {
            println!("PtrHash: results...");
            let mut csv_file = file("PtrHash", &conf, i.0.len(), i.1.len(), "speed");
            match conf.key_source {
                KeySource::xs32 | KeySource::xs64 => ptrhash_benchmark::<ptr_hash::hash::FxHash, _>(&mut csv_file, i, conf, speed),
                _ => ptrhash_benchmark::<ptrhash::StrHasherForPtr, _>(&mut csv_file, i, conf, speed),
            }
        },
        Method::None => {}
    }
}

fn main() {
    let conf: Conf = Conf::parse();
    println!("multi-threaded calculations use {} threads (to set by the RAYON_NUM_THREADS environment variable)", current_num_threads());
    println!("build and lookup times are averaged over {} and {} runs, respectively", conf.build_runs, conf.lookup_runs);
    println!("hasher:  integer {}  string {}", std::any::type_name::<IntHasher>(), std::any::type_name::<StrHasher>());
    match conf.key_source {
        KeySource::xs32 => run(&conf, &gen_data(conf.keys_num.unwrap(), conf.foreign_keys_num, XorShift32(conf.seed as u32))),
        KeySource::xs64 => run(&conf, &gen_data(conf.keys_num.unwrap(), conf.foreign_keys_num, XorShift64(conf.seed))),
        KeySource::stdin|KeySource::stdinz => {
            //let lines = std::io::stdin().lock().lines().map(|l| l.unwrap());
            let lines = if conf.key_source == KeySource::stdin {
                RawLines::separated_by_newlines(std::io::stdin().lock())
            } else {
                RawLines::separated_by_zeros(std::io::stdin().lock())
            }.map(|l| l.unwrap());
            let i = if let Some(keys_num) = conf.keys_num {
                gen_data(keys_num, conf.foreign_keys_num, lines)
            } else {
                (lines.collect(), Vec::new())
            };
            print_input_stats("key set", &i.0);
            print_input_stats("foreign key set", &i.1);
            run(&conf, &i);
        },
        KeySource::randstr => run(&conf, &gen_data(conf.keys_num.unwrap(), conf.foreign_keys_num, RandomStrings::new(10..50, conf.seed)))
    };
}
