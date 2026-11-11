`phast` is an internal BSuccinct program (by Piotr Beling) that helps develop the PHast algorithm.
It can run different versions of PHast, measure execution times, and numerically optimize parameters.

## Rescaling the weight tables (`scale_weights.py`)

`scale_weights.py` multiplies every number of every 7-element array literal (i.e. a bucket-weight table, see `Weights` in `ph/src/phast/evaluator.rs`) in `ph/src/phast` by the given factor (4 by default), preserving the formatting. It is meant for changing the per-bucket-number penalty in `Weights::eval` from `1024` to `4096`: multiplying both the weights and the penalty by the same factor keeps the evaluation order unchanged while giving the weights more resolution. The direct-weight bounds in `internal/phast/src/optim.rs` have to be scaled as well. The script is not idempotent, so revert unwanted changes with `git checkout -- ph/src/phast`.

```sh
./internal/phast/scale_weights.py --dry-run   # report only, do not write
./internal/phast/scale_weights.py             # multiply every weight by 4
```
