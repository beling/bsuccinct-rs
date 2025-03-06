use std::fs::File;
use std::hash::Hash;
use std::fmt::Debug;
use std::io::Write;
#[cfg(feature = "boomphf")] use boomphf::Mphf;
use dyn_size_of::GetSize;
use ph::{fmph, BuildSeededHasher};
use ph::fmph::keyset::SliceSourceWithRefs;
#[cfg(feature = "fmph-key-access")] use ph::fmph::keyset::ImmutableSlice;

use crate::{file, BenchmarkResult, Conf, FMPHConf, IntHasher, KeyAccess, KeySource, MPHFBuilder, StrHasher};

#[cfg(feature = "boomphf")]
pub struct BooMPHFConf { pub gamma: f64 }

#[cfg(feature = "boomphf")]
impl<K: std::hash::Hash + std::fmt::Debug + Sync + Send> MPHFBuilder<K> for BooMPHFConf {
    type MPHF = Mphf<K>;
    type Value = Option<u64>;

    fn new(&self, keys: &[K], use_multiple_threads: bool) -> Self::MPHF {
        if use_multiple_threads {
            Mphf::new_parallel(self.gamma, keys, None)
        } else {
            Mphf::new(self.gamma, keys)
        }
    }

    #[inline(always)] fn value_ex(mphf: &Self::MPHF, key: &K, levels: &mut u64) -> Option<u64> {
        mphf.try_hash_bench(&key, levels)
    }

    #[inline(always)] fn value(mphf: &Self::MPHF, key: &K) -> Self::Value {
        mphf.try_hash(key)
    }

    fn mphf_size(mphf: &Self::MPHF) -> usize { mphf.size_bytes() }
}


impl<K: Hash + Sync + Send + Clone, S: BuildSeededHasher + Clone + Sync> MPHFBuilder<K> for (fmph::BuildConf<S>, KeyAccess) {
    type MPHF = fmph::Function<S>;
    type Value = Option<u64>;

    fn new(&self, keys: &[K], use_multiple_threads: bool) -> Self::MPHF {
        let mut conf = self.0.clone();
        conf.use_multiple_threads = use_multiple_threads;
        #[cfg(feature = "fmph-key-access")]
        match self.1 {
            KeyAccess::Indices8 => Self::MPHF::with_conf(SliceSourceWithRefs::<_, u8>::new(keys), conf),
            KeyAccess::Indices16 => Self::MPHF::with_conf(SliceSourceWithRefs::<_, u16>::new(keys), conf),
            KeyAccess::Copy => Self::MPHF::with_conf(ImmutableSlice::cached(keys, usize::MAX), conf)
        }
        #[cfg(not(feature = "fmph-key-access"))] Self::MPHF::with_conf(SliceSourceWithRefs::<_, u8>::new(keys), conf)
    }

    #[inline(always)] fn value_ex(mphf: &Self::MPHF, key: &K, levels: &mut u64) -> Option<u64> {
        mphf.get_stats(key, levels)
    }

    #[inline(always)] fn value(mphf: &Self::MPHF, key: &K) -> Self::Value {
        mphf.get(key)
    }
    
    fn mphf_size(mphf: &Self::MPHF) -> usize { mphf.size_bytes() }
}

impl<K: Hash + Sync + Send + Clone, GS: fmph::GroupSize + Sync, SS: fmph::SeedSize, S: BuildSeededHasher + Clone + Sync> MPHFBuilder<K> for (fmph::GOBuildConf<GS, SS, S>, KeyAccess) {
    type MPHF = fmph::GOFunction<GS, SS, S>;
    type Value = Option<u64>;

    fn new(&self, keys: &[K], use_multiple_threads: bool) -> Self::MPHF {
        let mut conf = self.0.clone();
        conf.use_multiple_threads = use_multiple_threads;
        match self.1 {
            //KeyAccess::Sequential => Self::MPHF::with_builder(
            //    CachedKeySet::new(DynamicKeySet::with_len(|| keys.iter(), keys.len(), true), keys.len() / 10),
            //    conf),
            KeyAccess::Indices8 => Self::MPHF::with_conf(SliceSourceWithRefs::<_, u8>::new(keys), conf),
            #[cfg(feature = "fmph-key-access")] KeyAccess::Indices16 => Self::MPHF::with_conf(SliceSourceWithRefs::<_, u16>::new(keys), conf),
            #[cfg(feature = "fmph-key-access")] KeyAccess::Copy => Self::MPHF::with_conf(ImmutableSlice::cached(keys, usize::MAX), conf)

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

    #[inline(always)] fn value_ex(mphf: &Self::MPHF, key: &K, levels: &mut u64) -> Option<u64> {
        mphf.get_stats(key, levels)
    }

    #[inline(always)] fn value(mphf: &Self::MPHF, key: &K) -> Self::Value {
        mphf.get(key)
    }

    fn mphf_size(mphf: &Self::MPHF) -> usize { mphf.size_bytes() }
}

pub const FMPHGO_HEADER: &'static str = "cache_threshold bits_per_group_seed relative_level_size bits_per_group";

pub struct FMPHGOBuildParams {
    pub relative_level_size: u16,
    pub cache_threshold: usize,
    pub key_access: KeyAccess
}

pub fn h2bench<GS, SS, K>(bits_per_group_seed: SS, bits_per_group: GS, i: &(Vec<K>, Vec<K>), conf: &Conf, p: &FMPHGOBuildParams) -> BenchmarkResult
    where GS: fmph::GroupSize + Sync + Copy, SS: fmph::SeedSize + Copy, K: Hash + Sync + Send + Clone
{
    if conf.key_source == KeySource::xs32 || conf.key_source == KeySource::xs64 {
        (fmph::GOBuildConf::with_lsize_ct_mt(
            fmph::GOConf::hash_bps_bpg(IntHasher::default(), bits_per_group_seed, bits_per_group),
            p.relative_level_size, p.cache_threshold, false), p.key_access)
        .benchmark(i, conf)
    } else {
        (fmph::GOBuildConf::with_lsize_ct_mt(
            fmph::GOConf::hash_bps_bpg(StrHasher::default(), bits_per_group_seed, bits_per_group),
            p.relative_level_size, p.cache_threshold, false), p.key_access)
        .benchmark(i, conf)
    }
}

pub fn h2b<GS, K>(bits_per_group_seed: u8, bits_per_group: GS, i: &(Vec<K>, Vec<K>), conf: &Conf, p: &FMPHGOBuildParams) -> BenchmarkResult
    where GS: fmph::GroupSize + Sync + Copy, K: Hash + Sync + Send + Clone
{
    match bits_per_group_seed {
        1 => h2bench(fmph::TwoToPowerBitsStatic::<0>, bits_per_group, i, conf, p),
        2 => h2bench(fmph::TwoToPowerBitsStatic::<1>, bits_per_group, i, conf, p),
        4 => h2bench(fmph::TwoToPowerBitsStatic::<2>, bits_per_group, i, conf, p),
        8 => h2bench(fmph::Bits8, bits_per_group, i, conf, p),
        //16 => h2bench(TwoToPowerBitsStatic::<5>, bits_per_group, i, conf, p),
        _ => h2bench(fmph::Bits(bits_per_group_seed), bits_per_group, i, conf, p)
    }
}

pub fn fmphgo<K>(file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf, bits_per_group_seed: u8, bits_per_group: u8, p: &FMPHGOBuildParams)
                -> BenchmarkResult
    where K: Hash + Sync + Send + Clone
{
    let b = if bits_per_group.is_power_of_two() {
        match bits_per_group {
            //1 => h2b(bits_per_group_seed, TwoToPowerBitsStatic::<0>, i, conf, p),
            //2 => h2b(bits_per_group_seed, TwoToPowerBitsStatic::<1>, i, conf, p),
            //4 => h2b(bits_per_group_seed, TwoToPowerBitsStatic::<2>, i, conf, p),
            8 => h2b(bits_per_group_seed, fmph::TwoToPowerBitsStatic::<3>, i, conf, p),
            16 => h2b(bits_per_group_seed, fmph::TwoToPowerBitsStatic::<4>, i, conf, p),
            32 => h2b(bits_per_group_seed, fmph::TwoToPowerBitsStatic::<5>, i, conf, p),
            //64 => h2b(bits_per_group_seed, TwoToPowerBitsStatic::<6>, i, conf, p),
            _ => h2b(bits_per_group_seed, fmph::TwoToPowerBits::new(bits_per_group.trailing_zeros() as u8), i, conf, p)
        }
    } else {
        h2b(bits_per_group_seed, fmph::Bits(bits_per_group), i, conf, p)
    };
    if let Some(ref mut f) = file {
        writeln!(f, "{} {} {} {} {}", p.cache_threshold, bits_per_group_seed, p.relative_level_size, bits_per_group, b.all()).unwrap();
    }
    b
}

pub fn fmphgo_benchmark<K>(file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf, bits_per_group_seed: u8, bits_per_group: u8, p: &FMPHGOBuildParams)
    where K: Hash + Sync + Send + Clone
{
    let b = fmphgo(file, i, conf, bits_per_group_seed, bits_per_group, p);
    print_fmphgo_result(bits_per_group_seed, bits_per_group, p, b);
}

#[inline(never)]
fn print_fmphgo_result(bits_per_group_seed: u8, bits_per_group: u8, p: &FMPHGOBuildParams, b: BenchmarkResult) {
    println!(" {} {} {:.1}\t{}", bits_per_group_seed, bits_per_group, p.relative_level_size as f64/100.0, b);
}

pub fn fmphgo_run<K>(file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf, bits_per_group_seed: u8, bits_per_group: u8, p: &mut FMPHGOBuildParams)
    where K: Hash + Sync + Send + Clone
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

pub fn fmphgo_benchmark_all<K>(mut csv_file: Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf, key_access: KeyAccess)
where K: Hash + Sync + Send + Clone
{
    print_fmphgo_all_header();
    let mut p = FMPHGOBuildParams {
        relative_level_size: 0,
        cache_threshold: usize::MAX,
        key_access
    };
    for bits_per_group_seed in 1u8..=10u8 {
        for relative_level_size in (100..=200).step_by(/*50*/100) {
            p.relative_level_size = relative_level_size;
            print!("{} {}", bits_per_group_seed, relative_level_size);
            //for bits_per_group_log2 in 3u8..=7u8 {
            for bits_per_group in 2u8..=62u8/*.step_by(2)*/ {
                //let (_, b) = Conf::bps_bpg_lsize(bits_per_group_seed, TwoToPowerBits::new(bits_per_group_log2), relative_level_size).benchmark(verify);
                let b = fmphgo(&mut csv_file, i, conf, bits_per_group_seed, bits_per_group, &p);
                print_bit_per_value(b.bits_per_value);
            }
            println!();
        }
    }
}

#[inline(never)]
fn print_fmphgo_all_header() {
    println!("bps rls \\ bpglog 2 3 4 5 ... 62");
}

#[inline(never)]
fn print_bit_per_value(bits_per_value: f64) {
    print!(" {:.2}", bits_per_value);
    std::io::stdout().flush().unwrap();
}

pub const FMPH_BENCHMARK_HEADER: &'static str = "cache_threshold relative_level_size";
pub const BOOMPHF_BENCHMARK_HEADER: &'static str = "relative_level_size";

pub fn fmph_benchmark<S, K>(i: &(Vec<K>, Vec<K>), conf: &Conf, level_size: Option<u16>, use_fmph: Option<(S, &FMPHConf)>)
where S: BuildSeededHasher + Clone + Sync, K: Hash + Sync + Send + Debug + Clone
{
    let mut file = get_fmph_file(i.0.len(), i.1.len(), conf, &use_fmph.as_ref().map(|p| p.1));
    for relative_level_size in level_size.map_or(100..=200, |r| r..=r).step_by(/*50*/100) {
        let gamma = relative_level_size as f64 / 100.0f64;
        if let Some((ref hash, fc)) = use_fmph {
            let b = (fmph::BuildConf::hash_lsize_ct_mt(hash.clone(), relative_level_size, fc.cache_threshold, false), fc.key_access).benchmark(i, &conf);
            print_fmph_result(&mut file, relative_level_size, gamma, b, fc);
        } else {
            #[cfg(feature = "boomphf")] {
                let b = BooMPHFConf { gamma }.benchmark(i, &conf);
                println!(" {:.1}\t{}", gamma, b);
                if let Some(ref mut f) = file { writeln!(f, "{} {}", relative_level_size, b.all()).unwrap(); }
            }
        };
    }
}

#[inline(never)]
fn print_fmph_result(file: &mut Option<File>, relative_level_size: u16, gamma: f64, b: BenchmarkResult, fc: &FMPHConf) {
    println!(" {:.1}\t{}", gamma, b);
    if let Some(ref mut f) = *file { writeln!(f, "{} {} {}", fc.cache_threshold, relative_level_size, b.all()).unwrap(); }
}

#[inline(never)]
fn get_fmph_file(i0_len: usize, i1_len: usize, conf: &Conf, use_fmph: &Option<&FMPHConf>) -> Option<File> {
    let file = if let Some(fc) = *use_fmph {
        println!("FMPH hash caching threshold={}: gamma results...", fc.cache_threshold);
        file("FMPH", &conf, i0_len, i1_len, FMPH_BENCHMARK_HEADER)
    } else {
        println!("boomphf: gamma results...");
        file("boomphf", &conf, i0_len, i1_len, BOOMPHF_BENCHMARK_HEADER)
    };
    file
}