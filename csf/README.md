`csf` is the Rust library (by Piotr Beling) of (compressed) static functions that use perfect hashing (and value compression).

`csf` contains the following types that implement static functions: [`ls::Map`], [`fp::Map`].
They can represent functions (immutable maps) from a set of (hashable) keys to unsigned integers of given bit-size.
They take somewhat more than *nb* bits to represent a function from an *n*-element set into a set of *b*-bit values.
The expected time complexities of their construction and evaluation are *O(n)* and *O(1)*, respectively.

`csf` contains the following types that implement compressed static functions: [`ls::CMap`], [`fp::CMap`], [`fp::GOCMap`].
They can represent functions (immutable maps) from a set of (hashable) keys to a set of values of any type.
To represent a function *f:X→Y*, they use the space slightly larger than *|X|H*,
where *H* is the entropy of the distribution of the *f* values over *X*.
Their expected time complexity is *O(c)* for evaluation and *O(|X|c)* for construction
(not counting building the encoding dictionary),
where *c* is the average codeword length (given in code fragments) of the values.

None of the static functions (including the compressed ones) included in `csf` explicitly store keys.
Therefore, these functions are usually unable to detect whether an item
belongs to the set of keys for which they were constructed.
Thus, queried for a value assigned to an item outside that set, they can return any value.

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