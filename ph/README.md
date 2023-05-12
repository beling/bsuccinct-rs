`ph` is the Rust library (by Piotr Beling) of data structures based on perfect hashing.

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