`ph` is the Rust library (by Piotr Beling) of data structures based on perfect hashing.

# Example
```rust
use ph::FPHash;

let keys = ['a', 'b', 'c', 'z'];
let f = FPHash::from(&keys[..]);
// f assigns each key a unique number from the set {0, 1, 2, 3}
let mut values: Vec<_> = keys.iter().map(|k| f.get(k).unwrap()).collect();
values.sort();
assert_eq!(values, [0, 1, 2, 3]);
```