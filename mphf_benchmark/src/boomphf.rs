use boomphf::Mphf;
use ph::GetSize;

use crate::MPHFBuilder;

pub struct BooMPHFConf { pub gamma: f64 }

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
