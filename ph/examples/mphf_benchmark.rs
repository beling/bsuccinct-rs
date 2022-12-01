use clap::{Parser, ValueEnum, Subcommand, Args};
use ph::fp::{FPHash, FPHashConf, FPHash2, FPHash2Conf, Bits, Bits8, GroupSize, SeedSize, TwoToPowerBits, TwoToPowerBitsStatic, FPHash2Builder};
use bitm::{BitAccess, BitVec};
use std::hash::Hash;
use std::fmt::{Debug, Display, Formatter};
use std::io::{stdout, Write, BufRead, Seek};
use cpu_time::{ProcessTime, ThreadTime};
use std::fs::{File, OpenOptions};
use std::mem::size_of;
use std::time::Instant;
use boomphf::Mphf;
use cmph_sys::{cmph_io_struct_vector_adapter,cmph_config_new,cmph_config_set_algo,
               CMPH_ALGO_CMPH_CHD,cmph_config_set_graphsize,cmph_config_set_b,cmph_uint32,
               cmph_new,cmph_config_destroy,cmph_io_struct_vector_adapter_destroy,
               cmph_packed_size,cmph_pack,cmph_destroy,cmph_search_packed};
use rayon::current_num_threads;
use dyn_size_of::GetSize;
use ph::BuildSeededHasher;
use ph::fp::keyset::{CachedKeySet, DynamicKeySet, SliceSourceWithClones, SliceSourceWithRefs};
use ph::seedable_hash::BuildWyHash;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum KeyAccess {
    /// Only sequential access to the keys is allowed, upto 10% of keys can be cached (for random access).
    Sequential,  //(usize),
    /// Random-access, read-only access to the keys is allowed. The algorithm store indices of the remaining keys.
    Indices,
    /// Vector of keys can be modified. The method stores remaining keys, and removes the rest from the vector.
    Copy
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Threads {
    /// Single thread
    Single = 1,
    /// Multiple threads
    Multi = 2,
    /// Single and multiple threads too
    Both = 2 | 1
}

#[allow(non_camel_case_types)]
#[derive(Args)]
struct FMPHConf {
    /// Relative level size as percent of number of keys, equals to *100γ*.
    #[arg(short='l', long)]
    level_size: Option<u16>,
    /// How FMPH can access keys.
    #[arg(value_enum, short='a', long, default_value_t = KeyAccess::Indices)]
    key_access: KeyAccess,
}


#[allow(non_camel_case_types)]
#[derive(Args)]
struct FMPHGOConf {
    /// Number of bits to store seed of each group, *s*
    #[arg(short='s', long, value_parser = clap::value_parser!(u8).range(1..16))]
    bits_per_group_seed: Option<u8>,
    /// The size of each group, *b*
    #[arg(short='b', long, value_parser = clap::value_parser!(u8).range(1..63))]
    group_size: Option<u8>,
    /// Relative level size as percent of number of keys, equals to *100γ*
    #[arg(short='l', long)]
    level_size: Option<u16>,
    /// FMPHGO caches 64-bit hashes of keys when their number (at the constructed level) is below this threshold
    #[arg(short='p', long, default_value_t = usize::MAX)]
    pre_hash_threshold: usize,
    /// How FMPHGO can access keys
    #[arg(value_enum, short='a', long, default_value_t = KeyAccess::Indices)]
    key_access: KeyAccess,
}

#[allow(non_camel_case_types)]
//#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[derive(Subcommand)]
enum Method {
    // Most methods
    //Most,
    /// FMPHGO with all settings
    FMPHGO_all,
    /// FMPHGO with selected settings
    FMPHGO(FMPHGOConf),
    /// FMPH
    FMPH(FMPHConf),
    /// BooMPHF
    Boomphf {
        /// Relative level size as percent of number of keys, equals to *100γ*
        #[arg(short='l', long)]
        level_size: Option<u16>
    },
    /// CHD
    CHD {
        /// The average number of keys per bucket. By default tests all lambdas from 1 to 6
        #[arg(short='l', long, value_parser = clap::value_parser!(u8).range(1..32))]
        lambda: Option<u8>
    }
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum KeySource {
    /// Generate 32 bit keys with xor-shift 32
    xs32,
    /// Generate 64 bit keys with xor-shift 64
    xs64,
    /// Standard input
    stdin
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
/// Minimal perfect hashing benchmark.
struct Conf {
    /// Method to run
    #[command(subcommand)]
    method: Method,

    /// Number of times to perform the lookup test
    #[arg(short='l', long, default_value_t = 1)]
    lookup_runs: u32,

    /// Number of times to perform the construction
    #[arg(short='b', long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
    build_runs: u32,

    /// Whether to check the validity of built MPHFs
    #[arg(short='v', long, default_value_t = false)]
    verify: bool,

    #[arg(short='s', long, value_enum, default_value_t = KeySource::stdin)]
    key_source: KeySource,

    /// The number of random keys to use or maximum number of keys to read from stdin
    #[arg(short='n', long)]
    keys_num: Option<usize>,

    /// Number of foreign keys (to generate or read) used to test the frequency of detection of non-contained keys
    #[arg(short='f', long, default_value_t = 0)]
    foreign_keys_num: usize,

    /// Whether to build MPHF using single or multiple threads, or try both (default). Ignored by the methods that do not support building with single or multiple threads
    #[arg(short='t', long, value_enum, default_value_t = Threads::Both)]
    threads: Threads,

    /// Save detailed results to CSV-like (but space separated) file
    #[arg(short='d', long, default_value_t = false)]
    save_details: bool,
}

/// Represents average (per value) lookup: level searched, times (seconds).
pub struct SearchStats {
    /// average number of level searched
    pub avg_deep: f64,
    /// average lookup time
    pub avg_lookup_time: f64,
    /// proportion of elements not found
    pub absences_found: f64
}

impl SearchStats {
    /// Lookups `h` for all keys in `input` and returns search statistics.
    /// If `verify` is `true`, checks if the MPHF `h` is valid for the given `input`.
    fn new<K: Hash, F: Fn(&K, &mut u64) -> Option<u64>>(input: &[K], h: F, verify: bool, lookup_runs: u32) -> Self {
        if input.is_empty() || lookup_runs == 0 { return Self::nan(); }
        let mut extra_levels_searched = 0u64;
        let mut not_found = 0usize;
        let start_process_moment = ProcessTime::now();
        if verify {
            let mut seen = Box::<[u64]>::with_zeroed_bits(input.len());
            for v in input {
                if let Some(index) = h(v, &mut extra_levels_searched) {
                    let index = index as usize;
                    assert!(index < input.len(), "MPHF assigns too large value {}>{}.", index, input.len());
                    assert!(!seen.get_bit(index), "MPHF assigns the same value to two keys of input.");
                    seen.set_bit(index);
                } else {
                    not_found += 1;
                }
            }
        } else {
            for v in input {
                if h(v, &mut extra_levels_searched).is_none() {
                    not_found += 1;
                }
            }
        }
        for _ in 1..lookup_runs {
            let mut dump = 0;
            for v in input { h(v, &mut dump); }
        }
        let seconds = start_process_moment.elapsed().as_secs_f64();
        let divider = input.len() as f64;
        Self {
            avg_deep: extra_levels_searched as f64 / divider,
            avg_lookup_time: seconds / (divider * lookup_runs as f64),
            absences_found: not_found as f64 / divider
        }
    }

    pub fn nan() -> Self {
        Self { avg_deep: f64::NAN, avg_lookup_time: f64::NAN, absences_found: f64::NAN }
    }
}

struct BuildStats {
    time_st: f64,
    time_mt: f64
}

impl Display for BuildStats {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match (!self.time_st.is_nan(), !self.time_mt.is_nan()) {
            (true, false) => write!(f, "build time [ms] ST: {:.0}", self.time_st * 1_000.0),
            (false, true) => write!(f, "build time [ms] MT: {:.0}", self.time_mt * 1_000.0),
            (true, true) => write!(f, "build time [ms] ST, MT: {:.0}, {:.0}", self.time_st * 1_000.0, self.time_mt * 1_000.0),
            _ => write!(f, "build time is unknown")
        }
    }
}

const BENCHMARK_HEADER: &'static str = "size_bytes bits_per_value avg_deep avg_lookup_time build_time_st build_time_mt absent_avg_deep absent_avg_lookup_time absences_found";

struct BenchmarkResult {
    included: SearchStats,
    absent: SearchStats,
    size_bytes: usize,
    bits_per_value: f64,
    build: BuildStats
}

impl BenchmarkResult {
    fn all<'a>(&'a self) -> impl Display + 'a {
        struct All<'a>(&'a BenchmarkResult);
        impl<'a> Display for All<'a> {
            fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
                write!(f, "{} {} {} {} {} {} {} {} {}", self.0.size_bytes,
                       self.0.bits_per_value, self.0.included.avg_deep, self.0.included.avg_lookup_time,
                       self.0.build.time_st, self.0.build.time_mt,
                       self.0.absent.avg_lookup_time, self.0.absent.avg_lookup_time, self.0.absent.absences_found
                )
            }
        }
        All(self)
    }
}

impl Display for BenchmarkResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "size [bits/key]: {:.2}", self.bits_per_value)?;
        if !self.included.avg_lookup_time.is_nan() {
            write!(f, "\tlookup time [ns]: {:.0}", self.included.avg_lookup_time * 1_000_000_000.0)?;
        }
        write!(f, "\t{}", self.build)?;
        Ok(())
    }
}

trait MPHFBuilder<K: Hash> {
    const CAN_DETECT_ABSENCE: bool = true;
    const BUILD_THREADS: Threads = Threads::Both;
    const BUILD_THREADS_DOES_NOT_CHANGE_SIZE: bool = true;

    type MPHF: GetSize;
    fn new(&self, keys: &[K], use_multiple_threads: bool) -> Self::MPHF;
    fn value(mphf: &Self::MPHF, key: &K, levels: &mut u64) -> Option<u64>;

    /// Builds the MPHF and measure the CPU thread time of building. Returns: the MPHF, the time measured.
    fn benchmark_build_st(&self, keys: &[K], repeats: u32) -> (Self::MPHF, BuildStats) {
        let start_moment = ThreadTime::now();   // ProcessTime?
        for _ in 1..repeats { self.new(keys, false); }
        let h = self.new(keys, false);
        let build_time_seconds = start_moment.elapsed().as_secs_f64();
        (h, BuildStats { time_st: build_time_seconds / repeats as f64, time_mt: f64::NAN } )
    }

    /// Builds the MPHF and measure the CPU thread time of building. Returns: the MPHF, the time measured.
    fn benchmark_build_mt(&self, keys: &[K], repeats: u32) -> (Self::MPHF, BuildStats) {
        let start_moment =  Instant::now();
        for _ in 1..repeats { self.new(keys, true); }
        let h = self.new(keys, true);
        let build_time_seconds = start_moment.elapsed().as_secs_f64();
        (h, BuildStats { time_st: f64::NAN, time_mt: build_time_seconds / repeats as f64 } )
    }

    /// Builds MPHF and measure the time of building. Returns: MPHF, and either single-thread and multiple-thread time of building, and one NaN.
    fn benchmark_build(&self, keys: &[K], conf: &Conf) -> (Self::MPHF, BuildStats) {
        match (Self::BUILD_THREADS, conf.threads) {
            (Threads::Single, _) | (Threads::Both, Threads::Single) => self.benchmark_build_st(keys, conf.build_runs),
            (Threads::Multi, _) | (Threads::Both, Threads::Multi) => self.benchmark_build_mt(keys, conf.build_runs),
            _ => {  // (Both, Both) pair
                let (mphf, result_st) = self.benchmark_build_st(keys, conf.build_runs);
                let size_st = Self::BUILD_THREADS_DOES_NOT_CHANGE_SIZE.then(|| mphf.size_bytes());
                drop(mphf);
                let (mphf, mut result_mt) = self.benchmark_build_mt(keys, conf.build_runs);
                if let Some(size_st) = size_st {
                    let size_mt = mphf.size_bytes();
                    if size_st != size_mt {
                        eprintln!("WARNING: ST/MT differ in sizes, {} != {}", size_st, size_mt);
                    }
                }
                result_mt.time_st = result_st.time_st;
                (mphf, result_mt)
            }
        }
    }

    /// Builds, tests, and returns MPHF.
    fn benchmark(&self, i: &(Vec<K>, Vec<K>), conf: &Conf) -> BenchmarkResult {
        let (h, build) = self.benchmark_build(&i.0, conf);
        let size_bytes = h.size_bytes();
        let bits_per_value = 8.0 * size_bytes as f64 / i.0.len() as f64;
        if conf.lookup_runs == 0 {
            return BenchmarkResult { included: SearchStats::nan(), absent: SearchStats::nan(), size_bytes, bits_per_value, build }
        }
        let included = SearchStats::new(&i.0, |k, s| Self::value(&h, k, s), conf.verify, conf.lookup_runs);
        assert_eq!(included.absences_found, 0.0, "MPHF does not assign the value for {}% keys of the input", included.absences_found*100.0);
        let absent = if conf.save_details && Self::CAN_DETECT_ABSENCE {
            SearchStats::new(&i.1, |k, s| Self::value(&h, k, s), false, conf.lookup_runs)
        } else {
            SearchStats::nan()
        };
        BenchmarkResult { included, absent, size_bytes, bits_per_value, build }
    }
}

impl<K: Hash + Sync + Send + Clone, S: BuildSeededHasher + Clone + Sync> MPHFBuilder<K> for (FPHashConf<S>, KeyAccess) {
    type MPHF = FPHash<S>;

    fn new(&self, keys: &[K], use_multiple_threads: bool) -> Self::MPHF {
        let mut conf = self.0.clone();
        conf.use_multiple_threads = use_multiple_threads;
        match self.1 {
            //KeyAccess::LoMem(0) => Self::MPHF::with_conf(DynamicKeySet::with_len(|| keys.iter(), keys.len(), true), self.0.clone()),
            KeyAccess::Sequential => Self::MPHF::with_conf(
                CachedKeySet::new(DynamicKeySet::with_len(|| keys.iter(), keys.len(), true), keys.len() / 10),
                conf),
            KeyAccess::Indices => Self::MPHF::with_conf(SliceSourceWithRefs::new(keys), conf),
            KeyAccess::Copy => Self::MPHF::with_conf(SliceSourceWithClones::new(keys), conf)
        }
    }

    #[inline(always)] fn value(mphf: &Self::MPHF, key: &K, levels: &mut u64) -> Option<u64> {
        mphf.get_stats(key, levels)
    }
}

impl<K: Hash + Sync + Send + Clone, GS: GroupSize + Sync, SS: SeedSize, S: BuildSeededHasher + Clone + Sync> MPHFBuilder<K> for (FPHash2Builder<GS, SS, S>, KeyAccess) {
    type MPHF = FPHash2<GS, SS, S>;

    fn new(&self, keys: &[K], use_multiple_threads: bool) -> Self::MPHF {
        let mut conf = self.0.clone();
        conf.use_multiple_threads = use_multiple_threads;
        match self.1 {
            KeyAccess::Sequential => Self::MPHF::with_builder(
                CachedKeySet::new(DynamicKeySet::with_len(|| keys.iter(), keys.len(), true), keys.len() / 10),
                conf),
            KeyAccess::Indices => Self::MPHF::with_builder(SliceSourceWithRefs::new(keys), conf),
            KeyAccess::Copy => Self::MPHF::with_builder(SliceSourceWithClones::new(keys), conf)

            /*KeyAccess::LoMem(0) => Self::MPHF::with_builder(DynamicKeySet::with_len(|| keys.iter(), keys.len(), true), self.0.clone()),
            KeyAccess::LoMem(clone_threshold) => Self::MPHF::with_builder(
                CachedKeySet::new(DynamicKeySet::with_len(|| keys.iter(), keys.len(), true), clone_threshold),
                self.0.clone()),
            //KeyAccess::StoreIndices => Self::MPHF::from_slice_with_conf(keys, self.0.clone()),
            KeyAccess::StoreIndices => Self::MPHF::with_builder(SliceSourceWithRefs::new(keys), self.0.clone()),
            //KeyAccess::StoreIndices => Self::MPHF::with_conf(CachedKeySet::slice(keys, keys.len()/10), self.0.clone()),
            KeyAccess::CopyKeys => Self::MPHF::with_builder(SliceSourceWithClones::new(keys), self.0.clone())*/
        }
    }

    #[inline(always)] fn value(mphf: &Self::MPHF, key: &K, levels: &mut u64) -> Option<u64> {
        mphf.get_stats(key, levels)
    }
}

struct CHDConf { lambda: u8 }

impl<K: Hash> MPHFBuilder<K> for CHDConf {
    type MPHF = Box<[u8]>;

    const CAN_DETECT_ABSENCE: bool = false;
    const BUILD_THREADS: Threads = Threads::Single;

    fn new(&self, keys: &[K], _use_multiple_threads: bool) -> Self::MPHF {
        unsafe {
            let source = cmph_io_struct_vector_adapter(
                keys.as_ptr() as *mut ::std::os::raw::c_void,         // structs
                size_of::<K>() as u32, // struct_size
                0,           // key_offset
                size_of::<K>() as u32, // key_len
                keys.len() as u32); // nkeys

            let config = cmph_config_new(source);
            //cmph_config_set_algo(config, CMPH_CHD_PH); // CMPH_CHD or CMPH_BDZ
            cmph_config_set_algo(config, CMPH_ALGO_CMPH_CHD); // CMPH_CHD or CMPH_BDZ
            cmph_config_set_graphsize(config, 1.01);
            cmph_config_set_b(config, self.lambda as cmph_uint32);
            let hash = cmph_new(config);
            cmph_config_destroy(config);
            cmph_io_struct_vector_adapter_destroy(source);//was: cmph_io_vector_adapter_destroy(source);
            //to_find_perfect_hash.release();

            //let mut packed_hash = vec![MaybeUninit::<u8>::uninit(); cmph_packed_size(hash) as usize].into_boxed_slice();
            let mut packed_hash = vec![0u8; cmph_packed_size(hash) as usize].into_boxed_slice();
            cmph_pack(hash, packed_hash.as_mut_ptr() as *mut ::std::os::raw::c_void);
            cmph_destroy(hash);

            packed_hash
        }
    }

    #[inline(always)] fn value(mphf: &Self::MPHF, key: &K, _levels: &mut u64) -> Option<u64> {
        Some(unsafe{ cmph_search_packed(
            mphf.as_ptr() as *mut ::std::os::raw::c_void,
            key as *const K as *const i8,
            size_of::<K>() as u32) as u64 })
    }
}

struct BooMPHFConf { gamma: f64 }

impl<K: Hash + Debug + Sync + Send> MPHFBuilder<K> for BooMPHFConf {
    type MPHF = Mphf<K>;

    fn new(&self, keys: &[K], use_multiple_threads: bool) -> Self::MPHF {
        if use_multiple_threads {
            Mphf::new_parallel(self.gamma, keys, None)
        } else {
            Mphf::new(self.gamma, keys)
        }
    }

    #[inline(always)] fn value(mphf: &Self::MPHF, key: &K, levels: &mut u64) -> Option<u64> {
        mphf.try_hash_bench(&key, levels)
    }
}

const FMPHGO_HEADER: &'static str = "prehash_threshold bits_per_group_seed relative_level_size bits_per_group";

struct FMPHGOBuildParams<S> {
    hash: S,
    relative_level_size: u16,
    prehash_threshold: usize,
    key_access: KeyAccess
}

fn h2bench<GS, SS, S, K>(bits_per_group_seed: SS, bits_per_group: GS, i: &(Vec<K>, Vec<K>), conf: &Conf, p: &FMPHGOBuildParams<S>) -> BenchmarkResult
    where GS: GroupSize + Sync + Copy, SS: SeedSize + Copy, S: BuildSeededHasher + Sync + Clone, K: Hash + Sync + Send + Clone
{
    (FPHash2Builder::with_lsize_pht_mt(
        FPHash2Conf::hash_bps_bpg(p.hash.clone(), bits_per_group_seed, bits_per_group),
        p.relative_level_size, p.prehash_threshold, false), p.key_access)
    .benchmark(i, conf)
}

fn h2b<GS, S, K>(bits_per_group_seed: u8, bits_per_group: GS, i: &(Vec<K>, Vec<K>), conf: &Conf, p: &FMPHGOBuildParams<S>) -> BenchmarkResult
    where GS: GroupSize + Sync + Copy, S: BuildSeededHasher + Sync + Clone, K: Hash + Sync + Send + Clone
{
    if bits_per_group_seed.is_power_of_two() {
        match bits_per_group_seed {
            1 => h2bench(TwoToPowerBitsStatic::<0>, bits_per_group, i, conf, p),
            2 => h2bench(TwoToPowerBitsStatic::<1>, bits_per_group, i, conf, p),
            4 => h2bench(TwoToPowerBitsStatic::<2>, bits_per_group, i, conf, p),
            8 => h2bench(Bits8, bits_per_group, i, conf, p),
            16 => h2bench(TwoToPowerBitsStatic::<5>, bits_per_group, i, conf, p),
            _ => unreachable!()
        }
    } else {
        h2bench(Bits(bits_per_group_seed), bits_per_group, i, conf, p)
    }
}

fn fmphgo<S, K>(file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf, bits_per_group_seed: u8, bits_per_group: u8, p: &FMPHGOBuildParams<S>)
                -> BenchmarkResult
    where S: BuildSeededHasher + Clone + Sync, K: Hash + Sync + Send + Clone
{
    let b = if bits_per_group.is_power_of_two() {
        h2b(bits_per_group_seed, TwoToPowerBits::new(bits_per_group.trailing_zeros() as u8), i, conf, p)
    } else {
        h2b(bits_per_group_seed, Bits(bits_per_group), i, conf, p)
    };
    if let Some(ref mut f) = file {
        writeln!(f, "{} {} {} {} {}", p.prehash_threshold, bits_per_group_seed, p.relative_level_size, bits_per_group, b.all()).unwrap();
    }
    b
}

fn fmphgo_benchmark<S, K>(file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf, bits_per_group_seed: u8, bits_per_group: u8, p: &FMPHGOBuildParams<S>)
    where S: BuildSeededHasher + Clone + Sync, K: Hash + Sync + Send + Clone
{
    let b = fmphgo(file, i, conf, bits_per_group_seed, bits_per_group, p);
    println!(" {} {} {:.1}\t{}", bits_per_group_seed, bits_per_group, p.relative_level_size as f64/100.0, b);
}

fn fmphgo_run<S, K>(file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf, bits_per_group_seed: u8, bits_per_group: u8, p: &mut FMPHGOBuildParams<S>)
    where S: BuildSeededHasher + Clone + Sync, K: Hash + Sync + Send + Clone
{
    if p.relative_level_size == 0 {
        for relative_level_size in (100..=200).step_by(/*50*/100) {
            p.relative_level_size = relative_level_size;
            fmphgo_benchmark(file, i, conf, bits_per_group_seed, bits_per_group, &p);
        }
        p.relative_level_size = 0;
    } else {
        fmphgo_benchmark(file, i, conf, bits_per_group_seed, bits_per_group, &p);
    }
}

fn fmphgo_benchmark_all<S, K>(mut csv_file: Option<File>, hash: S, i: &(Vec<K>, Vec<K>), conf: &Conf, key_access: KeyAccess)
where S: BuildSeededHasher + Clone + Sync, K: Hash + Sync + Send + Clone
{
    println!("bps rls \\ bpglog 2 3 4 5 ... 62");
    let mut p = FMPHGOBuildParams {
        hash,
        relative_level_size: 0,
        prehash_threshold: usize::MAX,
        key_access
    };
    for bits_per_group_seed in 1u8..=10u8 {
        for relative_level_size in (100..=200).step_by(/*50*/100) {
            p.relative_level_size = relative_level_size;
            print!("{} {}", bits_per_group_seed, relative_level_size);
            //for bits_per_group_log2 in 3u8..=7u8 {
            for bits_per_group in (2u8..=62u8).step_by(2) {
                //let (_, b) = Conf::bps_bpg_lsize(bits_per_group_seed, TwoToPowerBits::new(bits_per_group_log2), relative_level_size).benchmark(verify);
                let b = fmphgo(&mut csv_file, i, conf, bits_per_group_seed, bits_per_group, &p);
                print!(" {:.2}", b.bits_per_value);
                stdout().flush().unwrap();
            }
            println!();
        }
    }
}



const FMPH_BENCHMARK_HEADER: &'static str = "gamma";

fn fmph_benchmark<S, K>(i: &(Vec<K>, Vec<K>), conf: &Conf, level_size: Option<u16>, use_fmph: Option<(S, KeyAccess)>)
where S: BuildSeededHasher + Clone + Sync, K: Hash + Sync + Send + Debug + Clone
{
    let method_name = if use_fmph.is_some() { "FMPH" } else { "BooMPHF" };
    println!("{}: gamma results...", method_name);
    let mut file = file(method_name, &conf, i, FMPH_BENCHMARK_HEADER);
    for relative_level_size in level_size.map_or(100..=200, |r| r..=r).step_by(/*50*/100) {
        let gamma = relative_level_size as f64 / 100.0f64;
        let b = if let Some((ref hash, key_access)) = use_fmph {
            (FPHashConf::hash_lsize_threads(hash.clone(), relative_level_size, false), key_access).benchmark(i, &conf)
        } else {
            BooMPHFConf { gamma }.benchmark(i, &conf)
        };
        println!(" {:.1}\t{}", gamma, b);
        if let Some(ref mut f) = file { writeln!(f, "{} {}", gamma, b.all()).unwrap(); }
    }
}

fn chd_benchmark<K: Hash>(csv_file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf, lambda: u8) {
    let b = CHDConf{ lambda }.benchmark(i, conf);
    if let Some(ref mut f) = csv_file { writeln!(f, "{} {}", lambda, b.all()).unwrap(); }
    println!(" {}\t{}", lambda, b);
}

fn file<K>(method_name: &str, conf: &Conf, i: &(Vec<K>, Vec<K>), extra_header: &str) -> Option<File> {
    if !conf.save_details { return None; }
    let ks_name = match conf.key_source {
        KeySource::xs32 => "32",
        KeySource::xs64 => "64",
        KeySource::stdin => "str",
    };
    let mut file = OpenOptions::new().append(true).create(true).open(format!("{}_{}_{}_{}.csv", method_name, ks_name, i.0.len(), i.1.len())).unwrap();
    if file.stream_position().unwrap() == 0 { writeln!(file, "{} {}", extra_header, BENCHMARK_HEADER).unwrap(); }
    Some(file)
}

fn run<K: Hash + Sync + Send + Clone + Debug>(conf: &Conf, i: &(Vec<K>, Vec<K>)) {
    match conf.method {
        Method::FMPHGO_all => {
            fmphgo_benchmark_all(file("FMPHGO_all", &conf, i, FMPHGO_HEADER), BuildWyHash::default(), &i, &conf, KeyAccess::Indices);
        }
        Method::FMPHGO(ref fmphgo_conf) => {
            let mut file = file("FMPHGO", &conf, i, FMPHGO_HEADER);
            println!("FMPHGO pre-hash threshold={}: s b gamma results...", fmphgo_conf.pre_hash_threshold);
            let mut p = FMPHGOBuildParams {
                hash: BuildWyHash::default(),
                relative_level_size: fmphgo_conf.level_size.unwrap_or(0),
                prehash_threshold: fmphgo_conf.pre_hash_threshold,
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
        Method::FMPH(ref fmph_conf) => {
            fmph_benchmark(i, conf, fmph_conf.level_size, Some((BuildWyHash::default(), fmph_conf.key_access)));
        }
        Method::Boomphf{level_size} => {
            fmph_benchmark::<BuildWyHash, _>(i, conf, level_size, None);
        }
        Method::CHD{lambda} => {
            if conf.key_source == KeySource::stdin {
                eprintln!("Benchmarking CHD with keys from stdin is not supported.")
            } else {
                println!("CHD: lambda results...");
                let mut csv = file("CHD", &conf, i, "lambda");
                if let Some(lambda) = lambda {
                    chd_benchmark(&mut csv, i, conf, lambda);
                } else {
                    for lambda in 1..=6 { chd_benchmark(&mut csv, i, conf, lambda); }
                }
            }
        }
    }
}

/// Infinitive iterator over random u32 values generated by xorshift 32 algorithm.
///
/// It must be initialized by non-zero seed, never generates zero, and has period 2^32-1.
///
/// See <https://www.jstatsoft.org/article/view/v008i14>.
struct XorShift32(u32);

impl Iterator for XorShift32 {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        Some(self.0)
    }
}

/*struct Generate32x<const N: usize>(XorShift32);
impl<const N: usize> Generate32x<N> {
    pub fn new(seed: u32) -> Self { Self(XorShift32(seed)) }
}

impl<const N: usize> Iterator for Generate32x<N> {
    type Item = [u32; N];

    fn next(&mut self) -> Option<Self::Item> {
        /*let mut result: [MaybeUninit<u32>; N] = unsafe {
            MaybeUninit::uninit().assume_init()
        };
        for v in &mut result { v.write(self.0.next().unwrap()); }
        Some(unsafe{std::mem::transmute::<_, [u32; N]>(result)})*/
        let mut result = [0u32; N];
        for v in &mut result { *v = self.0.next().unwrap(); }
        Some(result)
    }
}*/

struct XorShift64(u64);

impl Iterator for XorShift64 {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        Some(self.0)
    }
}

fn gen_data<I: Iterator>(keys_num: usize, foreign_keys_num: usize, mut generator: I) -> (Vec<I::Item>, Vec<I::Item>) {
    (generator.by_ref().take(keys_num).collect(), generator.take(foreign_keys_num).collect())
}

//fn test_data_32x<const N: usize>(how_many: usize) -> (Vec<[u32; N]>, Vec<[u32; N]>) { test_data(how_many, Generate32x::<N>::new(5678)) }

fn print_input_stats(setname: &str, strings: &[String]){
    if strings.len() == 0 {
        println!("{} is empty", setname);
    } else {
        println!("{} has {} strings with an average length of {:.1} bytes", setname, strings.len(), strings.iter().map(|s| s.len()).sum::<usize>() as f64 / strings.len() as f64)
    }
}

fn main() {
    let conf = Conf::parse();
    println!("{} threads available for multi-threaded calculations", current_num_threads());
    match conf.key_source {
        KeySource::xs32 => { run(&conf, &gen_data(conf.keys_num.unwrap(), conf.foreign_keys_num, XorShift32(1234))); },
        KeySource::xs64 => { run(&conf, &gen_data(conf.keys_num.unwrap(), conf.foreign_keys_num, XorShift64(1234))); },
        KeySource::stdin => {
            let lines = std::io::stdin().lock().lines().map(|l| l.unwrap());
            let i = if let Some(keys_num) = conf.keys_num {
                gen_data(keys_num, conf.foreign_keys_num, lines)
            } else {
                (lines.collect(), Vec::new())
            };
            print_input_stats("key set", &i.0);
            print_input_stats("foreign key set", &i.1);
            run(&conf, &i);
        }
    };
}
