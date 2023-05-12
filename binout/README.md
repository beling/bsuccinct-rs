`binout` is the Rust library by Piotr Beling for low-level, portable, bytes-oriented,
binary encoding, decoding, serialization, deserialization of integers and arrays of integers.

It supports slightly improved *VByte*/*LEB128* format as well as simple, little-endian, as-is (little-endian) serialization.

# Example
```rust
use binout::{VByte, Serializer};

let value = 123456u64;
let mut buff = Vec::new();
assert!(VByte::write(&mut buff, value).is_ok());
assert_eq!(buff.len(), VByte::size(value));
let read: u64 = VByte::read(&mut &buff[..]).unwrap();
assert_eq!(read, value);
```