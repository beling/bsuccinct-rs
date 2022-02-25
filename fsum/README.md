`fsum` is the Rust library by Piotr Beling to calculate accurate sum of floats.

# Example

```rust
use fsum::FSum;

assert_eq!(FSum::new().add(1e100).add(1.0).add(-1e100).value(), 1.0);
assert_eq!(FSum::with_all((0..10).map(|_| 0.1)).value(), 1.0);
assert_eq!(FSum::with_all(&[1e100, 1.0, -1e100]).value(), 1.0);

let mut s = FSum::new();
assert_eq!(s.value(), 0.0);
s += 3.0;
assert_eq!(s.value(), 3.0);
s -= 1.0;
assert_eq!(s.value(), 2.0);
```

# Complexity

The complexities of summing *n* numbers are:
- time: from *O(n)* (optimistic) to *O(n²)* (pessimistic)
- memory: from *O(1)* (optimistic) to *O(n)* (pessimistic)

Usually the complexities are close to optimistic.

# References

Calculation code bases on (is mostly copied from) `sum` method
of `test::stats::Stats` implementation for `f64`
(which probably reimplements `math.fsum` from Python's library)
and source of CPython.
See also:
- <https://github.com/python/cpython/blob/2b7411df5ca0b6ef714377730fd4d94693f26abd/Lib/test/test_math.py#L647>
- <https://bugs.python.org/file10357/msum4.py>
- <http://code.activestate.com/recipes/393090/>

The method sacrifices performance at the altar of accuracy
Depends on IEEE-754 arithmetic guarantees. See proof of the correctness in
[Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric
Predicates][paper]

[paper]: <http://www.cs.cmu.edu/~quake-papers/robust-arithmetic.ps>
