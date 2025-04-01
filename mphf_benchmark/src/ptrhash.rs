use std::fs::File;
use std::io::Write;

use mem_dbg::MemSize;
use ptr_hash::{bucket_fn::BucketFn, PtrHash, PtrHashParams};

use crate::{Conf, MPHFBuilder, Threads};

impl<BF: BucketFn + MemSize, K: std::hash::Hash + Sync + Send + Default> MPHFBuilder<K> for PtrHashParams<BF>
where PtrHash<K, BF>: MemSize {
    type MPHF = PtrHash<K, BF>;
    type Value = usize;

    fn new(&self, keys: &[K], _use_multiple_threads: bool) -> Self::MPHF {
        <PtrHash<K, BF>>::new(keys, *self)
    }

    #[inline(always)] fn value_ex(mphf: &Self::MPHF, key: &K, _levels: &mut u64) -> Option<u64> {
        Some(mphf.index(key) as u64)
    }

    #[inline(always)] fn value(mphf: &Self::MPHF, key: &K) -> Self::Value {
        mphf.index(key)
    }

    fn mphf_size(mphf: &Self::MPHF) -> usize { 
        mphf.mem_size(mem_dbg::SizeFlags::default())
    }

    const CAN_DETECT_ABSENCE: bool = false;
    const BUILD_THREADS: crate::Threads = Threads::Multi;
}

pub fn ptrhash_benchmark<K: std::hash::Hash + Sync + Send + Default>(csv_file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf, speed: u8) {
    let b= match speed {
        0 => PtrHashParams::default_compact().benchmark(i, conf),
        2 => PtrHashParams::default_fast().benchmark(i, conf),
        _ => PtrHashParams::default().benchmark(i, conf)
    };
    if let Some(ref mut f) = csv_file { writeln!(f, "{speed} {}", b.all()).unwrap(); }
    println!(" \t{}", b);
}