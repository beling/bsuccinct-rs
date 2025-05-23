//! Managing sets of keys during construction of minimal perfect hash functions.

use std::mem;
use std::mem::MaybeUninit;
use rayon::join;
use rayon::prelude::*;
use bitm::{BitAccess, ceiling_div};

/// A trait for accessing and managing sets of keys (of the type `K`) during construction of
/// [`fmph::Function`](super::Function) or [`fmph::GOFunction`](super::GOFunction).
pub trait KeySet<K> {
    /// Returns number of retained keys. Guarantee to be very fast.
    fn keys_len(&self) -> usize;

    /// Returns `true` only if [`Self::par_for_each_key`] can use multiple threads.
    #[inline(always)] fn has_par_for_each_key(&self) -> bool { false }

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
        where M: Fn(&K)->R + Sync + Send, R: Send, P: Fn(&K) -> bool + Sync + Send
    { self.map_each_key(map, retained_hint) }

    /// Calls either `map_each_key` (if `use_mt` is `false`) or `par_map_each_key` (if `use_mt` is `true`).
    #[inline(always)]
    fn maybe_par_map_each_key<R, M, P>(&self, map: M, retained_hint: P, use_mt: bool) -> Vec<R>
        where M: Fn(&K)->R + Sync + Send, R: Send, P: Fn(&K) -> bool + Sync + Send
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

    /// Convert `self` into the vector of retained keys.
    /// 
    /// If `self` doesn't remember which keys are retained it uses `retained_hint` to check this.
    #[inline] fn into_vec<P>(self, retained_hint: P) -> Vec<K>
        where P: FnMut(&K) -> bool, K: Clone, Self: Sized
    {
        self.map_each_key(|k| (*k).clone(), retained_hint)
    }

    /// Multi-threaded version of `into_vec`.
    #[inline] fn par_into_vec<P>(self, retained_hint: P) -> Vec<K>
        where P: Fn(&K) -> bool + Sync + Send, Self: Sized, K: Clone + Send
    {
        self.par_map_each_key(|k| (*k).clone(), retained_hint)
    }
}

impl<K: Sync + Send> KeySet<K> for Vec<K> {
    #[inline(always)] fn keys_len(&self) -> usize {
        self.len()
    }

    #[inline(always)] fn has_par_for_each_key(&self) -> bool { true }

    #[inline(always)] fn for_each_key<F, P>(&self, f: F, _retained_hint: P)
        where F: FnMut(&K)
    {
        self.iter().for_each(f)
    }

    #[inline(always)] fn map_each_key<R, M, P>(&self, map: M, _retained_hint: P) -> Vec<R>
        where M: FnMut(&K) -> R { self.iter().map(map).collect() }

    #[inline(always)] fn par_for_each_key<F, P>(&self, f: F, _retained_hint: P)
        where F: Fn(&K) + Sync + Send
    {
        self.into_par_iter().for_each(f)
    }

    #[inline(always)] fn par_map_each_key<R, M, P>(&self, map: M, _retained_hint: P) -> Vec<R>
        where M: Fn(&K)->R + Sync + Send, R: Send
    {
        self.into_par_iter().map(map).collect()
    }

    #[inline(always)] fn retain_keys<F, P, R>(&mut self, filter: F, _retained_earlier: P, _remove_count: R)
        where F: FnMut(&K) -> bool
    {
        self.retain(filter)
    }

    #[inline(always)] fn par_retain_keys<F, P, R>(&mut self, filter: F, _retained_earlier: P, remove_count: R)
        where F: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        let mut result = Vec::with_capacity(self.len() - remove_count());
        std::mem::swap(self, &mut result);
        self.par_extend(result.into_par_iter().filter(filter));
        //*self = (std::mem::take(self)).into_par_iter().filter(filter).collect();
    }

    #[inline(always)] fn retain_keys_with_indices<IF, F, P, R>(&mut self, mut index_filter: IF, _filter: F, _retained_earlier: P, _remove_count: R)
        where IF: FnMut(usize) -> bool
    {
        let mut index = 0;
        self.retain(|_| (index_filter(index), index += 1).0)
    }

    fn par_retain_keys_with_indices<IF, F, P, R>(&mut self, index_filter: IF, _filter: F, _retained_earlier: P, remove_count: R)
        where IF: Fn(usize) -> bool + Sync + Send, R: Fn() -> usize
    {
        let mut result = Vec::with_capacity(self.len() - remove_count());
        std::mem::swap(self, &mut result);
        self.par_extend(result.into_par_iter().enumerate().filter_map(|(i, k)| index_filter(i).then_some(k)));
        //*self = (std::mem::take(self)).into_par_iter().enumerate().filter_map(|(i, k)| index_filter(i).then_some(k)).collect();
    }

    #[inline(always)] fn into_vec<P>(self, _retained_hint: P) -> Vec<K>
    {
        self
    }

    #[inline(always)] fn par_into_vec<P>(self, _retained_hint: P) -> Vec<K>
    {
        self
    }
}

/// Implements [`KeySet`], storing keys in the mutable slice.
///
/// Retain operations reorder the slice, putting retained keys at the beginning of the slice.
pub struct SliceMutSource<'k, K> {
    /// All keys (retained ones occupy `len` beginning indices).
    pub slice: &'k mut [K],
    /// How many first keys are retained.
    pub len: usize
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

    #[inline(always)] fn for_each_key<F, P>(&self, f: F, _retained_hint: P) where F: FnMut(&K) {
        self.slice[0..self.len].iter().for_each(f)
    }

    #[inline(always)] fn par_for_each_key<F, P>(&self, f: F, _retained_hint: P)
        where F: Fn(&K) + Sync + Send
    {
        self.slice[0..self.len].into_par_iter().for_each(f)
    }

    #[inline(always)] fn map_each_key<R, M, P>(&self, map: M, _retained_hint: P) -> Vec<R>
        where M: FnMut(&K) -> R
    {
        self.slice[0..self.len].into_iter().map(map).collect()
    }

    #[inline(always)] fn par_map_each_key<R, M, P>(&self, map: M, _retained_hint: P) -> Vec<R>
        where M: Fn(&K)->R + Sync + Send, R: Send
    {
        self.slice[0..self.len].into_par_iter().map(map).collect()
    }

    fn retain_keys<F, P, R>(&mut self, mut filter: F, _retained_hint: P, _remove_count: R)
        where F: FnMut(&K) -> bool
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

/// Implements [`KeySet`] that use immutable slice.
///
/// It does not use any index for pointing remain keys.
/// That is why it is recommended to use it with [`CachedKeySet`].
pub struct ImmutableSlice<'k, K> {
    slice: &'k [K],
    len: usize  // how many elements are in use
}

impl<'k, K: Sync> ImmutableSlice<'k, K> {
    pub fn new(slice: &'k [K]) -> Self {
        Self { slice, len: slice.len() }
    }

    pub fn cached(slice: &[K], clone_threshold: usize) -> CachedKeySet<K, ImmutableSlice<K>> {
        CachedKeySet::new(ImmutableSlice::new(slice), clone_threshold)
    }
}

impl<'k, K: Sync + Send + Clone> KeySet<K> for ImmutableSlice<'k, K> {
    #[inline(always)] fn keys_len(&self) -> usize { self.len }

    #[inline(always)] fn has_par_for_each_key(&self) -> bool { true }

    #[inline(always)] fn for_each_key<F, P>(&self, mut f: F, mut retained_hint: P)
    where F: FnMut(&K), P: FnMut(&K) -> bool
    {
        if self.slice.len() == self.len {
            self.slice.into_iter().for_each(f)
        } else {
            for k in self.slice { if retained_hint(k) { f(k) } }
        }
    }

    #[inline(always)] fn map_each_key<R, M, P>(&self, mut map: M, mut retained_hint: P) -> Vec<R>
        where M: FnMut(&K) -> R, P: FnMut(&K) -> bool
    {
        if self.slice.len() == self.len {
            self.slice.into_iter().map(map).collect()
        } else {
            self.slice.into_iter().filter_map(|k| retained_hint(k).then(|| map(k))).collect()
        }
    }

    #[inline(always)] fn par_for_each_key<F, P>(&self, f: F, retained_hint: P)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        if self.slice.len() == self.len {
            (*self.slice).into_par_iter().for_each(f)
        } else {
            (*self.slice).into_par_iter().filter(|k| retained_hint(*k)).for_each(f)
        }
    }

    #[inline(always)] fn retain_keys<F, P, R>(&mut self, _filter: F, _retained_earlier: P, mut remove_count: R)
        where R: FnMut() -> usize
    {
        self.len -= remove_count();
    }

    /*fn par_retain_keys<F, P, R>(&mut self, filter: F, retained_earlier: P, remove_count: R)
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
    }*/
}

struct SegmentMetadata {
    first_index: usize,   // first index described by the segment
    first_key: usize,   // first key described by the segment
}

pub trait RefsIndex: Copy {
    const SEGMENT_SIZE: usize;
    fn from_usize(u: usize) -> Self;
    fn as_usize(self) -> usize;
}

impl RefsIndex for u8 {
    const SEGMENT_SIZE: usize = 1<<8;
    #[inline(always)] fn from_usize(u: usize) -> Self { u as Self }
    #[inline(always)] fn as_usize(self) -> usize { self as usize }
}

impl RefsIndex for u16 {
    const SEGMENT_SIZE: usize = 1<<16;
    #[inline(always)] fn from_usize(u: usize) -> Self { u as Self }
    #[inline(always)] fn as_usize(self) -> usize { self as usize }
}

/// [`KeySet`] implementation that stores reference to slice with keys,
/// and indices of this slice that points retained keys.
/// Indices are stored partitioned to segments and stored as 8 (if `I=u8`) or 16-bit (if `I=u16`) integers.
/// Each segment covers $2^8$ or $2^{16}$ consecutive keys.
/// Empty segments are not stored.
pub struct SliceSourceWithRefs<'k, K, I: RefsIndex = u8> {
    keys: &'k [K],
    indices: Vec<I>,  // lowest 16 bits of each key index retained so far
    segments: Vec<SegmentMetadata>,   // segments metadata: each element of the vector is (index in indices, index in keys)
}

impl<'k, K: Sync + 'k, I: RefsIndex + Send + Sync> SliceSourceWithRefs<'k, K, I> {
    pub fn new(slice: &'k [K]) -> Self {
        Self { keys: slice, indices: Vec::new(), segments: Vec::new() }
    }

    #[inline(always)] fn for_each_in_segment<F: FnMut(&K)>(&self, seg_i: usize, mut f: F) {
        let slice = &self.keys[self.segments[seg_i].first_key..];
        for d in &self.indices[self.segments[seg_i].first_index..self.segments[seg_i+1].first_index] {
            f(unsafe{slice.get_unchecked(d.as_usize())});
        }
    }

    /*fn par_for_each<F: Fn(&K) + Sync>(&self, f: &F, seg_beg: usize, seg_end: usize) {
        let len = seg_end - seg_beg;
        if len > 1 && self.segments[seg_end].first_index - self.segments[seg_beg].first_index > 1024 {
            let mid = seg_beg + len/2;
            join(
                || self.par_for_each(f, seg_beg, mid),
                || self.par_for_each(f, mid, seg_end)
            );
        } else {
            for s in seg_beg..seg_end {
                self.for_each_in_segment(s, f);
            }
        }
    }*/

    /// Copy `indices` accepted by `filter` to the beginning of each segment and stores new lengths of each segment in `new_lengths`.
    fn par_map<R, M>(&self, dst: &mut [MaybeUninit<R>], map: &M, segments: &[SegmentMetadata])
        where R: Send, M: Fn(&K) -> R + Sync + Send
    {
        if segments.len() > 2 /*&& dst.len() < 1024*/ {
            let mid = segments.len()/2;
            let (dst0, dst1) = dst.split_at_mut(segments[mid].first_index - segments[0].first_index);
            join(
                || self.par_map(dst0, map, &segments[..=mid]),
                || self.par_map(dst1, map, &segments[mid..])
            );
        } else {
            let keys = &self.keys[segments[0].first_key..];
            for (i, d) in self.indices[segments[0].first_index..segments[1].first_index].into_iter().zip(dst) {
                d.write(map(unsafe { keys.get_unchecked(I::as_usize(*i)) }));
            }

            /*let mut di = 0;
            for segments in segments.windows(2) {
                let keys = &self.keys[segments[0].first_key..];
                for i in self.indices[segments[0].first_index..segments[1].first_index].into_iter() {
                    dst[di].write(map(unsafe { keys.get_unchecked(I::as_usize(*i)) }));
                    di += 1;
                }
            }*/
        }
    }

    /// Copy `indices` accepted by `filter` to the beginning of each segment and stores new lengths of each segment in `new_lengths`.
    fn par_pre_retain<F>(filter: &F, indices: &mut [I], segments: &[SegmentMetadata], new_lengths: &mut [u32])
        where F: Fn(usize, usize) -> bool + Sync    // filter is called with indices of: keys and indices
    {
        if segments.len() > 1 && indices.len() > 1024 { // TODO check if it is not better to comment indices.len() > 1024 and then use commented out code below
            let mid = segments.len()/2;
            let segments = segments.split_at(mid);
            let new_lens = new_lengths.split_at_mut(mid);
            let indices = indices.split_at_mut(segments.1[0].first_index - segments.0[0].first_index);
            join(
                || Self::par_pre_retain(filter, indices.0, segments.0, new_lens.0),
                || Self::par_pre_retain(filter, indices.1, segments.1, new_lens.1)
            );
        } else {
            /*let seg = &segments[0];
            let mut len = 0;
            let mut i = 0;
            while i < indices.len() {
                if filter(seg.first_key + indices[i].as_usize(), seg.first_index + i) {
                    indices[len as usize] = indices[i];
                    len += 1;
                }
                i += 1;
            }
            new_lengths[0] = len;*/
            for seg_i in 0..segments.len() {
                let seg = &segments[seg_i];
                let first_i = seg.first_index - segments[0].first_index;
                let last_i = segments.get(seg_i+1).map_or_else(|| indices.len(), |s| s.first_index - segments[0].first_index);
                let indices = &mut indices[first_i..last_i];
                let mut len = 0;
                let mut i = 0;
                while i < indices.len() {
                    if filter(seg.first_key + indices[i].as_usize(), seg.first_index + i) {   // index in self.indices is seg.first_index + i
                        indices[len] = indices[i];
                        len += 1;
                    }
                    i += 1;
                }
                new_lengths[seg_i] = len as u32;
            }
        }
    }

    fn par_retain_index<F>(indices: &mut Vec<I>, segments: &mut Vec<SegmentMetadata>, filter: F)
        where F: Fn(usize, usize) -> bool + Sync
    {
        let real_seg_len = segments.len()-1;
        let mut new_lenghts = vec![0; real_seg_len];
        Self::par_pre_retain(&filter, indices, &mut segments[0..real_seg_len], &mut new_lenghts);
        let mut new_seg_len = 0;    // where to copy segment[seg_i]
        let mut new_indices_len = 0;    // where to copy segment[seg_i]
        for seg_i in 0..real_seg_len {
            let new_seg_i_len = new_lenghts[seg_i] as usize;
            if new_seg_i_len > 0 {
                indices.copy_within(
                    segments[seg_i].first_index .. segments[seg_i].first_index + new_seg_i_len,
                    new_indices_len
                );
                segments[new_seg_len].first_index = new_indices_len;
                segments[new_seg_len].first_key = segments[seg_i].first_key;
                new_indices_len += new_seg_i_len;
                new_seg_len += 1;
            }
        }
        segments[new_seg_len].first_index = new_indices_len;    // the last indices index of the last segment
        // note self.segments[new_seg_len].1 is not used any more and we do not need update it
        segments.resize_with(new_seg_len+1, || unreachable!());
        indices.resize_with(new_indices_len, || unreachable!());
    }
}

impl<'k, K, I: RefsIndex> SliceSourceWithRefs<'k, K, I> {
    fn append_segments_from_bitmap(&mut self, slice_index: &mut usize, accepted_keys: &Vec<u64>) {
        for accepted in accepted_keys.chunks( I::SEGMENT_SIZE >> 6) {
            self.indices.extend(accepted.bit_ones().map(|b| I::from_usize(b)));
            *slice_index += I::SEGMENT_SIZE;
            self.segments.push(SegmentMetadata { first_index: self.indices.len(), first_key: *slice_index });
        }
    }
}

impl<'k, K: Sync, I: RefsIndex + Sync + Send> KeySet<K> for SliceSourceWithRefs<'k, K, I> {
    #[inline(always)] fn keys_len(&self) -> usize {
        if self.segments.is_empty() { self.keys.len() } else { self.indices.len() }
    }

    #[inline(always)] fn has_par_for_each_key(&self) -> bool { true }

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
            //self.par_for_each(&f, 0, self.segments.len()-1);
        }
    }

    fn par_map_each_key<R, M, P>(&self, map: M, _retained_hint: P) -> Vec<R>
        where M: Fn(&K) -> R + Sync + Send, R: Send, P: Fn(&K) -> bool
    {
        if self.segments.is_empty() {
            (*self.keys).into_par_iter().map(map).collect()
        } else {
            let len = self.indices.len();
            let mut result = Vec::with_capacity(len);
            self.par_map(result.spare_capacity_mut(), &map, &self.segments);
            unsafe { result.set_len(len); }
            result
        }
    }

    fn retain_keys<F, P, R>(&mut self, mut filter: F, _retained_earlier: P, mut remove_count: R)
        where F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        if self.segments.is_empty() {
            self.indices.reserve(self.keys.len() - remove_count());
            self.segments.reserve(ceiling_div(self.keys.len(), I::SEGMENT_SIZE) + 1);
            let mut slice_index = 0;
            self.segments.push(SegmentMetadata { first_index: 0, first_key: slice_index });
            for keys in self.keys.chunks(I::SEGMENT_SIZE) {
                self.indices.extend(keys.into_iter().enumerate().filter_map(|(i,k)| filter(k).then_some(I::from_usize(i))));
                slice_index += I::SEGMENT_SIZE;
                self.segments.push(SegmentMetadata { first_index: self.indices.len(), first_key: slice_index });
            }
        } else {
            let mut new_indices_len = 0;
            let mut new_seg_len = 0;    // where to copy segment[seg_i]
            for seg_i in 0..self.segments.len()-1 {
                let new_delta_index = new_indices_len;
                let si = &self.segments[seg_i];
                let keys = &self.keys[si.first_key..];
                for i in si.first_index..self.segments[seg_i+1].first_index {
                    let i = *unsafe { self.indices.get_unchecked(i) };
                    if filter(unsafe { keys.get_unchecked(i.as_usize()) }) {
                        self.indices[new_indices_len] = i;
                        new_indices_len += 1;
                    }
                }
                if new_delta_index != new_indices_len {    // segment seg_i is not empty and have to be preserved
                    self.segments[new_seg_len].first_index = new_delta_index;
                    self.segments[new_seg_len].first_key = self.segments[seg_i].first_key;
                    new_seg_len += 1;
                }
            }
            self.segments[new_seg_len].first_index = new_indices_len;    // the last delta index of the last segment
            // note self.segments[new_seg_len].1 is not used any more and we do not need update it
            self.segments.resize_with(new_seg_len+1, || unreachable!());
            self.indices.resize_with(new_indices_len, || unreachable!());
        }
    }

    fn par_retain_keys<F, P, R>(&mut self, filter: F, _retained_earlier: P, remove_count: R)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if self.segments.is_empty() {
            self.indices.reserve(self.keys.len() - remove_count());
            self.segments.reserve(ceiling_div(self.keys.len(), I::SEGMENT_SIZE) + 1);
            let mut slice_index = 0;
            let mut accepted_keys = Vec::<u64>::new();  // first par_extend should set proper capacity
            self.segments.push(SegmentMetadata { first_index: 0, first_key: slice_index });
            for keys in self.keys.chunks(1<<18) {
                accepted_keys.clear();
                accepted_keys.par_extend(keys.par_chunks(64).map(|keys| {
                    let mut r = 0;
                    for (i, k) in keys.iter().enumerate() {
                        if filter(k) { r |= 1 << i; }
                    }
                    r
                }));
                self.append_segments_from_bitmap(&mut slice_index, &accepted_keys);
            }

            /*let mut accepted = [false; 1<<16];
            self.build_index(remove_count, |indices, keys, _| {
                accepted.par_iter_mut().zip(keys.into_par_iter()).for_each(|(v, k)| {
                    *v = filter(k);
                });
                for i in 0..keys.len() { if accepted[i] { indices.push(i as u16); } }
                indices.extend((0..keys.len()).filter(|i| buff[*i]));
            });*/
        } else {
            Self::par_retain_index(&mut self.indices, &mut self.segments, |k, _| filter(&self.keys[k]));
        }
    }

    fn retain_keys_with_indices<IF, F, P, R>(&mut self, mut index_filter: IF, _filter: F, retained_earlier: P, remove_count: R)
        where IF: FnMut(usize) -> bool, F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        let mut index = 0;
        self.retain_keys(|_| (index_filter(index), index += 1).0, retained_earlier, remove_count)
    }

    fn par_retain_keys_with_indices<IF, F, P, R>(&mut self, index_filter: IF, _filter: F, _retained_earlier: P, remove_count: R)
        where IF: Fn(usize) -> bool + Sync + Send, F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if self.segments.is_empty() {
            self.indices.reserve(self.keys.len() - remove_count());
            self.segments.reserve(ceiling_div(self.keys.len(), I::SEGMENT_SIZE) + 1);
            let mut slice_index = 0;
            let mut accepted_keys = Vec::<u64>::new();  // first par_extend should set proper capacity
            self.segments.push(SegmentMetadata { first_index: 0, first_key: slice_index });
            for keys_begin in (0..self.keys.len()).step_by(1<<18) {
                let keys_end = self.keys.len().min(keys_begin + (1<<18));
                accepted_keys.clear();
                accepted_keys.par_extend((keys_begin..keys_end).into_par_iter().step_by(64).map(|first_key| {
                    let mut r = 0;
                    for i in first_key..keys_end.min(first_key+64) {
                        if index_filter(i) { r |= 1u64 << (i-first_key); }
                    }
                    r
                }));
                self.append_segments_from_bitmap(&mut slice_index, &accepted_keys);
            }

            /*self.build_index(remove_count, |indices, keys, shift| {
                indices.par_extend(
                    (0..keys.len()).into_par_iter()
                        .filter_map(|key_nr| index_filter(shift + key_nr).then_some(I::from_usize(key_nr)))
                );
            })*/
        } else {
            Self::par_retain_index(&mut self.indices, &mut self.segments, |_, ii| index_filter(ii));
        }
    }
}

/// Trait for returning iterator and (optionally) parallel iterator over keys.
pub trait GetIterator {
    /// Key type.
    type Item;
    /// Iterator type.
    type Iterator: Iterator<Item = Self::Item>;
    /// Parallel iterator type.
    type ParallelIterator: ParallelIterator<Item = Self::Item> where Self::Item: Send;

    /// Returns iterator over keys.
    fn iter(&self) -> Self::Iterator;

    /// If possible, returns parallel iterator over keys.
    #[inline(always)] fn par_iter(&self) -> Option<Self::ParallelIterator> where Self::Item: Send { None }

    /// Returns `true` only if `par_iter` returns `Some`.
    #[inline(always)] fn has_par_iter(&self) -> bool { false /*self.par_iter().is_some()*/ }

    /// Returns the number of items produced by iterators returned by `iter` and `par_iter`.
    fn len(&self) -> usize {
        self.iter().count()   // TODO faster alternative
    }
}

impl<I: Iterator, F: Fn() -> I> GetIterator for F {
    type Item = I::Item;

    type Iterator = I;

    type ParallelIterator = rayon::iter::Empty<Self::Item> where Self::Item: Send;  // never used

    #[inline(always)] fn iter(&self) -> Self::Iterator {
        self()
    }
}

impl<I: Iterator, PI: ParallelIterator<Item = I::Item>, F: Fn() -> I, PF: Fn() -> PI> GetIterator for (F, PF) {
    type Item = I::Item;

    type Iterator = I;

    type ParallelIterator = PI where Self::Item: Send;

    #[inline(always)] fn iter(&self) -> Self::Iterator {
        self.0()
    }

    #[inline(always)] fn par_iter(&self) -> Option<Self::ParallelIterator> where Self::Item: Send {
        Some(self.1())
    }

    #[inline(always)] fn has_par_iter(&self) -> bool { true }
}

/// Implementation of [`KeySet`] that stores only the object that returns iterator over all keys
/// (the iterator can even expose the keys that have been removed earlier by `retain` methods).
/// It is usually a good idea to use it within [`CachedKeySet`], see [`CachedKeySet::dynamic`].
pub struct DynamicKeySet<GetKeyIter: GetIterator> {
    pub keys: GetKeyIter,
    pub len: usize,
}

impl<GetKeyIter: GetIterator> DynamicKeySet<GetKeyIter> {
    /// Constructs a [`DynamicKeySet`] that obtains the keys by `keys`.
    pub fn new(keys: GetKeyIter) -> Self {
        let len = keys.len();
        Self { keys, len }
    }

    /// Constructs a [`DynamicKeySet`] that obtains the keys by `keys` (which should produce `len` keys).
    pub fn with_len(keys: GetKeyIter, len: usize) -> Self {
        Self { keys, len }
    }
}

impl<GetKeyIter: GetIterator> KeySet<GetKeyIter::Item> for DynamicKeySet<GetKeyIter> where GetKeyIter::Item: Send {
    #[inline(always)] fn keys_len(&self) -> usize {
        self.len
    }

    #[inline(always)] fn has_par_for_each_key(&self) -> bool {
        self.keys.has_par_iter()
    }

    #[inline(always)] fn for_each_key<F, P>(&self, mut f: F, retained_hint: P)
        where F: FnMut(&GetKeyIter::Item), P: FnMut(&GetKeyIter::Item) -> bool
    {
        self.keys.iter().filter(retained_hint).for_each(|k| f(&k))
    }

    #[inline(always)] fn par_for_each_key<F, P>(&self, f: F, retained_hint: P)
        where F: Fn(&GetKeyIter::Item) + Sync + Send, P: Fn(&GetKeyIter::Item) -> bool + Sync + Send
    {
        if let Some(par_iter) = self.keys.par_iter() {
            par_iter.filter(retained_hint).for_each(|k| f(&k))
        } else {
            self.for_each_key(f, retained_hint)
        }
    }

    /*#[inline(always)] fn map_each_key<R, M, P>(&self, mut map: M, retained_hint: P) -> Vec<R>
            where M: FnMut(&GetKeyIter::Item) -> R, P: FnMut(&GetKeyIter::Item) -> bool
    {
        let mut result = Vec::with_capacity(self.len);
        self.keys.iter().filter(retained_hint).map(|k| map(&k)).collect_into(&mut result);
        result
    }*/

    #[inline(always)] fn par_map_each_key<R, M, P>(&self, map: M, retained_hint: P) -> Vec<R>
            where M: Fn(&GetKeyIter::Item)->R + Sync + Send, R: Send, P: Fn(&GetKeyIter::Item) -> bool + Sync + Send
    {
        if let Some(par_iter) = self.keys.par_iter() {
            // TODO somehow use information about len
            par_iter.filter(retained_hint).map(|k| map(&k)).collect()
        } else {
            self.map_each_key(map, retained_hint)
        }
    }

    #[inline(always)] fn retain_keys<F, P, R>(&mut self, _filter: F, _retained_earlier: P, mut remove_count: R)
        where F: FnMut(&GetKeyIter::Item) -> bool, P: FnMut(&GetKeyIter::Item) -> bool, R: FnMut() -> usize
    {
        self.len -= remove_count();
    }

    #[inline] fn into_vec<P>(self, retained_hint: P) -> Vec<GetKeyIter::Item>
    where P: FnMut(&GetKeyIter::Item) -> bool, Self: Sized
    {
        self.keys.iter().filter(retained_hint).collect()
    }

    #[inline] fn par_into_vec<P>(self, retained_hint: P) -> Vec<GetKeyIter::Item>
        where P: Fn(&GetKeyIter::Item) -> bool + Sync + Send, Self: Sized, GetKeyIter::Item: Send
    {
        if let Some(par_iter) = self.keys.par_iter() {
            par_iter.filter(retained_hint).collect()
        } else {
            self.keys.iter().filter(retained_hint).collect()
        }
    }
}

/// Implementation of [`KeySet`] that initially stores another [`KeySet`] 
/// (which is usually succinct but slow, such as [`DynamicKeySet`]),
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
    /// Constructs cached `key_set`. The keys are cloned and cached as soon as their number drops below `clone_threshold`.
    pub fn new(key_set: KS, clone_threshold: usize) -> Self {
        Self::Dynamic(key_set, clone_threshold)
    }
}

impl<K, PI: GetIterator> CachedKeySet<K, DynamicKeySet<PI>> {
    /// Constructs cached [`DynamicKeySet`] that obtains the keys by `keys` that returns iterator over keys.
    /// The keys are cloned and cached as soon as their number drops below `clone_threshold`.
    ///
    /// If the number of keys is known, it is more efficient to call [`CachedKeySet::dynamic_with_len`] instead.
    ///
    /// # Example
    ///
    /// ```
    /// use {ph::fmph::keyset::{KeySet, CachedKeySet}, rayon::iter::{ParallelIterator, IntoParallelIterator}};
    /// // Constructing a dynamic key set consisting of the squares of the integers from 1 to 100,
    /// // part of which will be cached by the first call of any of the retain methods:
    /// let ks = CachedKeySet::dynamic(|| (1..=100).map(|v| v*v), usize::MAX);
    /// assert_eq!(ks.keys_len(), 100);
    /// // Same as above but using multiple threads to generate keys:
    /// let ks = CachedKeySet::dynamic((|| (1..=100).map(|v| v*v), || (1..=100).into_par_iter().map(|v| v*v)), usize::MAX);
    /// assert_eq!(ks.keys_len(), 100);
    /// ```
    pub fn dynamic(keys: PI, clone_threshold: usize) -> Self {
        Self::new(DynamicKeySet::new(keys), clone_threshold)
    }

    /// Constructs cached [`DynamicKeySet`] that obtains the keys by `keys` that returns iterator over exactly `len` keys.
    /// The keys are cloned and cached as soon as their number drops below `clone_threshold`.
    /// 
    /// # Example
    /// 
    /// ```
    /// use {ph::fmph::keyset::{KeySet, CachedKeySet}, rayon::iter::{ParallelIterator, IntoParallelIterator}};
    /// // Constructing a dynamic key set consisting of the squares of the integers from 1 to 100,
    /// // part of which will be cached by the first call of any of the retain methods:
    /// let ks = CachedKeySet::dynamic_with_len(|| (1..=100).map(|v| v*v), 100, usize::MAX);
    /// assert_eq!(ks.keys_len(), 100);
    /// // Same as above but using multiple threads to generate keys:
    /// let ks = CachedKeySet::dynamic_with_len((|| (1..=100).map(|v| v*v), || (1..=100).into_par_iter().map(|v| v*v)), 100, usize::MAX);
    /// assert_eq!(ks.keys_len(), 100);
    /// ```
    pub fn dynamic_with_len(keys: PI, len: usize, clone_threshold: usize) -> Self {
        Self::new(DynamicKeySet::with_len(keys, len), clone_threshold)
    }
}

impl<'k, K: Sync> CachedKeySet<K, SliceSourceWithRefs<'k, K>> {
    /// Constructs cached [`SliceSourceWithRefs`] that wraps given `keys`.
    /// The keys are cloned and cached as soon as their number drops below `clone_threshold`.
    /// After cloning, the keys are placed in a continuous memory area which is friendly to the CPU cache.
    pub fn slice(keys: &'k [K], clone_threshold: usize) -> Self {
        Self::new(SliceSourceWithRefs::new(keys), clone_threshold)
    }
}

impl<K: Clone + Sync + Send, KS: KeySet<K>> KeySet<K> for CachedKeySet<K, KS>
{
    fn keys_len(&self) -> usize {
        match self {
            Self::Dynamic(dynamic_key_set, _) => dynamic_key_set.keys_len(),
            Self::Cached(v) => v.keys_len()
        }
    }

    fn has_par_for_each_key(&self) -> bool {
        match self {
            Self::Dynamic(dynamic_key_set, _) => dynamic_key_set.has_par_for_each_key(),
            Self::Cached(v) => v.has_par_for_each_key()
        }
    }

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
        where M: Fn(&K)->R + Sync + Send, R: Send, P: Fn(&K) -> bool + Sync + Send
    {
        match self {
            Self::Dynamic(dynamic_key_set, _) => dynamic_key_set.par_map_each_key(map, retained_hint),
            Self::Cached(v) => v.par_map_each_key(map, retained_hint)
        }
    }

    #[inline] fn retain_keys<F, P, R>(&mut self, mut filter: F, mut retained_earlier: P, remove_count: R)
        where F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        match self {
            Self::Dynamic(key_set, clone_threshold) => {
                key_set.retain_keys(&mut filter, &mut retained_earlier, remove_count);
                if key_set.keys_len() < *clone_threshold {
                    if let Self::Dynamic(key_set, _) = mem::take(self) {
                        *self = Self::Cached(key_set.into_vec(|k| retained_earlier(k) && filter(k)))
                    }
                }
            },
            Self::Cached(v) => v.retain_keys(filter, retained_earlier, remove_count)
        }
    }

    #[inline] fn par_retain_keys<F, P, R>(&mut self, filter: F, retained_earlier: P, remove_count: R)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        match self {
            Self::Dynamic(key_set, clone_threshold) => {
                key_set.par_retain_keys(&filter, &retained_earlier, remove_count);
                if key_set.keys_len() < *clone_threshold {
                    if let Self::Dynamic(key_set, _) = mem::take(self) {
                        *self = Self::Cached(key_set.par_into_vec(|k| retained_earlier(k) && filter(k)))
                    }
                }
            },
            Self::Cached(v) => v.par_retain_keys(filter, retained_earlier, remove_count)
        }
    }

    #[inline] fn retain_keys_with_indices<IF, F, P, R>(&mut self, index_filter: IF, mut filter: F, mut retained_earlier: P, remove_count: R)
        where IF: FnMut(usize) -> bool, F: FnMut(&K) -> bool, P: FnMut(&K) -> bool, R: FnMut() -> usize
    {
        match self {
            Self::Dynamic(key_set, clone_threshold) => {
                key_set.retain_keys_with_indices(index_filter, &mut filter, &mut retained_earlier, remove_count);
                if key_set.keys_len() < *clone_threshold {
                    if let Self::Dynamic(key_set, _) = mem::take(self) {
                        *self = Self::Cached(key_set.into_vec(|k| retained_earlier(k) && filter(k)))
                    }
                }
            },
            Self::Cached(v) => v.retain_keys_with_indices(index_filter, filter, retained_earlier, remove_count)
        }
    }

    #[inline] fn par_retain_keys_with_indices<IF, F, P, R>(&mut self, index_filter: IF, filter: F, retained_earlier: P, remove_count: R)
        where IF: Fn(usize) -> bool + Sync + Send, F: Fn(&K) -> bool + Sync + Send,
              P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        match self {
            Self::Dynamic(key_set, clone_threshold) => {
                key_set.par_retain_keys_with_indices(index_filter, &filter, &retained_earlier, remove_count);
                if key_set.keys_len() < *clone_threshold {
                    if let Self::Dynamic(key_set, _) = mem::take(self) {
                        *self = Self::Cached(key_set.par_into_vec(|k| retained_earlier(k) && filter(k)))
                    }
                }
            },
            Self::Cached(v) => v.par_retain_keys_with_indices(index_filter, filter, retained_earlier, remove_count)
        }
    }

    fn into_vec<P>(self, retained_hint: P) -> Vec<K>
    where P: FnMut(&K) -> bool, K: Clone, Self: Sized
    {
        match self {
            Self::Dynamic(key_set, _) => key_set.into_vec(retained_hint),
            Self::Cached(v) => v
        }
    }

    fn par_into_vec<P>(self, retained_hint: P) -> Vec<K>
        where P: Fn(&K) -> bool + Sync + Send, Self: Sized, K: Clone + Send
    {
        match self {
            Self::Dynamic(key_set, _) => key_set.par_into_vec(retained_hint),
            Self::Cached(v) => v
        }
    }
}