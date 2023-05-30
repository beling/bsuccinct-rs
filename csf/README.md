`csf` is the Rust library (by Piotr Beling) of compressed static functions (maps) that use perfect hashing and value compression.

The compressed static functions contained in `csf` represent immutable maps from a set of (hashable) keys *K* into a set of values *V*.
Since they do not explicitly store keys and compress values, their size usually slightly exceeds the entropy of the values alone.
They can quickly (usually in *O(1)* time) return the value assigned to a given key *k*. However, they are not always able to detect that *k* is not in *K*, and may for such *k* return an arbitrary value from *V*.