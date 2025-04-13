#![doc = include_str!("../README.md")]

mod map;
pub use map::{map16_to_16, map32_to_32, map64_to_32, map64_to_64, map_usize};

use std::hash::{BuildHasher, Hash, Hasher};

/// Family of hash functions that allows the creation of
/// [`Hasher`] instances initialized with a given seed.
pub trait BuildSeededHasher {
    type Hasher: Hasher;
    type ProcessedSeed;

    fn process_seed(&self, seed: u64) -> Self::ProcessedSeed;
    fn build_from_processed(processed_seed: &Self::ProcessedSeed, seed: u64) -> Self::Hasher;

    /// Creates a new hasher initialized with the given 64-bit `seed`.
    #[inline(always)] fn build_hasher(&self, seed: u64) -> Self::Hasher {
        Self::build_from_processed(&self.process_seed(seed), seed)
    }

    /// Calculates the hash of a single value `x`, using given 64-bit `seed`.
    #[inline(always)]
    fn hash_one<T: Hash>(&self, x: T, seed: u64) -> u64 {
        let mut h = self.build_hasher(seed);
        x.hash(&mut h);
        h.finish()
    }

    /// Calculates the hash of a single value `x`, using given 64-bit `seed`.
    #[inline(always)]
    fn hash_one_processed<T: Hash>(x: T, processed_seed: &Self::ProcessedSeed, seed: u64) -> u64 {
        let mut h = Self::build_from_processed(processed_seed, seed);
        x.hash(&mut h);
        h.finish()
    }
}

/// [`BuildSeededHasher`] that uses standard [`BuildHasher`].
#[derive(Default, Copy, Clone)]
pub struct Seedable<BH: BuildHasher + Clone>(pub BH);

impl<BH: BuildHasher + Clone> BuildSeededHasher for Seedable<BH> {
    type Hasher = BH::Hasher;
    type ProcessedSeed = Self;

    #[inline(always)]
    fn build_hasher(&self, seed: u64) -> Self::Hasher {
        let mut result = self.0.build_hasher();
        result.write_u64(seed);
        result
    }
    
    #[inline(always)]
    fn process_seed(&self, _seed: u64) -> Self::ProcessedSeed { self.clone() }
    
    #[inline(always)]
    fn build_from_processed(processed_seed: &Self::ProcessedSeed, seed: u64) -> Self::Hasher {
        processed_seed.build_hasher(seed)
    }

    #[inline(always)]
    fn hash_one_processed<T: Hash>(x: T, processed_seed: &Self::ProcessedSeed, seed: u64) -> u64 {
        processed_seed.hash_one(x, seed)
    }
}

/// [`BuildSeededHasher`] that uses standard [`BuildHasher`].
#[derive(Default, Copy, Clone)]
pub struct SeedableCloned<BH: BuildHasher>(pub BH);

impl<BH: BuildHasher> BuildSeededHasher for SeedableCloned<BH> where BH::Hasher: Clone {
    type Hasher = BH::Hasher;
    type ProcessedSeed = BH::Hasher;

    #[inline(always)]
    fn build_hasher(&self, seed: u64) -> Self::Hasher {
        let mut result = self.0.build_hasher();
        result.write_u64(seed);
        result
    }
    
    #[inline(always)]
    fn process_seed(&self, seed: u64) -> Self::ProcessedSeed {
        let mut result = self.0.build_hasher();
        result.write_u64(seed);
        result
    }
    
    #[inline(always)]
    fn build_from_processed(processed_seed: &Self::ProcessedSeed, _seed: u64) -> Self::Hasher {
        processed_seed.clone()
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

    #[inline] fn build_hasher64(&self, seed: u64) -> Self::Hasher {
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
    type ProcessedSeed = Self;

    #[inline]
    fn build_hasher(&self, seed: u64) -> Self::Hasher {
        Self::Hasher::with_seed(seed)
    }
    
    #[inline]
    fn process_seed(&self, _seed: u64) -> Self::ProcessedSeed { BuildWyHash }
    
    #[inline]
    fn build_from_processed(processed_seed: &Self::ProcessedSeed, seed: u64) -> Self::Hasher {
        processed_seed.build_hasher(seed)
    }
}

/// [`BuildSeededHasher`] that uses `Xxh3` from `xxhash_rust` crate.
#[cfg(feature = "xxhash-rust")]
#[derive(Default, Copy, Clone)]
pub struct BuildXxh3;

#[cfg(feature = "xxhash-rust")]
impl BuildSeededHasher for BuildXxh3 {
    type Hasher = xxhash_rust::xxh3::Xxh3;

    #[inline] fn build_hasher(&self, seed: u32) -> Self::Hasher {
        Self::Hasher::with_seed(seed as u64)
    }

    #[inline]
    fn build_hasher64(&self, seed: u64) -> Self::Hasher {
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
    fn build_hasher64(&self, seed: u64) -> Self::Hasher {
        Self::Hasher::with_key(seed)
    }
}


/// [`BuildSeededHasher`] that uses `GxHasher` from `gxhash` crate.
#[cfg(feature = "gxhash")]
#[derive(Default, Copy, Clone)]
pub struct BuildGxHash;

//type BuildGxHash = gxhash::GxBuildHasher;

/// [`BuildSeededHasher`] that uses `gxhash` crate.
#[cfg(feature = "gxhash")]
impl BuildSeededHasher for BuildGxHash {
    type Hasher = gxhash::GxHasher;
    type ProcessedSeed = gxhash::GxBuildHasher;
    
    #[inline]
    fn process_seed(&self, seed: u64) -> Self::ProcessedSeed {
        Self::ProcessedSeed::with_seed(seed as i64)
    }

    #[inline]
    fn build_from_processed(processed_seed: &Self::ProcessedSeed, _seed: u64) -> Self::Hasher {
        processed_seed.build_hasher()
    }

    #[inline] fn build_hasher(&self, seed: u64) -> Self::Hasher {
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

    #[inline] fn build_hasher64(&self, seed: u64) -> Self::Hasher {
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

    #[inline] fn build_hasher64(&self, seed: u64) -> Self::Hasher {
        Self::Hasher::new(seed)
    }
}

/// The default [`BuildSeededHasher`].
#[cfg(feature = "gxhash")]
pub type BuildDefaultSeededHasher = BuildGxHash;

/// The default [`BuildSeededHasher`].
#[cfg(all(feature = "wyhash", not(feature = "gxhash")))]
pub type BuildDefaultSeededHasher = BuildWyHash;

/// The default [`BuildSeededHasher`].
#[cfg(all(feature = "xxhash-rust", not(feature = "gxhash"), not(feature = "wyhash")))]
pub type BuildDefaultSeededHasher = BuildXxh3;

/// The default [`BuildSeededHasher`].
#[cfg(all(feature = "sip13", not(feature = "gxhash"), not(feature = "wyhash"), not(feature = "xxhash-rust")))]
pub type BuildDefaultSeededHasher = BuildSip13;

/// The default [`BuildSeededHasher`].
#[cfg(all(feature = "fnv", not(feature = "gxhash"), not(feature = "sip13"), not(feature = "wyhash"), not(feature = "xxhash-rust")))]
pub type BuildDefaultSeededHasher = fnv::FnvBuildHasher;

/// The default [`BuildSeededHasher`].
#[cfg(all(not(feature = "gxhash"), not(feature = "wyhash"), not(feature = "xxhash-rust"), not(feature = "fnv"), not(feature = "sip13")))]
pub type BuildDefaultSeededHasher = Seedable<std::hash::BuildHasherDefault<std::collections::hash_map::DefaultHasher>>;