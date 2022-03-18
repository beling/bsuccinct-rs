`dyn_size_of` is the Rust library by Piotr Beling to report approximate amount of memory consumed by variables,
including the memory allocated on heap.

# Examples

## Simple usage
```rust
use dyn_size_of::GetSize;

let bs = vec![1u32, 2u32, 3u32].into_boxed_slice();
assert_eq!(bs.size_bytes_dyn(), 3*4);
assert_eq!(bs.size_bytes(), 3*4 + std::mem::size_of_val(&bs));
```

## Implementing GetSize for a custom type
```rust
use dyn_size_of::GetSize;

struct NoHeapMem {
    a: u32,
    b: u8
}

// default implementation is fine for types that do not use heap allocations
impl GetSize for NoHeapMem {}

struct WithHeapMem {
    a: Vec<u32>,
    b: Vec<u8>,
    c: u32
}

// For types that use heap allocations:
impl GetSize for WithHeapMem {
    // size_bytes_dyn must be implemented and return amount of heap memory used
    fn size_bytes_dyn(&self) -> usize {
        self.a.size_bytes_dyn() + self.b.size_bytes_dyn()
    }
    // USES_DYN_MEM must be set to true
    const USES_DYN_MEM: bool = true;
}

let s = NoHeapMem { a: 1, b: 2 };
assert_eq!(NoHeapMem::USES_DYN_MEM, false);
assert_eq!(s.size_bytes_dyn(), 0);
assert_eq!(s.size_bytes(), std::mem::size_of_val(&s));

let d = WithHeapMem { a: vec![1, 2], b: vec![3, 4], c: 5 };
assert_eq!(WithHeapMem::USES_DYN_MEM, true);
assert_eq!(d.size_bytes_dyn(), 2*4 + 2*1);
assert_eq!(d.size_bytes(), 2*4 + 2*1 + std::mem::size_of_val(&d));
```