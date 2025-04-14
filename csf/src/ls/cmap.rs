use std::io;
use std::hash::{BuildHasherDefault, Hash};
use std::collections::hash_map::DefaultHasher;
use super::Map;
use crate::coding::{Coding, Decoder, SerializableCoding, BuildCoding};
use super::conf::{MapConf, ValuesPreFiller};
use bitm::{BitAccess, BitVec};
use ph::stats::AccessStatsCollector;
use ph::{BuildDefaultSeededHasher, BuildSeededHasher};
use std::collections::HashMap;
use dyn_size_of::GetSize;
use minimum_redundancy::{BitsPerFragment, DecodingResult};

/*pub struct KeyCodesIterator<'k, Key, Value, KeyValueIterator>
where Value: 'k, KeyValueIterator: Iterator<Item=(&'k Key, &'k Value)>
{
    key_value_iterator: KeyValueIterator,
    codes: HashMap<Value, Code>,
    current_code: Code,
    current_key: &'k Key,
    fragment_number: u8
}

impl<'k, Key, Value, KeyValueIterator> Iterator for KeyCodesIterator<'k, Key, Value, KeyValueIterator>
    where Value: Hash+Eq+Clone, KeyValueIterator: Iterator<Item=(&'k Key, &'k Value)>
{
    type Item = ((&'k Key, u8), u32);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_code.is_empty() {
            if let Some((key, value)) = self.key_value_iterator.next() {
                self.current_code = self.codes[value];
                self.current_key = key;
                self.fragment_number = 0;
            } else {
                return None;
            }
        }
        let result = Some(((self.current_key, self.fragment_number), self.current_code.extract_first()));
        self.fragment_number += 1;
        result
    }
}*/

/*
/// Wrapper over key/values iterator that is an iterator over (key, number of value fragments).
pub struct BDZHKeyIterator<'k, 'cl, Key, Value, KeyValueIterator, Enc>
    where Value: std::borrow::Borrow<Enc::Value> + 'k,
          KeyValueIterator: Iterator<Item=(&'k Key, &'k Value)>,
          Enc: Encoder
{
    /// key/values iterator
    key_value_iterator: KeyValueIterator,
    /// maps values to number of fragments
    code_lengths: &'cl Enc,
    /// key used in recent iteration
    current_key: Option<&'k Key>,
    /// number of fragments in value assigned to `current_key`
    current_code_len: u8,
    /// number of value fragment used in recent iteration
    fragment_number: u8
}

impl<'k, 'cl, Key, Value, KeyValueIterator, Enc> BDZHKeyIterator<'k, 'cl, Key, Value, KeyValueIterator, Enc>
    where Value: std::borrow::Borrow<Enc::Value> + 'k,
          KeyValueIterator: Iterator<Item=(&'k Key, &'k Value)>,
          Enc: Encoder
{
    pub fn new(key_value_iterator: KeyValueIterator, code_lengths: &'cl Enc) -> Self {
        Self { key_value_iterator, code_lengths, current_key: None, current_code_len: 0, fragment_number: 0 }
    }
}

impl<'k, 'cl, Key, Value, KeyValueIterator, Enc> Iterator for BDZHKeyIterator<'k, 'cl, Key, Value, KeyValueIterator, Enc>
    where Value: std::borrow::Borrow<Enc::Value> + 'k,
          KeyValueIterator: Iterator<Item=(&'k Key, &'k Value)>,
          Enc: Encoder
{
    type Item = (&'k Key, u8);

    fn next(&mut self) -> Option<Self::Item> {
        if self.fragment_number == self.current_code_len {
            if let Some((key, value)) = self.key_value_iterator.next() {
                self.current_code_len = self.code_lengths.code_len(value);
                self.current_key = Some(key);
                self.fragment_number = 0;
            } else {
                return None;
            }
        }
        let result = Some((self.current_key.unwrap(), self.fragment_number));
        self.fragment_number += 1;
        result
    }
}

impl<'k, 'cl, Key, Value, KeyValueIterator, Enc> FusedIterator for BDZHKeyIterator<'k, 'cl, Key, Value, KeyValueIterator, Enc>
where Value: std::borrow::Borrow<Enc::Value> + 'k, KeyValueIterator: Iterator<Item=(&'k Key, &'k Value)>, Enc: Encoder {}*/

/*pub struct BDZHKeyIntoIterator<'k, Key, Value, KeyValueIntoIterator>
    where Key: 'k, Value: 'k, KeyValueIntoIterator: Copy + IntoIterator<Item=(&'k Key, &'k Value)> + 'k
{
    into_key_value_iterator: KeyValueIntoIterator,
    code_lengths: HashMap<Value, u8>,
}

impl<'k, 'cl, Key, Value, KeyValueIntoIterator> IntoIterator for &'cl BDZHKeyIntoIterator<'k, Key, Value, KeyValueIntoIterator>
    where Value: Hash + Eq + Clone, KeyValueIntoIterator:  Copy + IntoIterator<Item=(&'k Key, &'k Value)> + 'k
{
    type Item = (&'k Key, u8);
    type IntoIter = BDZHKeyIterator<'k, 'cl, Key, Value, KeyValueIntoIterator::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter::new(self.into_key_value_iterator.into_iter(), &self.code_lengths)
    }
}*/

/// Compressed static function (immutable map) that maps hashable keys to values of any type.
/// 
/// To represent a function *f:X→Y*, it uses the space slightly larger than *|X|H*
/// (the overhead is 23% or slightly more),
/// where *H* is the entropy of the distribution of the *f* values over *X*.
/// The time complexity is *O(c)* for evaluation and *O(|X|c)* for construction
/// (not counting building the encoding dictionary),
/// where *c* is the average codeword length (given in code fragments) of the values.
/// 
/// It uses [`Map`] based on solving linear system to store fragments of value codes and
/// usually [`minimum_redundancy::Coding`] to compress values.
pub struct CMap<C /*= Coding<V>*/, S = BuildDefaultSeededHasher> {
    pub value_fragments: Map<S>,
    pub value_coding: C,
}

impl<C: GetSize, S> GetSize for CMap<C, S> {
    fn size_bytes_dyn(&self) -> usize {
        self.value_fragments.size_bytes_dyn() + self.value_coding.size_bytes_dyn()
    }
    const USES_DYN_MEM: bool = Map::<S>::USES_DYN_MEM || C::USES_DYN_MEM;
}

impl<C: SerializableCoding, S> CMap<C, S> {
    /// Returns the number of bytes which `write` will write, assuming that each call to `write_value` writes `bytes_per_value` bytes.
    pub fn write_bytes(&self, bytes_per_value: usize) -> usize {
        self.value_fragments.write_bytes() + self.value_coding.write_bytes(bytes_per_value)
    }

    /// Writes `self` to the `output` using `write_value` to write values.
    pub fn write<F>(&self, output: &mut dyn io::Write, write_value: F) -> io::Result<()>
        where F: FnMut(&mut dyn io::Write, &C::Value) -> io::Result<()>
    {
        self.value_fragments.write(output)?;
        self.value_coding.write(output, write_value)
    }

    /// Reads `self` from the input, using `read_value` to read values (`hasher` must be the same as used by stored `BDZHMap`).
    pub fn read_with_hasher<F>(input: &mut dyn io::Read, read_value: F, hasher: S) -> io::Result<Self>
        where F: FnMut(&mut dyn io::Read) -> io::Result<C::Value>
    {
        Ok(Self {
            value_fragments: Map::<S>::read_with_hasher(input, hasher)?,
            value_coding: C::read(input, read_value)?
        })
    }
}

impl<C: SerializableCoding> CMap<C, BuildHasherDefault<DefaultHasher>> {
    /// Reads `BDZHMap` from the input using `read_value` to read values.
    /// Only `BDZHMap`s that use default hasher can be read by this method.
    pub fn read<F>(input: &mut dyn io::Read, read_value: F) -> io::Result<Self>
        where F: FnMut(&mut dyn io::Read) -> io::Result<C::Value>
    {
        Self::read_with_hasher(input, read_value, Default::default())
    }
}

impl<C: Coding, /*V: Hash+Eq+Clone,*/ S: BuildSeededHasher> CMap<C, S> {

    //fn value_vec(codes: &HashMap<V, Code>, ) -> Box<[u64]>

    /// Underlying [Map] uses `value_coding.bits_per_fragment()+extra_bits_per_fragment` bits per fragment.
    /// `extra_bits_per_fragment>0` increases a chance of detection absence of the key by `get` and `get_stats`.
    pub fn try_from_mapf_with_coding_conf<'a, K, V, KvIntoIter, FKvIntoIter, BM>(
        map: FKvIntoIter, value_coding: C, conf: MapConf<BM, S>, extra_bits_per_fragment: u8
    ) -> Option<Self>
        where K: Hash + 'a,
              V: std::borrow::Borrow<<C as Coding>::Value> + 'a,
              KvIntoIter: IntoIterator<Item=(&'a K, &'a V)> + 'a,
              FKvIntoIter: Fn() -> KvIntoIter,
              BM: ValuesPreFiller // buffer creator (and initializer)
    {
        let encoder = value_coding.encoder();
        let keys_len = map().into_iter().map(|(_,v)| value_coding.len_of_encoded(&encoder, v) as usize).sum();
        let mut values = Box::<[u64]>::with_zeroed_bits(value_coding.bits_per_fragment() as usize*keys_len);
        let mut values_len = 0;
        for (_, v) in map() {
            for f in value_coding.fragments_of_encoded(&encoder, v) {
                values.init_fragment(values_len, f as u64, value_coding.bits_per_fragment());
                values_len += 1;
            }
        }
        debug_assert_eq!(values_len, keys_len);
        //if let Some(value_fragments) = Map::try_with_conf_bitset(&keys, &values, value_coding.bits_per_fragment, conf) {
        /*if let Some(value_fragments) = Map::try_with_conf_fn(
           || BDZHKeyIterator::new( map().into_iter(), &code_lengths),
            keys_len,
            |index, _bdz_bits_per_value| values.get_fragment(index, value_coding.bits_per_fragment),
           value_coding.bits_per_fragment+bdz_extra_bits_per_fragment,
            conf)
        {
            Some(Self { value_fragments, value_coding })
        } else { None }*/

        let r = Map::try_with_conf_fn(
            || //BDZHKeyIterator::new( map().into_iter(), &codes),
                map().into_iter()
                    .flat_map(|(k, v)|
                         value_coding.fragments_of_encoded(&encoder, v)
                            .enumerate().map(move |(i, f)| ((k, i as u8), f as u64)) ),
            keys_len,
            value_coding.bits_per_fragment()+extra_bits_per_fragment,
            conf);
        drop(encoder);
        r.map(|value_fragments| Self { value_fragments, value_coding })
    }

    #[inline(always)]
    pub fn try_from_map_with_coding_conf<K, MS, BM, V>(map: &HashMap<K, V, MS>, value_coding: C, conf: MapConf<BM, S>, bdz_extra_bits_per_fragment: u8) -> Option<Self>
        where K: Hash, BM: ValuesPreFiller, V: std::borrow::Borrow<<C as Coding>::Value> {
        Self::try_from_mapf_with_coding_conf(|| map, value_coding, conf, bdz_extra_bits_per_fragment)
    }

    #[inline(always)]
    pub fn try_from_kv_with_coding_conf<K, BM, V>(keys: &[K], values: &[V], value_coding: C, conf: MapConf<BM, S>, bdz_extra_bits_per_fragment: u8) -> Option<Self>
        where K: Hash, BM: ValuesPreFiller, V: std::borrow::Borrow<<C as Coding>::Value> {
        Self::try_from_mapf_with_coding_conf(|| keys.iter().zip(values), value_coding, conf, bdz_extra_bits_per_fragment)
    }

    /*#[inline(always)]
    pub fn try_from_nestle_vec_with_coding_conf<K, BM>(levels_of_values: &[Vec::<V>], value_coding: Coding<V>, conf: BDZConf<BM, S>) -> Option<Self>
        where K: Hash, BM: BDZBufferManager
    {
        Self::try_from_mapf_with_coding_conf(|| keys.iter().zip(values), value_coding, conf)
    }*/
}

impl<C: Coding, /*V: Hash+Eq+Clone,*/> CMap<C> {
    #[inline(always)]
    pub fn try_from_map_with_coding<K, MS, V>(map: &HashMap<K, V, MS>, value_coding: C, bdz_extra_bits_per_fragment: u8) -> Option<Self>
        where K: Hash, V: std::borrow::Borrow<<C as Coding>::Value> {
        Self::try_from_map_with_coding_conf(map, value_coding, MapConf::<(), _>::default(), bdz_extra_bits_per_fragment)
    }

    #[inline(always)]
    pub fn try_from_mapf_with_coding<'a, K, KvIntoIter, FKvIntoIter, V>(map: FKvIntoIter, value_coding: C, bdz_extra_bits_per_fragment: u8) -> Option<Self>
        where K: Hash + 'a,
              V: std::borrow::Borrow<<C as Coding>::Value> + 'a,
              KvIntoIter: IntoIterator<Item=(&'a K, &'a V)> + 'a,
              FKvIntoIter: Fn() -> KvIntoIter
    {
        Self::try_from_mapf_with_coding_conf(map, value_coding, MapConf::<(), _>::default(), bdz_extra_bits_per_fragment)
    }

    #[inline(always)]
    pub fn try_from_kv_with_coding<K: Hash, V>(keys: &[K], values: &[V], value_coding: C, bdz_extra_bits_per_fragment: u8) -> Option<Self>
    where K: Hash, V: std::borrow::Borrow<<C as Coding>::Value>
    {
        Self::try_from_kv_with_coding_conf(keys, values, value_coding, MapConf::<(), _>::default(), bdz_extra_bits_per_fragment)
    }
}

impl<C: Coding, S: BuildSeededHasher> CMap<C, S> {
    #[inline(always)]
    pub fn try_from_map_with_builder_bpf_conf<K, MS, BM, BC>(map: &HashMap<K, C::Value, MS>, build_coding: &BC, bits_per_fragment: u8, conf: MapConf<BM, S>, bdz_extra_bits_per_fragment: u8) -> Option<Self>
        where K: Hash,
              BM: ValuesPreFiller,
              BC: BuildCoding<C::Value, Coding=C>
    {
        Self::try_from_mapf_with_coding_conf(|| map,
                                             build_coding.build_from_iter(map.values(), bits_per_fragment),
                                             conf, bdz_extra_bits_per_fragment)
    }

    #[inline(always)]
    pub fn try_from_map_with_builder_conf<K, MS, BM, BC>(map: &HashMap<K, C::Value, MS>, build_coding: &BC, conf: MapConf<BM, S>, bdz_extra_bits_per_fragment: u8) -> Option<Self>
        where K: Hash,
              BM: ValuesPreFiller,
              BC: BuildCoding<C::Value, Coding=C>
    {
        Self::try_from_mapf_with_coding_conf(|| map,
                                             build_coding.build_from_iter(map.values(), 0),
                                             conf, bdz_extra_bits_per_fragment)
    }

    #[inline(always)]
    pub fn try_from_kv_with_builder_bpf_conf<K, BM, BC>(keys: &[K], values: &[C::Value], build_coding: &BC, bits_per_fragment: u8, conf: MapConf<BM, S>, bdz_extra_bits_per_fragment: u8) -> Option<Self>
        where K: Hash,
              BM: ValuesPreFiller,
              BC: BuildCoding<C::Value, Coding=C>
    {
        Self::try_from_kv_with_coding_conf(keys, values, build_coding.build_from_iter(values.iter(), bits_per_fragment), conf, bdz_extra_bits_per_fragment)
    }

    #[inline(always)]
    pub fn try_from_kv_with_builder_conf<K, BM, BC>(keys: &[K], values: &[C::Value], build_coding: &BC, conf: MapConf<BM, S>, bdz_extra_bits_per_fragment: u8) -> Option<Self>
        where K: Hash,
              BM: ValuesPreFiller,
              BC: BuildCoding<C::Value, Coding=C>
    {
        Self::try_from_kv_with_coding_conf(keys, values, build_coding.build_from_iter(values.iter(), 0), conf, bdz_extra_bits_per_fragment)
    }
}

impl<V: Hash+Eq+Clone, S: BuildSeededHasher> CMap<minimum_redundancy::Coding<V>, S> {
    #[inline(always)]
    pub fn try_from_map_with_conf<K, MS, BM>(map: &HashMap<K, V, MS>, bits_per_fragment: u8, conf: MapConf<BM, S>, bdz_extra_bits_per_fragment: u8) -> Option<Self>
        where K: Hash,
              BM: ValuesPreFiller
    {
        Self::try_from_mapf_with_coding_conf(|| map,
                                             minimum_redundancy::Coding::<V>::from_iter(BitsPerFragment(bits_per_fragment), map.values()),
                                             conf, bdz_extra_bits_per_fragment)
    }

    #[inline(always)]
    pub fn try_from_kv_with_conf<K, BM>(keys: &[K], values: &[V], bits_per_fragment: u8, conf: MapConf<BM, S>, bdz_extra_bits_per_fragment: u8) -> Option<Self>
        where K: Hash, BM: ValuesPreFiller {
        Self::try_from_kv_with_coding_conf(keys, values, minimum_redundancy::Coding::<V>::from_iter(BitsPerFragment(bits_per_fragment), values.iter()), conf, bdz_extra_bits_per_fragment)
    }
}

impl<V: Hash+Eq+Clone> CMap<minimum_redundancy::Coding<V>> {
    #[inline(always)]
    pub fn try_from_map<K: Hash, MS>(map: &HashMap<K, V, MS>, bits_per_fragment: u8, bdz_extra_bits_per_fragment: u8) -> Option<Self> {
        Self::try_from_map_with_conf(map, bits_per_fragment, MapConf::<(), _>::default(), bdz_extra_bits_per_fragment)
    }

    #[inline(always)]
    pub fn try_from_mapf<'a, K, KvIntoIter, FKvIntoIter, BM>(map: FKvIntoIter, bits_per_fragment: u8, bdz_extra_bits_per_fragment: u8) -> Option<Self>
        where K: Hash + 'a,
              V: 'a,
              KvIntoIter: IntoIterator<Item=(&'a K, &'a V)> + 'a,
              FKvIntoIter: Fn() -> KvIntoIter
    {
        let value_coding = minimum_redundancy::Coding::<V>::from_iter(BitsPerFragment(bits_per_fragment), map().into_iter().map(|(_, v)| v));
        Self::try_from_mapf_with_coding(map, value_coding, bdz_extra_bits_per_fragment)
    }

    #[inline(always)]
    pub fn try_from_kv<K: Hash>(keys: &[K], values: &[V], bits_per_fragment: u8, bdz_extra_bits_per_fragment: u8) -> Option<Self> {
        Self::try_from_kv_with_conf(keys, values, bits_per_fragment, MapConf::<(), _>::default(), bdz_extra_bits_per_fragment)
    }
}

impl<C: Coding, S: BuildSeededHasher> CMap<C, S> {

    /// Gets the value associated with the given key `k` and reports statistics to `access_stats`.
    pub fn get_stats<K: Hash + ?Sized, A: AccessStatsCollector>(&self, k: &K, access_stats: &mut A) -> Option<<<C as Coding>::Decoder<'_> as Decoder>::Decoded> {
        let mut result_decoder = self.value_coding.decoder();
        let mut fragment_nr = 0u8;
        if self.value_fragments.bits_per_value == self.value_coding.bits_per_fragment() { // extra bits are not used
            loop {
                match result_decoder.consume(self.value_fragments.get(&(k, fragment_nr)) as u8) {
                    DecodingResult::Value(v) => {
                        access_stats.found_on_level(fragment_nr as usize);
                        return Some(v)
                    },
                    DecodingResult::Invalid => {
                        access_stats.fail_on_level(fragment_nr as usize);
                        return None
                    },
                    DecodingResult::Incomplete => {}
                }
                fragment_nr += 1;
            }
        } else {
            loop {
                match result_decoder.consume_checked(self.value_fragments.get(&(k, fragment_nr)) as u8) {
                    DecodingResult::Value(v) => {
                        access_stats.found_on_level(fragment_nr as usize);
                        return Some(v)
                    },
                    DecodingResult::Invalid => {
                        access_stats.fail_on_level(fragment_nr as usize);
                        return None
                    },
                    DecodingResult::Incomplete => {}
                }
                fragment_nr += 1;
            }
        }
    }

    /// Gets the value associated with the given key `k`.
    #[inline(always)] pub fn get<K: Hash + ?Sized>(&self, k: &K) -> Option<<<C as Coding>::Decoder<'_> as Decoder>::Decoded> {
        self.get_stats(k, &mut ())
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use maplit::hashmap;

    fn bdzhmap_3pairs_conf<BM: ValuesPreFiller>(conf: MapConf<BM>, bits_per_fragment: u8, bdz_extra_bits_per_fragment: u8) {
        let bdzhmap = CMap::try_from_map_with_conf(&hashmap!('a'=>0u8, 'b'=>3u8, 'c'=>8u8), bits_per_fragment, conf, bdz_extra_bits_per_fragment).unwrap();
        assert_eq!(bdzhmap.get(&'a'), Some(&0));
        assert_eq!(bdzhmap.get(&'b'), Some(&3));
        assert_eq!(bdzhmap.get(&'c'), Some(&8));
        assert_eq!(bdzhmap.value_fragments.bits_per_value, bits_per_fragment+bdz_extra_bits_per_fragment);
        assert_eq!(bdzhmap.value_coding.bits_per_fragment(), bits_per_fragment);
    }

    #[test]
    fn bdzhmap_3pairs_1bpf() {
        bdzhmap_3pairs_conf(MapConf::new(), 1, 0);
        bdzhmap_3pairs_conf(MapConf::new(), 1, 1);
        bdzhmap_3pairs_conf(MapConf::new(), 1, 2);
    }

    #[test]
    fn bdzhmap_3pairs_1bpf_bm123() {
        bdzhmap_3pairs_conf(MapConf::pattern(123u64), 1, 0);
        bdzhmap_3pairs_conf(MapConf::pattern(123u64), 1, 1);
        bdzhmap_3pairs_conf(MapConf::pattern(123u64), 1, 2);
    }

    #[test]
    fn bdzhmap_3pairs_2bpf() {
        bdzhmap_3pairs_conf(MapConf::new(), 2, 0);
        bdzhmap_3pairs_conf(MapConf::new(), 2, 1);
        bdzhmap_3pairs_conf(MapConf::new(), 2, 2);
    }

    #[test]
    fn bdzhmap_3pairs_2bpf_bm123() {
        bdzhmap_3pairs_conf(MapConf::pattern(123u64), 2, 0);
        bdzhmap_3pairs_conf(MapConf::pattern(123u64), 2, 1);
        bdzhmap_3pairs_conf(MapConf::pattern(123u64), 2, 2);
    }

    fn bdzhmap_8pairs_conf<BM: ValuesPreFiller>(conf: MapConf<BM>, bits_per_fragment: u8, bdz_extra_bits_per_fragment: u8) {
        let bdzhmap = CMap::try_from_map_with_conf(&hashmap!(
                'a' => 1u8, 'b' => 2u8, 'c' => 1u8, 'd' => 3u8,
                'e' => 4u8, 'f' => 1u8, 'g' => 5u8, 'h' => 6u8), bits_per_fragment, conf, bdz_extra_bits_per_fragment).unwrap();
        assert_eq!(bdzhmap.get(&'a'), Some(&1));
        assert_eq!(bdzhmap.get(&'b'), Some(&2));
        assert_eq!(bdzhmap.get(&'c'), Some(&1));
        assert_eq!(bdzhmap.get(&'d'), Some(&3));
        assert_eq!(bdzhmap.get(&'e'), Some(&4));
        assert_eq!(bdzhmap.get(&'f'), Some(&1));
        assert_eq!(bdzhmap.get(&'g'), Some(&5));
        assert_eq!(bdzhmap.get(&'h'), Some(&6));
        assert_eq!(bdzhmap.value_fragments.bits_per_value, bits_per_fragment+bdz_extra_bits_per_fragment);
        assert_eq!(bdzhmap.value_coding.bits_per_fragment(), bits_per_fragment);
    }

    #[test]
    fn bdzhmap_8pairs_1bpf() {
        bdzhmap_8pairs_conf(MapConf::new(), 1, 0);
        bdzhmap_8pairs_conf(MapConf::new(), 1, 1);
        bdzhmap_8pairs_conf(MapConf::new(), 1, 2);
    }

    #[test]
    fn bdzhmap_8pairs_1bpf_bm123() {
        bdzhmap_8pairs_conf(MapConf::pattern(123u64), 1, 0);
        bdzhmap_8pairs_conf(MapConf::pattern(123u64), 1, 1);
        bdzhmap_8pairs_conf(MapConf::pattern(123u64), 1, 2);
    }

    #[test]
    fn bdzhmap_8pairs_2bpf() {
        bdzhmap_8pairs_conf(MapConf::new(), 2, 0);
        bdzhmap_8pairs_conf(MapConf::new(), 2, 1);
        bdzhmap_8pairs_conf(MapConf::new(), 2, 2);
    }

    #[test]
    fn bdzhmap_8pairs_2bpf_bm123() {
        bdzhmap_8pairs_conf(MapConf::pattern(123u64), 2, 0);
        bdzhmap_8pairs_conf(MapConf::pattern(123u64), 2, 1);
        bdzhmap_8pairs_conf(MapConf::pattern(123u64), 2, 2);
    }

    #[test]
    fn bdzhmap_8pairs_3bpf() {
        bdzhmap_8pairs_conf(MapConf::new(), 3, 0);
        bdzhmap_8pairs_conf(MapConf::new(), 3, 1);
        bdzhmap_8pairs_conf(MapConf::new(), 3, 2);
    }

    #[test]
    fn bdzhmap_8pairs_3bpf_bm123() {
        bdzhmap_8pairs_conf(MapConf::pattern(123u64), 3, 0);
        bdzhmap_8pairs_conf(MapConf::pattern(123u64), 3, 1);
        bdzhmap_8pairs_conf(MapConf::pattern(123u64), 3, 2);
    }
}