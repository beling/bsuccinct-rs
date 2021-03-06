#![doc = include_str!("../README.md")]

#![feature(atomic_from_mut)]
#![feature(hashmap_internals)]

pub mod utils;
pub mod stats;
pub mod seedable_hash;
pub use seedable_hash::{BuildSeededHasher, Seedable, BuildDefaultSeededHasher};

pub mod fp;
pub use fp::{FPHash, FPHashConf, FPHash2, FPHash2Conf};

pub use dyn_size_of::GetSize;