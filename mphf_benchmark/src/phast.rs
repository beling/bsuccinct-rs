use ph::{fmph::Bits8, phast::DefaultCompressedArray, BuildDefaultSeededHasher, GetSize};

use crate::{Conf, MPHFBuilder};
use std::{fs::File, hash::Hash, io::Write};

pub struct PHastConf;

impl<K: Hash + Sync + Send + Clone> MPHFBuilder<K> for PHastConf {
    type MPHF = ph::phast::Function<Bits8, DefaultCompressedArray, BuildDefaultSeededHasher>;

    type Value = usize;

    fn new(&self, keys: &[K], use_multiple_threads: bool) -> Self::MPHF {
        if use_multiple_threads { Self::MPHF::new(keys.to_vec()) } else { Self::MPHF::new_st(keys.to_vec()) }
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

pub fn phast_benchmark<K: std::hash::Hash + Sync + Send + Default + Clone>(csv_file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf) {
    let b = PHastConf.benchmark(i, conf);
    if let Some(ref mut f) = csv_file { writeln!(f, "{}", b.all()).unwrap(); }
    println!(" \t{}", b);
}