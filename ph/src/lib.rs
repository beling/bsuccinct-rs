#![doc = include_str!("../README.md")]

//#![feature(atomic_from_mut)]
#![cfg_attr(feature = "sip13", feature(hashmap_internals))]

pub mod utils;
pub mod stats;
pub mod fmph;
pub mod seeds;
pub mod phast;

pub use seedable_hash::{self, BuildSeededHasher, Seedable, BuildDefaultSeededHasher};
pub use dyn_size_of::GetSize;
