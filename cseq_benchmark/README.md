`cseq_benchmark` is the program (by Piotr Beling) for benchmarking compact sequences and bitmaps.

It can test the listed algorithms contained in the following crates:
- [cseq](https://crates.io/crates/cseq): Elias-Fano (experimental);
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
included in libraries written in Rust (we used rustc 1.75.0 to compile), can be obtained by running:

```shell
cseq_benchmark -f -t 60 -c 20 -q 10000000 -u 1000000000 -n 500000000 bv
cseq_benchmark -f -t 60 -c 20 -q 10000000 -u 1000000000 -n 100000000 bv
```

Notes:
- The `-t 60 -c 20` switches force a long testing time
  (60s for warming up + about 60s for performing each test + 20s cooling/sleeping between tests).
  It can be omitted to get results faster, but averaged over fewer repetitions.
- The `-f` switch causes the results to be written to files.
  It also can be skipped, as the results are printed to the screen anyway.

The results for the methods contained in [SDSL2](https://github.com/simongog/sdsl-lite)
(which is written in C++; we used clang 14.0.6 to compile)
can be obtained using the program available at <https://github.com/beling/benchmark-succinct>
(the page also contains compilation instructions) by running:

```shell
rank_sel 1000000000 500000000 60 10000000
rank_sel 1000000000 100000000 60 10000000
```

# Benchmark results
## Notes on benchmarks of structures supporting rank and select queries on bit vectors
- We do not distinguish *rank<sub>0</sub>* from *rank<sub>1</sub>* as each is trivially computable from the other.
- In a bit vector with *adversarial* distribution and *n* ones, 99% of them occupy the last *n* indices.
- Versions of the tested crates: *bitm* 0.4.1, *succinct* 0.5.2, *sucds* 0.8.1, *sux* 0.2.0, *vers* 1.1.0.
- Structures supporting select marked with * use or are integrated with (in the case of *vers RsVec*) the corresponding rank structure and the space overhead given for them is additional.
- We conducted the benchmarks using: long measurement (t=60) and cooling (c=20) times, 10<sup>7</sup> random query arguments, AMD Ryzen 5600G @3.9GHz CPU and compilation with native optimizations enabled.

## Rank for uniform distribution of ones in the bit vector
<table style="text-align: center;"><thead>
<tr><th>bit vector length</th><th colspan="4">10<sup>10</sup></th><th colspan="4">10<sup>9</sup></th><th colspan="4">10<sup>8</sup></th></tr>
<tr><th>percent of ones</th><th colspan="2">50</th><th colspan="2">10</th><th colspan="2">50</th><th colspan="2">10</th><th colspan="2">50</th><th colspan="2">10</th></tr>
<tr style="font-size: 0.75em;"><th></th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th></tr>
</thead><tbody>
<tr><th style="font-size: 0.75em;">bitm RankSelect101111</th><td>3.1</td><td>23</td><td>3.1</td><td>23</td><td>3.1</td><td>21</td><td>3.1</td><td>21</td><td>3.1</td><td>6</td><td>3.1</td><td>7</td></tr>
<tr><th style="font-size: 0.75em;">succinct JacobsonRank</th><td>22.8</td><td>52</td><td>22.8</td><td>52</td><td>18.8</td><td>45</td><td>18.8</td><td>44</td><td>22.7</td><td>12</td><td>22.7</td><td>13</td></tr>
<tr><th style="font-size: 0.75em;">succinct Rank9</th><td>25.0</td><td>19</td><td>25.0</td><td>19</td><td>25.0</td><td>18</td><td>25.0</td><td>17</td><td>25.0</td><td>7</td><td>25.0</td><td>7</td></tr>
<tr><th style="font-size: 0.75em;">sucds Rank9Sel</th><td>25.0</td><td>19</td><td>25.0</td><td>18</td><td>25.0</td><td>17</td><td>25.0</td><td>17</td><td>25.0</td><td>7</td><td>25.0</td><td>7</td></tr>
<tr><th style="font-size: 0.75em;">vers RsVec</th><td>4.7</td><td>26</td><td>5.3</td><td>26</td><td>4.7</td><td>24</td><td>5.3</td><td>24</td><td>4.7</td><td>6</td><td>5.3</td><td>6</td></tr>
</tbody></table>

## Select<sub>1</sub> for uniform distribution of ones in the bit vector
<table style="text-align: center;"><thead>
<tr><th>bit vector length</th><th colspan="4">10<sup>10</sup></th><th colspan="4">10<sup>9</sup></th><th colspan="4">10<sup>8</sup></th></tr>
<tr><th>percent of ones</th><th colspan="2">50</th><th colspan="2">10</th><th colspan="2">50</th><th colspan="2">10</th><th colspan="2">50</th><th colspan="2">10</th></tr>
<tr style="font-size: 0.75em;"><th></th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th></tr>
</thead><tbody>
<tr><th style="font-size: 0.75em;">bitm RankSelect101111 binary search*</th><td>0.0</td><td>395</td><td>0.0</td><td>396</td><td>0.0</td><td>177</td><td>0.0</td><td>178</td><td>0.0</td><td>87</td><td>0.0</td><td>87</td></tr>
<tr><th style="font-size: 0.75em;">bitm RankSelect101111 combined sampling*</th><td>0.4</td><td>91</td><td>0.3</td><td>92</td><td>0.4</td><td>60</td><td>0.3</td><td>61</td><td>0.4</td><td>29</td><td>0.3</td><td>31</td></tr>
<tr><th style="font-size: 0.75em;">succinct JacobsonRank*</th><td>0.0</td><td>1486</td><td>0.0</td><td>1449</td><td>0.0</td><td>931</td><td>0.0</td><td>931</td><td>0.0</td><td>436</td><td>0.0</td><td>437</td></tr>
<tr><th style="font-size: 0.75em;">succinct Rank9*</th><td>0.0</td><td>838</td><td>0.0</td><td>806</td><td>0.0</td><td>534</td><td>0.0</td><td>516</td><td>0.0</td><td>207</td><td>0.0</td><td>208</td></tr>
<tr><th style="font-size: 0.75em;">sucds Rank9Sel + hints*</th><td>3.1</td><td>168</td><td>0.6</td><td>144</td><td>3.1</td><td>107</td><td>0.6</td><td>109</td><td>3.1</td><td>33</td><td>0.6</td><td>41</td></tr>
<tr><th style="font-size: 0.75em;">sucds Rank9Sel*</th><td>0.0</td><td>511</td><td>0.0</td><td>448</td><td>0.0</td><td>249</td><td>0.0</td><td>252</td><td>0.0</td><td>103</td><td>0.0</td><td>105</td></tr>
<tr><th style="font-size: 0.75em;">sux SelectFixed1</th><td>12.5</td><td>71</td><td>2.5</td><td>204</td><td>12.5</td><td>50</td><td>2.5</td><td>138</td><td>12.5</td><td>15</td><td>2.5</td><td>34</td></tr>
<tr><th style="font-size: 0.75em;">sux SelectFixed2</th><td>15.6</td><td>39</td><td>3.1</td><td>90</td><td>15.6</td><td>33</td><td>3.1</td><td>55</td><td>15.6</td><td>10</td><td>3.1</td><td>19</td></tr>
<tr><th style="font-size: 0.75em;">vers RsVec*</th><td>0.0</td><td>151</td><td>0.0</td><td>159</td><td>0.0</td><td>82</td><td>0.0</td><td>101</td><td>0.0</td><td>32</td><td>0.0</td><td>38</td></tr>
</tbody></table>
* uses a corresponding structure supporting rank queries; space overhead is extra

## Select<sub>0</sub> for uniform distribution of ones in the bit vector
<table style="text-align: center;"><thead>
<tr><th>bit vector length</th><th colspan="4">10<sup>10</sup></th><th colspan="4">10<sup>9</sup></th><th colspan="4">10<sup>8</sup></th></tr>
<tr><th>percent of ones</th><th colspan="2">50</th><th colspan="2">10</th><th colspan="2">50</th><th colspan="2">10</th><th colspan="2">50</th><th colspan="2">10</th></tr>
<tr style="font-size: 0.75em;"><th></th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th></tr>
</thead><tbody>
<tr><th style="font-size: 0.75em;">bitm RankSelect101111 binary search*</th><td>0.0</td><td>417</td><td>0.0</td><td>416</td><td>0.0</td><td>193</td><td>0.0</td><td>195</td><td>0.0</td><td>89</td><td>0.0</td><td>90</td></tr>
<tr><th style="font-size: 0.75em;">bitm RankSelect101111 combined sampling*</th><td>0.4</td><td>120</td><td>0.4</td><td>122</td><td>0.4</td><td>77</td><td>0.4</td><td>77</td><td>0.4</td><td>32</td><td>0.4</td><td>33</td></tr>
<tr><th style="font-size: 0.75em;">succinct JacobsonRank*</th><td>0.0</td><td>1508</td><td>0.0</td><td>1451</td><td>0.0</td><td>979</td><td>0.0</td><td>975</td><td>0.0</td><td>452</td><td>0.0</td><td>454</td></tr>
<tr><th style="font-size: 0.75em;">succinct Rank9*</th><td>0.0</td><td>841</td><td>0.0</td><td>798</td><td>0.0</td><td>547</td><td>0.0</td><td>531</td><td>0.0</td><td>217</td><td>0.0</td><td>219</td></tr>
<tr><th style="font-size: 0.75em;">sucds Rank9Sel + hints*</th><td>3.1</td><td>170</td><td>5.6</td><td>165</td><td>3.1</td><td>108</td><td>5.6</td><td>112</td><td>3.1</td><td>35</td><td>5.6</td><td>34</td></tr>
<tr><th style="font-size: 0.75em;">sucds Rank9Sel*</th><td>0.0</td><td>534</td><td>0.0</td><td>456</td><td>0.0</td><td>272</td><td>0.0</td><td>273</td><td>0.0</td><td>107</td><td>0.0</td><td>109</td></tr>
<tr><th style="font-size: 0.75em;">sux SelectFixed1</th><td>12.5</td><td>93</td><td>22.5</td><td>61</td><td>12.5</td><td>62</td><td>22.5</td><td>50</td><td>12.5</td><td>17</td><td>22.5</td><td>15</td></tr>
<tr><th style="font-size: 0.75em;">sux SelectFixed2</th><td>15.6</td><td>41</td><td>28.1</td><td>37</td><td>15.6</td><td>34</td><td>28.1</td><td>34</td><td>15.6</td><td>10</td><td>28.1</td><td>12</td></tr>
<tr><th style="font-size: 0.75em;">vers RsVec*</th><td>0.0</td><td>138</td><td>0.0</td><td>146</td><td>0.0</td><td>74</td><td>0.0</td><td>72</td><td>0.0</td><td>30</td><td>0.0</td><td>30</td></tr>
</tbody></table>
* uses a corresponding structure supporting rank queries; space overhead is extra

## Rank for adversarial distribution with 99% of ones near the end of the vector
<table style="text-align: center;"><thead>
<tr><th>bit vector length</th><th colspan="4">10<sup>10</sup></th><th colspan="4">10<sup>9</sup></th><th colspan="4">10<sup>8</sup></th></tr>
<tr><th>percent of ones</th><th colspan="2">50</th><th colspan="2">10</th><th colspan="2">50</th><th colspan="2">10</th><th colspan="2">50</th><th colspan="2">10</th></tr>
<tr style="font-size: 0.75em;"><th></th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th></tr>
</thead><tbody>
<tr><th style="font-size: 0.75em;">bitm RankSelect101111</th><td>3.1</td><td>23</td><td>3.1</td><td>23</td><td>3.1</td><td>21</td><td>3.1</td><td>21</td><td>3.1</td><td>7</td><td>3.1</td><td>7</td></tr>
<tr><th style="font-size: 0.75em;">succinct JacobsonRank</th><td>22.8</td><td>52</td><td>22.8</td><td>52</td><td>18.8</td><td>44</td><td>18.8</td><td>44</td><td>22.7</td><td>12</td><td>22.7</td><td>12</td></tr>
<tr><th style="font-size: 0.75em;">succinct Rank9</th><td>25.0</td><td>19</td><td>25.0</td><td>18</td><td>25.0</td><td>17</td><td>25.0</td><td>17</td><td>25.0</td><td>6</td><td>25.0</td><td>7</td></tr>
<tr><th style="font-size: 0.75em;">sucds Rank9Sel</th><td>25.0</td><td>19</td><td>25.0</td><td>18</td><td>25.0</td><td>17</td><td>25.0</td><td>17</td><td>25.0</td><td>6</td><td>25.0</td><td>7</td></tr>
<tr><th style="font-size: 0.75em;">vers RsVec</th><td>4.7</td><td>26</td><td>5.3</td><td>26</td><td>4.7</td><td>24</td><td>5.3</td><td>24</td><td>4.7</td><td>6</td><td>5.3</td><td>6</td></tr>
</tbody></table>

## Select<sub>1</sub> for adversarial distribution with 99% of ones near the end of the vector
<table style="text-align: center;"><thead>
<tr><th>bit vector length</th><th colspan="4">10<sup>10</sup></th><th colspan="4">10<sup>9</sup></th><th colspan="4">10<sup>8</sup></th></tr>
<tr><th>percent of ones</th><th colspan="2">50</th><th colspan="2">10</th><th colspan="2">50</th><th colspan="2">10</th><th colspan="2">50</th><th colspan="2">10</th></tr>
<tr style="font-size: 0.75em;"><th></th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th></tr>
</thead><tbody>
<tr><th style="font-size: 0.75em;">bitm RankSelect101111 binary search*</th><td>0.0</td><td>296</td><td>0.0</td><td>187</td><td>0.0</td><td>166</td><td>0.0</td><td>97</td><td>0.0</td><td>82</td><td>0.0</td><td>72</td></tr>
<tr><th style="font-size: 0.75em;">bitm RankSelect101111 combined sampling*</th><td>0.4</td><td>82</td><td>0.3</td><td>65</td><td>0.4</td><td>58</td><td>0.3</td><td>33</td><td>0.4</td><td>28</td><td>0.3</td><td>25</td></tr>
<tr><th style="font-size: 0.75em;">succinct JacobsonRank*</th><td>0.0</td><td>1318</td><td>0.0</td><td>1045</td><td>0.0</td><td>819</td><td>0.0</td><td>477</td><td>0.0</td><td>415</td><td>0.0</td><td>369</td></tr>
<tr><th style="font-size: 0.75em;">succinct Rank9*</th><td>0.0</td><td>715</td><td>0.0</td><td>571</td><td>0.0</td><td>477</td><td>0.0</td><td>232</td><td>0.0</td><td>195</td><td>0.0</td><td>168</td></tr>
<tr><th style="font-size: 0.75em;">sucds Rank9Sel + hints*</th><td>3.1</td><td>159</td><td>0.6</td><td>115</td><td>3.1</td><td>97</td><td>0.6</td><td>46</td><td>3.1</td><td>26</td><td>0.6</td><td>21</td></tr>
<tr><th style="font-size: 0.75em;">sucds Rank9Sel*</th><td>0.0</td><td>435</td><td>0.0</td><td>278</td><td>0.0</td><td>195</td><td>0.0</td><td>127</td><td>0.0</td><td>93</td><td>0.0</td><td>73</td></tr>
<tr><th style="font-size: 0.75em;">sux SelectFixed1</th><td>12.5</td><td>51</td><td>2.5</td><td>50</td><td>12.5</td><td>39</td><td>2.5</td><td>31</td><td>12.5</td><td>12</td><td>2.5</td><td>16</td></tr>
<tr><th style="font-size: 0.75em;">sux SelectFixed2</th><td>15.6</td><td>37</td><td>3.1</td><td>41</td><td>15.6</td><td>33</td><td>3.1</td><td>32</td><td>15.6</td><td>10</td><td>3.1</td><td>13</td></tr>
<tr><th style="font-size: 0.75em;">vers RsVec*</th><td>0.0</td><td>142</td><td>0.0</td><td>87</td><td>0.0</td><td>77</td><td>0.0</td><td>39</td><td>0.0</td><td>31</td><td>0.0</td><td>30</td></tr>
</tbody></table>
* uses a corresponding structure supporting rank queries; space overhead is extra

## Select<sub>0</sub> for adversarial distribution with 99% of ones near the end of the vector
<table style="text-align: center;"><thead>
<tr><th>bit vector length</th><th colspan="4">10<sup>10</sup></th><th colspan="4">10<sup>9</sup></th><th colspan="4">10<sup>8</sup></th></tr>
<tr><th>percent of ones</th><th colspan="2">50</th><th colspan="2">10</th><th colspan="2">50</th><th colspan="2">10</th><th colspan="2">50</th><th colspan="2">10</th></tr>
<tr style="font-size: 0.75em;"><th></th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th><th>space overhead [%]</th><th>time / query [ns]</th></tr>
</thead><tbody>
<tr><th style="font-size: 0.75em;">bitm RankSelect101111 binary search*</th><td>0.0</td><td>335</td><td>0.0</td><td>418</td><td>0.0</td><td>182</td><td>0.0</td><td>192</td><td>0.0</td><td>84</td><td>0.0</td><td>88</td></tr>
<tr><th style="font-size: 0.75em;">bitm RankSelect101111 combined sampling*</th><td>0.4</td><td>107</td><td>0.4</td><td>118</td><td>0.4</td><td>74</td><td>0.4</td><td>76</td><td>0.4</td><td>31</td><td>0.4</td><td>32</td></tr>
<tr><th style="font-size: 0.75em;">succinct JacobsonRank*</th><td>0.0</td><td>1391</td><td>0.0</td><td>1427</td><td>0.0</td><td>894</td><td>0.0</td><td>963</td><td>0.0</td><td>437</td><td>0.0</td><td>449</td></tr>
<tr><th style="font-size: 0.75em;">succinct Rank9*</th><td>0.0</td><td>737</td><td>0.0</td><td>808</td><td>0.0</td><td>490</td><td>0.0</td><td>519</td><td>0.0</td><td>207</td><td>0.0</td><td>216</td></tr>
<tr><th style="font-size: 0.75em;">sucds Rank9Sel + hints*</th><td>3.1</td><td>160</td><td>5.6</td><td>164</td><td>3.1</td><td>98</td><td>5.6</td><td>110</td><td>3.1</td><td>27</td><td>5.6</td><td>30</td></tr>
<tr><th style="font-size: 0.75em;">sucds Rank9Sel*</th><td>0.0</td><td>415</td><td>0.0</td><td>450</td><td>0.0</td><td>220</td><td>0.0</td><td>264</td><td>0.0</td><td>96</td><td>0.0</td><td>105</td></tr>
<tr><th style="font-size: 0.75em;">sux SelectFixed1</th><td>12.5</td><td>56</td><td>22.5</td><td>54</td><td>12.5</td><td>43</td><td>22.5</td><td>46</td><td>12.5</td><td>13</td><td>22.5</td><td>14</td></tr>
<tr><th style="font-size: 0.75em;">sux SelectFixed2</th><td>15.6</td><td>39</td><td>28.1</td><td>36</td><td>15.6</td><td>34</td><td>28.1</td><td>34</td><td>15.6</td><td>10</td><td>28.1</td><td>13</td></tr>
<tr><th style="font-size: 0.75em;">vers RsVec*</th><td>0.0</td><td>132</td><td>0.0</td><td>146</td><td>0.0</td><td>70</td><td>0.0</td><td>71</td><td>0.0</td><td>30</td><td>0.0</td><td>31</td></tr>
</tbody></table>
* uses a corresponding structure supporting rank queries; space overhead is extra