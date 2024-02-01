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

    /// Returns stored number of `value` occurrences.
    fn occurrences_of(&mut self, value: &Self::Value) -> u32;

    /// Adds one to the stored number of `value` occurrences.
    fn add_occurrence_of(&mut self, value: Self::Value);

    /// Returns number of values with a non-zero number of occurrences.
    fn number_of_occurring_values(&self) -> usize;

    /// Returns the total number of occurrences of all values.
    #[inline] fn total_occurrences(&self) -> u32 { self.occurrences().sum() }

    /// Returns occurring values along with non-zero numbers of their occurrences.
    /// Leaves `self` in an undefined state.
    /// 
    /// Number of yielded items can be obtained by [`Self::number_of_occurring_values`].
    fn drain_frequencies(&mut self) -> impl Iterator<Item=(Self::Value, u32)>;

    /// Returns occurring values along with non-zero numbers of their occurrences.
    /// 
    /// Number of yielded items can be obtained by [`Self::number_of_occurring_values`].
    fn frequencies(&self) -> impl Iterator<Item=(Self::Value, u32)> where Self::Value: Clone;

    /// Returns a non-zero number of occurrences of occurring values.
    /// 
    /// Number of yielded items can be obtained by [`Self::number_of_occurring_values`].
    fn occurrences(&self) -> impl Iterator<Item = u32>;

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

    /// Calls [`Self::add_occurence_of`] for all items exposed by `iter`.
    fn add_occurences_of<Iter>(&mut self, iter: Iter)
        where Iter: IntoIterator, Iter::Item: Borrow<Self::Value>, Self::Value: Clone
    {
        for v in iter { self.add_occurrence_of(v.borrow().clone()); }
    }

    /// Returns the Shannon entropy of the values counted so far.
    fn entropy(&self) -> f64 {
        let sum = self.total_occurrences() as f64;
        - FSum::with_all(self.occurrences()
            .map(|v| { let p = v as f64 / sum; p * p.log2()})).value()
    }

    /// Converts `self` to the pair of boxed slices that contain
    /// distinct values and numbers of their occurrences respectively.
    fn into_unsorted(mut self) -> (Box<[Self::Value]>, Box<[u32]>) where Self: Sized {
        let len = self.number_of_occurring_values();
        let mut freq = Vec::<u32>::with_capacity(len);
        let mut values = Vec::<Self::Value>::with_capacity(len);
        for (val, fr) in self.drain_frequencies() {
            freq.push(fr);
            values.push(val);
        }
        (values.into_boxed_slice(), freq.into_boxed_slice())
    }

    /// Converts `self` to the pair of boxed slices that contain
    /// distinct values and numbers of their occurrences (in non decreasing order) respectively.
    fn into_sorted(self) -> (Box<[Self::Value]>, Box<[u32]>) where Self: Sized {
        let (mut values, mut freq) = self.into_unsorted();
        co_sort!(freq, values);
        (values, freq)
    }

    /// Returns the pair of boxed slices that contain
    /// distinct values and numbers of their occurrences respectively.
    fn unsorted(&self) -> (Box<[Self::Value]>, Box<[u32]>) where Self: Sized, Self::Value: Clone {
        let len = self.number_of_occurring_values();
        let mut freq = Vec::<u32>::with_capacity(len);
        let mut values = Vec::<Self::Value>::with_capacity(len);
        for (val, fr) in self.frequencies() {
            freq.push(fr);
            values.push(val);
        }
        (values.into_boxed_slice(), freq.into_boxed_slice())
    }

    /// Returns the pair of boxed slices that contain
    /// distinct values and numbers of their occurrences (in non decreasing order) respectively.
    fn sorted(&self) -> (Box<[Self::Value]>, Box<[u32]>) where Self: Sized, Self::Value: Clone {
        let (mut values, mut freq) = self.unsorted();
        co_sort!(freq, values);
        (values, freq)
    }
}

impl<Value: Eq + Hash, S: BuildHasher + Default> Frequencies for HashMap<Value, u32, S> {
    type Value = Value;

    #[inline(always)] fn occurrences_of(&mut self, value: &Self::Value) -> u32 {
        self.get(value).map_or(0, |v| *v)
    }

    #[inline(always)] fn number_of_occurring_values(&self) -> usize { self.len() }

    #[inline(always)] fn drain_frequencies(&mut self) -> impl Iterator<Item=(Self::Value, u32)> { HashMap::drain(self) }

    #[inline(always)] fn frequencies(&self) -> impl Iterator<Item=(Self::Value, u32)> where Self::Value: Clone {
        self.into_iter().map(|(k, v)| (k.clone(), *v))
    }

    #[inline(always)] fn occurrences(&self) -> impl Iterator<Item = u32> { self.values().cloned() }

    #[inline(always)] fn add_occurrence_of(&mut self, value: Value) {
        *self.entry(value).or_insert(0) += 1;
    }

    #[inline(always)] fn without_occurrences() -> Self { Default::default() }

    
}

impl Frequencies for [u32; 256] {
    type Value = u8;

    #[inline(always)] fn occurrences_of(&mut self, value: &Self::Value) -> u32 {
        self[*value as usize]
    }

    #[inline(always)] fn add_occurrence_of(&mut self, value: Self::Value) {
        self[value as usize] += 1
    }

    #[inline(always)] fn number_of_occurring_values(&self) -> usize {
        self.iter().filter(|occ| **occ > 0).count()
    }

    #[inline(always)] fn drain_frequencies(&mut self) -> impl Iterator<Item=(Self::Value, u32)> {
        self.frequencies()
    }

    #[inline(always)] fn frequencies(&self) -> impl Iterator<Item=(Self::Value, u32)> where Self::Value: Clone {
        self.iter().enumerate().filter_map(|(v, o)| (*o > 0).then(|| (v as u8, *o)))
    }

    #[inline(always)] fn occurrences(&self) -> impl Iterator<Item = u32> {
        self.iter().filter_map(|occ| (*occ > 0).then_some(*occ))
    }

    #[inline(always)] fn without_occurrences() -> Self { [0; 256] }
}