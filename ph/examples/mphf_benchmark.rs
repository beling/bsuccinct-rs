use clap::{Parser, ValueEnum};
use ph::fp::{FPHash, FPHashConf, FPHash2, FPHash2Conf, Bits, Bits8, GroupSize, SeedSize, TwoToPowerBits, TwoToPowerBitsStatic, FPHash2Builder};
use bitm::{BitAccess, BitVec};
use std::hash::Hash;
use std::fmt::{Debug, Display, Formatter};
use std::io::{stdout, Write, BufRead};
use cpu_time::ProcessTime;
use std::fs::File;
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

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Method {
    /// Most methods
    Most,
    /// FMPHGO with all settings
    FMPHGO_all,
    /// FMPH
    FMPH,
    /// FMPHGO with selected settings
    FMPHGO,
    /// BooMPHF
    Boomphf,
    /// CHD
    CHD,
    /// FMPH restricted to sequence access to keys, with coping only 10% of them (for random access)
    FMPH_lomem,
    /// FMPHGO restricted to sequence access to keys, with coping only 10% of them (for random access)
    FMPHGO_lomem,
    /// FMPH with keys coping
    FMPH_copy,
    /// FMPHGO with keys coping
    FMPHGO_copy
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum KeySource {
    /// Generate 32 bit keys with xor-shift 32
    xs32,
    /// Generate 64 bit keys with xor-shift 64
    xs64,
    /// Standard input.
    stdin
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
/// Minimal perfect hashing benchmark.
struct Conf {
    /// Method to run
    #[arg(value_enum, default_value_t = Method::Most)]
    method: Method,

    /// Number of times to perform the lookup test
    #[arg(short='l', long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
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

    /// Save detailed results to CSV file
    #[arg(short='d', long, default_value_t = false)]
    save_csv: bool,
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
        if input.is_empty() { return Self::nan(); }
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

const BENCHMARK_HEADER: &'static str = "size_bytes bits_per_value avg_deep avg_lookup_time build_time build_process_time absent_avg_deep absent_avg_lookup_time absences_found";

struct BenchmarkResult {
    included: SearchStats,
    absent: SearchStats,
    size_bytes: usize,
    bits_per_value: f64,
    build_process_time_seconds: f64,
    build_time_seconds: f64
}

impl Display for BenchmarkResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} {} {} {} {} {} {} {}", self.size_bytes,
               self.bits_per_value, self.included.avg_deep, self.included.avg_lookup_time,
               self.build_time_seconds, self.build_process_time_seconds,
               self.absent.avg_lookup_time, self.absent.avg_lookup_time, self.absent.absences_found
        )
    }
}

trait MPHFBuilder<K: Hash> {
    const CAN_DETECT_ABSENCE: bool = true;

    type MPHF: GetSize;
    fn new(&self, keys: &[K]) -> Self::MPHF;
    fn value(mphf: &Self::MPHF, key: &K, levels: &mut u64) -> Option<u64>;

    /// Builds MPHF and measure the time of building. Returns: MPHF, wall-clock time of building, CPU time of building (consumed by the whole process)
    fn benchmark_build(&self, keys: &[K], repeats: u32) -> (Self::MPHF, f64, f64) {
        let start_process_moment = ProcessTime::now();
        let start_moment = Instant::now();
        for _ in 1..repeats { self.new(keys); }
        let h = self.new(keys);
        let build_time_seconds = start_moment.elapsed().as_secs_f64();
        let build_process_time_seconds = start_process_moment.elapsed().as_secs_f64();
        (h, build_time_seconds / repeats as f64, build_process_time_seconds / repeats as f64)
    }

    /// Builds, tests, and returns MPHF.
    fn benchmark(&self, i: &(Vec<K>, Vec<K>), conf: &Conf) -> (Self::MPHF, BenchmarkResult) {
        let (h, build_time_seconds, build_process_time_seconds) = self.benchmark_build(&i.0, conf.build_runs);
        let included = SearchStats::new(&i.0, |k, s| Self::value(&h, k, s), conf.verify, conf.lookup_runs);
        assert_eq!(included.absences_found, 0.0, "MPHF does not assign the value for {}% keys of the input", included.absences_found*100.0);
        let size_bytes = h.size_bytes();
        let bits_per_value = 8.0 * size_bytes as f64 / i.0.len() as f64;
        let absent = if conf.save_csv && Self::CAN_DETECT_ABSENCE {
            SearchStats::new(&i.1, |k, s| Self::value(&h, k, s), false, conf.lookup_runs)
        } else {
            SearchStats::nan()
        };
        (h, BenchmarkResult { included, absent, size_bytes, bits_per_value, build_process_time_seconds, build_time_seconds })
    }
}

#[derive(Copy, Clone)]
enum KeyAccess { LoMem(usize), StoreIndices, CopyKeys }

impl<K: Hash + Sync + Send + Clone, S: BuildSeededHasher + Clone + Sync> MPHFBuilder<K> for (FPHashConf<S>, KeyAccess) {
    type MPHF = FPHash<S>;

    fn new(&self, keys: &[K]) -> Self::MPHF {
        match self.1 {
            KeyAccess::LoMem(0) => Self::MPHF::with_conf(DynamicKeySet::with_len(|| keys.iter(), keys.len(), true), self.0.clone()),
            KeyAccess::LoMem(clone_threshold) => Self::MPHF::with_conf(
                CachedKeySet::new(DynamicKeySet::with_len(|| keys.iter(), keys.len(), true), clone_threshold),
                self.0.clone()),
            //KeyAccess::StoreIndices => Self::MPHF::from_slice_with_conf(keys, self.0.clone()),
            KeyAccess::StoreIndices => Self::MPHF::with_conf(SliceSourceWithRefs::<_, u8>::new(keys), self.0.clone()),
            //KeyAccess::StoreIndices => Self::MPHF::with_conf(CachedKeySet::slice(keys, keys.len()/10), self.0.clone()),
            KeyAccess::CopyKeys => Self::MPHF::with_conf(SliceSourceWithClones::new(keys), self.0.clone())
        }
    }

    #[inline(always)] fn value(mphf: &Self::MPHF, key: &K, levels: &mut u64) -> Option<u64> {
        mphf.get_stats(key, levels)
    }
}

impl<K: Hash + Sync + Send + Clone, GS: GroupSize + Sync, SS: SeedSize, S: BuildSeededHasher + Clone + Sync> MPHFBuilder<K> for (FPHash2Builder<GS, SS, S>, KeyAccess) {
    type MPHF = FPHash2<GS, SS, S>;

    fn new(&self, keys: &[K]) -> Self::MPHF {
        match self.1 {
            KeyAccess::LoMem(0) => Self::MPHF::with_builder(DynamicKeySet::with_len(|| keys.iter(), keys.len(), true), self.0.clone()),
            KeyAccess::LoMem(clone_threshold) => Self::MPHF::with_builder(
                CachedKeySet::new(DynamicKeySet::with_len(|| keys.iter(), keys.len(), true), clone_threshold),
                self.0.clone()),
            //KeyAccess::StoreIndices => Self::MPHF::from_slice_with_conf(keys, self.0.clone()),
            KeyAccess::StoreIndices => Self::MPHF::with_builder(SliceSourceWithRefs::<_, u8>::new(keys), self.0.clone()),
            //KeyAccess::StoreIndices => Self::MPHF::with_conf(CachedKeySet::slice(keys, keys.len()/10), self.0.clone()),
            KeyAccess::CopyKeys => Self::MPHF::with_builder(SliceSourceWithClones::new(keys), self.0.clone())
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

    fn new(&self, keys: &[K]) -> Self::MPHF {
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

struct BooMPHFConf { gamma: f64, mt: bool }

impl<K: Hash + Debug + Sync + Send> MPHFBuilder<K> for BooMPHFConf {
    type MPHF = Mphf<K>;

    fn new(&self, keys: &[K]) -> Self::MPHF {
        if self.mt {
            Mphf::new_parallel(self.gamma, keys, None)
        } else {
            Mphf::new(self.gamma, keys)
        }
    }

    #[inline(always)] fn value(mphf: &Self::MPHF, key: &K, levels: &mut u64) -> Option<u64> {
        mphf.try_hash_bench(&key, levels)
    }
}

fn h2bench<GS, SS, S, K>(hash: S, bits_per_group_seed: SS, bits_per_group: GS, relative_level_size: u16, i: &(Vec<K>, Vec<K>), conf: &Conf, key_access: KeyAccess) -> (f64, BenchmarkResult)
where GS: GroupSize + Sync + Copy, SS: SeedSize + Copy, S: BuildSeededHasher + Sync + Clone, K: Hash + Sync + Send + Clone {
    let (mphf, _, st_cpu_time) = (FPHash2Builder::with_lsize_mt(FPHash2Conf::hash_bps_bpg(hash.clone(), bits_per_group_seed, bits_per_group), relative_level_size, false), key_access).benchmark_build(&i.0, conf.build_runs);
    let st_size = mphf.size_bytes();
    drop(mphf);
    let (mt_mphf, mt_bench_results) = (FPHash2Builder::with_lsize_mt(FPHash2Conf::hash_bps_bpg(hash.clone(), bits_per_group_seed, bits_per_group), relative_level_size, true), key_access).benchmark(i, conf);
    if mt_mphf.size_bytes() != st_size { eprintln!("WARNING: FMPHGO ST/MT have different sizes, {} != {}", st_size, mt_mphf.size_bytes()); }
    (st_cpu_time, mt_bench_results)
}

fn h2b<GS, S, K>(hash: S, bits_per_group_seed: u8, bits_per_group: GS, relative_level_size: u16, i: &(Vec<K>, Vec<K>), conf: &Conf, key_access: KeyAccess) -> (f64, BenchmarkResult)
    where GS: GroupSize + Sync + Copy, S: BuildSeededHasher + Sync + Clone, K: Hash + Sync + Send + Clone
{
    if bits_per_group_seed.is_power_of_two() {
        match bits_per_group_seed {
            1 => h2bench(hash, TwoToPowerBitsStatic::<0>, bits_per_group, relative_level_size, i, conf, key_access),
            2 => h2bench(hash, TwoToPowerBitsStatic::<1>, bits_per_group, relative_level_size, i, conf, key_access),
            4 => h2bench(hash, TwoToPowerBitsStatic::<2>, bits_per_group, relative_level_size, i, conf, key_access),
            8 => h2bench(hash, Bits8, bits_per_group, relative_level_size, i, conf, key_access),
            //16 => h2bench(hash, TwoToPowerBitsStatic::<5>, bits_per_group, relative_level_size, i, conf, key_access),
            _ => unreachable!()
        }
    } else {
        h2bench(hash, Bits(bits_per_group_seed), bits_per_group, relative_level_size, i, conf, key_access)
    }
}

fn fmphgo<S, K>(csv_file: &mut Option<File>, hash: &S, i: &(Vec<K>, Vec<K>), conf: &Conf, bits_per_group_seed: u8, relative_level_size: u16, bits_per_group: u8, key_access: KeyAccess)
                -> (f64, BenchmarkResult)
    where S: BuildSeededHasher + Clone + Sync, K: Hash + Sync + Send + Clone
{
    let (st_build_time, b) = if bits_per_group.is_power_of_two() {
        h2b(hash.clone(), bits_per_group_seed, TwoToPowerBits::new(bits_per_group.trailing_zeros() as u8), relative_level_size, i, conf, key_access)
    } else {
        h2b(hash.clone(), bits_per_group_seed, Bits(bits_per_group), relative_level_size, i, conf, key_access)
    };
    if let Some(ref mut f) = csv_file {
        writeln!(f, "{} {} {} {} {}", bits_per_group_seed, relative_level_size, bits_per_group, st_build_time, b).unwrap();
    }
    (st_build_time, b)
}

const HASH2_BENCHMARK_HEADER: &'static str = "bits_per_group_seed relative_level_size bits_per_group ST_build_time";

fn fmphgo_benchmark_all<S, K>(mut csv_file: Option<File>, hash: S, i: &(Vec<K>, Vec<K>), conf: &Conf, key_access: KeyAccess)
where S: BuildSeededHasher + Clone + Sync, K: Hash + Sync + Send + Clone
{
    if let Some(ref mut f) = csv_file {
        writeln!(f, "{} {}", HASH2_BENCHMARK_HEADER, BENCHMARK_HEADER).unwrap()
    }
    println!("bps rls \\ bpglog 2 3 4 5 ... 62");
    for bits_per_group_seed in 1u8..=10u8 {
        for relative_level_size in (100..=200).step_by(/*50*/100) {
            print!("{} {}", bits_per_group_seed, relative_level_size);
            //for bits_per_group_log2 in 3u8..=7u8 {
            for bits_per_group in (2u8..=62u8).step_by(2) {
                //let (_, b) = Conf::bps_bpg_lsize(bits_per_group_seed, TwoToPowerBits::new(bits_per_group_log2), relative_level_size).benchmark(verify);
                let (_st_build_time, b) = fmphgo(&mut csv_file, &hash, i, conf, bits_per_group_seed, relative_level_size, bits_per_group, key_access);
                print!(" {:.2}", b.bits_per_value);
                stdout().flush().unwrap();
            }
            println!();
        }
    }
}

fn fmphgo_benchmark<S, K>(mut csv_file: Option<File>, hash: S, i: &(Vec<K>, Vec<K>), conf: &Conf, key_access: KeyAccess)
    where S: BuildSeededHasher + Clone + Sync, K: Hash + Sync + Send + Clone
{
    if let Some(ref mut f) = csv_file {
        writeln!(f, "{} {}", HASH2_BENCHMARK_HEADER, BENCHMARK_HEADER).unwrap()
    }
    println!("FMPHGO: s b gamma results...");
    for (bits_per_group_seed, bits_per_group) in [(1, 8), (2, 16), (4, 16), (8, 32)] {
        for relative_level_size in (100..=200).step_by(/*50*/100) {
            let (st_build_time, b) = fmphgo(&mut csv_file, &hash, i, conf, bits_per_group_seed, relative_level_size, bits_per_group, key_access);
            println!(" {} {} {:.1}\tsize [bits/key]: {:.2}\tlookup time [ns]: {:.0}\tbuild time [ms] ST, MT: {:.0}, {:.0}",
                     bits_per_group_seed, bits_per_group, relative_level_size as f64/100.0,
                     b.bits_per_value, b.included.avg_lookup_time * 1_000_000_000.0, st_build_time*1000.0, b.build_time_seconds*1000.0);
        }
    }
}

fn fmph_benchmark<S, K>(mut csv_file: Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf, use_fp: Option<(S, KeyAccess)>)
where S: BuildSeededHasher + Clone + Sync, K: Hash + Sync + Send + Debug + Clone
{
    if let Some(ref mut f) = csv_file {
        writeln!(f, "gamma ST_build_time {}", BENCHMARK_HEADER).unwrap()
    }
    if use_fp.is_some() { print!("FMPH"); } else { print!("boomphf") }
    println!(": gamma results...");
    for relative_level_size in (100..=200).step_by(/*50*/100) {
        let gamma = relative_level_size as f64 / 100.0f64;
        let (st_build_time, b) = if let Some((ref hash, key_access)) = use_fp {
            //let mut r = bbmap::bb::hash::Conf::hash_lsize(/*fnv::FnvBuildHasher::default()*/ hash.clone(), relative_level_size).benchmark(verify).1;
            //(std::mem::replace(&mut r.build_time_seconds, f64::NAN), r)
            ((FPHashConf::hash_lsize_threads(hash.clone(), relative_level_size, false), key_access).benchmark_build(&i.0, conf.build_runs).2,
             (FPHashConf::hash_lsize_threads(hash.clone(), relative_level_size, true), key_access).benchmark(i, conf).1)
        } else {
            (BooMPHFConf { gamma, mt: false }.benchmark_build(&i.0, conf.build_runs).2,
             BooMPHFConf { gamma, mt: true }.benchmark(i, conf).1)
        };
        println!(" {:.1}\tsize [bits/key]: {:.2}\tlookup time [ns]: {:.0}\tbuild time [ms] ST, MT: {:.0}, {:.0}", gamma, b.bits_per_value, b.included.avg_lookup_time * 1_000_000_000.0, st_build_time * 1000.0, b.build_time_seconds * 1000.0);
        if let Some(ref mut f) = csv_file {
            writeln!(f, "{} {} {}", gamma, st_build_time, b).unwrap();
        }
    }
}

fn chd_benchmark<K>(mut csv_file: Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf)
where K: Hash
{
    if let Some(ref mut f) = csv_file {
        writeln!(f, "lambda {}", BENCHMARK_HEADER).unwrap()
    }
    println!("CHD: lambda results...");
    for lambda in 1..=6 {
        let b = CHDConf{ lambda }.benchmark(i, conf).1;
        if let Some(ref mut f) = csv_file {
            writeln!(f, "{} {}", lambda, b).unwrap();
        }
        println!(" {}\tsize [bits/key]: {:.2}\tlookup time [ns]: {:.0}\tbuild time [ms]: {:.0}", lambda, b.bits_per_value, b.included.avg_lookup_time * 1_000_000_000.0, b.build_process_time_seconds * 1_000.0);
    }
}

fn file<K>(method_name: &str, conf: &Conf, i: &(Vec<K>, Vec<K>)) -> Option<File> {
    if !conf.save_csv { return None; }
    let ks_name = match conf.key_source {
        KeySource::xs32 => "32",
        KeySource::xs64 => "64",
        KeySource::stdin => "str",
    };
    Some(File::create(format!("{}_{}_{}_{}.csv", method_name, ks_name, i.0.len(), i.1.len())).unwrap())
}

fn run<K: Hash + Sync + Send + Clone + Debug>(conf: &Conf, i: &(Vec<K>, Vec<K>)) {
    if conf.method == Method::FMPHGO_all {
        fmphgo_benchmark_all(file("FMPHGO_all", &conf, i), BuildWyHash::default(), &i, &conf, KeyAccess::StoreIndices);
    }
    if conf.method == Method::Boomphf || conf.method == Method::Most {
        fmph_benchmark::<BuildWyHash, _>(file("BooMPHF", &conf, i), i, conf, None);
    }
    if conf.method == Method::FMPH || conf.method == Method::Most {
        fmph_benchmark(file("FMPH", &conf, i), i, conf, Some((BuildWyHash::default(), KeyAccess::StoreIndices)));
    }
    if conf.method == Method::FMPHGO || conf.method == Method::Most {
        fmphgo_benchmark(file("FMPHGO", &conf, i), BuildWyHash::default(), i, conf, KeyAccess::StoreIndices);
    }
    if conf.method == Method::CHD || conf.method == Method::Most {
        if conf.key_source == KeySource::stdin {
            eprintln!("Benchmarking CHD with keys from stdin is not supported.")
        } else {
            chd_benchmark(file("CHD", &conf, i), i, conf);
        }
    }
    if conf.method == Method::FMPH_copy {
        fmph_benchmark(file("FMPH_copy", &conf, i), i, conf, Some((BuildWyHash::default(), KeyAccess::CopyKeys)));
    }
    if conf.method == Method::FMPHGO_copy {
        fmphgo_benchmark(file("FMPHGO_copy", &conf, i), BuildWyHash::default(), i, conf, KeyAccess::CopyKeys);
    }
    if conf.method == Method::FMPH_lomem {
        let ten_percent = i.0.len() / 10;
        fmph_benchmark(file("FMPH_lomem", &conf, i), i, conf, Some((BuildWyHash::default(), KeyAccess::LoMem(ten_percent))));
    }
    if conf.method == Method::FMPHGO_lomem {
        let ten_percent = i.0.len() / 10;
        fmphgo_benchmark(file("FMPHGO_lomem", &conf, i), BuildWyHash::default(), i, conf, KeyAccess::LoMem(ten_percent));
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
