`seedable_hash` is the Rust library (by Piotr Beling) for calculating seedable hashes and fast reduction of their ranges.

`seedable_hash` conditionally supports hash functions from many crates:
- [GxHash](https://crates.io/crates/gxhash) - enabled by `gxhash` feature,
- [wyhash](https://crates.io/crates/wyhash) - enabled by (default) `wyhash` feature,
- [xxh3](https://crates.io/crates/xxhash-rust) - enabled by `xxhash-rust` feature,
- [rapidhash](https://crates.io/crates/rapidhash) - enabled by `rapidhash` feature,
- Sip13 using unstable standard library feature `hashmap_internals` - enabled by `sip13` feature,
- [Fowler–Noll–Vo](https://crates.io/crates/fnv) - enabled by `fnv` feature,
- standard `hash_map::DefaultHasher` via [`Seedable`] wrapper - always enabled,
- and others via [`Seedable`] wrapper.

[`BuildDefaultSeededHasher`] is an alias to the fastest of the enabled methods, selected according to the order of the above list.

We recommend [GxHash](https://crates.io/crates/gxhash) (`gxhash` feature) on the platforms it supports.

For hashing integers, we recommend [Fx Hash](https://crates.io/crates/fxhash) wrapped by [`Seedable`].