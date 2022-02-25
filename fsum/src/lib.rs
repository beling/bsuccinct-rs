#![doc = include_str!("../README.md")]

use std::mem;
use std::ops::{AddAssign, SubAssign};
use std::borrow::Borrow;

/// Accumulator that represents the exact sum of `f64` values and
/// allows additional `f64` values to be added without loss of precision.
#[derive(Default, Clone, Debug)]
pub struct FSum {
    partials: Vec<f64>
}

impl FSum {

    /// Constructs zeroed accumulator.
    ///
    /// # Example
    ///
    /// ```
    /// use fsum::FSum;
    ///
    /// assert_eq!(FSum::new().value(), 0.0);
    /// ```
    pub fn new() -> FSum {
        FSum{ partials: Vec::new() }
    }

    /// Constructs accumulator with given `initial_value`.
    ///
    /// # Example
    ///
    /// ```
    /// use fsum::FSum;
    ///
    /// assert_eq!(FSum::with_value(5.0).value(), 5.0);
    /// ```
    pub fn with_value(initial_value: f64) -> FSum {
        FSum{ partials: vec![initial_value] }
    }

    /// Constructs accumulator with all values from `iter`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fsum::FSum;
    ///
    /// assert_eq!(FSum::with_all((0..10).map(|_| 0.1)).value(), 1.0);
    /// ```
    pub fn with_all<Bf64, InIter>(values: InIter) -> FSum
        where Bf64: Borrow<f64>, InIter: IntoIterator<Item=Bf64>
    {
        let mut result = Self::new();
        result.add_all(values);
        result
    }

    /// Increases the sum by `x` and returns `self`.
    ///
    /// # Example
    ///
    /// ```
    /// use fsum::FSum;
    ///
    /// let mut s = FSum::new();
    /// assert_eq!(s.value(), 0.0);
    /// s.add(1.0);
    /// assert_eq!(s.value(), 1.0);
    /// s.add(2.0);
    /// assert_eq!(s.value(), 3.0);
    /// assert_eq!(s.add(5.0).value(), 8.0);
    /// ```
    ///
    /// # Complexity
    ///
    /// The complexities are:
    /// - time: from *O(1)* (optimistic) to *O(n)* (pessimistic), where *n* is the number of values added so far,
    /// - memory: *O(1)*, but internal vector stored in `self` can be increased by 1 element.
    ///
    /// Usually the time complexity is close to optimistic.
    pub fn add(&mut self, mut x: f64) -> &mut FSum {
        // https://github.com/python/cpython/blob/master/Modules/mathmodule.c#L1323
        let mut j = 0usize;
        // This inner loop applies `hi`/`lo` summation to each
        // partial so that the list of partial sums remains exact.
        for i in 0..self.partials.len() {
            let mut y: f64 = self.partials[i];
            if x.abs() < y.abs() { mem::swap(&mut x, &mut y); }
            // Rounded `x+y` is stored in `hi` with round-off stored in
            // `lo`. Together `hi+lo` are exactly equal to `x+y`.
            let hi = x + y;
            let lo = y - (hi - x);
            if lo != 0.0 {
                self.partials[j] = lo;
                j += 1;
            }
            x = hi;
        }
        if j >= self.partials.len() {
            self.partials.push(x);
        } else {
            self.partials[j] = x;
            self.partials.truncate(j + 1);
        }
        self
    }

    /// Increases the sum by all values from `iter`. Returns `self`.
    ///
    /// # Example
    ///
    /// ```
    /// use fsum::FSum;
    ///
    /// assert_eq!(FSum::new().add_all((0..10).map(|_| 0.1)).value(), 1.0);
    /// ```
    pub fn add_all<InIter, Bf64>(&mut self, values: InIter) -> &mut FSum
        where Bf64: Borrow<f64>, InIter: IntoIterator<Item=Bf64>
    {
        for x in values { self.add(*x.borrow()); }
        self
    }

    /// Returns the current value of the sum.
    ///
    /// The complexities are:
    /// - time: from *O(1)* (optimistic) to *O(n)* (pessimistic), where *n* is the number of values added so far,
    /// - memory: *O(1)*.
    ///
    /// Usually the time complexity is close to optimistic.
    ///
    /// # Example
    ///
    /// ```
    /// use fsum::FSum;
    ///
    /// assert_eq!(FSum::with_value(2.0).value(), 2.0);
    /// ```
    pub fn value(&self) -> f64 {
        // https://github.com/python/cpython/blob/2b7411df5ca0b6ef714377730fd4d94693f26abd/Lib/test/test_math.py#L647
        let mut n = self.partials.len();
        if n == 0 { return 0.0; }
        n -= 1;
        let mut total = self.partials[n];
        if n == 0 { return total; }
        loop {  // sum partials from the top, stop when the sum becomes inexact:
            let old_total = total;
            n -= 1;
            let x = self.partials[n];
            total = old_total + x;
            if n == 0 { return total; }
            let error = x - (total - old_total);
            if error != 0.0 {
                /* Make half-even rounding work across multiple partials.
                    Needed so that sum([1e-16, 1, 1e16]) will round-up the last
                    digit to two instead of down to zero (the 1e-16 makes the 1
                    slightly closer to two).  With a potential 1 ULP rounding
                    error fixed-up, math.fsum() can guarantee commutativity. */
                if (error < 0.0 && self.partials[n - 1] < 0.0) || (error > 0.0 && self.partials[n - 1] > 0.0) {
                    let y = error * 2.0;
                    let x = total + y;
                    if y == x - total { return x; }
                }
                return total;
            }
        };
        //self.partials.iter().fold(0.0f64, |p, q| p + *q)
    }

    /// Sets the current sum to `0` and returns `self`.
    pub fn reset(&mut self) -> &mut FSum {
        self.partials.clear();
        self
    }

    /// Sets the current sum to `value` and returns `self`.
    ///
    /// # Example
    ///
    /// ```
    /// use fsum::FSum;
    ///
    /// assert_eq!(FSum::new().set(1.0).value(), 1.0);
    /// ```
    pub fn set(&mut self, value: f64) -> &mut FSum {
        self.partials = vec![value];
        self
    }
}

impl AddAssign<f64> for FSum {
    #[inline] fn add_assign(&mut self, other: f64) { self.add(other); }
}

impl SubAssign<f64> for FSum {
    #[inline] fn sub_assign(&mut self, other: f64) { self.add(- other); }
}

impl From<f64> for FSum {
    #[inline] fn from(initial_value: f64) -> Self {
        Self::with_value(initial_value)
    }
}

impl From<FSum> for f64 {
    #[inline] fn from(fsum: FSum) -> Self {
        fsum.value()
    }
}

impl From<&FSum> for f64 {
    #[inline] fn from(fsum: &FSum) -> Self {
        fsum.value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fsum() {
        assert_eq!(FSum::new().add(2.0).add(3.0).value(), 5.0);
        assert_eq!(FSum::with_value(2.0).add(3.0).value(), 5.0);
        assert_eq!(FSum::with_all((0..10).map(|_| 0.1)).value(), 1.0);
        assert_eq!(FSum::new().add(1e100).add(1.0).add(-1e100).value(), 1.0);
        assert_eq!(FSum::with_all(&[1e100, 1.0, -1e100, 1e-100, 1e50, -1.0, -1e50]).value(), 1e-100);
        assert_eq!(FSum::with_all(&[-1e308, 1e308, 1e308]).value(), 1e308);
        assert_eq!(FSum::with_all(&[1e308, -1e308, 1e308]).value(), 1e308);
    }

    #[test]
    fn fsum_add_sub_assign_reset_into() {
        let mut s: FSum = Default::default();
        assert_eq!(s.value(), 0.0);
        s += 1.0;
        assert_eq!(s.value(), 1.0);
        s += 2.0;
        assert_eq!(s.value(), 3.0);
        s -= 1.0;
        assert_eq!(s.value(), 2.0);
        s.reset();
        assert_eq!(s.value(), 0.0);
        s.set(5.0);
        assert_eq!(s.value(), 5.0);
        assert_eq!(5.0, s.into());
    }
}
