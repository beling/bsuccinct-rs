`bitm` is a Rust library by Piotr Beling for bit and bitmap (bit vector) manipulation.

# Example

```rust
use bitm::{BitAccess, BitVec, Rank, ArrayWithRank101111};

let mut b = Box::<[u64]>::with_zeroed_bits(2048);    // b can store 2048 bits
assert_eq!(b.get_bit(100), false);  // b is zeroed so bit at index 100 is not set  
b.set_bit(100);                     // set the bit
assert_eq!(b.get_bit(100), true);   // now it is set
assert_eq!(b.get_bits(99, 5), 0b00010); // 5 bits, beginning from index 99, should be 00010

let (r, ones) = ArrayWithRank101111::build(b);
assert_eq!(ones, 1);        // one bit is set in b
assert_eq!(r.rank(100), 0); // no ones in the first 100 bits of b
assert_eq!(r.rank(101), 1); // one set bit in the first 101 bits of b
assert_eq!(r.rank(999), 1); // one set bit in the first 999 bits of b
```

# Features

The following [cargo features](https://doc.rust-lang.org/cargo/reference/features.html) are supported:

- `aligned-vec` – enables the implementation of the [`BitVec`](https://docs.rs/bitm/latest/bitm/trait.BitVec.html) trait
  for `aligned_vec::ABox<[u64]>` (with an arbitrary constant alignment), which allows constructing bit vectors aligned
  to the CPU cache line (or other boundary). Such vectors usually speed up rank and select queries
  (see [`RankSelect101111`](https://docs.rs/bitm/latest/bitm/struct.RankSelect101111.html)).
  It can be enabled, e.g., by adding `bitm = { version = "0.5", features = ["aligned-vec"] }` to the `[dependencies]`
  section of your `Cargo.toml`.

# Benchmarks
The performance of some of the structures included in `bitm` can be tested with the [cseq_benchmark](https://crates.io/crates/cseq_benchmark) crate. Its documentation contains benchmark results.
The crate also contains its own benchmarks (based on criterion and iai-callgrind), which can be run with `cargo bench`.

