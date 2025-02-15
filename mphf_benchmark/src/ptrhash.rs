use std::fs::File;
use std::io::Write;

use ptr_hash::{PtrHash, PtrHashParams};

use crate::{Conf, MPHFBuilder, Threads};

impl<K: std::hash::Hash + Sync + Send + Default> MPHFBuilder<K> for PtrHashParams {
    type MPHF = PtrHash<K>;

    fn new(&self, keys: &[K], _use_multiple_threads: bool) -> Self::MPHF {
        <PtrHash<K>>::new(keys, *self)
    }

    #[inline(always)] fn value(mphf: &Self::MPHF, key: &K, _levels: &mut u64) -> Option<u64> {
        Some(mphf.index_minimal(key) as u64)
    }

    fn mphf_size(_mphf: &Self::MPHF) -> usize { 
        0
    }

    const CAN_DETECT_ABSENCE: bool = false;
    const BUILD_THREADS: crate::Threads = Threads::Multi;
}

pub fn ptrhash_benchmark<K: std::hash::Hash + Sync + Send + Default>(csv_file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf) {
    let b = PtrHashParams::default().benchmark(i, conf);
    if let Some(ref mut f) = csv_file { writeln!(f, "{}", b.all()).unwrap(); }
    println!(" \t{}", b);
}