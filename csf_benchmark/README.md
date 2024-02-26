`csf_benchmark` is the program (by Piotr Beling) for benchmarking Compressed Static Functions.

It can test the algorithms included in the following crates:
- [csf](https://crates.io/crates/csf).

The [csf documentation](https://docs.rs/csf/) contains plots created using `csf_benchmark`.

Please run the program with the `--help` switch to see the available options.

Below you can find instruction for [installing](#installation) `csf_benchmark`.


# Installation
`csf_benchmark` can be compiled and installed from sources. To do this, a Rust compiler is needed.
The easiest way to obtain the compiler along with other necessary tools (like `cargo`) is
to use [rustup](https://www.rust-lang.org/tools/install).

Please follow the instructions at <https://www.rust-lang.org/tools/install>.

Once Rust is installed, just execute the following to install `csf_benchmark` with native optimizations:

```RUSTFLAGS="-C target-cpu=native" cargo install csf_benchmark```

Note that the instruction have been tested under GNU/Linux and may require some modifications for other systems.