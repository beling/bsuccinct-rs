`ph` is the Rust library (by Piotr Beling) of data structures based on perfect hashing.

The library contains an implementation of two variants of the *fingerprint-based minimal perfect hash function* (*FMPH* for short): without (*FMPH*, [`FPHash`]) and with (*FMPHGO*, [`FPHash2`]) group optimization.
(A minimal perfect hash function (MPHF) is a bijection from a key set *K* to the set *{0, 1, ..., |K|−1}*.)

FMPH and FMPHGO can be constructed for any set *K* (given in advance) of hashable items and represented using about *2.8* and *2.1* bits per key, respectively.
FMPH and FMPHGO are fast (*O(1)*) to evaluate. Their construction requires very little auxiliary memory, takes a short (*O(|K|)*) time (which especially true for FMPH) and, in addition, can be parallelized.

# Bibliography
When using `ph` for research purposes, please cite the following paper which provides details on FMPH and FMPHGO:

* Piotr Beling, *Fingerprinting-based minimal perfect hashing revisited*, ACM Journal of Experimental Algorithmics, 2023, <https://doi.org/10.1145/3596453>

# Example
```rust
use ph::FPHash;

let keys = ['a', 'b', 'z'];
let f = FPHash::from(&keys[..]);
// f assigns each key a unique number from the set {0, 1, 2}
for k in keys { println!("The key {} is assigned the value {}.", k, f.get(&k).unwrap()); }
let mut values = [f.get(&'a').unwrap(), f.get(&'b').unwrap(), f.get(&'z').unwrap()];
values.sort();
assert_eq!(values, [0, 1, 2]);
```