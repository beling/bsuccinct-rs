use crate::bits_to_store_any_of_ref;

use super::CollisionSolver;

/// Moves all non-zeros to the begging of `values` and returns their number.
pub fn remove_zeros(values: &mut [usize]) -> usize {
    let mut new_len: usize = 0usize;
    for i in 0usize..values.len() {
        if values[i] != 0 {
            values[new_len] = values[i];
            new_len += 1;
        }
    }
    new_len
}

/// A trait for accessing and managing sets of key (of the type `K`) and value pairs
/// during construction of [`fp::Map`](super::Map) or [`fp::GOMap`](super::GOMap).
pub trait KVSet<K> {
    /// Returns number of retained key-value pairs. Guarantee to be very fast.
    fn kv_len(&self) -> usize;

    /// Call `f` for each key-value pair in the set, using single thread.
    ///
    /// If `self` doesn't remember which keys are retained it uses `retained_hint` to check this.
    fn for_each_key_value<F>(&self, f: F/*, retained_hint: P*/) where F: FnMut(&K, u8)/*, P: FnMut(&K) -> bool*/;

    /// Call `f` for each key in the set, using single thread.
    ///
    /// If `self` doesn't remember which keys are retained it uses `retained_hint` to check this.
    #[inline(always)]
    fn for_each_key<F>(&self, mut f: F/*, retained_hint: P*/) where F: FnMut(&K)/*, P: FnMut(&K) -> bool*/ {
        self.for_each_key_value(|k, _| f(k));
    }

    /// Call `collision_solver.process_value(key_to_index(key), value, self.bits_per_value())` for each `key`-`value` pair.
    #[inline]
    fn process_all_values<I, CS>(&self, mut key_to_index: I, collision_solver: &mut CS)
        where I: FnMut(&K) -> usize, CS: CollisionSolver
    {
        let bits_per_value = self.bits_per_value();
        self.for_each_key_value(|key, value| {
            collision_solver.process_value(key_to_index(key), value, bits_per_value);
        });
    }

    /// Returns minimal number of bits that can store any value.
    fn bits_per_value(&self) -> u8;

    /// Returns the (non-zero) numbers of all the different remaining values. 
    fn value_distribution(&self/*, retained_hint: P*/) -> Box<[usize]>;

    /// Calls `map` for each key-value pair in the set, and returns outputs of these calls. Uses single thread.
    ///
    /// If `self` doesn't remember which keys are retained it uses `retained_hint` to check this.
    fn map_each_key_value<R, M>(&self, mut map: M/*, retained_hint: P*/) -> Vec<R>
        where M: FnMut(&K, u8) -> R/*, P: FnMut(&K) -> bool*/
    {
        let mut result = Vec::with_capacity(self.kv_len());
        self.for_each_key_value(|k, v| result.push(map(k, v))/*, retained_hint*/);
        result
    }

    /// Retains in `self` keys pointed by the `filter` and remove the rest, using single thread.
    /// - `filter` shows the keys to be retained (the result of the function can be unspecified for keys removed earlier),
    /// - `retained_earlier` shows the keys that have not been removed earlier,
    /// - `remove_count` returns number of keys to remove.
    fn retain_keys<F>(&mut self, filter: F/*, retained_earlier: P, remove_count: R*/)
        where F: FnMut(&K) -> bool/*, P: FnMut(&K) -> bool, R: FnMut() -> usize*/;

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
    fn retain_keys_with_indices<IF, F>(&mut self, _index_filter: IF, filter: F/*, retained_earlier: P, remove_count: R*/)
        where IF: FnMut(usize) -> bool, F: FnMut(&K) -> bool/*, P: FnMut(&K) -> bool, R: FnMut() -> usize*/
    {
        self.retain_keys(filter/*, retained_earlier, remove_count*/)
    }

    /// Convert `self` into the vector of retained key-value pairs.
    /// 
    /// If `self` doesn't remember which keys are retained it uses `retained_hint` to check this.
    #[inline] fn into_vec<P>(self/*, retained_hint: P*/) -> Vec<(K, u8)>    // TODO maybe return a pair of vectors
        where P: FnMut(&K) -> bool, K: Clone, Self: Sized
    {
        self.map_each_key_value(|k, v| ((*k).clone(), v)/*, retained_hint*/)
    }
}

/*impl<K, S> KVSet<K> for HashMap<K, u8, S> {
    #[inline] fn kv_len(&self) -> usize { self.len() }

    fn for_each_key_value<F, P>(&self, mut f: F/*, _retained_hint: P*/) where F: FnMut(&K, u8)/*, P: FnMut(&K) -> bool*/ {
        for (k, v) in self { f(k, *v) }
    }

    fn bits_per_value(&self) -> u8 {    // TODO wrong, should always returns the same
        bits_to_store_any_of_ref(self.values())
    }

    fn retain_keys<F, P, R>(&mut self, mut filter: F, _retained_earlier: P, _remove_count: R)
        where F: FnMut(&K) -> bool/*, P: FnMut(&K) -> bool*/, R: FnMut() -> usize
    {
        self.retain(|k, _| filter(k));
    }
}

impl<K: Ord> KVSet<K> for BTreeMap<K, u8> {
    #[inline] fn kv_len(&self) -> usize { self.len() }

    fn for_each_key_value<F, P>(&self, mut f: F/*, _retained_hint: P*/) where F: FnMut(&K, u8)/*, P: FnMut(&K) -> bool*/ {
        for (k, v) in self { f(k, *v) }
    }

    fn bits_per_value(&self) -> u8 {    // TODO wrong, should always returns the same
        bits_to_store_any_of_ref(self.values())
    }

    fn retain_keys<F, P, R>(&mut self, mut filter: F, _retained_earlier: P, _remove_count: R)
        where F: FnMut(&K) -> bool/*, P: FnMut(&K) -> bool*/, R: FnMut() -> usize
    {
        self.retain(|k, _| filter(k));
    }
}*/

/// Implements [`KVSet`], storing keys and values in the mutable slices.
///
/// Retain operations reorder the slices, putting retained items at the beginning of the slice.
pub struct SlicesMutSource<'k, K> {
    /// All keys (retained ones occupy `len` beginning indices).
    pub keys: &'k mut [K],
    /// All values (retained ones occupy `len` beginning indices).
    pub values: &'k mut [u8],
    /// How many first keys and values are retained.
    pub len: usize,
    /// Number of bits per each value.
    bits_per_value: u8
}

impl<'k, K> SlicesMutSource<'k, K> {
    /// Constructs [`SlicesMutSource`] with given `keys`, `values` and `bits_per_value` (which can be `0` for auto-detection)
    pub fn new(keys: &'k mut [K], values: &'k mut [u8], mut bits_per_value: u8) -> Self {
        let len = keys.len();
        let vlen = values.len();
        assert_eq!(len, vlen, "key and value slices must be of the same length, but are {len} and {vlen} respectively");
        if bits_per_value == 0 { bits_per_value = bits_to_store_any_of_ref(values.iter()); }
        Self { keys, values, len, bits_per_value }
    }
}

impl<'k, K> KVSet<K> for SlicesMutSource<'k, K> {
    #[inline(always)] fn kv_len(&self) -> usize { self.len }

    #[inline(always)] fn for_each_key_value<F>(&self, mut f: F/*, _retained_hint: P*/) where F: FnMut(&K, u8)/*, P: FnMut(&K) -> bool*/ {
        for (k, v) in self.keys[0..self.len].iter().zip(self.values[0..self.len].iter()) {
            f(k, *v); 
        }
    }

    #[inline] fn bits_per_value(&self) -> u8 {
        self.bits_per_value
    }

    fn value_distribution(&self/*, _retained_hint: P*/) -> Box<[usize]> {
        let mut counts = [0usize; 256];
        for v in self.values.iter() { counts[*v as usize] += 1; }
        let counts_len = remove_zeros(&mut counts);
        counts[0..counts_len].into_iter().cloned().collect()
    }

    #[inline(always)] fn map_each_key_value<R, M>(&self, mut map: M/*, _retained_hint: P*/) -> Vec<R>
    where M: FnMut(&K, u8) -> R/*, P: FnMut(&K) -> bool*/
    {
        self.keys[0..self.len].into_iter().zip(self.values[0..self.len].into_iter()).map(|(k, v)| map(k, *v)).collect()
    }

    fn retain_keys<F>(&mut self, mut filter: F/*, _retained_earlier: P, _remove_count: R*/)
        where F: FnMut(&K) -> bool/*, P: FnMut(&K) -> bool, R: FnMut() -> usize*/
    {
        let mut i = 0usize;
        while i < self.len {
            if filter(&self.keys[i]) {
                i += 1;
            } else {
                // remove i-th element by replacing it with the last one
                self.len -= 1;
                self.keys.swap(i, self.len);
                self.values.swap(i, self.len);
            }
        }
    }
    

}
