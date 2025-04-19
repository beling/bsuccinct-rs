use std::{fs::File, marker::PhantomData};
use std::io::Write;

use mem_dbg::MemSize;
use ptr_hash::{bucket_fn::BucketFn, PtrHash, PtrHashParams};

use crate::builder::TypeToQuery;
use crate::{Conf, MPHFBuilder, Threads};

#[derive(Clone, Copy)]
pub struct StrHasherForPtr;

impl<K: std::hash::Hash> ptr_hash::hash::Hasher<K> for StrHasherForPtr {
    type H = u64;

    #[inline]
    fn hash(x: &K, seed: u64) -> Self::H {
        ph::BuildSeededHasher::hash_one(&crate::StrHasher::default(), x, seed)
    }
}

impl<Hx, BF, K: TypeToQuery> MPHFBuilder<K> for (PtrHashParams<BF>, PhantomData<Hx>)
where PtrHash<K, BF>: MemSize, Hx: ptr_hash::hash::Hasher<K>, BF: BucketFn + MemSize, K: std::hash::Hash + Sync + Send + Default
{
    type MPHF = PtrHash<K, BF, cacheline_ef::CachelineEfVec, Hx, Vec<u8>>;
    type Value = usize;

    fn new(&self, keys: &[K], _use_multiple_threads: bool) -> Self::MPHF {
        Self::MPHF::new(keys, self.0)
    }

    #[inline(always)] fn value_ex(mphf: &Self::MPHF, key: &K, _levels: &mut usize) -> Option<u64> {
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

pub fn ptrhash_benchmark<Hx, K>(csv_file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf, speed: u8)
where Hx: ptr_hash::hash::Hasher<K>, K: std::hash::Hash + Sync + Send + Default + TypeToQuery
{
    let b= match speed {
        0 => (PtrHashParams::default_compact(), PhantomData::<Hx>).benchmark(i, conf),
        2 => (PtrHashParams::default_fast(), PhantomData::<Hx>).benchmark(i, conf),
        _ => (PtrHashParams::default(), PhantomData::<Hx>).benchmark(i, conf)
    };
    if let Some(ref mut f) = csv_file { writeln!(f, "{speed} {}", b.all()).unwrap(); }
    println!(" \t{}", b);
}