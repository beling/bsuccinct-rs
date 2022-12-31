`mphf_benchmark` is the program (by Piotr Beling) for benchmarking Minimal Perfect Hash Functions.

It can test the algorithms included in the following creates:
- [ph](https://crates.io/crates/ph),
- [boomphf](https://crates.io/crates/boomphf),
- [cmph-sys](https://crates.io/crates/cmph-sys) (only if compiled with `cmph-sys` feature, and only *CHD* algorithm is supported).

Below you can find instructions for [installing](#installation) `mphf_benchmark` and
[reproducing experiments](#reproducing-experiments-from-the-papers) performed with it,
which can be found in published or under review papers.
Note that these instructions have been tested under Linux and may require some modifications for other systems.

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

# Reproducing experiments from the papers

## Fingerprinting-based minimal perfect hashing revisited

The results for FMPHGO wide range of parameters and 1,000,000 64-bit integer keys generated uniformly at random, can be calculated by:
```shell
mphf_benchmark -d -s xs64 -n 1000000 fmphgo_all
```

The results for FMPH, FMPHGO and boomphf, for 39,459,925 64-bit integer keys generated uniformly at random, can be calculated by:

```shell
mphf_benchmark -d -s xs64 -n 39459925 -b 30 -l 30 fmph
mphf_benchmark -d -s xs64 -n 39459925 -b 30 -l 30 fmph -c 0
mphf_benchmark -d -s xs64 -n 39459925 -b 30 -l 30 fmphgo
mphf_benchmark -d -s xs64 -n 39459925 -b 30 -l 30 fmphgo -c 0
mphf_benchmark -d -s xs64 -n 39459925 -b 30 -l 30 boomphf
```

Subsequent parts/flags of the above calls have the following meaning:
- `-d` causes accurate results to be written to a file (alternatively, you may not use this flag when the information printed on the screen is enough),
- `-s xs64` points to pseudo-random number generator XorShift64 as key source,
- `-n` specify the number of keys,
- `-b 30 -l 30`  indicate that both build times and average (over keys) lookup (evaluation) times are averaged over 30 runs,
- `-c 0` (given after the method name) disables hash caching in FMPH or FMPHGO.

We use similar calls for 500,000,000 keys, but with `-n 500000000` and `-b 5`.

To perform tests with a collection of .uk URLs collected in 2005 by UbiCrawler,
first download the [uk-2005-nat.urls.gz](http://data.law.di.unimi.it/webdata/uk-2005/uk-2005-nat.urls.gz)
file from https://law.di.unimi.it/webdata/uk-2005/ and unpack it:

```shell
gzip -d uk-2005-nat.urls.gz
```

And run (we use [cat](https://man7.org/linux/man-pages/man1/cat.1.html) to print the unpacked `uk-2005.urls` file to the standard output,
which we then redirect to `mphf_benchmark`):
```shell
cat uk-2005.urls | mphf_benchmark -d -s stdin -b 30 -l 30 fmph
cat uk-2005.urls | mphf_benchmark -d -s stdin -b 30 -l 30 fmph -c 0
cat uk-2005.urls | mphf_benchmark -d -s stdin -b 30 -l 30 fmphgo
cat uk-2005.urls | mphf_benchmark -d -s stdin -b 30 -l 30 fmphgo -c 0
cat uk-2005.urls | mphf_benchmark -d -s stdin -b 30 -l 30 boomphf
```

To measure memory consumption of construction processes,
we use the [memusage](https://man7.org/linux/man-pages/man1/memusage.1.html) profiler
and run methods with each set of parameters separately. For example, we run
```shell
memusage mphf_benchmark -s xs64 -n 39459925 -b 1 -l 0 -t single fmphgo -c 0 -s 1 -l 100
```
to measure memory consumption of single-threaded (`-t single`) construction
of FMPHGO s=1 (`-s 1`) b=8 (chosen according to s) $\gamma=1$ (`-l 100`) without caching hashes (`-c 0`).

To subtract the memory occupied by the keys and data unrelated to the construction process
(which are actually negligible), we also measure the memory consumption for the execution
of benchmark programs that terminates as soon as the keys are loaded or generated, for example:
```shell
memusage mphf_benchmark -s xs64 -n 39459925 none
```