`csf` is the Rust library (by Piotr Beling) of (compressed) static functions that use perfect hashing (and value compression).

The compressed static functions contained in `csf` represent immutable maps from a set of (hashable) keys *K* into a set of values *V*.
Since they do not explicitly store keys and compress values, their size usually slightly exceeds the entropy of the values alone.
They can quickly (usually in *O(1)* time) return the value assigned to a given key *k*. However, they are not always able to detect that *k* is not in *K*, and may for such *k* return an arbitrary value from *V*.

# Example

```rust
use csf::ls;

let subset: ls::Map = [("alpha", 1u8), ("beta", 0), ("gamma", 1)].as_ref().into();
assert_eq!(subset.get(&"alpha"), 1);
assert_eq!(subset.get(&"beta"), 0);
assert_eq!(subset.get(&"gamma"), 1);
// Any 1-bit value (either 0 or 1) can be returned for other arguments:
assert!(subset.get(&"other") <= 1);
```