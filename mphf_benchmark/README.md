`mphf_benchmark` is the program (by Piotr Beling) for benchmarking Minimal Perfect Hash Functions.

It can test the algorithms included in the following crates:
- [ph](https://crates.io/crates/ph) (FMPH and FMPHGO are available only with `fmph` feature),
- [boomphf](https://crates.io/crates/boomphf) (only if compiled with `boomphf` feature),
- [cmph-sys](https://crates.io/crates/cmph-sys) (only if compiled with `cmph-sys` feature, and only *CHD* algorithm is supported),
- [ptr_hash](https://crates.io/crates/ptr_hash) (only if compiled with `ptr_hash` feature).

Please run the program with the `--help` switch to see the available options.

The availability of some options depends on the activation of the following features:
- *fmph-key-access* - allows a choice of multiple methods of accessing keys by FMPH(GO).

Below you can find instructions for [installing](#installation) `mphf_benchmark` and
[reproducing experiments](#reproducing-experiments-from-the-papers) performed with it,
which can be found in published or under review papers.
Note that these instructions have been tested under GNU/Linux and may require some modifications for other systems.

# Installation
`mphf_benchmark` can be compiled and installed from sources. To do this, a Rust compiler is needed.
The easiest way to obtain the compiler along with other necessary tools (like `cargo`) is
to use [rustup](https://www.rust-lang.org/tools/install).

Please follow the instructions at <https://www.rust-lang.org/tools/install>.

Once Rust is installed, just execute the following to install `mphf_benchmark` with native optimizations:

```RUSTFLAGS="-C target-cpu=native" cargo install mphf_benchmark```

# Reproducing experiments from the papers

## Fingerprinting-based minimal perfect hashing revisited
(Piotr Beling, *Fingerprinting-based minimal perfect hashing revisited*, ACM Journal of Experimental Algorithmics, 2023, DOI: <https://doi.org/10.1145/3596453>)

The results for FMPHGO with wide range of parameters and 100,000,000 64-bit integer keys generated uniformly at random,
can be calculated by:

```shell
mphf_benchmark -d -s xs64 -n 100000000 fmphgo-all
```

The results for FMPH, FMPHGO and boomphf, for 39,459,925 64-bit integer keys generated uniformly at random,
can be calculated by:

```shell
mphf_benchmark -d -s xs64 -n 39459925 -b 30 -l 30 fmph
mphf_benchmark -d -s xs64 -n 39459925 -b 30 -l 30 fmph -c 0
mphf_benchmark -d -s xs64 -n 39459925 -b 30 -l 30 fmphgo
mphf_benchmark -d -s xs64 -n 39459925 -b 30 -l 30 fmphgo -c 0
mphf_benchmark -d -s xs64 -n 39459925 -b 30 -l 30 boomphf
```

Subsequent parts/flags of the above calls have the following meaning:
- `-d` causes accurate results to be written to a file (alternatively, you may not use this flag when the information printed on the screen is enough),
- `-s xs64` points to pseudo-random number generator [XorShift64](https://doi.org/10.18637%2Fjss.v008.i14) as key source,
- `-n` specify the number of keys,
- `-b 30 -l 30`  indicate that both build times and average (over keys) lookup (evaluation) times are averaged over 30 runs,
- `-c 0` (given after the method name) disables hash caching in FMPH or FMPHGO.

We use similar calls for 500,000,000 keys, but with `-n 500000000` and `-b 5`.

Please run

```shell
mphf_benchmark --help
```

to see the options available for all methods or

```
mphf_benchmark help <method name>
```

(e.g., `mphf_benchmark help fmph`) to see the method-specific options given after the method name.

To perform tests with a collection of *.uk* URLs collected in 2005 by UbiCrawler,
first download the [uk-2005-nat.urls.gz](http://data.law.di.unimi.it/webdata/uk-2005/uk-2005-nat.urls.gz)
file from <https://law.di.unimi.it/webdata/uk-2005/> and unpack it:

```shell
gzip -d uk-2005-nat.urls.gz
```

Then run (we use [cat](https://man7.org/linux/man-pages/man1/cat.1.html) to print the unpacked `uk-nat-2005.urls` file
to the standard output, which we then redirect to `mphf_benchmark`):
```shell
cat uk-nat-2005.urls | mphf_benchmark -d -s stdin -n 39459925 -b 30 -l 30 fmph
cat uk-nat-2005.urls | mphf_benchmark -d -s stdin -n 39459925 -b 30 -l 30 fmph -c 0
cat uk-nat-2005.urls | mphf_benchmark -d -s stdin -n 39459925 -b 30 -l 30 fmphgo
cat uk-nat-2005.urls | mphf_benchmark -d -s stdin -n 39459925 -b 30 -l 30 fmphgo -c 0
cat uk-nat-2005.urls | mphf_benchmark -d -s stdin -n 39459925 -b 30 -l 30 boomphf
```

To measure memory consumption of construction processes,
we use the [memusage](https://man7.org/linux/man-pages/man1/memusage.1.html) profiler
and run methods with each set of parameters separately. For example, we run

```shell
memusage mphf_benchmark -s xs64 -n 39459925 -b 1 -l 0 -t single fmphgo -c 0 -s 1 -l 100
```

to measure memory consumption of single-threaded (`-t single`) construction
of FMPHGO s=1 (`-s 1`) b=8 (chosen according to s) $\gamma$=1 (`-l 100`) without caching hashes (`-c 0`).

To use `cat` and `memusage` simultaneously (which is non-trivial),
we create a bash script `mphf_uk2005.sh` with the following content:

```shell
cat uk-2005-nat.urls | mphf_benchmark "$@"
```

And we run it like this, for example:

```shell
memusage mphf_uk2005.sh -s stdin -n 39459925 -b 1 -l 0 -t single fmphgo -c 0 -s 1 -l 100
```

To subtract the memory occupied by the keys and data unrelated to the construction process
(which are actually negligible), we also measure the memory consumption for the execution
of benchmark programs that terminates as soon as the keys are loaded or generated:

```shell
memusage mphf_benchmark -s xs64 -n 39459925 none
memusage mphf_benchmark -s xs64 -n 500000000 none
memusage mphf_uk2005.sh -s stdin -n 39459925 none
```

To benchmark RecSplit, PTHash, and CHD, we use another program with the same name (`mphf_benchmark`),
but written in C++ by Giulio Ermanno Pibiri and Roberto Trani, and available at
<https://github.com/roberto-trani/mphf_benchmark> (this site also contains compilation instructions; it is built with native optimizations by default).
The authors have accepted our modifications to their program, which, among other things,
ensure that both benchmark programs can generate exactly the same keys for testing MPHFs.

For example, the results for RecSplit, PTHash, and CHD,
for 39,459,925 64-bit integer keys generated uniformly at random with *XorShift64*,
and with averaging over 30 runs, can be calculated by:

```shell
mphf_benchmark recsplit -n 39459925 --gen xs64 --num_construction_runs 30 --num_lookup_runs 30
mphf_benchmark pthash -n 39459925 --gen xs64 --num_construction_runs 30 --num_lookup_runs 30
mphf_benchmark chd -n 39459925 --gen xs64 --num_construction_runs 30 --num_lookup_runs 30
```

And similar calls, but using URLs from the *uk-2005* collection as keys looks like this:

```shell
cat uk-2005-nat.urls | mphf_benchmark recsplit -n 39459925 --gen stdin --num_construction_runs 30 --num_lookup_runs 30
cat uk-2005-nat.urls | mphf_benchmark pthash -n 39459925 --gen stdin --num_construction_runs 30 --num_lookup_runs 30
cat uk-2005-nat.urls | mphf_benchmark chd -n 39459925 --gen stdin --num_construction_runs 30 --num_lookup_runs 30
```

Again, measuring memory consumption with `memusage` requires running one variant at a time,
which can be achieved with the `--variant` switch, whose argument is the number of subsequent method variant to test.
For example, measuring the first variant of RecSplit might look like this
(note that for the *uk-2005* collection, we use the `mphf_uk2005.sh` script again):

```shell
memusage mphf_benchmark recsplit --variant 1 -n 39459925 --gen xs64 --num_construction_runs 1 --num_lookup_runs 0
memusage mphf_benchmark recsplit --variant 1 -n 500000000 --gen xs64 --num_construction_runs 1 --num_lookup_runs 0
memusage mphf_uk2005.sh recsplit --variant 1 -n 39459925 --gen stdin --num_construction_runs 1 --num_lookup_runs 0
```

A non-existent number (e.g., 9) of the variant can be given to measure the memory consumption
for the execution of benchmark programs that terminates as soon as the keys are loaded or generated:

```shell
memusage mphf_benchmark recsplit --variant 9 -n 39459925 --gen xs64
memusage mphf_benchmark recsplit --variant 9 -n 500000000 --gen xs64
memusage mphf_uk2005.sh recsplit --variant 9 -n 39459925 --gen stdin
```