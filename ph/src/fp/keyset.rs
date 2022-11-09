
use std::mem;
use rayon::join;
use rayon::prelude::*;
use bitm::ceiling_div;

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

struct RetainedIndexes {
    segment_begin_index: Vec<usize>,    // segment_begin_index[i] is index in delta, where i<<16 segment begins
    deltas: Vec<u16>
}

/// `KeySet` implementation that stores reference to slice with keys,
/// and indices of this slice that points retained keys.
/// Indices are stored in vector of vectors of 16-bit integers.
/// Each vector covers $2^{16}$ consecutive keys.
pub struct SliceSourceWithRefs<'k, K> {
    slice: &'k [K],
    retained: Option<RetainedIndexes>,
}

impl<'k, K: Sync> SliceSourceWithRefs<'k, K> {
    pub fn new(slice: &'k [K]) -> Self {
        Self { slice, retained: None }
    }
}

impl<'k, K: Sync> KeySet<K> for SliceSourceWithRefs<'k, K> {
    #[inline(always)] fn keys_len(&self) -> usize {
        if let Some(ref indices) = self.retained {
            indices.deltas.len()
        } else {
            self.slice.len()
        }
    }

    #[inline(always)] fn has_par_for_each_key(&self) -> bool { true }

    #[inline(always)] fn has_par_retain_keys(&self) -> bool { true }

    #[inline(always)] fn for_each_key<F, P>(&self, mut f: F, _retained_hint: P)
        where F: FnMut(&K), P: FnMut(&K) -> bool
    {
        if let Some(ref indices) = self.retained {
            for (delta_indices, v) in indices.segment_begin_index.windows(2).zip(self.slice.chunks(1<<16)) {
                indices.deltas[delta_indices[0]..delta_indices[1]].into_iter().for_each(|i| f(unsafe{v.get_unchecked(*i as usize)}));
            }
        } else {
            self.slice.into_iter().for_each(f);
        }
    }

    #[inline(always)] fn par_for_each_key<F, P>(&self, f: F, _retained_hint: P)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        if let Some(ref r) = self.retained {
            /*for (delta_indices, v) in indices.segment_begin_index.windows(2).zip(self.slice.chunks(1<<16)) {
                indices.deltas[delta_indices[0]..delta_indices[1]].into_par_iter().for_each(|i| f(unsafe{v.get_unchecked(*i as usize)}));
            }*/
            /*for (seg_i, v) in self.slice.chunks(1<<16).enumerate() {
                r.deltas[r.segment_begin_index[seg_i]..r.segment_begin_index[seg_i+1]].into_par_iter().for_each(|i| f(unsafe{v.get_unchecked(*i as usize)}));
            }*/
            self.slice.par_chunks(1<<16).enumerate().for_each(|(seg_i, v)|
                //r.deltas[r.segment_begin_index[seg_i]..r.segment_begin_index[seg_i+1]].iter().for_each(|i| f(unsafe{v.get_unchecked(*i as usize)}))
                for i in &r.deltas[r.segment_begin_index[seg_i]..r.segment_begin_index[seg_i+1]] {
                    f(unsafe{v.get_unchecked(*i as usize)})
                }
            )
        } else {
            (*self.slice).into_par_iter().for_each(f);
        }
    }

    fn par_map_each_key<R, M, P>(&self, map: M, _retained_hint: P) -> Vec<R>
        where M: Fn(&K) -> R + Sync + Send, R: Send, P: Fn(&K) -> bool
    {
        if let Some(ref r) = self.retained {
            let mut result = Vec::with_capacity(self.keys_len());
            for (seg_i, v) in self.slice.chunks(1<<16).enumerate() {
                result.par_extend(
                    r.deltas[r.segment_begin_index[seg_i]..r.segment_begin_index[seg_i+1]]
                        .into_par_iter()
                        .map(|i| map(unsafe{v.get_unchecked(*i as usize)})));
            }
            result
        } else {
            self.slice.into_par_iter().map(map).collect()
        }
    }

    fn retain_keys<F, P, R>(&mut self, mut filter: F, _retained_earlier: P, mut remove_count: R)
        where F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        if let Some(ref mut r) = self.retained {
            let mut new_deltas = Vec::with_capacity(r.deltas.len() - remove_count());
            /*let mut delta_index = 0;
            let mut segment = 0;
            let mut retained_count = 0;
            r.deltas.retain(|d| {
                while delta_index < r.segment_begin_index[segment] {
                    r.segment_begin_index[segment] = retained_count;
                    segment += 1
                }
                delta_index += 1;
                let result = filter(&self.slice[(segment << 16) + *d as usize]);
                if result { retained_count += 1 };
                result
            });
            for v in &mut r.segment_begin_index[segment..] { *v = retained_count; }*/

            for (seg_i, v) in self.slice.chunks(1<<16).enumerate() {
                let new_segment_begin = new_deltas.len();
                for i in &r.deltas[r.segment_begin_index[seg_i]..r.segment_begin_index[seg_i+1]] {
                    //if filter(unsafe{slice.get_unchecked(ci | (*i as usize))}
                    if filter(unsafe{v.get_unchecked(*i as usize)}) { new_deltas.push(*i); }
                }
                r.segment_begin_index[seg_i] = new_segment_begin;
            }
            *r.segment_begin_index.last_mut().unwrap() = new_deltas.len();
            r.deltas = new_deltas;
        } else {
            let mut new_deltas = Vec::with_capacity(self.slice.len() - remove_count());
            let mut segment_begin_index = Vec::with_capacity(ceiling_div(self.slice.len(), 1<<16)+1);
            segment_begin_index.push(0);
            for v in self.slice.chunks(1<<16) {
                new_deltas.extend(v.into_iter().enumerate().filter_map(|(i,k)| filter(k).then_some(i as u16)));
                segment_begin_index.push(new_deltas.len());
            }
            self.retained = Some(RetainedIndexes{ deltas: new_deltas, segment_begin_index });
        }
    }

    fn par_retain_keys<F, P, R>(&mut self, filter: F, _retained_earlier: P, remove_count: R)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if let Some(ref mut r) = self.retained {
            let mut new_deltas = Vec::with_capacity(r.deltas.len() - remove_count());
            for (seg_i, v) in self.slice.chunks(1<<16).enumerate() {
                let new_segment_begin = new_deltas.len();
                new_deltas.par_extend(
                    r.deltas[r.segment_begin_index[seg_i]..r.segment_begin_index[seg_i+1]]
                        .into_par_iter().copied()
                        .filter(|i| filter(unsafe{v.get_unchecked(*i as usize)}))
                );
                r.segment_begin_index[seg_i] = new_segment_begin;
            }
            *r.segment_begin_index.last_mut().unwrap() = new_deltas.len();
            r.deltas = new_deltas;
        } else {
            let mut new_deltas = Vec::with_capacity(self.slice.len() - remove_count());
            let mut segment_begin_index = Vec::with_capacity(ceiling_div(self.slice.len(), 1<<16)+1);
            segment_begin_index.push(0);
            for v in self.slice.chunks(1<<16) {
                new_deltas.par_extend(v.into_par_iter().enumerate().filter_map(|(i,k)| filter(k).then_some(i as u16)));
                segment_begin_index.push(new_deltas.len());
            }
            self.retained = Some(RetainedIndexes{ deltas: new_deltas, segment_begin_index });

            /*self.retained = Some(self.slice.chunks(1 << 16).map(|c|
                c.into_par_iter().enumerate().filter_map(|(i, k)| filter(k).then(|| i as u16)).collect()
            ).collect());*/
        }
    }

    /*fn retain_keys_with_indices<IF, F, P, R>(&mut self, mut index_filter: IF, _filter: F, _retained_earlier: P, _remove_count: R)
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
    }*/
}

struct SegmentMetadata {
    first_index: usize,   // first index described by the segment
    first_key: usize,   // first key described by the segment
}

/// `KeySet` implementation that stores reference to slice with keys,
/// and indices of this slice that points retained keys.
/// Indices are stored partitioned to segments and stored as 16-bit integers.
/// Each segment covers $2^{16}$ consecutive keys.
/// Empty segments ore not stored.
pub struct SliceSourceWithRefsEmptyCleaning<'k, K> {
    keys: &'k [K],
    indices: Vec<u16>,  // lowest 16 bits of each key index retained so far
    segments: Vec<SegmentMetadata>,   // segments metadata: each element of the vector is (index in indices, index in keys)
}

impl<'k, K: Sync + 'k> SliceSourceWithRefsEmptyCleaning<'k, K> {
    pub fn new(slice: &'k [K]) -> Self {
        Self { keys: slice, indices: Vec::new(), segments: Vec::new() }
    }

    #[inline(always)] fn for_each_in_segment<F: FnMut(&K)>(&self, seg_i: usize, mut f: F) {
        let slice = &self.keys[self.segments[seg_i].first_key..];
        for d in &self.indices[self.segments[seg_i].first_index..self.segments[seg_i+1].first_index] {
            f(unsafe{slice.get_unchecked(*d as usize)});
        }
    }

    fn build_index<R, E>(&mut self, mut remove_count: R, mut extend_with_slice: E)
        where R: FnMut() -> usize,
              E: FnMut(&mut Vec<u16>, &[K], usize) // extends vector by some indices of slice, of keys pointed by filter
            // extra usize in E is index in indices
    {
        self.indices.reserve(self.keys.len() - remove_count());
        self.segments.reserve(ceiling_div(self.keys.len(), 1 << 16) + 1);
        let mut slice_index = 0;
        self.segments.push(SegmentMetadata { first_index: 0, first_key: slice_index });
        for v in self.keys.chunks(1 << 16) {
            extend_with_slice(&mut self.indices, v, slice_index);
            slice_index += 1 << 16;
            self.segments.push(SegmentMetadata { first_index: self.indices.len(), first_key: slice_index });
        }
    }

    /// Copy `indices` accepted by `filter` to the beginning of each segment and stores new lengths of each segment in `new_lengths`.
    fn par_pre_retain<F>(filter: &F, indices: &mut [u16], segments: &[SegmentMetadata], new_lengths: &mut [u32])
        where F: Fn(usize, usize) -> bool + Sync    // filter is called with indices of: keys and indices
    {
        if segments.len() > 1 /*&& indices.len() > 1024*/ {
            let mid = segments.len()/2;
            let segments = segments.split_at(mid);
            let new_lens = new_lengths.split_at_mut(mid);
            let indices = indices.split_at_mut(segments.1[0].first_index - segments.0[0].first_index);
            join(
                || Self::par_pre_retain(filter, indices.0, segments.0, new_lens.0),
                || Self::par_pre_retain(filter, indices.1, segments.1, new_lens.1)
            );
        } else {
            let seg = &segments[0];
            let mut len = 0;
            let mut i = 0;
            while i < indices.len() {
                if filter(seg.first_key + indices[i] as usize, seg.first_index + i) {
                    indices[len as usize] = indices[i];
                    len += 1;
                }
                i += 1;
            }
            new_lengths[0] = len;
            /*for seg_i in 0..segments.len() {
                let seg = &segments[seg_i];
                let first_i = seg.first_index - segments[0].first_index;
                let last_i = segments.get(seg_i+1).map_or_else(|| indices.len(), |s| s.first_index - segments[0].first_index);
                let indices = &mut indices[first_i..last_i];
                let mut len = 0;
                let mut i = 0;
                while i < indices.len() {
                    if filter(seg.first_key + indices[i] as usize) {   // index in self.indices is seg.first_index + i
                        indices[len] = indices[i];
                        len += 1;
                    }
                    i += 1;
                }
                new_lengths[seg_i] = len as u32;
            }*/
        }
    }

    fn par_retain_index<F>(&mut self, filter: F)
        where F: Fn(&K, usize) -> bool + Sync
    {
        let real_seg_len = self.segments.len()-1;
        let mut new_lenghts = vec![0; real_seg_len];
        Self::par_pre_retain(&|ki, ii| filter(&self.keys[ki], ii), &mut self.indices, &mut self.segments[0..real_seg_len], &mut new_lenghts);
        let mut new_seg_len = 0;    // where to copy segment[seg_i]
        let mut new_indices_len = 0;    // where to copy segment[seg_i]
        for seg_i in 0..real_seg_len {
            let new_seg_i_len = new_lenghts[seg_i] as usize;
            if new_seg_i_len > 0 {
                self.indices.copy_within(
                    self.segments[seg_i].first_index .. self.segments[seg_i].first_index + new_seg_i_len,
                    new_indices_len
                );
                self.segments[new_seg_len].first_index = new_indices_len;
                self.segments[new_seg_len].first_key = self.segments[seg_i].first_key;
                new_indices_len += new_seg_i_len;
                new_seg_len += 1;
            }
        }
        self.segments[new_seg_len].first_index = new_indices_len;    // the last indices index of the last segment
        // note self.segments[new_seg_len].1 is not used any more and we do not need update it
        self.segments.resize_with(new_seg_len+1, || unreachable!());
        self.indices.resize_with(new_indices_len, || unreachable!());
    }

    fn retain_index<R, E>(&mut self, mut remove_count: R, mut extend_with_segment: E)
        where R: FnMut() -> usize,
              E: FnMut(&mut Vec<u16>, &[K], &[u16], usize), // extends vector by indices from the given segment of keys pointed by filter
            // extra usize in E1 and E2 is index in indices
    {
        let mut new_indices = Vec::with_capacity(self.indices.len() - remove_count());
        let mut new_seg_len = 0;    // where to copy segment[seg_i]
        for seg_i in 0..self.segments.len()-1 {
            let new_delta_index = new_indices.len();
            let si = &self.segments[seg_i];
            extend_with_segment(&mut new_indices,
                                &self.keys[si.first_key..],
                                &self.indices[si.first_index..self.segments[seg_i+1].first_index],
                                si.first_index);
            if new_delta_index != new_indices.len() {    // segment seg_i is not empty and have to be preserved
                self.segments[new_seg_len].first_index = new_delta_index;
                self.segments[new_seg_len].first_key = self.segments[seg_i].first_key;
                new_seg_len += 1;
            }
        }
        self.segments[new_seg_len].first_index = new_indices.len();    // the last delta index of the last segment
        // note self.segments[new_seg_len].1 is not used any more and we do not need update it
        self.indices = new_indices;   // free some memory
        self.segments.resize_with(new_seg_len+1, || unreachable!());
    }
}

impl<'k, K: Sync> KeySet<K> for SliceSourceWithRefsEmptyCleaning<'k, K> {
    #[inline(always)] fn keys_len(&self) -> usize {
        if self.segments.is_empty() { self.keys.len() } else { self.indices.len() }
    }

    #[inline(always)] fn has_par_for_each_key(&self) -> bool { true }

    #[inline(always)] fn has_par_retain_keys(&self) -> bool { true }

    #[inline(always)] fn for_each_key<F, P>(&self, mut f: F, _retained_hint: P)
        where F: FnMut(&K), P: FnMut(&K) -> bool
    {
        if self.segments.is_empty() {
            self.keys.into_iter().for_each(f);
        } else {
            for seg_i in 0..self.segments.len()-1 {
                self.for_each_in_segment(seg_i, &mut f);
            };
        }
    }

    #[inline(always)] fn par_for_each_key<F, P>(&self, f: F, _retained_hint: P)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        if self.segments.is_empty() {
            (*self.keys).into_par_iter().for_each(f);
        } else {
            (0..self.segments.len()-1).into_par_iter().for_each(|seg_i| {
                self.for_each_in_segment(seg_i, &f);
            });
        }
    }

    fn par_map_each_key<R, M, P>(&self, map: M, _retained_hint: P) -> Vec<R>
        where M: Fn(&K) -> R + Sync + Send, R: Send, P: Fn(&K) -> bool
    {
        if self.segments.is_empty() {
            (*self.keys).into_par_iter().map(map).collect()
        } else {
            let mut result = Vec::with_capacity(self.indices.len());
            for seg_i in 0..self.segments.len()-1 {
                let slice = &self.keys[self.segments[seg_i].first_key..];
                result.par_extend(
                    self.indices[self.segments[seg_i].first_index..self.segments[seg_i+1].first_index]
                        .into_par_iter()
                        .map(|d| map(unsafe{slice.get_unchecked(*d as usize)})));
            };
            result
        }
    }

    fn retain_keys<F, P, R>(&mut self, mut filter: F, _retained_earlier: P, remove_count: R)
        where F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        if self.segments.is_empty() {
            self.build_index(remove_count, |indices, keys, _| {
                indices.extend(keys.into_iter().enumerate().filter_map(|(i,k)| filter(k).then_some(i as u16)));
            })
        } else {
            self.retain_index(remove_count, |indices, keys, key_subset, _| {
                for i in key_subset {
                    if filter(unsafe { keys.get_unchecked(*i as usize) }) { indices.push(*i); }
                }
            })
        }
    }

    fn par_retain_keys<F, P, R>(&mut self, filter: F, _retained_earlier: P, remove_count: R)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if self.segments.is_empty() {
            self.build_index(remove_count, |indices, keys, _| {
                indices.par_extend(keys.into_par_iter().enumerate().filter_map(|(i,k)| filter(k).then_some(i as u16)))
            })
        } else {
            self.par_retain_index(|k, _| filter(k));
        }
    }

    fn retain_keys_with_indices<IF, F, P, R>(&mut self, mut index_filter: IF, _filter: F, retained_earlier: P, remove_count: R)
        where IF: FnMut(usize) -> bool, F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        let mut index = 0;
        self.retain_keys(|_| (index_filter(index), index += 1).0, retained_earlier, remove_count)
    }

    fn par_retain_keys_with_indices<IF, F, P, R>(&mut self, index_filter: IF, filter: F, _retained_earlier: P, remove_count: R)
        where IF: Fn(usize) -> bool + Sync + Send, F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if self.segments.is_empty() {
            self.build_index(remove_count, |indices, keys, shift| {
                indices.par_extend(
                    (0..keys.len()).into_par_iter()
                        .filter_map(|key_nr| index_filter(shift + key_nr).then_some(key_nr as u16))
                );
            })
        } else {
            self.par_retain_index(|_, ii| index_filter(ii));
        }
    }
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
    #[inline] fn default() -> Self { Self::Cached(Default::default()) }   // construct an empty key set, needed for mem::take(self)
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
            Self::Dynamic(dynamic_key_set, _) => Self::Cached(dynamic_key_set.retain_keys_into_vec(filter, retained_earlier, remove_count)),
            Self::Cached(_) => self
        }
    }

    fn par_into_cache<F, P, R>(self, filter: F, retained_earlier: P, remove_count: R) -> Self
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        match self {
            Self::Dynamic(dynamic_key_set, _) => Self::Cached(dynamic_key_set.par_retain_keys_into_vec(filter, retained_earlier, remove_count)),
            Self::Cached(_) => self
        }
    }

    fn into_cache_with_indices<IF, F, P, R>(self, index_filter: IF, filter: F, retained_earlier: P, remove_count: R) -> Self
        where IF: FnMut(usize) -> bool, F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        match self {
            Self::Dynamic(dynamic_key_set, _) => Self::Cached(dynamic_key_set.retain_keys_with_indices_into_vec(index_filter, filter, retained_earlier, remove_count)),
            Self::Cached(_) => self
        }
    }

    fn par_into_cache_with_indices<IF, F, P, R>(self, index_filter: IF, filter: F, retained_earlier: P, remove_count: R) -> Self
        where IF: Fn(usize) -> bool + Sync + Send, F: Fn(&K) -> bool + Sync + Send,
              P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        match self {
            Self::Dynamic(dynamic_key_set, _) => Self::Cached(dynamic_key_set.par_retain_keys_with_indices_into_vec(index_filter, filter, retained_earlier, remove_count)),
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