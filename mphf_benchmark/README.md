`mphf_benchmark` is the program (by Piotr Beling) for benchmarking Minimal Perfect Hash Functions.

It can test the algorithms included in the following creates:
- [ph](https://crates.io/crates/ph),
- [boomphf](https://crates.io/crates/boomphf),
- [cmph-sys](https://crates.io/crates/cmph-sys) (only if compiled with `cmph-sys` feature, and only *CHD* algorithm is supported).

# Installation
`mphf_benchmark` can be compiled and installed from sources. To do this, a Rust compiler is needed.
The easiest way to obtain the compiler along with other necessary tools (like `cargo`) is
to use [rustup](https://www.rust-lang.org/tools/install).

Please follow the instructions at https://www.rust-lang.org/tools/install and
(as `mphf_benchmark` requires Rust Nightly) select *"Customize installation"* and
*"nightly"* as *"Default toolchain"* (for all other options the default values can be left).

In case Rust stable is already installed on the computer, it can be switched to nightly by executing:

```rustup default nightly```

Once Rust Nightly is installed, just execute the following to install `mphf_benchmark`:

```cargo install mphf_benchmark```