
use std::mem;
use rayon::prelude::*;
use crate::fp::keyset::CachedKeySet::Cached;

/// `KeySet` represent sets of keys (ot the type `K`) that can be used to construct `FPHash` or `FPHash2`.
pub trait KeySet<K> {
    /// Returns number of retained keys. Guarantee to be very fast.
    fn keys_len(&self) -> usize;

    #[inline(always)] fn has_par_for_each_key(&self) -> bool { false }

    #[inline(always)] fn has_par_retain_keys(&self) -> bool { false }

    /// Call `f` for each key in the set, using single thread.
    ///
    /// If `self` doesn't remember which keys are retained it uses `retained_hint` to check this.
    fn for_each_key<F, P>(&self, f: F, retained_hint: P) where F: FnMut(&K), P: FnMut(&K) -> bool;

    /// Multi-threaded version of `for_each_key`.
    #[inline(always)]
    fn par_for_each_key<F, P>(&self, f: F, retained_hint: P)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        self.for_each_key(f, retained_hint);
    }

    /// Calls `map` for each key in the set, and returns outputs of these calls. Uses single thread.
    ///
    /// If `self` doesn't remember which keys are retained it uses `retained_hint` to check this.
    fn map_each_key<R, M, P>(&self, mut map: M, retained_hint: P) -> Vec<R>
        where M: FnMut(&K) -> R, P: FnMut(&K) -> bool
    {
        let mut result = Vec::with_capacity(self.keys_len());
        self.for_each_key(|k| result.push(map(k)), retained_hint);
        result
    }

    /// Multi-threaded version of `map_each_key`.
    #[inline(always)]
    fn par_map_each_key<R, M, P>(&self, map: M, retained_hint: P) -> Vec<R>
        where M: Fn(&K)->R + Sync + Send, R: Send, P: Fn(&K) -> bool { self.map_each_key(map, retained_hint) }

    /// Calls either `map_each_key` (if `use_mt` is `false`) or `par_map_each_key` (if `use_mt` is `true`).
    #[inline(always)]
    fn maybe_par_map_each_key<R, M, P>(&self, map: M, retained_hint: P, use_mt: bool) -> Vec<R>
        where M: Fn(&K)->R + Sync + Send, R: Send, P: Fn(&K) -> bool
    {
        if use_mt { self.par_map_each_key(map, retained_hint) }
            else { self.map_each_key(map, retained_hint) }
    }

    /// Retains in `self` keys pointed by the `filter` and remove the rest, using single thread.
    /// - `filter` shows the keys to be retained (the result of the function can be unspecified for keys removed earlier),
    /// - `retained_earlier` shows the keys that have not been removed earlier,
    /// - `remove_count` returns number of keys to remove.
    fn retain_keys<F, P, R>(&mut self, filter: F, retained_earlier: P, remove_count: R)
        where F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize;

    /// Multi-threaded version of `retain_keys`.
    #[inline(always)]
    fn par_retain_keys<F, P, R>(&mut self, filter: F, retained_earlier: P, remove_count: R)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        self.retain_keys(filter, retained_earlier, remove_count)
    }

    /// Calls either `retain_keys` (if `use_mt` is `false`) or `par_retain_keys` (if `use_mt` is `true`).
    #[inline(always)]
    fn maybe_par_retain_keys<F, P, R>(&mut self, filter: F, retained_earlier: P, remove_count: R, use_mt: bool)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if use_mt /*&& self.has_par_retain_keys()*/ {
            self.par_retain_keys(filter, retained_earlier, remove_count)
        } else {
            self.retain_keys(filter, retained_earlier, remove_count)
        }
    }

    /// Retains in `self` keys pointed by the `index_filter`
    /// (or `filter` if `self` does not support `index_filter`)
    /// and remove the rest.
    /// Uses single thread.
    /// - `index_filter` shows indices (consistent with `par_map_each_key`) of keys to be retained,
    /// - `filter` shows the keys to be retained,
    /// - `retained_earlier` shows the keys that have not been removed earlier,
    /// - `remove_count` returns number of keys to remove.
    ///
    /// The results of `index_filter` and `filter` are unspecified for keys removed earlier.
    #[inline(always)]
    fn retain_keys_with_indices<IF, F, P, R>(&mut self, _index_filter: IF, filter: F, retained_earlier: P, remove_count: R)
        where IF: FnMut(usize) -> bool, F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        self.retain_keys(filter, retained_earlier, remove_count)
    }

    /// Multi-threaded version of `retain_keys_with_indices`.
    #[inline(always)]
    fn par_retain_keys_with_indices<IF, F, P, R>(&mut self, _index_filter: IF, filter: F, retained_earlier: P, remove_count: R)
        where IF: Fn(usize) -> bool + Sync + Send,  F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        self.par_retain_keys(filter, retained_earlier, remove_count)
    }

    /// Calls either `retain_keys_with_indices` (if `use_mt` is `false`) or `par_retain_keys_with_indices` (if `use_mt` is `true`).
    #[inline(always)]
    fn maybe_par_retain_keys_with_indices<IF, F, P, R>(&mut self, index_filter: IF, filter: F, retained_earlier: P, remove_count: R, use_mt: bool)
        where IF: Fn(usize) -> bool + Sync + Send, F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if use_mt /*&& self.has_par_retain_keys()*/ {
            self.par_retain_keys_with_indices(index_filter, filter, retained_earlier, remove_count)
        } else {
            self.retain_keys_with_indices(index_filter, filter, retained_earlier, remove_count)
        }
    }

    /// Works like `retain_keys` and converts `self` into the vector of retained keys.
    fn retain_keys_into_vec<F, P, R>(mut self, mut filter: F, mut retained_earlier: P, remove_count: R) -> Vec<K>
        where F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize, Self: Sized, K: Clone
    {
        self.retain_keys(&mut filter, &mut retained_earlier, remove_count);
        self.map_each_key(|k| (*k).clone(), |k| retained_earlier(k) && filter(k))
    }

    /// Works like `retain_keys_with_indices` and converts `self` into the vector of retained keys.
    fn retain_keys_with_indices_into_vec<IF, F, P, R>(mut self, index_filter: IF, mut filter: F, mut retained_earlier: P, remove_count: R) -> Vec<K>
        where IF: FnMut(usize) -> bool, F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize, Self: Sized, K: Clone
    {
        self.retain_keys_with_indices(index_filter, &mut filter, &mut retained_earlier, remove_count);
        self.map_each_key(|k| (*k).clone(), |k| retained_earlier(k) && filter(k))
    }

    /// Works like `par_retain_keys` and converts `self` into the vector of retained keys.
    fn par_retain_keys_into_vec<F, P, R>(mut self, filter: F, retained_earlier: P, remove_count: R) -> Vec<K>
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send,
              R: Fn() -> usize, Self: Sized, K: Clone + Send
    {
        self.par_retain_keys(&filter, &retained_earlier, remove_count);
        self.par_map_each_key(|k| (*k).clone(), |k| retained_earlier(k) && filter(k))
    }

    /// Works like `par_retain_keys_with_indices` and converts `self` into the vector of retained keys.
    fn par_retain_keys_with_indices_into_vec<IF, F, P, R>(mut self, index_filter: IF, filter: F, retained_earlier: P, remove_count: R) -> Vec<K>
        where IF: Fn(usize) -> bool + Sync + Send,  F: Fn(&K) -> bool + Sync + Send,
              P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize, Self: Sized, K: Clone + Send
    {
        self.par_retain_keys_with_indices(index_filter, &filter, &retained_earlier, remove_count);
        self.par_map_each_key(|k| (*k).clone(), |k| retained_earlier(k) && filter(k))
    }
}

impl<K: Sync + Send> KeySet<K> for Vec<K> {
    #[inline(always)] fn keys_len(&self) -> usize {
        self.len()
    }

    #[inline(always)] fn has_par_for_each_key(&self) -> bool { true }

    #[inline(always)] fn has_par_retain_keys(&self) -> bool { true }

    #[inline(always)] fn for_each_key<F, P>(&self, f: F, _retained_hint: P)
        where F: FnMut(&K), P: FnMut(&K) -> bool
    {
        self.iter().for_each(f)
    }

    #[inline(always)] fn map_each_key<R, M, P>(&self, map: M, _retained_hint: P) -> Vec<R>
        where M: FnMut(&K) -> R, P: FnMut(&K) -> bool { self.iter().map(map).collect() }

    #[inline(always)] fn par_for_each_key<F, P>(&self, f: F, _retained_hint: P)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        self.into_par_iter().for_each(f)
    }

    #[inline(always)] fn par_map_each_key<R, M, P>(&self, map: M, _retained_hint: P) -> Vec<R>
        where M: Fn(&K)->R + Sync + Send, R: Send, P: Fn(&K) -> bool
    {
        self.into_par_iter().map(map).collect()
    }

    #[inline(always)] fn retain_keys<F, P, R>(&mut self, filter: F, _retained_earlier: P, _remove_count: R)
        where F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        self.retain(filter)
    }

    #[inline(always)] fn par_retain_keys<F, P, R>(&mut self, filter: F, _retained_earlier: P, remove_count: R)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        let mut result = Vec::with_capacity(self.len() - remove_count());
        std::mem::swap(self, &mut result);
        self.par_extend(result.into_par_iter().filter(filter));
        //*self = (std::mem::take(self)).into_par_iter().filter(filter).collect();
    }

    #[inline(always)] fn retain_keys_with_indices<IF, F, P, R>(&mut self, mut index_filter: IF, _filter: F, _retained_earlier: P, _remove_count: R)
        where IF: FnMut(usize) -> bool, F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        let mut index = 0;
        self.retain(|_| (index_filter(index), index += 1).0)
    }

    fn par_retain_keys_with_indices<IF, F, P, R>(&mut self, index_filter: IF, _filter: F, _retained_earlier: P, remove_count: R)
        where IF: Fn(usize) -> bool + Sync + Send,  F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        let mut result = Vec::with_capacity(self.len() - remove_count());
        std::mem::swap(self, &mut result);
        self.par_extend(result.into_par_iter().enumerate().filter_map(|(i, k)| index_filter(i).then_some(k)));
        //*self = (std::mem::take(self)).into_par_iter().enumerate().filter_map(|(i, k)| index_filter(i).then_some(k)).collect();
    }

    fn retain_keys_into_vec<F, P, R>(mut self, filter: F, retained_earlier: P, remove_count: R) -> Vec<K>
        where F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize, Self: Sized, K: Clone
    {
        self.retain_keys(filter, retained_earlier, remove_count);
        self
    }

    /// Works like `retain_keys_with_indices` and converts `self` into the vector of retained keys.
    fn retain_keys_with_indices_into_vec<IF, F, P, R>(mut self, index_filter: IF, filter: F, retained_earlier: P, remove_count: R) -> Vec<K>
        where IF: FnMut(usize) -> bool, F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize, Self: Sized, K: Clone
    {
        self.retain_keys_with_indices(index_filter, filter, retained_earlier, remove_count);
        self
    }

    /// Works like `par_retain_keys` and converts `self` into the vector of retained keys.
    fn par_retain_keys_into_vec<F, P, R>(mut self, filter: F, retained_earlier: P, remove_count: R) -> Vec<K>
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send,
              R: Fn() -> usize, Self: Sized, K: Clone + Send
    {
        self.par_retain_keys(filter, retained_earlier, remove_count);
        self
    }

    /// Works like `par_retain_keys_with_indices` and converts `self` into the vector of retained keys.
    fn par_retain_keys_with_indices_into_vec<IF, F, P, R>(mut self, index_filter: IF, filter: F, retained_earlier: P, remove_count: R) -> Vec<K>
        where IF: Fn(usize) -> bool + Sync + Send,  F: Fn(&K) -> bool + Sync + Send,
              P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize, Self: Sized, K: Clone + Send
    {
        self.par_retain_keys_with_indices(index_filter, filter, retained_earlier, remove_count);
        self
    }
}

/// Implements `KeySet`, storing keys in the mutable slice.
///
/// Retain operations reorder the slice, putting retained keys at the beginning of the slice.
pub struct SliceMutSource<'k, K> {
    slice: &'k mut [K],
    len: usize  // how many first elements are in use
}

impl<'k, K> SliceMutSource<'k, K> {
    #[inline(always)] pub fn new(slice: &'k mut [K]) -> Self {
        let len = slice.len();
        Self { slice, len }
    }
}

impl<'k, K> From<&'k mut [K]> for SliceMutSource<'k, K> {
    #[inline(always)] fn from(slice: &'k mut [K]) -> Self { Self::new(slice) }
}

impl<'k, K: Sync> KeySet<K> for SliceMutSource<'k, K> {
    #[inline(always)] fn keys_len(&self) -> usize { self.len }

    #[inline(always)] fn has_par_for_each_key(&self) -> bool { true }

    #[inline(always)] fn for_each_key<F, P>(&self, f: F, _retained_hint: P) where F: FnMut(&K), P: FnMut(&K) -> bool {
        self.slice[0..self.len].iter().for_each(f)
    }

    #[inline(always)] fn par_for_each_key<F, P>(&self, f: F, _retained_hint: P)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        self.slice[0..self.len].into_par_iter().for_each(f)
    }

    #[inline(always)] fn map_each_key<R, M, P>(&self, map: M, _retained_hint: P) -> Vec<R>
        where M: FnMut(&K) -> R, P: FnMut(&K) -> bool
    {
        self.slice[0..self.len].into_iter().map(map).collect()
    }

    #[inline(always)] fn par_map_each_key<R, M, P>(&self, map: M, _retained_hint: P) -> Vec<R>
        where M: Fn(&K)->R + Sync + Send, R: Send, P: Fn(&K) -> bool
    {
        self.slice[0..self.len].into_par_iter().map(map).collect()
    }

    fn retain_keys<F, P, R>(&mut self, mut filter: F, _retained_hint: P, _remove_count: R)
        where F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        let mut i = 0usize;
        while i < self.len {
            if filter(&self.slice[i]) {
                i += 1;
            } else {
                // remove i-th element by replacing it with the last one
                self.len -= 1;
                self.slice.swap(i, self.len);
            }
        }
    }
}

/// Implements `KeySet` that use immutable slice.
///
/// Retain operations clone retained keys into the vector.
pub struct SliceSourceWithClones<'k, K> {
    slice: &'k [K],
    retained: Option<Vec<K>>,
}

impl<'k, K: Sync> SliceSourceWithClones<'k, K> {
    pub fn new(slice: &'k [K]) -> Self {
        Self { slice, retained: None }
    }
}

impl<'k, K: Sync + Send + Clone> KeySet<K> for SliceSourceWithClones<'k, K> {
    fn keys_len(&self) -> usize {
        if let Some(ref retained) = self.retained {
            retained.len()
        } else {
            self.slice.len()
        }
    }

    #[inline(always)] fn has_par_for_each_key(&self) -> bool { true }

    #[inline(always)] fn has_par_retain_keys(&self) -> bool { true }

    #[inline(always)] fn for_each_key<F, P>(&self, f: F, retained_hint: P) where F: FnMut(&K), P: FnMut(&K) -> bool {
        if let Some(ref retained) = self.retained {
            retained.for_each_key(f, retained_hint)
        } else {
            self.slice.into_iter().for_each(f)
        }
    }

    #[inline(always)] fn map_each_key<R, M, P>(&self, map: M, retained_hint: P) -> Vec<R>
        where M: FnMut(&K) -> R, P: FnMut(&K) -> bool
    {
        if let Some(ref retained) = self.retained {
            retained.map_each_key(map, retained_hint)
        } else {
            self.slice.into_iter().map(map).collect()
        }
    }

    #[inline(always)] fn par_for_each_key<F, P>(&self, f: F, retained_hint: P)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        if let Some(ref retained) = self.retained {
            retained.par_for_each_key(f, retained_hint)
        } else {
            (*self.slice).into_par_iter().for_each(f)
        }
    }

    fn retain_keys<F, P, R>(&mut self, mut filter: F, retained_earlier: P, remove_count: R)
        where F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        if let Some(ref mut retained) = self.retained {
            retained.retain_keys(filter, retained_earlier, remove_count)
        } else {
            self.retained = Some(self.slice.into_iter().filter_map(|k|filter(k).then(|| k.clone())).collect());
        }
    }

    fn par_retain_keys<F, P, R>(&mut self, filter: F, retained_earlier: P, remove_count: R)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if let Some(ref mut retained) = self.retained {
            retained.par_retain_keys(filter, retained_earlier, remove_count)
        } else {
            self.retained = Some(self.slice.into_par_iter().filter_map(|k|filter(k).then(|| k.clone())).collect())
        }
    }

    #[inline(always)] fn retain_keys_with_indices<IF, F, P, R>(&mut self, mut index_filter: IF, _filter: F, retained_earlier: P, remove_count: R)
        where IF: FnMut(usize) -> bool, F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        let mut index = 0;
        self.retain_keys(|_| (index_filter(index), index += 1).0, retained_earlier, remove_count)
    }

    fn par_retain_keys_with_indices<IF, F, P, R>(&mut self, index_filter: IF, filter: F, retained_earlier: P, remove_count: R)
        where IF: Fn(usize) -> bool + Sync + Send,  F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if let Some(ref mut retained) = self.retained {
            retained.par_retain_keys_with_indices(index_filter, filter, retained_earlier, remove_count)
        } else {
            self.retained = Some(self.slice.into_par_iter().enumerate().filter_map(|(i, k)| index_filter(i).then_some(k.clone())).collect())
        }
    }
}

/// `KeySet` implementation that stores reference to slice with keys,
/// and indices of this slice that points retained keys.
/// Indices are stored in vector of vectors of 16-bit integers.
/// Each vector covers $2^{16}$ consecutive keys.
pub struct SliceSourceWithRefs<'k, K> {
    slice: &'k [K],
    retained: Vec<Vec<u16>>,
    len: usize
}

impl<'k, K: Sync> SliceSourceWithRefs<'k, K> {
    pub fn new(slice: &'k [K]) -> Self {
        Self { slice, retained: Vec::new(), len: slice.len() }
    }

    fn update_len(&mut self) { self.len = self.retained.iter().map(|v| v.len()).sum(); }
}

impl<'k, K: Sync> KeySet<K> for SliceSourceWithRefs<'k, K> {
    #[inline(always)] fn keys_len(&self) -> usize { self.len }

    #[inline(always)] fn has_par_for_each_key(&self) -> bool { true }

    #[inline(always)] fn has_par_retain_keys(&self) -> bool { true }

    #[inline(always)] fn for_each_key<F, P>(&self, mut f: F, _retained_hint: P) where F: FnMut(&K), P: FnMut(&K) -> bool {
        if self.retained.is_empty() {
            self.slice.into_iter().for_each(f);
        } else {
            for (i, v) in self.retained.iter().zip(self.slice.chunks(1<<16)) {
                i.into_iter().for_each(|i| f(unsafe{v.get_unchecked(*i as usize)}));
            }
        }
    }

    #[inline(always)] fn par_for_each_key<F, P>(&self, f: F, _retained_hint: P)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        if self.retained.is_empty() {
            (*self.slice).into_par_iter().for_each(f);
        } else {
            /*for (i, v) in self.retained.iter().zip(self.slice.chunks(1 << 16)) {
                i.into_par_iter().for_each(|i| f(unsafe { v.get_unchecked(*i as usize) }));
            }*/
            self.retained.par_iter().zip(self.slice.par_chunks(1 << 16)).for_each(|(i, v)| {
                for i in i { f(unsafe { v.get_unchecked(*i as usize) }) };
            });
        }
    }

    fn par_map_each_key<R, M, P>(&self, map: M, _retained_hint: P) -> Vec<R>
        where M: Fn(&K) -> R + Sync + Send, R: Send, P: Fn(&K) -> bool
    {
        if self.retained.is_empty() {
            //let result = Vec::with_capacity(len_hint)
            //self.slice.into_par_iter().map(map).collect_into_vec(result)
            self.slice.into_par_iter().map(map).collect()
        } else {
            let mut result = Vec::with_capacity(self.keys_len());
            for (i, v) in self.retained.iter().zip(self.slice.chunks(1 << 16)) {
                result.par_extend(i.into_par_iter().map(|i| map(unsafe{v.get_unchecked(*i as usize)})))
            }
            result
        }
    }

    fn retain_keys<F, P, R>(&mut self, mut filter: F, _retained_earlier: P, _remove_count: R)
        where F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        if self.retained.is_empty() {
            self.retained = self.slice.chunks(1 << 16).map(|c|
                c.into_iter().enumerate().filter_map(|(i, k)| filter(k).then(|| i as u16)).collect()
            ).collect();
        } else {
            let slice = self.slice; // fixes "cannot borrow `self` as immutable because it is also borrowed as mutable"
            for (mut ci, c) in self.retained.iter_mut().enumerate() {
                ci <<= 16;
                c.retain(|i| filter(unsafe { slice.get_unchecked(ci | (*i as usize)) }));
            }
        }
        self.update_len();
    }

    fn par_retain_keys<F, P, R>(&mut self, filter: F, _retained_earlier: P, _remove_count: R)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if self.retained.is_empty() {
            self.retained = self.slice.chunks(1 << 16).map(|c|
                c.into_par_iter().enumerate().filter_map(|(i, k)| filter(k).then(|| i as u16)).collect()
            ).collect();
        } else {
            let slice = self.slice; // fixes "cannot borrow `self` as immutable because it is also borrowed as mutable"
            self.retained.par_iter_mut().enumerate().for_each(|(mut ci, c)| {
                ci <<= 16;
                c.retain(|i| filter(unsafe { slice.get_unchecked(ci | (*i as usize)) }));
            });
        }
        self.update_len();
    }

    fn retain_keys_with_indices<IF, F, P, R>(&mut self, mut index_filter: IF, _filter: F, _retained_earlier: P, _remove_count: R)
        where IF: FnMut(usize) -> bool, F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        let mut index = 0;
        if self.retained.is_empty() {
            self.retained = self.slice.chunks(1 << 16).map(|c| {
                (0..c.len()).filter_map(|i| (index_filter(index), index += 1).0.then(|| i as u16)).collect()
            }).collect();
        } else {
            for c in self.retained.iter_mut() {
                c.retain(|_| (index_filter(index), index += 1).0);
            }
        }
        self.update_len();
    }

    fn par_retain_keys_with_indices<IF, F, P, R>(&mut self, index_filter: IF, _filter: F, _retained_earlier: P, _remove_count: R)
        where IF: Fn(usize) -> bool + Sync + Send,  F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if self.retained.is_empty() {
            self.retained = self.slice.par_chunks(1 << 16).enumerate().map(|(ci, c)| {
                let delta = ci << 16;
                //c.into_par_iter().enumerate().filter_map(|(i, k)| index_filter(delta + i).then(|| i as u16)).collect()
                (0..c.len()).filter_map(|i| index_filter(delta + i).then(|| i as u16)).collect()
            }).collect();
        } else {
            let mut delta = 0;
            for c in &mut self.retained {
                let len_before = c.len();
                *c = c.par_iter().copied().enumerate().filter_map(|(i, k)| index_filter(delta+i).then_some(k)).collect();
                delta += len_before;
            }
        }
        self.update_len();
    }
}

/// `KeySet` implementation that stores reference to slice with keys,
/// and indices of this slice that points retained keys.
/// Indices are stored in vector of vectors of 16-bit integers.
/// Each vector covers $2^{16}$ consecutive keys, and is stored together with index of its first element.
/// Empty vectors of indices ore not stored.
pub struct SliceSourceWithRefsEmptyCleaning<'k, K> {
    slice: &'k [K],
    //retained: Option<Vec<Vec<u16>>>,
    retained: Option<Vec<(usize, Vec<u16>)>>,
    len: usize
}

impl<'k, K: Sync> SliceSourceWithRefsEmptyCleaning<'k, K> {
    pub fn new(slice: &'k [K]) -> Self {
        Self { slice, retained: None, len: slice.len() }
    }

    fn calc_len(r: &Vec<(usize, Vec<u16>)>) -> usize {
        r.iter().map(|v| v.1.len()).sum()
    }
}

impl<'k, K: Sync> KeySet<K> for SliceSourceWithRefsEmptyCleaning<'k, K> {
    #[inline(always)] fn keys_len(&self) -> usize { self.len }

    #[inline(always)] fn has_par_for_each_key(&self) -> bool { true }

    #[inline(always)] fn has_par_retain_keys(&self) -> bool { true }

    fn for_each_key<F, P>(&self, mut f: F, _retained_hint: P) where F: FnMut(&K), P: FnMut(&K) -> bool {
        if let Some(ref indices) = self.retained {
            for (shift, indices) in indices {
                let slice = &self.slice[*shift..];
                indices.into_iter().for_each(|i| f(unsafe{slice.get_unchecked(*i as usize)}));
            }
        } else {
            self.slice.into_iter().for_each(f);
        }
    }

    fn par_for_each_key<F, P>(&self, f: F, _retained_hint: P)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        if let Some(ref indices) = self.retained {
            /*for (shift, indices) in indices {
                let slice = &self.slice[*shift..];
                indices.into_par_iter().for_each(|i| f(unsafe{slice.get_unchecked(*i as usize)}));
            }*/
            indices.into_par_iter().for_each(|(shift, indices)| {
                let slice = &self.slice[*shift..];
                indices.into_iter().for_each(|i| f(unsafe{slice.get_unchecked(*i as usize)}));
            });
        } else {
            (*self.slice).into_par_iter().for_each(f);
        }
    }

    fn par_map_each_key<R, M, P>(&self, map: M, _retained_hint: P) -> Vec<R>
        where M: Fn(&K) -> R + Sync + Send, R: Send, P: Fn(&K) -> bool
    {
        if let Some(ref indices) = self.retained {
            let mut result = Vec::with_capacity(self.len);
            for (shift, indices) in indices {
                let slice = &self.slice[*shift..];
                result.par_extend(indices.into_par_iter().map(|i| map(unsafe{slice.get_unchecked(*i as usize)})))
            }
            result
        } else {
            //let result = Vec::with_capacity(len_hint)
            //self.slice.into_par_iter().map(map).collect_into_vec(result)
            (*self.slice).into_par_iter().map(map).collect()
        }
    }

    fn retain_keys<F, P, R>(&mut self, mut filter: F, _retained_hint: P, _remove_count: R)
        where F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        if let Some(ref mut r) = self.retained {
            self.len = 0;
            r.retain_mut(|(shift, indices)| {
                let slice = &self.slice[*shift..];
                indices.retain(|i| filter(unsafe { slice.get_unchecked(*i as usize) }));
                self.len += indices.len();
                !indices.is_empty()
            });
        } else {
            let r = self.slice.chunks(1 << 16).enumerate().filter_map(|(ci, slice)| {
                let v: Vec<_> = slice.into_iter().enumerate().filter_map(|(i, k)| filter(k).then(|| i as u16)).collect();
                (!v.is_empty()).then(|| (ci << 16, v))
            }).collect();
            self.len = Self::calc_len(&r);
            self.retained = Some(r);
        }
    }

    fn par_retain_keys<F, P, R>(&mut self, filter: F, _retained_hint: P, _remove_count: R)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if let Some(ref mut r) = self.retained {
            r.into_par_iter().for_each(|(shift, indices)| {
                let slice = &self.slice[*shift..];
                indices.retain(|i| filter(unsafe { slice.get_unchecked(*i as usize) }));
            });
            r.retain(|(_, v)| !v.is_empty());   // parallel version?
            self.len = Self::calc_len(&r);
        } else {
            let r = self.slice.chunks(1 << 16).enumerate().filter_map(|(ci, slice)| {
                let v: Vec<_> = slice.into_par_iter().enumerate().filter_map(|(i, k)| filter(k).then(|| i as u16)).collect();
                (!v.is_empty()).then(|| (ci << 16, v))
            }).collect();
            self.len = Self::calc_len(&r);
            self.retained = Some(r);
        }
    }

    // TODO (par_)retain_keys_with_indices
}

/// Implementation of `KeySet` that stores only the function that returns iterator over all keys
/// (the iterator can even expose the keys that have been removed earlier by `retain` methods).
pub struct DynamicKeySet<KeyIter: Iterator, GetKeyIter: Fn() -> KeyIter> {
    pub keys: GetKeyIter,
    pub len: usize,
    pub const_keys_order: bool // true only if keys are always produced in the same order
}

impl<KeyIter: Iterator, GetKeyIter: Fn() -> KeyIter> DynamicKeySet<KeyIter, GetKeyIter>{
    pub fn new(keys: GetKeyIter, const_keys_order: bool) -> Self {
        let len = keys().count();   // TODO faster alternative
        Self { keys, len, const_keys_order }
    }

    pub fn with_len(keys: GetKeyIter, len: usize, const_keys_order: bool) -> Self {
        Self { keys, len, const_keys_order }
    }
}

impl<KeyIter: Iterator, GetKeyIter: Fn() -> KeyIter> KeySet<KeyIter::Item> for DynamicKeySet<KeyIter, GetKeyIter> {
    #[inline(always)] fn keys_len(&self) -> usize {
        self.len
    }

    fn for_each_key<F, P>(&self, mut f: F, retained_hint: P)
        where F: FnMut(&KeyIter::Item), P: FnMut(&KeyIter::Item) -> bool
    {
        (self.keys)().filter(retained_hint).for_each(|k| f(&k))
    }

    #[inline] fn retain_keys<F, P, R>(&mut self, _filter: F, _retained_earlier: P, mut retains_count: R)
        where F: FnMut(&KeyIter::Item) -> bool, P: FnMut(&KeyIter::Item) -> bool, R: FnMut() -> usize
    {
        self.len = retains_count();
    }

    // TODO retain_keys_into_vec methods
}

/// Implementation of `KeySet` that stores initially stores another key set,
/// but when number of keys drops below given threshold,
/// the remaining keys are cached (cloned into the vector),
/// and later only the cache is used.
pub enum CachedKeySet<K, KS> {
    Dynamic(KS, usize), // the another key set and the threshold
    Cached(Vec<K>)
}

impl<K, KS> Default for CachedKeySet<K, KS> {
    #[inline] fn default() -> Self { Cached(Default::default()) }   // construct an empty key set, needed for mem::take(self)
}

impl<K, KS> CachedKeySet<K, KS> {
    pub fn new(key_set: KS, clone_threshold: usize) -> Self {
        Self::Dynamic(key_set, clone_threshold)
    }
}

impl<K, KeyIter: Iterator, GetKeyIter: Fn() -> KeyIter> CachedKeySet<K, DynamicKeySet<KeyIter, GetKeyIter>> {
    pub fn dynamic(keys: GetKeyIter, const_keys_order: bool, clone_threshold: usize) -> Self {
        Self::new(DynamicKeySet::new(keys, const_keys_order), clone_threshold)
    }
}

impl<'k, K: Sync> CachedKeySet<K, SliceSourceWithRefs<'k, K>> {
    pub fn slice(keys: &'k [K], clone_threshold: usize) -> Self {
        Self::new(SliceSourceWithRefs::new(keys), clone_threshold)
    }
}

impl<K: Clone + Send, KS: KeySet<K>> CachedKeySet<K, KS>
{
    fn into_cache<F, P, R>(self, filter: F, retained_earlier: P, remove_count: R) -> Self
        where F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize {
        match self {
            Self::Dynamic(dynamic_key_set, _) => Cached(dynamic_key_set.retain_keys_into_vec(filter, retained_earlier, remove_count)),
            Self::Cached(_) => self
        }
    }

    fn par_into_cache<F, P, R>(self, filter: F, retained_earlier: P, remove_count: R) -> Self
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        match self {
            Self::Dynamic(dynamic_key_set, _) => Cached(dynamic_key_set.par_retain_keys_into_vec(filter, retained_earlier, remove_count)),
            Self::Cached(_) => self
        }
    }

    fn into_cache_with_indices<IF, F, P, R>(self, index_filter: IF, filter: F, retained_earlier: P, remove_count: R) -> Self
        where IF: FnMut(usize) -> bool, F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        match self {
            Self::Dynamic(dynamic_key_set, _) => Cached(dynamic_key_set.retain_keys_with_indices_into_vec(index_filter, filter, retained_earlier, remove_count)),
            Self::Cached(_) => self
        }
    }

    fn par_into_cache_with_indices<IF, F, P, R>(self, index_filter: IF, filter: F, retained_earlier: P, remove_count: R) -> Self
        where IF: Fn(usize) -> bool + Sync + Send, F: Fn(&K) -> bool + Sync + Send,
              P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        match self {
            Self::Dynamic(dynamic_key_set, _) => Cached(dynamic_key_set.par_retain_keys_with_indices_into_vec(index_filter, filter, retained_earlier, remove_count)),
            Self::Cached(_) => self
        }
    }
}

impl<K: Clone + Sync + Send, KS: KeySet<K>> KeySet<K> for CachedKeySet<K, KS>
{
    fn keys_len(&self) -> usize {
        match self {
            Self::Dynamic(dynamic_key_set, _) => dynamic_key_set.keys_len(),
            Self::Cached(v) => v.len()
        }
    }

    fn has_par_for_each_key(&self) -> bool { true }  // as it is true for cached version

    fn has_par_retain_keys(&self) -> bool { true }  // as it is true for cached version

    #[inline]
    fn for_each_key<F, P>(&self, f: F, retained_hint: P)
        where F: FnMut(&K), P: FnMut(&K) -> bool
    {
        match self {
            Self::Dynamic(dynamic_key_set, _) => dynamic_key_set.for_each_key(f, retained_hint),
            Self::Cached(v) => v.for_each_key(f, retained_hint)
        }
    }

    #[inline]
    fn par_for_each_key<F, P>(&self, f: F, retained_hint: P)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        match self {
            Self::Dynamic(dynamic_key_set, _) => dynamic_key_set.par_for_each_key(f, retained_hint),
            Self::Cached(v) => v.par_for_each_key(f, retained_hint)
        }
    }

    #[inline]
    fn map_each_key<R, M, P>(&self, map: M, retained_hint: P) -> Vec<R>
        where M: FnMut(&K) -> R, P: FnMut(&K) -> bool
    {
        match self {
            Self::Dynamic(dynamic_key_set, _) => dynamic_key_set.map_each_key(map, retained_hint),
            Self::Cached(v) => v.map_each_key(map, retained_hint)
        }
    }

    #[inline]
    fn par_map_each_key<R, M, P>(&self, map: M, retained_hint: P) -> Vec<R>
        where M: Fn(&K)->R + Sync + Send, R: Send, P: Fn(&K) -> bool
    {
        match self {
            Self::Dynamic(dynamic_key_set, _) => dynamic_key_set.par_map_each_key(map, retained_hint),
            Self::Cached(v) => v.par_map_each_key(map, retained_hint)
        }
    }

    fn retain_keys<F, P, R>(&mut self, filter: F, retained_earlier: P, remove_count: R)
        where F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        match self {
            Self::Dynamic(key_set, clone_threshold) => {
                if key_set.keys_len() < *clone_threshold {
                    *self = mem::take(self).into_cache(filter, retained_earlier, remove_count)
                    //*self = Cached(key_set.retain_keys_into_vec(filter, retained_earlier, remove_count))
                } else {
                    key_set.retain_keys(filter, retained_earlier, remove_count)
                }
            },
            Self::Cached(v) => v.retain_keys(filter, retained_earlier, remove_count)
        }
    }

    fn par_retain_keys<F, P, R>(&mut self, filter: F, retained_earlier: P, remove_count: R)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        match self {
            Self::Dynamic(key_set, clone_threshold) => {
                if key_set.keys_len() < *clone_threshold {
                    *self = mem::take(self).par_into_cache(filter, retained_earlier, remove_count)
                } else {
                    key_set.par_retain_keys(filter, retained_earlier, remove_count)
                }
            },
            Self::Cached(v) => v.par_retain_keys(filter, retained_earlier, remove_count)
        }
    }

    fn retain_keys_with_indices<IF, F, P, R>(&mut self, index_filter: IF, filter: F, retained_earlier: P, remove_count: R)
        where IF: FnMut(usize) -> bool, F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        match self {
            Self::Dynamic(key_set, clone_threshold) => {
                if key_set.keys_len() < *clone_threshold {
                    *self = mem::take(self).into_cache_with_indices(index_filter, filter, retained_earlier, remove_count)
                } else {
                    key_set.retain_keys_with_indices(index_filter, filter, retained_earlier, remove_count)
                }
            },
            Self::Cached(v) => v.retain_keys_with_indices(index_filter, filter, retained_earlier, remove_count)
        }
    }

    fn par_retain_keys_with_indices<IF, F, P, R>(&mut self, index_filter: IF, filter: F, retained_earlier: P, remove_count: R)
        where IF: Fn(usize) -> bool + Sync + Send, F: Fn(&K) -> bool + Sync + Send,
              P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        match self {
            Self::Dynamic(key_set, clone_threshold) => {
                if key_set.keys_len() < *clone_threshold {
                    *self = mem::take(self).par_into_cache_with_indices(index_filter, filter, retained_earlier, remove_count)
                } else {
                    key_set.par_retain_keys_with_indices(index_filter, filter, retained_earlier, remove_count)
                }
            },
            Self::Cached(v) => v.par_retain_keys_with_indices(index_filter, filter, retained_earlier, remove_count)
        }
    }
}