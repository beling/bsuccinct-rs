`ph` is the Rust library (by Piotr Beling) of (minimal) perfect hash functions.

A minimal perfect hash function (MPHF) is a bijection from a key set *K* to the set *{0, 1, ..., |K|−1}*.

The library contains implementations of:
- [PHast](`phast::Function`) -- bucket-placement based function with very fast evaluation and size below 2 bits/key,
- two variants of the *fingerprint-based minimal perfect hash function*:
without (*FMPH*, [`fmph::Function`]) and with (*FMPHGO*, [`fmph::GOFunction`]) group optimization.

All of these functions can be constructed for any set *K* (given in advance) of hashable items.

FMPH and FMPHGO can be represented using about *2.8* and *2.1* bits per key (regardless of key types), respectively.
FMPH and FMPHGO are quite fast (*O(1)* in expectation) to evaluate. Their construction requires very little auxiliary space, takes a short (*O(|K|)* in expectation) time (which is especially true for FMPH) and, in addition, can be parallelized or carried out without holding keys in memory.

The speed of our functions is affected by the hash algorithm used.
The default one can be selected via features, which are delegated to [seedable_hash crate](seedable_hash) and described in the [seedable_hash documentation](seedable_hash).
We recommend [GxHash](https://crates.io/crates/gxhash) (enabled by `gxhash` feature) on the platforms it supports.

# Bibliography
When using `ph` for research purposes, please cite the following paper which provides details on:
* PHast and PHast+:
  
  Piotr Beling, Peter Sanders, *PHast - Perfect Hashing made fast*, SIAM Symposium on Algorithm Engineering and Experiments (ALENEX26), 2026

  (its preprint is also [available on arXiv](https://arxiv.org/abs/2504.17918))

* FMPH and FMPHGO:

  Piotr Beling, *Fingerprinting-based minimal perfect hashing revisited*, ACM Journal of Experimental Algorithmics, 2023, <https://doi.org/10.1145/3596453>

# Examples
The following examples illustrate the use of [`fmph::Function`], which, however, can be replaced with [`fmph::GOFunction`] without any other changes.

A basic example:
```rust
use ph::fmph;

let keys = ['a', 'b', 'z'];
let f = fmph::Function::from(keys.as_ref());
// f assigns each key a unique number from the set {0, 1, 2}
for k in keys { println!("The key {} is assigned the value {}.", k, f.get(&k).unwrap()); }
let mut values = [f.get(&'a').unwrap(), f.get(&'b').unwrap(), f.get(&'z').unwrap()];
values.sort();
assert_eq!(values, [0, 1, 2]);
```

An example of using [`fmph::Function`] and bitmap to represent subsets of a given set of hashable elements:
```rust
use ph::fmph;
use bitm::{BitAccess, BitVec};  // bitm is used to manipulate bitmaps
use std::hash::Hash;

pub struct Subset { // represents a subset of the given set
    hash: fmph::Function, // bijectively maps elements of the set to bits of bitmap
    bitmap: Box<[u64]> // the bit pointed by the hash for e is 1 <=> e is in the subset
}

impl Subset {
    pub fn of<E: Hash + Sync>(set: &[E]) -> Self { // constructs empty subset of the given set
        Subset {
            hash: set.into(),
            bitmap: Box::with_zeroed_bits(set.len())
        }
    }

    pub fn contain<E: Hash>(&self, e: &E) -> bool { // checks if e is in the subset
        self.bitmap.get_bit(self.hash.get_or_panic(e) as usize) as bool
    }

    pub fn insert<E: Hash>(&mut self, e: &E) { // inserts e into the subset
        self.bitmap.set_bit(self.hash.get_or_panic(e) as usize)
    }

    pub fn remove<E: Hash>(&mut self, e: &E) { // removes e from the subset
        self.bitmap.clear_bit(self.hash.get_or_panic(e) as usize)
    }

    pub fn len(&self) -> usize { // returns the number of elements in the subset
        self.bitmap.count_bit_ones()
    }
}

let mut subset = Subset::of(["alpha", "beta", "gamma"].as_ref());
assert_eq!(subset.len(), 0);
assert!(!subset.contain(&"alpha"));
assert!(!subset.contain(&"beta"));
subset.insert(&"beta");
subset.insert(&"gamma");
assert_eq!(subset.len(), 2);
assert!(subset.contain(&"beta"));
subset.remove(&"beta");
assert_eq!(subset.len(), 1);
assert!(!subset.contain(&"beta"));
// subset.insert(&"zeta"); // may either panic or insert any item into subset
```

Above `Subset` is an example of an *updatable retrieval data structure* with a 1-bit payload.
It can be generalized by replacing the bitmap with a vector of other payload.