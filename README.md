Succinct data structures and other Rust libraries and programs by Piotr Beling.

Included libraries:
- `ph` ([crate](https://crates.io/crates/ph), [doc](https://docs.rs/ph)) - data structures based on perfect hashing;
- `minimum_redundancy` ([crate](https://crates.io/crates/minimum_redundancy), [doc](https://docs.rs/minimum_redundancy)) -  encode and decode data
  with binary or non-binary Huffman coding;
- `fsum` ([crate](https://crates.io/crates/fsum), [doc](https://docs.rs/fsum)) - calculate accurate sum of floats;
- `bitm` ([crate](https://crates.io/crates/bitm), [doc](https://docs.rs/bitm)) - bit and bitmap manipulation;
- `binout` ([crate](https://crates.io/crates/binout), [doc](https://docs.rs/binout)) - binary encoding, decoding, serialization, deserialization;
- `dyn_size_of` ([crate](https://crates.io/crates/dyn_size_of), [doc](https://docs.rs/dyn_size_of)) - report approximate amount of memory consumed by variables, including the memory allocated on heap.

Included programs:
- `mphf_benchmark` ([crate](https://crates.io/crates/mphf_benchmark), [doc](https://docs.rs/mphf_benchmark)) - benchmarking Minimal Perfect Hash Functions.

# Installation
Programs can be compiled and installed from sources. To do this, a Rust compiler is needed.
The easiest way to obtain the compiler along with other necessary tools (like `cargo`) is
to use [rustup](https://www.rust-lang.org/tools/install).

## Installing rust toolchain
Please follow the instructions at https://www.rust-lang.org/tools/install.

## Installing rust programs
Once Rust is installed, to compile and install a program from sources, just execute:

```cargo install <program_name>```

for example

```cargo install mphf_benchmark```