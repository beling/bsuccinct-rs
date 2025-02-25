use ph::{fmph::Bits8, phast::{bits_per_seed_to_100_bucket_size, DefaultCompressedArray}, BuildSeededHasher, GetSize};

use crate::{Conf, MPHFBuilder};
use std::{fs::File, hash::Hash, io::Write};

#[derive(Default)]
pub struct PHastConf<S> {
    hash: std::marker::PhantomData<S>
}

impl<S: BuildSeededHasher + Default> PHastConf<S> {
    pub fn run<K: Hash + Sync + Send + Clone>(&self, csv_file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf) {
        let b = self.benchmark(i, conf);
        if let Some(ref mut f) = csv_file { writeln!(f, "{}", b.all()).unwrap(); }
        println!(" \t{}", b);
    }
}

impl<S: BuildSeededHasher + Default, K: Hash + Sync + Send + Clone> MPHFBuilder<K> for PHastConf<S> {
    type MPHF = ph::phast::Function<Bits8, DefaultCompressedArray, S>;

    type Value = usize;

    fn new(&self, keys: &[K], use_multiple_threads: bool) -> Self::MPHF {
        if use_multiple_threads {
            Self::MPHF::with_keys_bps_bs_threads_hash(keys.to_vec(), Bits8::default(),
                bits_per_seed_to_100_bucket_size(8),
                std::thread::available_parallelism().map_or(1, |v| v.into()),
                S::default()
            )
        } else {
            Self::MPHF::with_keys_bps_bs_threads_hash(keys.to_vec(), Bits8::default(),
                bits_per_seed_to_100_bucket_size(8), 1, S::default()
            )
        }
    }

    #[inline(always)] fn value_ex(mphf: &Self::MPHF, key: &K, _levels: &mut u64) -> Option<u64> {
        Some(mphf.get(key) as u64)  // TODO level support
    }

    #[inline(always)] fn value(mphf: &Self::MPHF, key: &K) -> Self::Value {
        mphf.get(key)
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