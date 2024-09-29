use std::hash::Hash;
use binout::{VByte, Serializer, AsIs};
use ph::utils::read_bits;
use ph::{BuildDefaultSeededHasher, BuildSeededHasher, stats, utils::ArrayWithRank};
use bitm::{BitAccess, BitVec, Rank};
use minimum_redundancy::DecodingResult;
use super::{LevelSizer, CollisionSolver};
use super::collision_solver::{CountPositiveCollisions, LoMemAcceptEqualsSolver};
use super::common::{encode_all, encode_all_from_map};
use std::collections::HashMap;
use std::io;

mod conf;
pub use conf::GOCMapConf;
use ph::fmph::{goindexing::group_nr, GroupSize, SeedSize, TwoToPowerBitsStatic};
pub use ph::fmph::GOConf;
use dyn_size_of::GetSize;
use crate::coding::{Coding, Decoder, SerializableCoding, BuildCoding};

/// Finger-printing based compressed static function (immutable map)
/// that uses group optimization and maps hashable keys to values of any type.
/// 
/// To represent a function *f:X→Y*, it uses the space slightly larger than *|X|H*,
/// where *H* is the entropy of the distribution of the *f* values over *X*.
/// The expected time complexity is *O(c)* for evaluation and *O(|X|c)* for construction
/// (not counting building the encoding dictionary),
/// where *c* is the average codeword length (given in code fragments) of the values.
pub struct GOCMap<C = minimum_redundancy::Coding<u8>, GS: GroupSize = TwoToPowerBitsStatic::<4>, SS: SeedSize = TwoToPowerBitsStatic<2>, S = BuildDefaultSeededHasher> {
    array: ArrayWithRank,
    value_fragments: Box<[u64]>,    // BitVec
    group_seeds: Box<[SS::VecElement]>,   //  Box<[u8]>,
    level_size: Box<[u64]>, // number of groups
    value_coding: C,
    goconf: GOConf<GS, SS, S>,
}

impl<C: GetSize, GS: GroupSize, SS: SeedSize, S> GetSize for GOCMap<C, GS, SS, S> {
    fn size_bytes_dyn(&self) -> usize {
        self.array.size_bytes_dyn()
            + self.value_fragments.size_bytes_dyn()
            + self.group_seeds.size_bytes_dyn()
            + self.level_size.size_bytes_dyn()
            + self.value_coding.size_bytes_dyn()
    }

    const USES_DYN_MEM: bool = true;
}

impl<C: Coding, GS: GroupSize, SS: SeedSize, S: BuildSeededHasher> GOCMap<C, GS, SS, S> {
    /// Maps value of each key to code fragment, and adds the fragment to collision solver.
    fn consider_all<K, LSC, GetGroupSeed, CS, BC>(conf: &GOCMapConf<BC, LSC, GS, SS, S>, coding: &C,
                                                  keys: &[K], values: &[C::Codeword], value_rev_indices: &[u8],
                                                  level_size_groups: u64, level_nr: u32,
                                                  group_seed: GetGroupSeed, collision_solver: &mut CS)
        where K: Hash, GetGroupSeed: Fn(u64) -> u16, CS: CollisionSolver  // returns group seed for group with given index
    {
        let bits_per_fragment = coding.bits_per_fragment();
        for i in 0..keys.len() {
            let hash = conf.goconf.hash_builder.hash_one(&keys[i], level_nr);
            let group = group_nr(hash, level_size_groups);
            let index = conf.goconf.bits_per_group.bit_index_for_seed(hash, group_seed(group), group);
            if collision_solver.is_under_collision(index) { continue }
            collision_solver.process_fragment(index,
                                              coding.rev_fragment_of(values[i], value_rev_indices[i]),
                                              bits_per_fragment);
        }
    }

    /// Counts number of positive collisions in each group.
    fn count_collisions_in_groups<K, LSC, BC>(conf: &GOCMapConf<BC, LSC, GS, SS, S>, coding: &C,
                                              keys: &[K], values: &[C::Codeword], value_rev_indices: &[u8],
                                              level_size_groups: u64, level_nr: u32, group_seed: u16) -> Box<[u8]>
        where K: Hash
    {
        let mut collision_solver = CountPositiveCollisions::new(conf.goconf.bits_per_group * (level_size_groups as usize));
        Self::consider_all(conf, coding, keys, values, value_rev_indices, level_size_groups, level_nr, |_| group_seed, &mut collision_solver);
        collision_solver.positive_collisions_of_groups(conf.goconf.bits_per_group.into(), coding.bits_per_fragment())
    }

    /// Gets the value associated with the given key `key` and reports statistics to `access_stats`.
    pub fn get_stats<K: Hash + ?Sized, A: stats::AccessStatsCollector>(&self, key: &K, access_stats: &mut A) -> Option<<<C as Coding>::Decoder<'_> as Decoder>::Decoded> {
        let mut result_decoder = self.value_coding.decoder();
        let mut groups_before = 0;
        let mut level_nr = 0u32;
        loop {
            let level_size_groups = *self.level_size.get(level_nr as usize)?;
            let hash = self.goconf.hash_builder.hash_one(key, level_nr);
            let group = groups_before + group_nr(hash, level_size_groups);
            let seed = self.goconf.bits_per_seed.get_seed(&self.group_seeds, group as usize);
            let i = self.goconf.bits_per_group.bit_index_for_seed(hash, seed, group);
            if self.array.content.get_bit(i) {
                match result_decoder.consume(self.value_fragments.get_fragment(self.array.rank(i), self.value_coding.bits_per_fragment()) as u8) {
                    DecodingResult::Value(v) => {
                        access_stats.found_on_level(level_nr);
                        return Some(v)
                    },
                    DecodingResult::Invalid => {
                        access_stats.fail_on_level(level_nr);
                        return None
                    },
                    DecodingResult::Incomplete => {}
                }
            }
            groups_before += level_size_groups;
            level_nr += 1;
        }
    }

    /// Gets the value associated with the given key `k`.
    #[inline(always)]
    pub fn get<K: Hash + ?Sized>(&self, k: &K) -> Option<<<C as Coding>::Decoder<'_> as Decoder>::Decoded> {
        self.get_stats(k, &mut ())
    }

    /// Build `GOCMap` for given keys -> values map, where:
    /// - keys are given directly
    /// - values are encoded by Minimum-Redundancy (value_coding) and given in as values_fragments and corresponding values_fragments_sizes
    /// All three arrays must be of the same length.
    /// Note: conf.bits_per_fragment is ignored (since value_coding.bits_per_fragment is used).
    pub fn with_fragments<K, LSC, BS, BC>(
        keys: &mut [K], values: &mut [C::Codeword],
        value_coding: C, conf: GOCMapConf<BC, LSC, GS, SS, S>, stats: &mut BS) -> Self
        where K: Hash,
              LSC: LevelSizer,
              BS: stats::BuildStatsCollector
    {
        conf.goconf.validate();
        let mut level_size = Vec::<u64>::new();
        let mut arrays = Vec::<Box<[u64]>>::new();
        let mut group_seeds = Vec::<Box<[SS::VecElement]>>::new();
        let mut input_size = keys.len();
        let mut value_rev_indices: Box<[u8]> = values.iter().map(|c| value_coding.len_of(*c)-1).collect();
        let mut level_nr = 0u32;
        while input_size != 0 {
            let in_keys = &keys[0..input_size];
            let in_values = &values[0..input_size];
            let in_value_rev_indices = &value_rev_indices[0..input_size];
            let suggested_level_size_segments = conf.level_sizer.size_segments_for_values(
                || in_values.iter().zip(in_value_rev_indices.iter()).map(|(c, ri)| value_coding.rev_fragment_of(*c, *ri) as u64),
                input_size,
                value_coding.bits_per_fragment());
            
            let (level_size_groups, level_size_segments) = conf.goconf.bits_per_group.level_size_groups_segments(suggested_level_size_segments * 64);
            //let seed = level_nr;
            stats.level(input_size, level_size_segments * 64);
            let mut best_seeds = conf.goconf.bits_per_seed.new_zeroed_seed_vec(level_size_groups);
            let mut best_counts = Self::count_collisions_in_groups(&conf, &value_coding, in_keys, in_values, in_value_rev_indices,
                                                                   level_size_groups as u64,
                                                                   level_nr, 0);
            for new_seed in 1u16..=((1u32 << conf.goconf.bits_per_seed.into())-1) as u16 {
                let with_new_seed = Self::count_collisions_in_groups(&conf, &value_coding, in_keys, in_values, in_value_rev_indices,
                                                                     level_size_groups as u64, level_nr, new_seed);
                for group_index in 0..level_size_groups {
                    let new = with_new_seed[group_index];
                    let best = &mut best_counts[group_index];
                    if new > *best {
                        *best = new;
                        conf.goconf.bits_per_seed.set_seed(&mut best_seeds, group_index, new_seed);
                    }
                }
            }
            let mut collision_solver = LoMemAcceptEqualsSolver::new(level_size_segments, value_coding.bits_per_fragment());
            Self::consider_all(&conf, &value_coding, in_keys, in_values, in_value_rev_indices,
                               level_size_groups as u64, level_nr,
                               |group_index| conf.goconf.bits_per_seed.get_seed(&best_seeds, group_index as usize),
                               &mut collision_solver);
            let current_array = collision_solver.to_collision_array();
            let mut i = 0usize;
            while i < input_size {
                let hash = conf.goconf.hash_builder.hash_one(&keys[i], level_nr);
                let group = group_nr(hash, level_size_groups as u64);
                let bit_index = conf.goconf.bits_per_group.bit_index_for_seed(hash, conf.goconf.bits_per_seed.get_seed(&best_seeds, group as usize), group);
                if current_array.get_bit(bit_index) { // no collision
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
            level_size.push(level_size_groups as u64);
            group_seeds.push(best_seeds);
            level_nr += 1;
        }
        let (array, out_fragments_num) = ArrayWithRank::build(arrays.concat().into_boxed_slice());
        let group_seeds = conf.goconf.bits_per_seed.concatenate_seed_vecs(&level_size, group_seeds);
        let mut output_value_fragments = Box::<[u64]>::with_zeroed_bits(out_fragments_num as usize * value_coding.bits_per_fragment() as usize);
        for input_index in 0..keys.len() {
            let mut groups_before = 0u64;
            let mut level_nr = 0u32;
            loop {
                let level_size_groups = level_size[level_nr as usize];
                let hash = conf.goconf.hash_builder.hash_one(&keys[input_index], level_nr);
                let group = groups_before + group_nr(hash, level_size_groups);
                let i = conf.goconf.bits_per_group.bit_index_for_seed(hash, conf.goconf.bits_per_seed.get_seed(&group_seeds, group as usize), group);
                if array.content.get_bit(i) {
                    let code = &mut values[input_index];
                    output_value_fragments.init_fragment(   // AcceptEquals::set_value
                                                            array.rank(i),
                                                            value_coding.first_fragment_of(*code) as u64,
                                                            value_coding.bits_per_fragment());
                    if value_coding.remove_first_fragment_of(code) {
                        // stats.value_on_level(level_nr); // TODO do we need this? we can get average levels from lookups
                        break;
                    }
                }
                groups_before += level_size_groups;
                level_nr += 1;
            }
        }
        stats.end(0);
        Self {
            array,
            value_fragments: output_value_fragments,
            group_seeds,
            level_size: level_size.into_boxed_slice(),
            value_coding,
            goconf: conf.goconf,
        }
    }
}

impl<C: SerializableCoding, S: BuildSeededHasher, GS: GroupSize, SS: SeedSize> GOCMap<C, GS, SS, S> {
    /// Returns number of bytes which `write` will write, assuming that each call to `write_value` writes `bytes_per_value` bytes.
    pub fn write_bytes(&self, bytes_per_value: usize) -> usize {
        2*std::mem::size_of::<u8>()
            + VByte::array_size(&self.level_size)
            + AsIs::array_content_size(&self.array.content)
            + self.group_seeds.size_bytes_dyn()
            + self.value_coding.write_bytes(bytes_per_value)
            + AsIs::array_content_size(&self.value_fragments)
    }

    /// Writes `self` to the `output`, using `write_value` to write values.
    pub fn write<F>(&self, output: &mut dyn io::Write, write_value: F) -> io::Result<()>
        where F: FnMut(&mut dyn io::Write, &C::Value) -> io::Result<()>
    {
        self.goconf.bits_per_group.write(output)?;
        VByte::write_array(output, &self.level_size)?;
        AsIs::write_all(output, self.array.content.iter())?;
        self.goconf.bits_per_seed.write_seed_vec(output, &self.group_seeds)?;
        self.value_coding.write(output, write_value)?;
        AsIs::write_all(output, self.value_fragments.iter())
    }

    /// Reads `Self` from the `input`, using `read_value` to read values.
    /// Hasher must be the same as the one used to write.
    pub fn read_with_hasher<F>(input: &mut dyn io::Read, read_value: F, hasher: S) -> io::Result<Self>
        where F: FnMut(&mut dyn io::Read) -> io::Result<C::Value>
    {
        let bits_per_group = GS::read(input)?;
        let level_size = VByte::read_array(input)?;
        let number_of_groups = level_size.iter().map(|v|*v as usize).sum::<usize>();

        let array_content = read_bits(input, bits_per_group * number_of_groups)?;
        let (array_with_rank, number_of_ones) = ArrayWithRank::build(array_content);

        let (bits_per_seed, group_seeds) = SS::read_seed_vec(input, number_of_groups)?;

        let value_coding = C::read(input, read_value)?;
        let value_fragments = read_bits(input, number_of_ones as usize * value_coding.bits_per_fragment() as usize)?;
        Ok(Self {
            array: array_with_rank,
            value_fragments,
            group_seeds,
            level_size,
            value_coding,
            goconf: GOConf {
                bits_per_group,
                bits_per_seed,
                hash_builder: hasher
            }
        })
    }
}

impl<GS: GroupSize, SS: SeedSize, C: SerializableCoding> GOCMap<C, GS, SS> {
    /// Reads `Self` from the `input`, using `read_value` to read values.
    /// Only `GOCMap`s that use default hasher can be read by this method.
    pub fn read<F>(input: &mut dyn io::Read, read_value: F) -> io::Result<Self>
        where F: FnMut(&mut dyn io::Read) -> io::Result<C::Value>
    {
        Self::read_with_hasher(input, read_value, Default::default())
    }
}


impl<GS: GroupSize, SS: SeedSize, C: Coding, S: BuildSeededHasher> GOCMap<C, GS, SS, S> {
    pub fn from_slices_with_coding_conf<K, LSC, BS, BC>(
        keys: &mut [K], values: &[C::Value],
        value_coding: C, conf: GOCMapConf<BC, LSC, GS, SS, S>,
        stats: &mut BS
    ) -> Self
        where K: Hash,
              LSC: LevelSizer,
              BS: stats::BuildStatsCollector
    {
        Self::with_fragments(keys, &mut encode_all(&value_coding, values), value_coding, conf, stats)
    }

    pub fn from_slices_with_conf<K, LSC, BS, BC>(
        keys: &mut [K], values: &[C::Value], conf: GOCMapConf<BC, LSC, GS, SS, S>, stats: &mut BS
    ) -> Self
        where K: Hash,
              LSC: LevelSizer,
              BS: stats::BuildStatsCollector,
              BC: BuildCoding<C::Value, Coding=C>
    {
        Self::from_slices_with_coding_conf(keys, values, conf.coding.build_from_iter(values, 0), conf, stats)
    }

    pub fn from_map_with_coding_conf<K, H, LSC, BS, BC>(
        map: &HashMap<K, C::Value, H>, value_coding: C, conf: GOCMapConf<BC, LSC, GS, SS, S>, stats: &mut BS
    ) -> Self
        where K: Hash + Clone,
              LSC: LevelSizer,
              BS: stats::BuildStatsCollector,
              BC: BuildCoding<C::Value, Coding=C>
    {
        let (mut keys, mut values) = encode_all_from_map(&value_coding, map);
        Self::with_fragments(&mut keys, &mut values, value_coding, conf, stats)
    }

    pub fn from_map_with_conf<K, H, LSC, BS, BC>(
        map: &HashMap<K, C::Value, H>, conf: GOCMapConf<BC, LSC, GS, SS, S>, stats: &mut BS
    ) -> Self
        where K: Hash + Clone,
              LSC: LevelSizer,
              BS: stats::BuildStatsCollector,
              BC: BuildCoding<C::Value, Coding=C>
    {
        Self::from_map_with_coding_conf(map, conf.coding.build_from_iter(map.values(), 0), conf, stats)
    }
}

impl<C: Coding> GOCMap<C> {
    pub fn from_slices_with_coding<K: Hash, BS: stats::BuildStatsCollector>(keys: &mut [K], values: &[C::Value], value_coding: C, stats: &mut BS) -> Self {
        Self::from_slices_with_coding_conf(keys, values, value_coding, GOCMapConf::default(), stats)
    }
}

impl<V: Hash + Eq + Clone> GOCMap<minimum_redundancy::Coding<V>> {
    pub fn from_slices<K: Hash, BS: stats::BuildStatsCollector>(keys: &mut [K], values: &[V], stats: &mut BS) -> Self {
        Self::from_slices_with_conf(keys, values, Default::default(), stats)
    }

    pub fn from_map<K: Hash + Clone, H, BS: stats::BuildStatsCollector>(map: &HashMap<K, V, H>, stats: &mut BS) -> Self {
        Self::from_map_with_conf(map, Default::default(), stats)
    }
}

impl<K: Hash + Clone, V: Hash + Eq + Clone, H> From<&HashMap<K, V, H>> for GOCMap<minimum_redundancy::Coding<V>> {
    fn from(map: &HashMap<K, V, H>) -> Self {
        Self::from_map(map, &mut ())
    }
}

impl<K: Hash + Clone, V: Hash + Eq + Clone, H> From<HashMap<K, V, H>> for GOCMap<minimum_redundancy::Coding<V>> {
    fn from(map: HashMap<K, V, H>) -> Self {
        Self::from_map(&map, &mut ())
    }
}


#[cfg(test)]
mod tests {
    use std::fmt::Debug;
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use bitm::ceiling_div;
    use maplit::hashmap;
    use crate::coding::BuildMinimumRedundancy;
    //use minimum_redundancy::{write_int, read_int};

    fn test_read_write<GS: GroupSize, SS: SeedSize, C: SerializableCoding<Value=u8>>(fpmap: &GOCMap<C, GS, SS>) where SS::VecElement: PartialEq + Debug {
        let mut buff = Vec::new();
        fpmap.write(&mut buff, |b, v| AsIs::write(b, *v)).unwrap();
        assert_eq!(buff.len(), fpmap.write_bytes(1));
        let read = GOCMap::<C, GS, SS>::read(&mut &buff[..], |b| AsIs::read(b)).unwrap();
        assert_eq!(fpmap.level_size, read.level_size);
        assert_eq!(fpmap.array.content, read.array.content);
        assert_eq!(fpmap.group_seeds, read.group_seeds);
        assert_eq!(fpmap.value_fragments, read.value_fragments);
        assert_eq!(fpmap.goconf.bits_per_group.into(), read.goconf.bits_per_group.into());
        assert_eq!(fpmap.goconf.bits_per_seed.into(), read.goconf.bits_per_seed.into());
    }

    fn test_fpcmap_invariants<GS: GroupSize, SS: SeedSize, C: Coding>(fpmap: &GOCMap<C, GS, SS>) {
        let number_of_groups = fpmap.level_size.iter().map(|v| *v as usize).sum::<usize>();
        assert_eq!(fpmap.goconf.bits_per_group * number_of_groups, fpmap.array.content.len() * 64);
        assert_eq!(ceiling_div(fpmap.goconf.bits_per_seed.into() as usize * number_of_groups, 64), fpmap.group_seeds.len());
        assert_eq!(
            ceiling_div(fpmap.array.content.iter().map(|v|v.count_ones()).sum::<u32>() as usize * fpmap.value_coding.bits_per_fragment() as usize, 64),
            fpmap.value_fragments.len()
        );
    }

    fn test_4pairs<GS: GroupSize, SS: SeedSize, LSC: LevelSizer>(conf: GOCMapConf<BuildMinimumRedundancy, LSC, GS, SS>) where SS::VecElement: PartialEq + Debug {
        let fpmap = GOCMap::from_map_with_conf(&hashmap!('a'=>1u8, 'b'=>2u8, 'c'=>1u8, 'd'=>3u8), conf, &mut ());
        assert_eq!(fpmap.get(&'a'), Some(&1));
        assert_eq!(fpmap.get(&'b'), Some(&2));
        assert_eq!(fpmap.get(&'c'), Some(&1));
        assert_eq!(fpmap.get(&'d'), Some(&3));
        test_fpcmap_invariants(&fpmap);
        test_read_write(&fpmap);
    }

    #[test]
    fn with_hashmap_bpf1() {
        test_4pairs(GOCMapConf::bpf(1));
    }

    fn test_8pairs<GS: GroupSize, SS: SeedSize, LSC: LevelSizer>(conf: GOCMapConf<BuildMinimumRedundancy, LSC, GS, SS>) where SS::VecElement: PartialEq + Debug {
        let fpmap = GOCMap::from_map_with_conf(&hashmap!(
            'a' => 1, 'b' => 2, 'c' => 1, 'd' => 3,
            'e' => 4, 'f' => 1, 'g' => 5, 'h' => 6), conf, &mut ());
        assert_eq!(fpmap.get(&'a'), Some(&1));
        assert_eq!(fpmap.get(&'b'), Some(&2));
        assert_eq!(fpmap.get(&'c'), Some(&1));
        assert_eq!(fpmap.get(&'d'), Some(&3));
        assert_eq!(fpmap.get(&'e'), Some(&4));
        assert_eq!(fpmap.get(&'f'), Some(&1));
        assert_eq!(fpmap.get(&'g'), Some(&5));
        assert_eq!(fpmap.get(&'h'), Some(&6));
        test_fpcmap_invariants(&fpmap);
        test_read_write(&fpmap);
    }

    #[test]
    fn with_hashmap_bpf2() {
        test_8pairs(GOCMapConf::bpf(2));
    }

}