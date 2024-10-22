#![doc = include_str!("../README.md")]

mod map;
pub use map::{map16_to_16, map32_to_32, map64_to_32, map64_to_64, map_usize};

use std::hash::{BuildHasher, Hash, Hasher};

#[cfg(all(not(feature = "fnv"), not(feature = "sip13"), not(feature = "wyhash")))]
use std::{hash::BuildHasherDefault, collections::hash_map::DefaultHasher};

#[cfg(feature = "sip13")]
#[allow(deprecated)]
use std::hash::SipHasher13;

/// Family of hash functions that allows the creation of
/// [`Hasher`] instances initialized with a given seed.
pub trait BuildSeededHasher {
    type Hasher: Hasher;

    /// Creates a new hasher initialized with the given `seed`.
    fn build_hasher(&self, seed: u32) -> Self::Hasher;

    /// Calculates the hash of a single value `x`, using given `seed`.
    #[inline]
    fn hash_one<T: Hash>(&self, x: T, seed: u32) -> u64 {
        let mut h = self.build_hasher(seed);
        x.hash(&mut h);
        h.finish()
    }
}

/// [`BuildSeededHasher`] that uses standard [`BuildHasher`].
#[derive(Default, Copy, Clone)]
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

#[cfg(feature = "sip13")]
#[derive(Default, Copy, Clone)]
pub struct BuildSip13;

#[cfg(feature = "sip13")]
#[allow(deprecated)]
impl BuildSeededHasher for BuildSip13 {
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

/*#[cfg(feature = "wyhash_git")]
#[derive(Default, Copy, Clone)]
pub struct BuildWyHashGit;

#[cfg(feature = "wyhash_git")]
impl BuildSeededHasher for BuildWyHashGit {
    type Hasher = wyhash_git::WyHash;

    #[inline] fn build_hasher(&self, mut seed: u32) -> Self::Hasher {
        Self::Hasher::new(seed as u64, [0xa076_1d64_78bd_642f, 0xe703_7ed1_a0b4_28db, 0x8ebc_6af0_9c88_c6e3, 0x5899_65cc_7537_4cc3])
    }
}*/

#[cfg(feature = "fnv")]
impl BuildSeededHasher for fnv::FnvBuildHasher {
    type Hasher = fnv::FnvHasher;

    #[inline] fn build_hasher(&self, seed: u32) -> Self::Hasher {
        Self::Hasher::with_key(seed as u64)
    }
}

#[cfg(feature = "wyhash")]
pub type BuildDefaultSeededHasher = BuildWyHash;

#[cfg(all(feature = "sip13", not(feature = "wyhash")))]
pub type BuildDefaultSeededHasher = BuildSip13;

#[cfg(all(feature = "fnv", not(feature = "sip13"), not(feature = "wyhash")))]
pub type BuildDefaultSeededHasher = fnv::FnvBuildHasher;

#[cfg(all(not(feature = "fnv"), not(feature = "sip13"), not(feature = "wyhash")))]
pub type BuildDefaultSeededHasher = Seedable<BuildHasherDefault<DefaultHasher>>;