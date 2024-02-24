`bitm` is the Rust library by Piotr Beling for bit and bitmap (bit vector) manipulation.

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
assert_eq!(r.rank(101), 1); // 1 one in the first 101 bits of b
assert_eq!(r.rank(999), 1); // 1 one in the first 999 bits of b
```

# Benchmarks
The performance of some of the structures included in `bitm` can be tested with the [cseq_benchmark](https://crates.io/crates/cseq_benchmark) crate. Its [documentation](https://docs.rs/crate/cseq_benchmark/) contains benchmark results.

