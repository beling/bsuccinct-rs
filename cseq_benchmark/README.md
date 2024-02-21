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

Results for structures that support rank and select queries on bit vectors,
included in libraries written in Rust, can be obtained by running:

```shell
cseq_benchmark -f -t 60 -q 10000000 -u 1000000000 -n 500000000 bv
cseq_benchmark -f -t 60 -q 10000000 -u 1000000000 -n 100000000 bv
```

Notes:
- The `-t 60` switch forces a long testing time (60s for warming up + about 60s for performing each test).
  It can be omitted to get results faster, but averaged over fewer repetitions.
- The `-f` switch causes the results to be written to files.
  It also can be skipped, as the results are printed to the screen anyway.

The results for the methods contained in [SDSL2](https://github.com/simongog/sdsl-lite) (which is written in C++)
can be obtained using the program available at <https://github.com/beling/benchmark-succinct>
(the page also contains compilation instructions) by running:

```shell
rank_sel 1000000000 500000000 60 10000000
rank_sel 1000000000 100000000 60 10000000
```
