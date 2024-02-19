`cseq_benchmark` is the program (by Piotr Beling) for benchmarking compact sequences and bitmaps.

It can test the listed algorithms contained in the following crates:
- [cseq](https://crates.io/crates/cseq): Elias-Fano;
- [bitm](https://crates.io/crates/bitm): rank and select queries on bit vectors;
- [sucds](https://crates.io/crates/sucds): rank and select queries on bit vectors;
- [succinct](https://crates.io/crates/succinct): rank and select queries on bit vectors;
- [sux](https://crates.io/crates/sux): select queries on bit vectors;
- [vers](https://crates.io/crates/vers-vecs) (only if compiled with `vers-vecs` feature): rank and select queries on bit vectors.

Please run the program with the `--help` switch to see the available options.

Below you can find instruction for [installing](#installation) `cseq_benchmark` and
[reproducing experiments](#reproducing-experiments-from-the-papers) performed with it,
which can be found in published or under review papers.
Note that these instructions have been tested under GNU/Linux and may require some modifications for other systems.


# Installation
`cseq_benchmark` can be compiled and installed from sources. To do this, a Rust compiler is needed.
The easiest way to obtain the compiler along with other necessary tools (like `cargo`) is
to use [rustup](https://www.rust-lang.org/tools/install).

Please follow the instructions at <https://www.rust-lang.org/tools/install>.

Once Rust is installed, just execute the following to install `cseq_benchmark` with native optimizations:

```RUSTFLAGS="-C target-cpu=native" cargo install --features=vers-vecs cseq_benchmark```

The `--features=vers-vecs` flag enables compilation of the non-portable [vers](https://crates.io/crates/vers-vecs) crate.
It should be omitted in case of compilation problems.


# Reproducing experiments from the papers

## Rust libraries and programs focused on succinct data structures
(Piotr Beling *Rust libraries and programs focused on succinct data structures* submitted to SoftwareX)