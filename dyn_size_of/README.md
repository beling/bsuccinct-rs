`dyn_size_of` is the Rust library by Piotr Beling to report approximate amount of memory consumed by variables,
including the memory allocated on heap.

Please note that the library has very limited capabilities and is mainly developed for my other projects.

# Example

```rust
use dyn_size_of::GetSize;

let bs = vec![1u32, 2u32, 3u32].into_boxed_slice();
assert_eq!(bs.size_bytes_dyn(), 3*4);
assert_eq!(bs.size_bytes(), 3*4 + std::mem::size_of_val(&bs));
```
