Succinct data structures and other Rust libraries and programs by Piotr Beling.

[![Build Status](https://img.shields.io/github/actions/workflow/status/beling/bsuccinct-rs/rust.yml?style=flat-square)](https://github.com/beling/bsuccinct-rs/actions/)
[![](https://tokei.rs/b1/github/beling/bsuccinct-rs?type=Rust,Python&style=flat-square)](https://github.com/beling/bsuccinct-rs)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE-MIT)

Included libraries:
- `ph` ([crate](https://crates.io/crates/ph), [doc](https://docs.rs/ph)) - minimal perfect hash functions (FMPH and FMPHGO);
- `csf` ([crate](https://crates.io/crates/csf), [doc](https://docs.rs/csf)) - compressed static functions (maps);
- `cseq` ([crate](https://crates.io/crates/cseq), [doc](https://docs.rs/cseq)) - compact sequences (new and not well tested yet);
- `minimum_redundancy` ([crate](https://crates.io/crates/minimum_redundancy), [doc](https://docs.rs/minimum_redundancy)) - encode and decode data with binary or non-binary Huffman coding;
- `fsum` ([crate](https://crates.io/crates/fsum), [doc](https://docs.rs/fsum)) - calculate accurate sum of floats;
- `bitm` ([crate](https://crates.io/crates/bitm), [doc](https://docs.rs/bitm)) - bit and bitmap manipulation;
- `binout` ([crate](https://crates.io/crates/binout), [doc](https://docs.rs/binout)) - binary encoding, decoding, serialization, deserialization;
- `dyn_size_of` ([crate](https://crates.io/crates/dyn_size_of), [doc](https://docs.rs/dyn_size_of)) - report approximate amount of memory consumed by variables, including the memory allocated on heap,
- `butils` ([crate](https://crates.io/crates/butils), [doc](https://docs.rs/butils)) - (internal) utilities shared by software included in BSuccinct.

Included programs:
- `mphf_benchmark` ([crate](https://crates.io/crates/mphf_benchmark), [doc](https://docs.rs/mphf_benchmark)) - benchmarking minimal perfect hash functions,
- `csf_benchmark` ([crate](https://crates.io/crates/csf_benchmark), [doc](https://docs.rs/csf_benchmark)) - benchmarking compressed static functions,
- `cseq_benchmark` ([crate](https://crates.io/crates/cseq_benchmark), [doc](https://docs.rs/cseq_benchmark)) - benchmarking compact sequences,
- `coding_benchmark` ([crate](https://crates.io/crates/coding_benchmark), [doc](https://docs.rs/coding_benchmark)) - benchmarking Huffman coding crates.

Everything is dual-licensed under [Apache 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT).

# Installation
Programs can be compiled and installed from sources. To do this, a Rust compiler is needed.
The easiest way to obtain the compiler along with other necessary tools (like `cargo`) is
to use [rustup](https://www.rust-lang.org/tools/install).

Please follow the instructions at https://www.rust-lang.org/tools/install.

## Installing rust programs
Once Rust is installed, to compile and install a program from sources and with native optimizations, just execute:

```RUSTFLAGS="-C target-cpu=native" cargo install <program_name>```

for example

```RUSTFLAGS="-C target-cpu=native" cargo install mphf_benchmark```
