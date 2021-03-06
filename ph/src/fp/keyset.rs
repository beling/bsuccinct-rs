use rayon::ThreadPool;

use rayon::prelude::*;

pub trait KeySet<K> {
    /// Returns number of retained keys.
    fn keys_len(&self) -> usize;

    #[inline(always)] fn has_par_for_each_key(&self) -> bool { false }

    #[inline(always)] fn has_par_retain_keys(&self) -> bool { false }

    /// Call `f` for each key in the set.
    ///
    /// If `self` doesn't remember which keys are retained it uses `retained_hint` to check this.
    fn for_each_key<F, P>(&self, f: F, retained_hint: P)
        where F: FnMut(&K), P: Fn(&K) -> bool;

    /// Call `f` (or `par_f` in the case of multiple thread calculations) for each key in the set.
    ///
    /// If `self` doesn't remember which keys are retained it uses `retained_hint` to check this.
    #[inline(always)]
    fn par_for_each_key<F, P>(&self, f: F, retained_hint: P, _thread_pool: &ThreadPool)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        self.for_each_key(f, retained_hint);
    }

    /// Retains in `self` keys pointed by the `filter` and remove the rest.
    ///
    /// `filter` shows which keys are not removed at the last level,
    /// `retained_earlier` shows which keys are not removed at previous levels,
    /// `retained_count` returns number of keys retained.
    fn retain_keys<F, P, R>(&mut self, filter: F, retained_earlier: P, retained_count: R, thread_pool: Option<&ThreadPool>)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize;
}

impl<K: Sync + Send> KeySet<K> for Vec<K> {
    #[inline(always)] fn keys_len(&self) -> usize {
        self.len()
    }

    #[inline(always)] fn has_par_for_each_key(&self) -> bool { true }

    #[inline(always)] fn has_par_retain_keys(&self) -> bool { true }

    #[inline(always)] fn for_each_key<F, P>(&self, f: F, _retained_hint: P)
        where F: FnMut(&K), P: Fn(&K) -> bool
    {
        self.iter().for_each(f)
    }

    #[inline(always)] fn par_for_each_key<F, P>(&self, f: F, _retained_hint: P, thread_pool: &ThreadPool)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        thread_pool.install(|| { self.into_par_iter().for_each(f) });
    }

    /*#[inline(always)] fn retain_keys<F, R>(&mut self, filter: F, _removed_count: R, _thread_pool: Option<&ThreadPool>)
        where F: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        self.retain(filter)
    }*/

    #[inline(always)] fn retain_keys<F, P, R>(&mut self, filter: F, _retained_earlier: P, _retained_count: R, thread_pool: Option<&ThreadPool>)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if let Some(thread_pool) = thread_pool {
            thread_pool.install(|| { *self = (std::mem::take(self)).into_par_iter().filter(filter).collect(); })
        } else {
            self.retain(filter)
        }
    }
}

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

    #[inline(always)] fn for_each_key<F, P>(&self, f: F, _retained_hint: P) where F: FnMut(&K), P: Fn(&K) -> bool {
        self.slice[0..self.len].iter().for_each(f)
    }

    #[inline(always)] fn par_for_each_key<F, P>(&self, f: F, _retained_hint: P, _thread_pool: &ThreadPool)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        self.slice[0..self.len].into_par_iter().for_each(f)
    }

    fn retain_keys<F, P, R>(&mut self, filter: F, _retained_hint: P, _retained_count: R, _thread_pool: Option<&ThreadPool>)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
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

    #[inline(always)] fn for_each_key<F, P>(&self, f: F, _retained_hint: P) where F: FnMut(&K), P: Fn(&K) -> bool {
        if let Some(ref retained) = self.retained {
            retained.for_each_key(f, _retained_hint)
        } else {
            self.slice.into_iter().for_each(f)
        }
    }

    #[inline(always)] fn par_for_each_key<F, P>(&self, f: F, _retained_hint: P, thread_pool: &ThreadPool)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        if let Some(ref retained) = self.retained {
            retained.par_for_each_key(f, _retained_hint, thread_pool)
        } else {
            thread_pool.install(|| {
                (*self.slice).into_par_iter().for_each(f);
            })
        }
    }

    fn retain_keys<F, P, R>(&mut self, filter: F, retained_earlier: P, retained_count: R, thread_pool: Option<&ThreadPool>)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if let Some(ref mut retained) = self.retained {
            retained.retain_keys(filter, retained_earlier, retained_count, thread_pool)
        } else {
            if let Some(thread_pool) = thread_pool {
                thread_pool.install(|| {
                    self.retained = Some(self.slice.into_par_iter().filter_map(|k|filter(k).then(|| k.clone())).collect());
                })
            } else {
                self.retained = Some(self.slice.into_iter().filter_map(|k|filter(k).then(|| k.clone())).collect());
            }
        }
    }
}


pub struct SliceSourceWithRefs<'k, K> {
    slice: &'k [K],
    retained: Option<Vec<Vec<u16>>>,
}

impl<'k, K: Sync> SliceSourceWithRefs<'k, K> {
    pub fn new(slice: &'k [K]) -> Self {
        Self { slice, retained: None }
    }
}

impl<'k, K: Sync> KeySet<K> for SliceSourceWithRefs<'k, K> {
    fn keys_len(&self) -> usize {
        if let Some(ref indices) = self.retained {
            indices.iter().map(|v| v.len()).sum()
        } else {
            self.slice.len()
        }
    }

    #[inline(always)] fn has_par_for_each_key(&self) -> bool { true }

    #[inline(always)] fn has_par_retain_keys(&self) -> bool { true }

    #[inline(always)] fn for_each_key<F, P>(&self, mut f: F, _retained_hint: P) where F: FnMut(&K), P: Fn(&K) -> bool {
        if let Some(ref indices) = self.retained {
            for (i, v) in indices.into_iter().zip(self.slice.chunks(1<<16)) {
                i.into_iter().for_each(|i| f(unsafe{v.get_unchecked(*i as usize)}));
            }
        } else {
            self.slice.into_iter().for_each(f);
        }
    }

    #[inline(always)] fn par_for_each_key<F, P>(&self, f: F, _retained_hint: P, thread_pool: &ThreadPool)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        thread_pool.install(|| {
            if let Some(ref indices) = self.retained {
                for (i, v) in indices.into_iter().zip(self.slice.chunks(1<<16)) {
                    i.into_par_iter().for_each(|i| f(unsafe{v.get_unchecked(*i as usize)}));
                }
                /*indices.into_par_iter().zip(self.slice.into_par_iter().chunks(1<<16)).for_each(|(i, v)| {
                    i.into_par_iter().for_each(|i| f(unsafe{v.get_unchecked(*i as usize)}));
                });*/
            } else {
                (*self.slice).into_par_iter().for_each(f);
            }
        })
    }

    fn retain_keys<F, P, R>(&mut self, filter: F, _retained_hint: P, _retained_count: R, thread_pool: Option<&ThreadPool>)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if let Some(thread_pool) = thread_pool {
            thread_pool.install(|| {
                if let Some(ref mut r) = self.retained {
                    let slice = self.slice; // fixes "cannot borrow `self` as immutable because it is also borrowed as mutable"
                    r.into_par_iter().enumerate().for_each(|(mut ci, c)| {
                        ci <<= 16;
                        c.retain(|i| filter(unsafe{slice.get_unchecked(ci | (*i as usize))}));
                    });
                } else {
                    self.retained = Some(self.slice.chunks(1 << 16).map(|c|
                        c.into_par_iter().enumerate().filter_map(|(i, k)| filter(k).then(|| i as u16)).collect()
                    ).collect());
                }
            });
        } else {
            if let Some(ref mut r) = self.retained {
                let slice = self.slice; // fixes "cannot borrow `self` as immutable because it is also borrowed as mutable"
                for (mut ci, c) in r.into_iter().enumerate() {
                    ci <<= 16;
                    c.retain(|i| filter(unsafe{slice.get_unchecked(ci | (*i as usize))}));
                }
            } else {
                self.retained = Some(self.slice.chunks(1 << 16).map(|c|
                    c.into_iter().enumerate().filter_map(|(i, k)| filter(k).then(|| i as u16)).collect()
                ).collect());
            }
        }
    }
}


pub struct SliceSourceWithRefsEmptyCleaning<'k, K> {
    slice: &'k [K],
    //retained: Option<Vec<Vec<u16>>>,
    retained: Option<Vec<(usize, Vec<u16>)>>,
}

impl<'k, K: Sync> SliceSourceWithRefsEmptyCleaning<'k, K> {
    pub fn new(slice: &'k [K]) -> Self {
        Self { slice, retained: None }
    }
}


impl<'k, K: Sync> KeySet<K> for SliceSourceWithRefsEmptyCleaning<'k, K> {
    fn keys_len(&self) -> usize {
        if let Some(ref indices) = self.retained {
            indices.iter().map(|v| v.1.len()).sum()
        } else {
            self.slice.len()
        }
    }

    #[inline(always)] fn has_par_for_each_key(&self) -> bool { true }

    #[inline(always)] fn has_par_retain_keys(&self) -> bool { true }

    fn for_each_key<F, P>(&self, mut f: F, _retained_hint: P) where F: FnMut(&K), P: Fn(&K) -> bool {
        if let Some(ref indices) = self.retained {
            for (shift, indices) in indices {
                let slice = &self.slice[*shift..];
                indices.into_iter().for_each(|i| f(unsafe{slice.get_unchecked(*i as usize)}));
            }
        } else {
            self.slice.into_iter().for_each(f);
        }
    }

    fn par_for_each_key<F, P>(&self, f: F, _retained_hint: P, _thread_pool: &ThreadPool)
        where F: Fn(&K) + Sync + Send, P: Fn(&K) -> bool + Sync + Send
    {
        if let Some(ref indices) = self.retained {
            for (shift, indices) in indices {
                let slice = &self.slice[*shift..];
                indices.into_par_iter().for_each(|i| f(unsafe{slice.get_unchecked(*i as usize)}));
            }
        } else {
            (*self.slice).into_par_iter().for_each(f);
        }
    }

    fn retain_keys<F, P, R>(&mut self, filter: F, _retained_hint: P, _retained_count: R, thread_pool: Option<&ThreadPool>)
        where F: Fn(&K) -> bool + Sync + Send, P: Fn(&K) -> bool + Sync + Send, R: Fn() -> usize
    {
        if let Some(thread_pool) = thread_pool {
            thread_pool.install(|| {
                if let Some(ref mut r) = self.retained {
                    r.into_par_iter().for_each(|(shift, indices)| {
                        let slice = &self.slice[*shift..];
                        indices.retain(|i| filter(unsafe { slice.get_unchecked(*i as usize) }));
                    });
                    r.retain(|(_, v)| !v.is_empty());   // parallel version?
                } else {
                    self.retained = Some(self.slice.chunks(1 << 16).enumerate().filter_map(|(ci, slice)| {
                        let v: Vec<_> = slice.into_par_iter().enumerate().filter_map(|(i, k)| filter(k).then(|| i as u16)).collect();
                        (!v.is_empty()).then(|| (ci << 16, v))
                    }).collect());
                }
            });
        } else {
            if let Some(ref mut r) = self.retained {
                r.retain_mut(|(shift, indices)| {
                    let slice = &self.slice[*shift..];
                    indices.retain(|i| filter(unsafe { slice.get_unchecked(*i as usize) }));
                    !indices.is_empty()
                });
            } else {
                self.retained = Some(self.slice.chunks(1 << 16).enumerate().filter_map(|(ci, slice)| {
                    let v: Vec<_> = slice.into_iter().enumerate().filter_map(|(i, k)| filter(k).then(|| i as u16)).collect();
                    (!v.is_empty()).then(|| (ci << 16, v))
                }).collect());
            }
        }
    }
}

pub struct DynamicKeySet<KeyIter: Iterator, GetKeyIter: Fn() -> KeyIter> {
    pub keys: GetKeyIter,
    pub len: usize
}

impl<KeyIter: Iterator, GetKeyIter: Fn() -> KeyIter> DynamicKeySet<KeyIter, GetKeyIter>{
    pub fn new(keys: GetKeyIter) -> Self {
        let len = keys().count();   // TODO faster alternative
        Self { keys, len }
    }
}

impl<KeyIter: Iterator, GetKeyIter: Fn() -> KeyIter> KeySet<KeyIter::Item> for DynamicKeySet<KeyIter, GetKeyIter> {
    #[inline(always)] fn keys_len(&self) -> usize {
        self.len
    }

    fn for_each_key<F, P>(&self, mut f: F, retained_hint: P)
        where F: FnMut(&KeyIter::Item), P: Fn(&KeyIter::Item) -> bool
    {
        (self.keys)().filter(retained_hint).for_each(|k| f(&k))
    }

    #[inline] fn retain_keys<F, P, R>(&mut self, _filter: F, _retained_earlier: P, retains_count: R, _thread_pool: Option<&ThreadPool>)
        where F: Fn(&KeyIter::Item) -> bool + Sync + Send, P: Fn(&KeyIter::Item) -> bool + Sync + Send, R: Fn() -> usize
    {
        self.len = retains_count();
    }
}

pub enum CachedDynamicKeySet<KeyIter: Iterator, GetKeyIter: Fn() -> KeyIter> {
    Dynamic((DynamicKeySet<KeyIter, GetKeyIter>, usize)),
    Cached(Vec<KeyIter::Item>)
}

impl<KeyIter: Iterator, GetKeyIter: Fn() -> KeyIter> CachedDynamicKeySet<KeyIter, GetKeyIter>
    where KeyIter::Item: Sync + Send + Clone
{
    pub fn with_keys_len_threshold(keys: GetKeyIter, len: usize, clone_threshold: usize) -> Self {
        if len < clone_threshold {
            let mut cache = Vec::with_capacity(len);
            for k in keys() {
                cache.push(k.clone())
            }
            Self::Cached(cache)
        } else {
            Self::Dynamic((DynamicKeySet{keys, len}, clone_threshold))
        }
    }

    pub fn with_keys_threshold(keys: GetKeyIter, clone_threshold: usize) -> Self {
        let len = keys().count();
        Self::with_keys_len_threshold(keys, len, clone_threshold)
    }
}

impl<KeyIter: Iterator, GetKeyIter: Fn() -> KeyIter> KeySet<KeyIter::Item> for CachedDynamicKeySet<KeyIter, GetKeyIter>
where KeyIter::Item: Sync + Send + Clone
{
    fn keys_len(&self) -> usize {
        match self {
            Self::Dynamic((dynamic_key_set, _)) => dynamic_key_set.keys_len(),
            Self::Cached(v) => v.len()
        }
    }

    fn has_par_for_each_key(&self) -> bool { true }  // as it is true for cached version

    fn has_par_retain_keys(&self) -> bool { true }  // as it is true for cached version

    #[inline]
    fn for_each_key<F, P>(&self, f: F, retained_hint: P)
        where F: FnMut(&KeyIter::Item), P: Fn(&KeyIter::Item) -> bool
    {
        match self {
            Self::Dynamic((dynamic_key_set, _)) => dynamic_key_set.for_each_key(f, retained_hint),
            Self::Cached(v) => v.for_each_key(f, retained_hint)
        }
    }

    #[inline]
    fn par_for_each_key<F, P>(&self, f: F, retained_hint: P, thread_pool: &ThreadPool)
        where F: Fn(&KeyIter::Item) + Sync + Send, P: Fn(&KeyIter::Item) -> bool + Sync + Send
    {
        match self {
            Self::Dynamic((dynamic_key_set, _)) => dynamic_key_set.par_for_each_key(f, retained_hint, thread_pool),
            Self::Cached(v) => v.par_for_each_key(f, retained_hint, thread_pool)
        }
    }

    fn retain_keys<F, P, R>(&mut self, filter: F, retained_earlier: P, retained_count: R, thread_pool: Option<&ThreadPool>)
        where F: Fn(&KeyIter::Item) -> bool + Sync + Send, P: Fn(&KeyIter::Item) -> bool + Sync + Send, R: Fn() -> usize
    {
        match self {
            Self::Dynamic((dynamic_key_set, clone_threshold)) => {
                let len = retained_count();
                if len < *clone_threshold {
                    let mut cache = Vec::with_capacity(len);
                    for k in (dynamic_key_set.keys)() {
                        if retained_earlier(&k) && filter(&k) {
                            cache.push(k.clone())
                        }
                    }
                    *self = Self::Cached(cache);
                } else {
                    dynamic_key_set.len = len;
                }
            },
            Self::Cached(v) => v.retain_keys(filter, retained_earlier, retained_count, thread_pool)
        }
    }
}