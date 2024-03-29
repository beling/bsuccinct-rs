//! Tools to count frequencies of values.

use std::collections::{BTreeMap, HashMap};
use fsum::FSum;
use co_sort::{Permutation, co_sort};
use std::borrow::Borrow;
use std::hash::{BuildHasher, Hash};

/// Implemented by types for counting occurrences of values, i.e. `usize`, `u32`, `u64`.
pub trait Weight: Copy + PartialOrd + std::ops::AddAssign + std::ops::Add<Self, Output=Self> + Ord {
    /// Converts `self` to `usize`.
    fn as_usize(self) -> usize;

    /// Converts `self` to `f64`.
    fn as_f64(self) -> f64;
    
    /// Returns `Self` with given value.
    fn of(value: u32) -> Self;
}

impl Weight for usize {
    #[inline(always)] fn as_usize(self) -> usize { self }
    #[inline(always)] fn as_f64(self) -> f64 { self as f64 }
    #[inline(always)] fn of(value: u32) -> Self { value as Self }
}

impl Weight for u64 {
    #[inline(always)] fn as_usize(self) -> usize { self as usize }
    #[inline(always)] fn as_f64(self) -> f64 { self as f64 }
    #[inline(always)] fn of(value: u32) -> Self { value as Self }
}

impl Weight for u32 {
    #[inline(always)] fn as_usize(self) -> usize { self as usize }
    #[inline(always)] fn as_f64(self) -> f64 { self as f64 }
    #[inline(always)] fn of(value: u32) -> Self { value }
}

/// Types that implement this trait can count number of occurrences of values.
pub trait Frequencies {
    /// Type of value.
    type Value;

    /// Type for counting occurrences, one of `usize`, `u32` or `u64`.
    type Weight: Weight;

    /// Returns stored number of `value` occurrences.
    fn occurrences_of(&mut self, value: &Self::Value) -> Self::Weight;

    /// Adds one to the stored number of `value` occurrences.
    fn add_occurrence_of(&mut self, value: Self::Value);

    /// Returns number of distinct values with a non-zero number of occurrences.
    fn number_of_occurring_values(&self) -> usize;

    /// Returns the total number of occurrences of all values.
    #[inline] fn total_occurrences(&self) -> usize { self.occurrences().map(Self::Weight::as_usize).sum() }

    /// Returns occurring values along with non-zero numbers of their occurrences.
    /// Leaves `self` in an undefined state.
    /// 
    /// Number of yielded items can be obtained by [`Self::number_of_occurring_values`].
    fn drain_frequencies(&mut self) -> impl Iterator<Item=(Self::Value, Self::Weight)>;

    /// Returns occurring values along with non-zero numbers of their occurrences.
    /// 
    /// Number of yielded items can be obtained by [`Self::number_of_occurring_values`].
    fn frequencies(&self) -> impl Iterator<Item=(Self::Value, Self::Weight)> where Self::Value: Clone;

    /// Returns a non-zero number of occurrences of occurring values.
    /// 
    /// Number of yielded items can be obtained by [`Self::number_of_occurring_values`].
    fn occurrences(&self) -> impl Iterator<Item = Self::Weight>;

    /// Constructs empty `Self` (without any occurrences).
    fn without_occurrences() -> Self;

    /// Constructs `Self` that counts occurrences of all values exposed by `iter`.
    fn with_occurrences_of<Iter>(iter: Iter) -> Self
        where Iter: IntoIterator, Iter::Item: Borrow<Self::Value>, Self::Value: Clone, Self: Sized
    {
        let mut result = Self::without_occurrences();
        result.add_occurences_of(iter);
        return result;
    }

    /// Calls [`Self::add_occurrence_of`] for all items exposed by `iter`.
    fn add_occurences_of<Iter>(&mut self, iter: Iter)
        where Iter: IntoIterator, Iter::Item: Borrow<Self::Value>, Self::Value: Clone
    {
        for v in iter { self.add_occurrence_of(v.borrow().clone()); }
    }

    /// Returns the Shannon entropy of the values counted so far.
    fn entropy(&self) -> f64 {
        let sum = self.total_occurrences() as f64;
        - FSum::with_all(self.occurrences()
            .map(|v| { let p = v.as_f64() / sum; p * p.log2()})).value()
    }

    /// Converts `self` to the pair of boxed slices that contain
    /// distinct values and numbers of their occurrences respectively.
    fn into_unsorted(mut self) -> (Box<[Self::Value]>, Box<[Self::Weight]>) where Self: Sized {
        let len = self.number_of_occurring_values();
        let mut freq = Vec::<Self::Weight>::with_capacity(len);
        let mut values = Vec::<Self::Value>::with_capacity(len);
        for (val, fr) in self.drain_frequencies() {
            freq.push(fr);
            values.push(val);
        }
        (values.into_boxed_slice(), freq.into_boxed_slice())
    }

    /// Converts `self` to the pair of boxed slices that contain
    /// distinct values and numbers of their occurrences (in non decreasing order) respectively.
    fn into_sorted(self) -> (Box<[Self::Value]>, Box<[Self::Weight]>) where Self: Sized {
        let (mut values, mut freq) = self.into_unsorted();
        co_sort!(freq, values);
        (values, freq)
    }

    /// Returns the pair of boxed slices that contain
    /// distinct values and numbers of their occurrences respectively.
    fn unsorted(&self) -> (Box<[Self::Value]>, Box<[Self::Weight]>) where Self: Sized, Self::Value: Clone {
        let len = self.number_of_occurring_values();
        let mut freq = Vec::<Self::Weight>::with_capacity(len);
        let mut values = Vec::<Self::Value>::with_capacity(len);
        for (val, fr) in self.frequencies() {
            freq.push(fr);
            values.push(val);
        }
        (values.into_boxed_slice(), freq.into_boxed_slice())
    }

    /// Returns the pair of boxed slices that contain
    /// distinct values and numbers of their occurrences (in non decreasing order) respectively.
    fn sorted(&self) -> (Box<[Self::Value]>, Box<[Self::Weight]>) where Self: Sized, Self::Value: Clone {
        let (mut values, mut freq) = self.unsorted();
        co_sort!(freq, values);
        (values, freq)
    }
}

impl<Value: Eq + Hash, W: Weight, S: BuildHasher + Default> Frequencies for HashMap<Value, W, S> {
    type Value = Value;
    type Weight = W;

    #[inline(always)] fn occurrences_of(&mut self, value: &Self::Value) -> Self::Weight {
        self.get(value).map_or(Self::Weight::of(0), |v| *v)
    }

    #[inline(always)] fn number_of_occurring_values(&self) -> usize { self.len() }

    #[inline(always)] fn drain_frequencies(&mut self) -> impl Iterator<Item=(Self::Value, Self::Weight)> {
        self.drain()
    }

    #[inline(always)] fn frequencies(&self) -> impl Iterator<Item=(Self::Value, Self::Weight)> where Self::Value: Clone {
        self.into_iter().map(|(k, v)| (k.clone(), *v))
    }

    #[inline(always)] fn occurrences(&self) -> impl Iterator<Item = Self::Weight> { self.values().cloned() }

    #[inline(always)] fn add_occurrence_of(&mut self, value: Value) {
        *self.entry(value).or_insert(Self::Weight::of(0)) += Self::Weight::of(1);
    }

    #[inline(always)] fn without_occurrences() -> Self { Default::default() }
}

impl<Value: Ord, W: Weight> Frequencies for BTreeMap<Value, W> {
    type Value = Value;
    type Weight = W;

    #[inline(always)] fn occurrences_of(&mut self, value: &Self::Value) -> Self::Weight {
        self.get(value).map_or(Self::Weight::of(0), |v| *v)
    }

    #[inline(always)] fn number_of_occurring_values(&self) -> usize { self.len() }

    #[inline(always)] fn drain_frequencies(&mut self) -> impl Iterator<Item=(Self::Value, Self::Weight)> {
        std::mem::take(self).into_iter()
    }

    #[inline(always)] fn frequencies(&self) -> impl Iterator<Item=(Self::Value, Self::Weight)> where Self::Value: Clone {
        self.into_iter().map(|(k, v)| (k.clone(), *v))
    }

    #[inline(always)] fn occurrences(&self) -> impl Iterator<Item = Self::Weight> { self.values().cloned() }

    #[inline(always)] fn add_occurrence_of(&mut self, value: Value) {
        *self.entry(value).or_insert(Self::Weight::of(0)) += Self::Weight::of(1);
    }

    #[inline(always)] fn without_occurrences() -> Self { Default::default() }
}

macro_rules! impl_frequencies_by_array_for {($Value:ty) => {
impl<W: Weight> Frequencies for [W; 1 << <$Value>::BITS] {
    type Value = $Value;
    type Weight = W;

    #[inline(always)] fn occurrences_of(&mut self, value: &Self::Value) -> Self::Weight {
        self[*value as usize]
    }

    #[inline(always)] fn add_occurrence_of(&mut self, value: Self::Value) {
        self[value as usize] += Self::Weight::of(1);
    }

    #[inline(always)] fn number_of_occurring_values(&self) -> usize {
        self.iter().filter(|occ| **occ > Self::Weight::of(0)).count()
    }

    #[inline(always)] fn drain_frequencies(&mut self) -> impl Iterator<Item=(Self::Value, Self::Weight)> {
        self.frequencies()
    }

    #[inline(always)] fn frequencies(&self) -> impl Iterator<Item=(Self::Value, Self::Weight)> where Self::Value: Clone {
        self.iter().enumerate().filter_map(|(v, o)| (*o > Self::Weight::of(0)).then(|| (v as $Value, *o)))
    }

    #[inline(always)] fn occurrences(&self) -> impl Iterator<Item = Self::Weight> {
        self.iter().filter_map(|occ| (*occ > Self::Weight::of(0)).then_some(*occ))
    }

    #[inline(always)] fn without_occurrences() -> Self { [Self::Weight::of(0); 1<< <$Value>::BITS] }
}
}}

impl_frequencies_by_array_for!(u8);
impl_frequencies_by_array_for!(u16);