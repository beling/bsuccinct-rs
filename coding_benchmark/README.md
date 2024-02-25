`coding_benchmark` is the program (by Piotr Beling) for benchmarking implementations of Huffman coding algorithms.

It can test the implementations included in the following crates:
- [minimum_redundancy](https://crates.io/crates/minimum_redudancy),
- [constriction](https://crates.io/crates/constriction),
- [huffman-compress](https://crates.io/crates/huffman-compress).

Please run the program with the `--help` switch to see the available options.

Below you can find instruction for [installing](#installation) `coding_benchmark` and
[reproducing experiments](#reproducing-experiments-from-the-papers) performed with it,
which can be found in published or under review papers.
Note that these instructions have been tested under GNU/Linux and may require some modifications for other systems.


# Installation
`coding_benchmark` can be compiled and installed from sources. To do this, a Rust compiler is needed.
The easiest way to obtain the compiler along with other necessary tools (like `cargo`) is
to use [rustup](https://www.rust-lang.org/tools/install).

Please follow the instructions at <https://www.rust-lang.org/tools/install>.

Once Rust is installed, just execute the following to install `coding_benchmark` with native optimizations:

```RUSTFLAGS="-C target-cpu=native" cargo install coding_benchmark```


# Reproducing experiments from the papers

## Rust libraries and programs focused on succinct data structures
(Piotr Beling *Rust libraries and programs focused on succinct data structures* submitted to SoftwareX)

To see the results for all implementations, 100MB (100\*1024\*1024=104857600)
text of randomly drawn (with a non-uniform distribution) 1 byte symbols
(with an entropy of 4.83 bits/symbol), just execute:

```shell
./coding_benchmark -t 100 -c 20 -l 104857600 all
```

Note that the `-t 100 -c 20` switches force a long testing time
(100s for warming up + about 100s for performing each test + 20s cooling/sleeping between tests).
They can be omitted to get results faster, but averaged over fewer repetitions.