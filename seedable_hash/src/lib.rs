#![doc = include_str!("../README.md")]

mod map;
pub use map::{map16_to_16, map32_to_32, map64_to_32, map64_to_64, map_usize};

use std::hash::{BuildHasher, Hash, Hasher};

/// Family of hash functions that allows the creation of
/// [`Hasher`] instances initialized with a given seed.
pub trait BuildSeededHasher {
    type Hasher: Hasher;

    /// Creates a new hasher initialized with the given `seed`.
    fn build_hasher(&self, seed: u32) -> Self::Hasher;

    /// Creates a new hasher initialized with the given `seed`.
    fn build_hasher_s64(&self, seed: u64) -> Self::Hasher;

    /// Calculates the hash of a single value `x`, using given `seed`.
    #[inline]
    fn hash_one<T: Hash>(&self, x: T, seed: u32) -> u64 {
        let mut h = self.build_hasher(seed);
        x.hash(&mut h);
        h.finish()
    }

    /// Calculates the hash of a single value `x`, using given `seed`.
    #[inline]
    fn hash_one_s64<T: Hash>(&self, x: T, seed: u64) -> u64 {
        let mut h = self.build_hasher_s64(seed);
        x.hash(&mut h);
        h.finish()
    }
}

/// [`BuildSeededHasher`] that uses standard [`BuildHasher`].
#[derive(Default, Copy, Clone)]
pub struct Seedable<BH: BuildHasher>(pub BH);

impl<BH: BuildHasher> BuildSeededHasher for Seedable<BH> {
    type Hasher = BH::Hasher;

    #[inline]
    fn build_hasher(&self, seed: u32) -> Self::Hasher {
        let mut result = self.0.build_hasher();
        result.write_u32(seed);
        result
    }

    #[inline]
    fn build_hasher_s64(&self, seed: u64) -> Self::Hasher {
        let mut result = self.0.build_hasher();
        result.write_u64(seed);
        result
    }
}

/// [`BuildSeededHasher`] that uses [`std::hash::SipHasher13`].
#[cfg(feature = "sip13")]
#[derive(Default, Copy, Clone)]
pub struct BuildSip13;

#[cfg(feature = "sip13")]
#[allow(deprecated)]
impl BuildSeededHasher for BuildSip13 {
    #[allow(deprecated)]
    type Hasher = std::hash::SipHasher13;

    #[inline] fn build_hasher(&self, seed: u32) -> Self::Hasher {
        Self::Hasher::new_with_keys(seed as u64, seed as u64)
    }
}

/// [`BuildSeededHasher`] that uses `wyhash` crate.
#[cfg(feature = "wyhash")]
#[derive(Default, Copy, Clone)]
pub struct BuildWyHash;

#[cfg(feature = "wyhash")]
impl BuildSeededHasher for BuildWyHash {
    type Hasher = wyhash::WyHash;

    #[inline] fn build_hasher(&self, seed: u32) -> Self::Hasher {
        Self::Hasher::with_seed(seed as u64)
    }

    #[inline]
    fn build_hasher_s64(&self, seed: u64) -> Self::Hasher {
        Self::Hasher::with_seed(seed)
    }
}

#[cfg(feature = "xxhash-rust")]
impl BuildSeededHasher for xxhash_rust::xxh3::Xxh3Builder {
    type Hasher = xxhash_rust::xxh3::Xxh3;

    #[inline] fn build_hasher(&self, seed: u32) -> Self::Hasher {
        Self::Hasher::with_seed(seed as u64)
    }

    #[inline]
    fn build_hasher_s64(&self, seed: u64) -> Self::Hasher {
        Self::Hasher::with_seed(seed)
    }
}

/// [`BuildSeededHasher`] that uses `fnv` crate.
#[cfg(feature = "fnv")]
impl BuildSeededHasher for fnv::FnvBuildHasher {
    type Hasher = fnv::FnvHasher;

    #[inline] fn build_hasher(&self, seed: u32) -> Self::Hasher {
        Self::Hasher::with_key(seed as u64)
    }

    #[inline]
    fn build_hasher_s64(&self, seed: u64) -> Self::Hasher {
        Self::Hasher::with_key(seed)
    }
}

/// [`BuildSeededHasher`] that uses `gxhash` crate.
#[cfg(feature = "gxhash")]
impl BuildSeededHasher for gxhash::GxBuildHasher {
    type Hasher = gxhash::GxHasher;

    #[inline] fn build_hasher(&self, seed: u32) -> Self::Hasher {
        Self::Hasher::with_seed(seed as i64)
    }

    #[inline] fn build_hasher_s64(&self, seed: u64) -> Self::Hasher {
        Self::Hasher::with_seed(seed as i64)
    }
}

/// [`BuildSeededHasher`] that uses `rapidhash::RapidBuildHasher`.
#[cfg(feature = "rapidhash")]
impl BuildSeededHasher for rapidhash::RapidBuildHasher {
    type Hasher = rapidhash::RapidHasher;

    #[inline] fn build_hasher(&self, seed: u32) -> Self::Hasher {
        Self::Hasher::new(seed as u64)
    }

    #[inline] fn build_hasher_s64(&self, seed: u64) -> Self::Hasher {
        Self::Hasher::new(seed)
    }
}

/// [`BuildSeededHasher`] that uses `rapidhash::RapidInlineBuildHasher`.
#[cfg(feature = "rapidhash")]
impl BuildSeededHasher for rapidhash::RapidInlineBuildHasher {
    type Hasher = rapidhash::RapidInlineHasher;

    #[inline] fn build_hasher(&self, seed: u32) -> Self::Hasher {
        Self::Hasher::new(seed as u64)
    }

    #[inline] fn build_hasher_s64(&self, seed: u64) -> Self::Hasher {
        Self::Hasher::new(seed)
    }
}

/// The default [`BuildSeededHasher`].
#[cfg(feature = "gxhash")]
pub type BuildDefaultSeededHasher = gxhash::GxBuildHasher;

/// The default [`BuildSeededHasher`].
#[cfg(all(feature = "wyhash", not(feature = "gxhash")))]
pub type BuildDefaultSeededHasher = BuildWyHash;

/// The default [`BuildSeededHasher`].
#[cfg(all(feature = "xxhash-rust", not(feature = "gxhash"), not(feature = "wyhash")))]
pub type BuildDefaultSeededHasher = xxhash_rust::xxh3::Xxh3Builder;

/// The default [`BuildSeededHasher`].
#[cfg(all(feature = "sip13", not(feature = "gxhash"), not(feature = "wyhash"), not(feature = "xxhash-rust")))]
pub type BuildDefaultSeededHasher = BuildSip13;

/// The default [`BuildSeededHasher`].
#[cfg(all(feature = "fnv", not(feature = "gxhash"), not(feature = "sip13"), not(feature = "wyhash"), not(feature = "xxhash-rust")))]
pub type BuildDefaultSeededHasher = fnv::FnvBuildHasher;

/// The default [`BuildSeededHasher`].
#[cfg(all(not(feature = "gxhash"), not(feature = "wyhash"), not(feature = "xxhash-rust"), not(feature = "fnv"), not(feature = "sip13")))]
pub type BuildDefaultSeededHasher = Seedable<std::hash::BuildHasherDefault<std::collections::hash_map::DefaultHasher>>;