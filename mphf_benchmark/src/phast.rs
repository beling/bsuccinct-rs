use ph::{fmph::TwoToPowerBitsStatic, phast::DefaultCompressedArray, seeds::{Bits8, BitsFast, SeedSize}, BuildSeededHasher, GetSize};

use crate::{builder::TypeToQuery, BenchmarkResult, Conf, MPHFBuilder, PHastConf};
use std::{fs::File, hash::Hash, io::Write};

#[derive(Default)]
pub struct PHastBencher<SS, S> {
    hash: std::marker::PhantomData<S>,
    bits_per_seed: SS,
    bucket_size_100: u16,
}

impl<SS: SeedSize, S: BuildSeededHasher + Default + Sync, K: Hash + Sync + Send + Clone + TypeToQuery> MPHFBuilder<K> for PHastBencher<SS, S> {
    type MPHF = ph::phast::Function<SS, DefaultCompressedArray, S>;

    type Value = usize;

    fn new(&self, keys: &[K], use_multiple_threads: bool) -> Self::MPHF {
        if use_multiple_threads {
            Self::MPHF::with_slice_bps_bs_threads_hash(keys, 
                self.bits_per_seed, self.bucket_size_100,
                std::thread::available_parallelism().map_or(1, |v| v.into()),
                S::default()
            )
        } else {
            Self::MPHF::with_slice_bps_bs_hash(keys,
                self.bits_per_seed, self.bucket_size_100, S::default()
            )
        }
    }

    #[inline(always)] fn value_ex(mphf: &Self::MPHF, key: &K, _levels: &mut u64) -> Option<u64> {
        Some(mphf.get(key.to_query_type()) as u64)  // TODO level support
    }

    #[inline(always)] fn value(mphf: &Self::MPHF, key: &K) -> Self::Value {
        mphf.get(key.to_query_type())
    }

    #[inline(always)] fn mphf_size(mphf: &Self::MPHF) -> usize {
        mphf.size_bytes()
    }

    const BUILD_THREADS_DOES_NOT_CHANGE_SIZE: bool = false;
}

/*pub fn phast_benchmark<K: std::hash::Hash + Sync + Send + Default + Clone>(csv_file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf) {
    let b = PHastConf.benchmark(i, conf);
    if let Some(ref mut f) = csv_file { writeln!(f, "{}", b.all()).unwrap(); }
    println!(" \t{}", b);
}*/

pub fn benchmark_with<S, SS, K>(bits_per_seed: SS, bucket_size_100: u16, i: &(Vec<K>, Vec<K>), conf: &Conf) -> BenchmarkResult
where SS: SeedSize, S: BuildSeededHasher + Default + Sync, K: Hash + Sync + Send + Clone + TypeToQuery
{
    PHastBencher { hash: std::marker::PhantomData::<S>::default(), bits_per_seed, bucket_size_100 }.benchmark(i, conf)
}

pub fn phast_benchmark<H: BuildSeededHasher+Default+Sync, K: Hash + Sync + Send + Clone + TypeToQuery>(csv_file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf, phast_conf: &PHastConf) {
    let bucket_size_100 = phast_conf.bucket_size();
    let b = match phast_conf.bits_per_seed {
        8 => benchmark_with::<H, _, _>(Bits8, bucket_size_100, i, conf),
        4 => benchmark_with::<H, _, _>(TwoToPowerBitsStatic::<2>, bucket_size_100, i, conf),
        b => benchmark_with::<H, _, _>(BitsFast(b), bucket_size_100, i, conf),
    };
    if let Some(ref mut f) = csv_file { writeln!(f, "{} {bucket_size_100} {}", phast_conf.bits_per_seed, b.all()).unwrap(); }
    println!(" \t{}", b);
}