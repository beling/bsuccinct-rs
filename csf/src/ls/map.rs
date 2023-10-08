use binout::{AsIs, Serializer, VByte};
use bitm::{BitAccess, ceiling_div, BitVec};
use ph::{BuildSeededHasher, BuildDefaultSeededHasher, utils::map64_to_64};
use std::hash::Hash;
use std::collections::HashMap;
use std::io;
use std::borrow::Borrow;
use dyn_size_of::GetSize;
use crate::{bits_to_store_any_of_ref, bits_to_store_any_of};

use super::graph3::{HyperGraph, VertexIndex};
use super::conf::{MapConf, ValuesPreFiller};

/// Static function that maps keys to integer values of given bit-size.
/// 
/// Its construction is based on solving linear system of equations by hyper-graphs peeling.
/// 
/// The implementation is based on the paper:
/// - D. Belazzougui, P. Boldi, G. Ottaviano, R. Venturini, S. Vigna, *Cache-Oblivious Peeling of Random Hypergraphs*, 
///   In A. Bilgin, M. W. Marcellin, J. Serra-Sagristà, & J. A. Storer (Eds.),
///   Proceedings of Data Compression Conference 26-28 March 2014, Snowbird,
///   Utah, USA (pp. 352-361). (Data Compression Conference. Proceedings; Vol. 2375-0391).
///   IEEE. <https://doi.org/10.1109/DCC.2014.48>
pub struct Map<S = BuildDefaultSeededHasher> {
    values: Box<[u64]>,
    hash_builder: S,
    hash_seeds: [u8; 3],
    third_of_values_len: usize,
    pub(crate) bits_per_value: u8
}

#[inline(always)] fn index<K: Hash, S: BuildSeededHasher>(hash_builder: &S, k: &K, fun_number_seed: u8, size: usize) -> usize {
    map64_to_64(hash_builder.hash_one(k, fun_number_seed as u32), size as u64) as usize
}

impl<S> GetSize for Map<S> {
    #[inline] fn size_bytes_dyn(&self) -> usize {
        self.values.size_bytes_dyn()
    }
    const USES_DYN_MEM: bool = true;
}

impl<S> Map<S> {
    /// Returns number of bytes which `write` will write.
    pub fn write_bytes(&self) -> usize {
        AsIs::array_size(&self.values)
            + VByte::array_content_size(&self.hash_seeds)
            + VByte::size(self.third_of_values_len)
            + AsIs::size(self.bits_per_value)
    }

    /// Writes `self` to the `output`.
    pub fn write(&self, output: &mut dyn io::Write) -> io::Result<()> {
        AsIs::write_array(output, &self.values)?;
        VByte::write_all_values(output, self.hash_seeds)?;
        VByte::write(output, self.third_of_values_len)?;
        AsIs::write(output, self.bits_per_value)
    }

    /// Reads `self` from the `input` (`hasher` must be the same as used by written [`Map`]).
    pub fn read_with_hasher(input: &mut dyn io::Read, hasher: S) -> io::Result<Self> {
        let values = AsIs::read_array(input)?;
        let hash_seeds = [VByte::read(input)?, VByte::read(input)?, VByte::read(input)?];
        let third_of_values_len = VByte::read(input)?;
        let bits_per_value = AsIs::read(input)?;
        Ok(Self {
            values,
            hash_builder: hasher,
            hash_seeds,
            third_of_values_len,
            bits_per_value
        })
    }
}

impl Map<BuildDefaultSeededHasher> {
    /// Reads `self` from the `input`. Only [`Map`]s that use default hasher can be read by this method.
    pub fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Self::read_with_hasher(input, Default::default())
    }

    pub fn try_with_fn<K, V, BC>(keys: &[K], values: V, bits_per_value: u8) -> Option<Self>
        where K: Hash,
              V: Fn(usize, u8) -> u64   // Value accessor: (index, bits_per_value) -> value
    {
        Self::try_with_conf_fn::<K, _, _, _, _>(|| keys.iter().enumerate().map(|(i, k)| (k, values(i, bits_per_value))),
             keys.len(), bits_per_value, MapConf::<(), _>::default())
    }

    #[inline]
    pub fn try_with_bitset<K, BC>(keys: &[K], values: &[u64], bits_per_value: u8) -> Option<Self>
    where K: Hash, BC: FnOnce(usize) -> Box<[u64]> {
        Self::try_with_conf_bitset(keys, values, bits_per_value, MapConf::<(), _>::default())
    }

    #[inline]
    pub fn try_with_kv_bpv<K, V, BC>(keys: &[K], values: &[V], bits_per_value: u8) -> Option<Self>
    where K: Hash, V: Into<u64> + Copy {
        Self::try_with_conf_kv_bpv(keys, values, bits_per_value, MapConf::<(), _>::default())
    }

    #[inline]
    pub fn try_with_kv<K, V, BC>(keys: &[K], values: &[V]) -> Option<Self>
    where K: Hash, V: Into<u64> + Copy + Ord {
        Self::try_with_conf_kv(keys, values, MapConf::<(), _>::default())
    }
}

impl<S: BuildSeededHasher> Map<S> {

    /// Returns [`Map`] that assigns `0` to each key.
    fn always_map_to_zero(hash_builder: S) -> Self {
        Self {  // max == 0, and so each get of map can return 0
            values: Box::new([0, 0, 0]),
            hash_builder,
            hash_seeds: [0, 0, 0],
            third_of_values_len: 1,
            bits_per_value: 1
        }
    }

    /// Part of [`Self::try_with_conf_fn`] implementation.
    #[inline(always)] fn try_with_vertex_t_conf_fn<VI, K, KBorrow, KVIntoIterator, FKVIntoIterator, BM>(
        kv: FKVIntoIterator, kv_len: usize, number_of_vertices: usize, third_of_vertices_len: usize,
        bits_per_value: u8, mut conf: MapConf<BM, S>
    ) -> Option<Self>
        where VI: VertexIndex,
              KVIntoIterator: IntoIterator<Item=(KBorrow, u64)>,    // Iterator over key-value pairs
              FKVIntoIterator: Fn() -> KVIntoIterator,      // Returns iterator over key-value pairs
              K: Hash,
              KBorrow: Borrow<K>,
              BM: ValuesPreFiller // buffer creator (and initializer)
    {
        for iteration in 0..127 {
            let mut g = HyperGraph::<VI, _>::with_bits_per_value(number_of_vertices, bits_per_value);
            let hash_seeds = [iteration*2, iteration*2+1, iteration*2+2];    // wybrać lepiej?
            for (key, value) in kv() {   // mapping
                g.add_edge_with_value(
                    index(&conf.hash_builder, key.borrow(), hash_seeds[0], third_of_vertices_len),
                    index(&conf.hash_builder, key.borrow(), hash_seeds[1], third_of_vertices_len) + third_of_vertices_len,
                    index(&conf.hash_builder, key.borrow(), hash_seeds[2], third_of_vertices_len) + 2*third_of_vertices_len,
                    value
                );
            }
            let mut values = Box::with_zeroed_bits(kv_len * bits_per_value as usize);
            let mut values_count = 0;
            let queue = g.peel_with_values(kv_len, |v| {
                 values.init_fragment(values_count, *v, bits_per_value);
                 values_count += 1;
            });
            if queue.len() == kv_len {
                let mut rvalues = conf.value_prefiller.create(number_of_vertices, bits_per_value);
                for (index, (v0, v1, v2)) in queue.into_iter().enumerate().rev() {
                    let value = values.get_fragment(index, bits_per_value)
                        ^ rvalues.get_fragment(v1.to_usize(), bits_per_value)
                        ^ rvalues.get_fragment(v2.to_usize(), bits_per_value);
                    conf.value_prefiller.init(&mut rvalues, v0.to_usize(), value, bits_per_value);
                }
                return Some(Self {
                    values: rvalues,
                    hash_builder: conf.hash_builder,
                    hash_seeds,
                    third_of_values_len: third_of_vertices_len,
                    bits_per_value
                });
            }
        }
        None
    }

    /// Tries to construct [`Map`] with key-value pairs produced by the iterator returned by the `kv` function,
    /// using the given configuration.
    /// 
    /// The iterator returned by `kv` should produce exactly `kv_len` key-value pairs.
    /// Each value should occupy up to `bits_per_value` (least significant) bits
    /// (the most significant bits must be zeroed).
    pub fn try_with_conf_fn<K, KBorrow, KVIntoIterator, FKVIntoIterator, BM>(
        kv: FKVIntoIterator, kv_len: usize, bits_per_value: u8, conf: MapConf<BM, S>
    ) -> Option<Self>
        where KVIntoIterator: IntoIterator<Item=(KBorrow, u64)>,    // Iterator over key-value pairs
              FKVIntoIterator: Fn() -> KVIntoIterator,      // Returns iterator over key-value pairs
              K: Hash,
              KBorrow: Borrow<K>,
              BM: ValuesPreFiller // buffer creator (and initializer)
    {
        if kv_len == 0 || bits_per_value == 0 { return Some(Self::always_map_to_zero(conf.hash_builder)); }
        // numer of key-value pairs = numbers of hyper-edges
        let mut third_of_vertices_len = ceiling_div(123 * kv_len, 300);
        let mut number_of_vertices = third_of_vertices_len * 3;  // liczba wierzchołków
        {   // optional block that makes the length of the value vector as close to multiple of 64 as possible
            let values_vec_len = number_of_vertices * bits_per_value as usize;
            let m = 64 - values_vec_len % 64;   // how much needs to be added to values_vec_len to make it a multiple of 64
            if m != 64 {    // add m/bits_per_value (rounded down to multiple of 3) to number_of_vertices
                third_of_vertices_len += m / (bits_per_value as usize) / 3;
                number_of_vertices = third_of_vertices_len * 3;
            }
        }
        if number_of_vertices <= 1>>32 {
            Self::try_with_vertex_t_conf_fn::<u32, K, _, _, _, _>(kv, kv_len, number_of_vertices, third_of_vertices_len, bits_per_value, conf)
        } else {
            Self::try_with_vertex_t_conf_fn::<usize, K, _, _, _, _>(kv, kv_len, number_of_vertices, third_of_vertices_len, bits_per_value, conf)
        }
    }

    /// Tries to construct [`Map`] with key-value pairs stored in `keys` and `values` respectively.
    /// The `values` array should contain (at least) `keys.len()` fragments, each with a size of `bits_per_value` bits.
    #[inline]
    pub fn try_with_conf_bitset<K, BM>(keys: &[K], values: &[u64], bits_per_value: u8, conf: MapConf<BM, S>) -> Option<Self>
    where K: Hash, BM: ValuesPreFiller {
        Self::try_with_conf_fn::<K, _, _, _, _>(
            || keys.iter().enumerate().map(|(i, k)| (k, values.get_fragment(i, bits_per_value))),
         keys.len(), bits_per_value, conf)
    }

    /// Tries to construct [`Map`] with key-value pairs stored in `keys` and `values` respectively.
    /// 
    /// The `values` array usually consists of `u8`, `u16`, `u32` or `u64` items.
    /// The `keys` and `values` arrays must have the same length.
    /// Each value must be convertible to `u64` and should occupy up to `bits_per_value` (least significant) bits
    /// (the most significant bits must be zeroed).
    #[inline]
    pub fn try_with_conf_kv_bpv<K, V, BM>(keys: &[K], values: &[V], bits_per_value: u8, conf: MapConf<BM, S>) -> Option<Self>
    where K: Hash, V: Into<u64> + Clone, BM: ValuesPreFiller {
        Self::try_with_conf_fn::<K, _, _, _, _>(|| keys.iter().zip(values.iter().map(|v| v.clone().into())),
             keys.len(), bits_per_value, conf)
    }

    /*#[inline]
    pub fn try_with_conf_vecs<K, V, BM>(value_levels: &[Vec<K>], conf: BDZConf<BM, S>) -> Option<Self>
        where K: Hash, V: Into<u64> + Clone, BM: BDZBufferManager
    {
        let acc_sum: Vec<usize> = value_levels.iter()
            .scan(0, |acc, &x| { *acc = *acc + x.len(); Some(*acc) })
            .collect();
        Self::try_with_conf_fn(value_levels.iter(), keys.len(),
                               |index, _| acc_sum.lower_bound(),
                               bits_to_store!(value_levels.len()), conf)
    }*/

    /// Tries to construct [`Map`] with key-value pairs stored in `keys` and `values` respectively.
    /// 
    /// The `values` array usually consists of `u8`, `u16`, `u32` or `u64` items.
    /// The `keys` and `values` arrays must have the same length.
    #[inline]
    pub fn try_with_conf_kv<K, V, BM>(keys: &[K], values: &[V], conf: MapConf<BM, S>) -> Option<Self>
    where K: Hash, V: Into<u64> + Clone, BM: ValuesPreFiller {
        let bits_per_value = bits_to_store_any_of_ref(values);
        Self::try_with_conf_kv_bpv(keys, values, bits_per_value, conf)
    }

    /// Tries to construct [`Map`] with key-value pairs stored in `map`.
    /// 
    /// Values are usually of type `u8`, `u16`, `u32` or `u64`.
    /// Each value must be convertible to `u64` and should occupy up to `bits_per_value` (least significant) bits
    /// (the most significant bits must be zeroed).
    pub fn try_from_hashmap_bpv<K, V, HMS, BM>(map: HashMap<K, V, HMS>, bits_per_value: u8, conf: MapConf<BM, S>) -> Option<Self>
        where K: Hash, V: Into<u64> + Clone, BM: ValuesPreFiller
    {
        //let (keys, values) = map_to_key_values(map, bits_per_value);
        //Self::try_with_conf_bitset(keys.as_slice(), &values, bits_per_value, conf)
        Self::try_with_conf_fn::<K, _, _, _, _>(|| map.iter().map(|(k, v)| (k, v.clone().into())), map.len(), bits_per_value, conf)
    }

    /// Tries to construct [`Map`] with key-value pairs stored in `map`.
    /// 
    /// Values are usually of type `u8`, `u16`, `u32` or `u64`. Each value must be convertible to `u64`.
    pub fn try_from_hashmap<K, V, HMS, BM>(map: HashMap<K, V, HMS>, conf: MapConf<BM, S>) -> Option<Self>
    where K: Hash, V: Into<u64> + Clone, BM: ValuesPreFiller
    {
        let bits_per_value = bits_to_store_any_of_ref(map.values());
        Self::try_from_hashmap_bpv(map, bits_per_value, conf)
    }

    /// Returns index of values `fun_number`-th fragment that is associated with given `key`.
    #[inline(always)]
    fn index<K: Hash>(&self, key: &K, fun_number: u8) -> usize {
        (fun_number as usize * self.third_of_values_len) +
            index(&self.hash_builder, key, self.hash_seeds[fun_number as usize], self.third_of_values_len)
    }

    #[inline(always)]
    fn value_part<K: Hash>(&self, key: &K, fun_number: u8) -> u64 {
        self.values.get_fragment(self.index(key, fun_number), self.bits_per_value)
    }

    /// Returns value assigned to the given `key`.
    /// If the `key` was not in the input collection, an unpredictable value is returned.
    #[inline(always)]
    pub fn get<K: Hash>(&self, key: &K) -> u64 {
        self.value_part(key, 0) ^ self.value_part(key, 1) ^ self.value_part(key, 2)
    }
}

/*fn map_to_key_values<K, V: Into<u64>, S>(map: HashMap<K, V, S>, bits_per_value: u8) -> (Vec<K>, Box<[u64]>) {
    let mut keys = Vec::<K>::with_capacity(map.len());
    let mut values = Box::<[u64]>::with_zeroed_bits(bits_per_value as usize*map.len());
    for (i, (k, v)) in map.into_iter().enumerate() {
        keys.push(k);
        values.init_fragment(i, v.into(), bits_per_value);
    }
    (keys, values)
}*/

impl<K: Hash, V: Into<u64> + Clone, S: BuildSeededHasher + Default, HMS> From<HashMap<K, V, HMS>> for Map<S> {
    #[inline] fn from(map: HashMap<K, V, HMS>) -> Self {
        Self::try_from_hashmap(map, MapConf::<(), S>::default()).expect("Constructing ls::Map failed. Probably the input contains duplicate keys.")
    }
}

impl<K: Hash, V: Into<u64> + Clone, S: BuildSeededHasher + Default> From<&[(K, V)]> for Map<S> {
    #[inline] fn from(map: &[(K, V)]) -> Self {
        Self::try_with_conf_fn::<K, _, _, _, _>(
            || map.iter().map(|(k, v)| (k, v.clone().into())), map.len(),
            bits_to_store_any_of(map.iter().map(|(_, v)| v.clone())),
            MapConf::<(), S>::default()
        ).expect("Constructing ls::Map failed. Probably the input contains duplicate keys.")
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use maplit::hashmap;

    fn lsmap_1bpv_conf<BM: ValuesPreFiller>(conf: MapConf<BM>) {
        let bdzmap = Map::try_from_hashmap( hashmap!('a'=>0u8, 'b'=>0u8, 'c'=>0u8).into(), conf).unwrap();
        assert_eq!(bdzmap.get(&'a'), 0);
        assert_eq!(bdzmap.get(&'b'), 0);
        assert_eq!(bdzmap.get(&'c'), 0);
        assert_eq!(bdzmap.bits_per_value, 1);
    }

    #[test]
    fn lsmap_1bpv() {
        lsmap_1bpv_conf(MapConf::new());
    }

    #[test]
    fn lsmap_1bpv_bm123() {
        lsmap_1bpv_conf(MapConf::pattern(123u64));
    }

    fn lsmap_2bpv_conf<BM: ValuesPreFiller>(conf: MapConf<BM>) {
        let lsmap: Map = Map::try_from_hashmap( hashmap!('a'=>1u8, 'b'=>2u8, 'c'=>1u8, 'd'=>3u8).into(), conf).unwrap();
        assert_eq!(lsmap.get(&'a'), 1);
        assert_eq!(lsmap.get(&'b'), 2);
        assert_eq!(lsmap.get(&'c'), 1);
        assert_eq!(lsmap.get(&'d'), 3);
        assert_eq!(lsmap.bits_per_value, 2);
    }

    #[test]
    fn lsmap_2bpv() {
        lsmap_2bpv_conf(MapConf::new());
    }

    #[test]
    fn lsmap_2bpv_bm123() {
        lsmap_2bpv_conf(MapConf::pattern(123u64));
    }

    fn lsmap_3bpv_conf<BM: ValuesPreFiller>(conf: MapConf<BM>) {
        let bdzmap: Map = Map::try_from_hashmap( hashmap!(
                'a' => 1u8, 'b' => 2u8, 'c' => 1u8, 'd' => 3u8,
                'e' => 4u8, 'f' => 1u8, 'g' => 5u8, 'h' => 6u8),
                    conf ).unwrap();
        assert_eq!(bdzmap.get(&'a'), 1);
        assert_eq!(bdzmap.get(&'b'), 2);
        assert_eq!(bdzmap.get(&'c'), 1);
        assert_eq!(bdzmap.get(&'d'), 3);
        assert_eq!(bdzmap.get(&'e'), 4);
        assert_eq!(bdzmap.get(&'f'), 1);
        assert_eq!(bdzmap.get(&'g'), 5);
        assert_eq!(bdzmap.get(&'h'), 6);
        assert_eq!(bdzmap.bits_per_value, 3);
    }

    #[test]
    fn lsmap_3bpv() {
        lsmap_3bpv_conf(MapConf::new());
    }

    #[test]
    fn lsmap_3bpv_bm123() {
        lsmap_3bpv_conf(MapConf::pattern(123u64));
    }
}