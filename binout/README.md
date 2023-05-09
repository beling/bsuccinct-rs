`binout` is the Rust library by Piotr Beling for binary encoding,
decoding, serialization, deserialization of data.
It supports slightly improved vbyte format.

# Example
```rust
let input = 123;
let mut buffer = Vec::new();
binout::vbyte_write(&mut buffer, input).unwrap();
assert_eq!(binout::vbyte_len(input) as usize, buffer.len());
assert_eq!(binout::vbyte_read(&mut &buffer[..]).unwrap(), input);
```