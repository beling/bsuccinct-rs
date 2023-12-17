use std::hash::Hash;
use binout::{VByte, AsIs, Serializer};
use minimum_redundancy::DecodingResult;
use bitm::{BitAccess, BitVec, Rank};
use crate::fp::level_size_chooser::LevelSizeChooser;

use ph::utils::{ArrayWithRank, read_bits};
use ph::{BuildDefaultSeededHasher, BuildSeededHasher, stats, utils};
use std::collections::HashMap;
use std::io;

mod conf;
pub use conf::CMapConf;

use crate::fp::collision_solver::{CollisionSolver, CollisionSolverBuilder, IsLossless};

use crate::fp::common::{encode_all, encode_all_from_map};
use dyn_size_of::GetSize;
use crate::coding::{Coding, Decoder, SerializableCoding, BuildCoding};

/// Finger-printing based compressed static function (immutable map)
/// that maps hashable keys to values of any type.
/// 
/// To represent a function *f:X→Y*, it uses the space slightly larger than *|X|H*,
/// where *H* is the entropy of the distribution of the *f* values over *X*.
/// The expected time complexity is *O(c)* for evaluation and *O(|X|c)* for construction
/// (not counting building the encoding dictionary),
/// where *c* is the average codeword length (given in code fragments) of the values.
pub struct CMap<C, S = BuildDefaultSeededHasher> {
    array: ArrayWithRank,
    value_fragments: Box<[u64]>,    // BitVec
    level_sizes: Box<[u64]>,
    value_coding: C,
    hash_builder: S
}

impl<C: GetSize, S> GetSize for CMap<C, S> {
    fn size_bytes_dyn(&self) -> usize {
        self.array.size_bytes_dyn()
            + self.value_fragments.size_bytes_dyn()
            + self.level_sizes.size_bytes_dyn()
            + self.value_coding.size_bytes_dyn()
    }
    const USES_DYN_MEM: bool = true;
}

impl<C, S: BuildSeededHasher> CMap<C, S> {
    #[inline(always)] fn index<K: Hash + ?Sized>(&self, k: &K, level_nr: u32, size: usize) -> usize {
        utils::map64_to_64(self.hash_builder.hash_one(k, level_nr), size as u64) as usize
    }
}

impl<C: Coding, S: BuildSeededHasher> CMap<C, S> {
    /// Gets the value associated with the given key `k` and reports statistics to `access_stats`.
    pub fn get_stats<K: Hash + ?Sized, A: stats::AccessStatsCollector>(&self, k: &K, access_stats: &mut A) -> Option<<<C as Coding>::Decoder<'_> as Decoder>::Decoded> {
        let mut result_decoder = self.value_coding.decoder();
        let mut array_begin_index = 0usize;
        let mut level = 0u32;
        loop {
            let level_size = (*self.level_sizes.get(level as usize)? as usize) << 6usize;
            let i = array_begin_index + self.index(k, level, level_size);
            if self.array.content.get_bit(i) {
                match result_decoder.consume(self.value_fragments.get_fragment(self.array.rank(i), self.value_coding.bits_per_fragment()) as u8) {
                    DecodingResult::Value(v) => {
                        access_stats.found_on_level(level);
                        return Some(v)
                    },
                    DecodingResult::Invalid => {
                        access_stats.fail_on_level(level);
                        return None
                    },
                    DecodingResult::Incomplete => {}
                }
            }
            array_begin_index += level_size;
            level += 1;
        }
    }

    /// Gets the value associated with the given key `k`.
    #[inline(always)]
    pub fn get<K: Hash + ?Sized>(&self, k: &K) -> Option<<<C as Coding>::Decoder<'_> as Decoder>::Decoded> {
        self.get_stats(k, &mut ())
    }

    /// Build BBMap for given keys -> values map, where:
    /// - keys are given directly
    /// - values are encoded by `value_coding` and given in as values_fragments and corresponding values_fragments_sizes
    /// All three arrays must be of the same length.
    ///
    /// Note: `conf.bits_per_fragment` is ignored (since `value_coding.bits_per_fragment` is used).
    fn with_fragments<K, LSC, CSB, BS, BC>(
        keys: &mut [K], values: &mut [C::Codeword],
        value_coding: C, conf: CMapConf<BC, LSC, CSB, S>,
        stats: &mut BS)
        -> Self
        where K: Hash,
              LSC: LevelSizeChooser,
              CSB: CollisionSolverBuilder + IsLossless,
              BS: stats::BuildStatsCollector
    {
        let mut levels = Vec::<u64>::new();
        let mut arrays = Vec::<Box<[u64]>>::new();
        let mut input_size = keys.len();
        let mut value_rev_indices: Box<[u8]> = values.iter().map(|c| value_coding.len_of(*c)-1).collect();
        let mut level_nr = 0u32;
        while input_size != 0 {
            let level_size_segments = conf.level_size_chooser.size_segments(
                &value_coding,
                &values[0..input_size], &value_rev_indices[0..input_size]);
            let level_size = level_size_segments * 64;
            stats.level(input_size, level_size);
            let mut collision_solver = conf.collision_solver.new(level_size_segments, value_coding.bits_per_fragment());
            for i in 0..input_size {
                let a_index = utils::map64_to_64(conf.hash.hash_one(&keys[i], level_nr), level_size as u64) as usize;
                if collision_solver.is_under_collision(a_index) { continue }
                collision_solver.process_fragment(a_index,
                                                  value_coding.rev_fragment_of(values[i], value_rev_indices[i]),
                                                  value_coding.bits_per_fragment());
            }

            let current_array = collision_solver.to_collision_array();
            let mut i = 0usize;
            while i < input_size {
                let a_index = utils::map64_to_64(conf.hash.hash_one(&keys[i], level_nr), level_size as u64) as usize;
                if current_array.get_bit(a_index) { // no collision
                    let rev_index = &mut value_rev_indices[i];
                    if *rev_index == 0 { // the value fully encoded:
                        // remove i-th element by replacing it with the last one
                        input_size -= 1;
                        keys.swap(i, input_size);
                        values.swap(i, input_size);
                        value_rev_indices.swap(i, input_size);
                    } else {    // the value has to be encoded farther, go to its next fragment:
                        *rev_index -= 1;
                        i += 1;
                    }
                } else {    // collision, has to be processed again, at the next level
                    i += 1;
                }
            }
            arrays.push(current_array);
            levels.push(level_size_segments as u64);
            level_nr += 1;
        }

        let (array, out_fragments_num) = ArrayWithRank::build(arrays.concat().into_boxed_slice());
        let mut output_value_fragments = Box::<[u64]>::with_zeroed_bits(out_fragments_num as usize * value_coding.bits_per_fragment() as usize);
        for input_index in 0..keys.len() {
            //let mut result_decoder = self.value_coding.decoder();
            let mut array_begin_index = 0usize;
            let mut level = 0u32;
            loop {
                let level_size = (levels[level as usize] as usize) << 6usize;
                let i = array_begin_index + utils::map64_to_64(conf.hash.hash_one(&keys[input_index], level), level_size as u64) as usize;
                if array.content.get_bit(i) {
                    let code = &mut values[input_index];
                    output_value_fragments.init_fragment(   // AcceptEquals::set_value
                                                            array.rank(i),
                                                            value_coding.first_fragment_of(*code) as _,
                                                            value_coding.bits_per_fragment());
                    if value_coding.remove_first_fragment_of(code) {
                        // stats.value_on_level(level); // TODO do we need this? we can get average levels from lookups
                        break;
                    }
                }
                array_begin_index += level_size;
                level += 1;
            }
        }
        stats.end(0);
        Self {
            array,
            value_fragments: output_value_fragments,
            level_sizes: levels.into_boxed_slice(),
            value_coding,
            hash_builder: conf.hash
        }
    }
}

impl<C: SerializableCoding, S: BuildSeededHasher> CMap<C, S> {

    /// Returns number of bytes which `write` will write, assuming that each call to `write_value` writes `bytes_per_value` bytes.
    pub fn write_bytes(&self, bytes_per_value: usize) -> usize {
        VByte::array_size(&self.level_sizes)
            + AsIs::array_content_size(&self.array.content)
            + self.value_coding.write_bytes(bytes_per_value)
            + AsIs::array_content_size(&self.value_fragments)
    }

    /// Writes `self` to the `output`, using `write_value` to write values.
    pub fn write<F>(&self, output: &mut dyn io::Write, write_value: F) -> io::Result<()>
        where F: FnMut(&mut dyn io::Write, &C::Value) -> io::Result<()>
    {
        VByte::write_array(output, &self.level_sizes)?;
        AsIs::write_all(output, self.array.content.iter())?;
        self.value_coding.write(output, write_value)?;
        AsIs::write_all(output, self.value_fragments.iter())
    }

    /// Read self from the input, using read_value to read values (hasher must be the same as used by written `BBMap`).
    pub fn read_with_hasher<F>(input: &mut dyn io::Read, read_value: F, hasher: S) -> io::Result<Self>
        where F: FnMut(&mut dyn io::Read) -> io::Result<C::Value>
    {
        let level_sizes = VByte::read_array(input)?;
        let array_content = AsIs::read_n(input, level_sizes.iter().map(|v|*v as usize).sum::<usize>())?;
        let (array_with_rank, number_of_ones) = ArrayWithRank::build(array_content);
        let value_coding = C::read(input, read_value)?;
        let value_fragments = read_bits(input, number_of_ones as usize * value_coding.bits_per_fragment() as usize)?;
        Ok(Self {
            array: array_with_rank,
            value_fragments: value_fragments,
            level_sizes: level_sizes,
            value_coding,
            hash_builder: hasher
        })
    }
}

impl<C: SerializableCoding> CMap<C> {
    /// Reads `Self` from the `input`, using `read_value` to read values.
    /// Only `BBMap`s that use default hasher can be read by this method.
    pub fn read<F>(input: &mut dyn io::Read, read_value: F) -> io::Result<Self>
        where F: FnMut(&mut dyn io::Read) -> io::Result<C::Value>
    {
        Self::read_with_hasher(input, read_value, Default::default())
    }
}

impl<C: Coding, S: BuildSeededHasher> CMap<C, S> {
    pub fn from_slices_with_coding_conf<K, LSC, CSB, BS, BC>(
        keys: &mut [K], values: &[C::Value],
        value_coding: C, conf: CMapConf<BC, LSC, CSB, S>,
        stats: &mut BS
    ) -> Self
        where K: Hash,
              LSC: LevelSizeChooser,
              CSB: CollisionSolverBuilder + IsLossless,
              BS: stats::BuildStatsCollector
    {
        Self::with_fragments(keys, &mut encode_all(&value_coding, values), value_coding, conf, stats)
    }

    pub fn from_slices_with_conf<K, LSC, CSB, BS, BC>(
        keys: &mut [K], values: &[C::Value], conf: CMapConf<BC, LSC, CSB, S>, stats: &mut BS
    ) -> Self
        where K: Hash,
              LSC: LevelSizeChooser,
              CSB: CollisionSolverBuilder + IsLossless,
              BS: stats::BuildStatsCollector,
            BC: BuildCoding<C::Value, Coding=C>
    {
        Self::from_slices_with_coding_conf(keys, values, conf.coding.build_from_iter(values, 0), conf, stats)
    }

    pub fn from_map_with_coding_conf<K, H, LSC, CSB, BS, BC>(
        map: &HashMap<K, C::Value, H>, value_coding: C, conf: CMapConf<BC, LSC, CSB, S>, stats: &mut BS
    ) -> Self
        where K: Hash + Clone,
              LSC: LevelSizeChooser,
              CSB: CollisionSolverBuilder+IsLossless,
              BS: stats::BuildStatsCollector
    {
        let (mut keys, mut values) = encode_all_from_map(&value_coding, map);
        Self::with_fragments(&mut keys, &mut values, value_coding, conf, stats)
    }

    pub fn from_map_with_conf<K, H, LSC, CSB, BS, BC>(
        map: &HashMap<K, C::Value, H>, conf: CMapConf<BC, LSC, CSB, S>, stats: &mut BS
    ) -> Self
        where K: Hash + Clone,
              LSC: LevelSizeChooser,
              CSB: CollisionSolverBuilder+IsLossless,
              BS: stats::BuildStatsCollector,
              BC: BuildCoding<C::Value, Coding=C>
    {
        Self::from_map_with_coding_conf(map, conf.coding.build_from_iter(map.values(), 0), conf, stats)
    }
}

impl<C: Coding> CMap<C> {
    pub fn from_slices_with_coding<K: Hash, BS: stats::BuildStatsCollector>(keys: &mut [K], values: &[C::Value], value_coding: C, stats: &mut BS) -> Self {
        Self::from_slices_with_coding_conf(keys, values, value_coding, CMapConf::default(), stats)
    }
}

impl<V: Hash + Eq + Clone> CMap<minimum_redundancy::Coding<V>> {
    pub fn from_slices<K: Hash, BS: stats::BuildStatsCollector>(keys: &mut [K], values: &[V], stats: &mut BS) -> Self {
        Self::from_slices_with_conf(keys, values, Default::default(), stats)
    }

    pub fn from_map<K: Hash + Clone, H, BS: stats::BuildStatsCollector>(map: &HashMap<K, V, H>, stats: &mut BS) -> Self {
        Self::from_map_with_conf(map, Default::default(), stats)
    }
}

impl<K: Hash + Clone, V: Hash + Eq + Clone, H> From<&HashMap<K, V, H>> for CMap<minimum_redundancy::Coding<V>> {
    fn from(map: &HashMap<K, V, H>) -> Self {
        Self::from_map(map, &mut ())
    }
}

impl<K: Hash + Clone, V: Hash + Eq + Clone, H> From<HashMap<K, V, H>> for CMap<minimum_redundancy::Coding<V>> {
    fn from(map: HashMap<K, V, H>) -> Self {
        Self::from_map(&map, &mut ())
    }
}


#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use binout::Serializer;
    use maplit::hashmap;
    use bitm::ceiling_div;
    use crate::coding::BuildMinimumRedundancy;

    fn test_read_write<C: SerializableCoding<Value=u8>>(bbmap: &CMap<C>) {
        let mut buff = Vec::new();
        bbmap.write(&mut buff, |b, v| AsIs::write(b, *v)).unwrap();
        assert_eq!(buff.len(), bbmap.write_bytes(1));
        let read = CMap::<C>::read(&mut &buff[..], |b| AsIs::read(b)).unwrap();
        assert_eq!(bbmap.array.content, read.array.content);
        assert_eq!(bbmap.level_sizes, read.level_sizes);
    }

    fn test_bbmap_invariants<C: Coding>(bbmap: &CMap<C>) {
        assert_eq!(bbmap.level_sizes.iter().map(|v|*v as usize).sum::<usize>(), bbmap.array.content.len());
        assert_eq!(
            ceiling_div(bbmap.array.content.iter().map(|v|v.count_ones()).sum::<u32>() as usize * bbmap.value_coding.bits_per_fragment() as usize, 64),
            bbmap.value_fragments.len()
        );
    }

    fn test_4pairs<LSC: LevelSizeChooser>(conf: CMapConf<BuildMinimumRedundancy, LSC>) {
        let bbmap = CMap::from_map_with_conf(&hashmap!('a'=>1u8, 'b'=>2u8, 'c'=>1u8, 'd'=>3u8), conf, &mut ());
        assert_eq!(bbmap.get(&'a'), Some(&1));
        assert_eq!(bbmap.get(&'b'), Some(&2));
        assert_eq!(bbmap.get(&'c'), Some(&1));
        assert_eq!(bbmap.get(&'d'), Some(&3));
        test_bbmap_invariants(&bbmap);
        test_read_write(&bbmap);
    }

    #[test]
    fn with_hashmap_bpf1() {
        test_4pairs(CMapConf::bpf(1));
    }

    fn test_8pairs<LSC: LevelSizeChooser>(conf: CMapConf<BuildMinimumRedundancy, LSC>) {
        let bbmap = CMap::from_map_with_conf(&hashmap!(
            'a' => 1, 'b' => 2, 'c' => 1, 'd' => 3,
            'e' => 4, 'f' => 1, 'g' => 5, 'h' => 6), conf, &mut ());
        assert_eq!(bbmap.get(&'a'), Some(&1));
        assert_eq!(bbmap.get(&'b'), Some(&2));
        assert_eq!(bbmap.get(&'c'), Some(&1));
        assert_eq!(bbmap.get(&'d'), Some(&3));
        assert_eq!(bbmap.get(&'e'), Some(&4));
        assert_eq!(bbmap.get(&'f'), Some(&1));
        assert_eq!(bbmap.get(&'g'), Some(&5));
        assert_eq!(bbmap.get(&'h'), Some(&6));
        test_bbmap_invariants(&bbmap);
        test_read_write(&bbmap);
    }

    #[test]
    fn with_hashmap_bpf2() {
        test_8pairs(CMapConf::bpf(2));
    }
}