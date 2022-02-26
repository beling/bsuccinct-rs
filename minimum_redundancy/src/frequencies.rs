//! Tools to count frequencies of values.

use std::collections::HashMap;
use fsum::FSum;
use co_sort::{Permutation, co_sort};
use std::borrow::Borrow;
use std::hash::{BuildHasher, Hash};

/// Types that implement this trait can count number of occurrences of values.
pub trait Frequencies {
    /// Type of value.
    type Value;

    /// Constructs `Self` that counts occurrences of all values exposed by `iter`.
    fn with_counted_all<Iter: IntoIterator>(iter: Iter) -> Self
        where Iter::Item: Borrow<Self::Value>, Self: Default, Self::Value: Clone
    {
        let mut result = Self::default();
        result.count_all(iter);
        return result;
    }

    /// Adds one to the stored number of `value` occurrences.
    fn count(&mut self, value: Self::Value);

    /// Calls `count` for all items exposed by `iter`.
    fn count_all<Iter: IntoIterator>(&mut self, iter: Iter) where Iter::Item: Borrow<Self::Value>, Self::Value: Clone {
        for v in iter { self.count(v.borrow().clone()); }
    }

    /// Returns the Shannon entropy of the values counted so far.
    fn entropy(&self) -> f64;

    /// Converts `self` to the pair of boxed slices that contain
    /// distinct values and numbers of their occurrences respectively.
    fn into_unsorted(self) -> (Box<[Self::Value]>, Box<[u32]>);

    /// Converts `self` to the pair of boxed slices that contain
    /// distinct values and numbers of their occurrences (in non decreasing order) respectively.
    fn into_sorted(self) -> (Box<[Self::Value]>, Box<[u32]>) where Self: Sized {
        let (mut values, mut freq) = self.into_unsorted();
        co_sort!(freq, values);
        (values, freq)
    }
}

impl<Value: Eq + Hash, S: BuildHasher> Frequencies for HashMap<Value, u32, S> {
    type Value = Value;

    fn count(&mut self, value: Value) {
        *self.entry(value).or_insert(0) += 1;
    }

    fn entropy(&self) -> f64 {
        let sum = self.values().sum::<u32>() as f64;
        - FSum::with_all(self.values()
            .map(|v| { let p = *v as f64 / sum; p * p.log2()})).value()
    }

    fn into_unsorted(mut self) -> (Box<[Self::Value]>, Box<[u32]>) {
        let len = self.len();
        let mut freq = Vec::<u32>::with_capacity(len);
        let mut values = Vec::<Self::Value>::with_capacity(len);
        for (val, fr) in self.drain() {
            freq.push(fr);
            values.push(val);
        }
        (values.into_boxed_slice(), freq.into_boxed_slice())
    }
}