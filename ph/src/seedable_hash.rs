#[allow(deprecated)]
use std::hash::{BuildHasher, Hash, Hasher, SipHasher13};

/// A trait for creating instances of [`Hasher`] that are initialized with a seed.
pub trait BuildSeededHasher {
    type Hasher: Hasher;

    /// Creates a new hasher initialized with given `seed`.
    fn build_hasher(&self, seed: u32) -> Self::Hasher;

    /// Calculates the hash of a single value, using given `seed`.
    #[inline]
    fn hash_one<T: Hash>(&self, x: T, seed: u32) -> u64 {
        let mut h = self.build_hasher(seed);
        x.hash(&mut h);
        h.finish()
    }
}

/// [`BuildSeededHasher`] that uses standard [`BuildHasher`].
pub struct Seedable<BH: BuildHasher>(BH);

impl<BH: BuildHasher> BuildSeededHasher for Seedable<BH> {
    type Hasher = BH::Hasher;

    #[inline]
    fn build_hasher(&self, seed: u32) -> Self::Hasher {
        let mut result = self.0.build_hasher();
        result.write_u32(seed);
        result
    }
}

#[derive(Default, Copy, Clone)]
pub struct BuildDefaultSeededHasher;

#[allow(deprecated)]
impl BuildSeededHasher for BuildDefaultSeededHasher {
    type Hasher = SipHasher13;

    #[inline] fn build_hasher(&self, seed: u32) -> Self::Hasher {
        Self::Hasher::new_with_keys(seed as u64, seed as u64)
    }
}

#[cfg(feature = "wyhash")]
#[derive(Default, Copy, Clone)]
pub struct BuildWyHash;

#[cfg(feature = "wyhash")]
impl BuildSeededHasher for BuildWyHash {
    type Hasher = wyhash::WyHash;

    #[inline] fn build_hasher(&self, seed: u32) -> Self::Hasher {
        Self::Hasher::with_seed(seed as u64)
    }
}

#[cfg(feature = "fnv")]
impl BuildSeededHasher for fnv::FnvBuildHasher {
    type Hasher = fnv::FnvHasher;

    #[inline] fn build_hasher(&self, seed: u32) -> Self::Hasher {
        Self::Hasher::with_key(seed as u64)
    }
}