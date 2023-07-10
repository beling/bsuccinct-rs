mod conf;
use binout::{AsIs, Serializer, VByte};
pub use conf::MapConf;

use std::hash::Hash;
use bitm::{BitAccess, BitArrayWithRank};

pub use super::level_size_chooser::{SimpleLevelSizeChooser, ProportionalLevelSize, OptimalLevelSize};
use ph::{BuildDefaultSeededHasher, BuildSeededHasher, utils, stats, utils::{ArrayWithRank, read_bits}};
use std::collections::HashMap;
use std::io;

use crate::{fp::collision_solver::{CollisionSolver, CollisionSolverBuilder}, bits_to_store};
use dyn_size_of::GetSize;

/// Finger-Printing based static function (map) that can only store integer values of given bit-size.
pub struct Map<S = BuildDefaultSeededHasher> {
    array: ArrayWithRank,
    values: Box<[u64]>,    // BitVec
    bits_per_value: u8,
    level_sizes: Box<[u64]>,
    hash_builder: S
}

impl<S: BuildSeededHasher> GetSize for Map<S> {
    fn size_bytes_dyn(&self) -> usize {
        self.array.size_bytes_dyn()
            + self.values.size_bytes_dyn()
            + self.level_sizes.size_bytes_dyn()
    }

    const USES_DYN_MEM: bool = true;
}

impl<S: BuildSeededHasher> Map<S> {

    #[inline(always)] fn index<K: Hash>(&self, k: &K, level_nr: u32, size: usize) -> usize {
        utils::map64_to_64(self.hash_builder.hash_one(k, level_nr), size as u64) as usize
    }

    /// Gets the value associated with the given key k and reports statistics to access_stats.
    pub fn get_stats<K: Hash, A: stats::AccessStatsCollector>(&self, k: &K, access_stats: &mut A) -> Option<u64> {
        let mut array_begin_index = 0usize;
        let mut level = 0u32;
        loop {
            let level_size = (*self.level_sizes.get(level as usize)? as usize) << 6usize;
            let i = array_begin_index + self.index(k, level, level_size);
            if self.array.content.get_bit(i) {
                access_stats.found_on_level(level);
                return Some(self.values.get_fragment(self.array.rank(i) as usize, self.bits_per_value));
            }
            array_begin_index += level_size;
            level += 1;
        }
    }

    /// Gets the value associated with the given key k.
    pub fn get<K: Hash>(&self, k: &K) -> Option<u64> {
        self.get_stats(k, &mut ())
    }

    /// Build BBMap for given keys -> values map, where:
    /// - keys are given directly,
    /// - TODO values are given as bit vector with bit_per_value.
    /// These arrays must be of the same length.
    fn with_slices_conf_stats<K, LSC, CSB, BS>(
        keys: &mut [K], values: &mut [u8],
        /*&mut [u64],*/ mut conf: MapConf<LSC, CSB, S>,
        stats: &mut BS
    ) -> Self
        where K: Hash,
              LSC: SimpleLevelSizeChooser,
              CSB: CollisionSolverBuilder,
              BS: stats::BuildStatsCollector

    {
        if conf.bits_per_value == 0 {
            conf.bits_per_value = bits_to_store!(Into::<u64>::into(values.iter().max().unwrap().clone()));
        }
        let mut level_sizes = Vec::<u64>::new();
        let mut arrays = Vec::<Box<[u64]>>::new();
        let mut input_size = keys.len();
        let mut level_nr = 0u32;
        while input_size != 0 {
            let level_size_segments = conf.level_size_chooser.size_segments(
                &values[0..input_size], conf.bits_per_value);
            let level_size = level_size_segments * 64;
            stats.level(input_size, level_size);
            let mut collision_solver = conf.collision_solver.new(level_size_segments, conf.bits_per_value);
            for i in 0..input_size {
                let a_index = utils::map64_to_64(conf.hash.hash_one(&keys[i], level_nr), level_size as u64) as usize;
                if collision_solver.is_under_collision(a_index) { continue }
                collision_solver.process_fragment(a_index, values[i], conf.bits_per_value);
            }

            let current_array = collision_solver.to_collision_array();
            let mut i = 0usize;
            while i < input_size {
                let a_index = utils::map64_to_64(conf.hash.hash_one(&keys[i], level_nr), level_size as u64) as usize;
                if current_array.get_bit(a_index) { // no collision
                    // remove i-th element by replacing it with the last one
                    input_size -= 1;
                    keys.swap(i, input_size);
                    //values.swap_fragments(i, input_size, bits_per_value);
                    values.swap(i, input_size);
                } else {    // collision, has to be processed again, at the next level
                    i += 1;
                }
            }
            arrays.push(current_array);
            level_sizes.push(level_size_segments as u64);
            level_nr += 1;
        }

        let (array, out_fragments_num)  = ArrayWithRank::build(arrays.concat().into_boxed_slice());
        let mut output_value_fragments = CSB::CollisionSolver::construct_value_array(out_fragments_num as usize, conf.bits_per_value);
        for input_index in 0..keys.len() {
            //let mut result_decoder = self.value_coding.decoder();
            let mut array_begin_index = 0usize;
            let mut level = 0u32;
            loop {
                let level_size = (level_sizes[level as usize] as usize) << 6usize;
                let i = array_begin_index + utils::map64_to_64(conf.hash.hash_one(&keys[input_index], level), level_size as u64) as usize;
                if array.content.get_bit(i) {
                    CSB::CollisionSolver::set_value(&mut output_value_fragments, array.rank(i) as usize, values[input_index], conf.bits_per_value);
                    // stats.value_on_level(level); // TODO do we need this? we can get average levels from lookups
                    break;
                }
                array_begin_index += level_size;
                level += 1;
            }
        }
        stats.end(0);
        Self {
            array,
            values: output_value_fragments,
            bits_per_value: conf.bits_per_value,
            level_sizes: level_sizes.into_boxed_slice(),
            hash_builder: conf.hash
        }
    }

    #[inline]
    pub fn with_slices_conf<K: Hash, LSC: SimpleLevelSizeChooser, CSB: CollisionSolverBuilder>(
        keys: &mut [K], values: &mut [u8], /*&mut [u64],*/ conf: MapConf<LSC, CSB, S>) -> Self
    {
        Self::with_slices_conf_stats(keys, values, conf, &mut ())
    }

    /// Returns number of bytes which write will write.
    pub fn write_bytes(&self) -> usize {
        AsIs::size(self.bits_per_value) +
        VByte::array_size(&self.level_sizes) +
        AsIs::array_content_size(&self.array.content) +
        AsIs::array_content_size(&self.values)
    }

    /// Write `self` to the output.
    pub fn write(&self, output: &mut dyn io::Write) -> io::Result<()>
    {
        AsIs::write(output, self.bits_per_value)?;
        VByte::write_array(output, &self.level_sizes)?;
        AsIs::write_all(output, self.array.content.iter())?;
        AsIs::write_all(output, self.values.iter())
    }

    /// Read `self` from the `input` (`hasher` must be the same as used by written [`Map`]).
    pub fn read_with_hasher(input: &mut dyn io::Read, hasher: S) -> io::Result<Self>
    {
        let bits_per_value = AsIs::read(input)?;
        let level_sizes = VByte::read_array(input)?;
        let array_content = AsIs::read_n(input, level_sizes.iter().map(|v|*v as usize).sum::<usize>())?;
        let (array_with_rank, number_of_ones) = ArrayWithRank::build(array_content);
        let values = read_bits(input, number_of_ones as usize * bits_per_value as usize)?;
        Ok(Self {
            array: array_with_rank,
            values,
            bits_per_value,
            level_sizes,
            hash_builder: hasher
        })
    }
}

impl Map {
    /// Read `self` from the `input`. Only `FPMap`s that use default hasher can be read by this method.
    pub fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Self::read_with_hasher(input, Default::default())
    }
}

impl<S: BuildSeededHasher> Map<S> {

    pub fn with_map_conf<K: Hash + Clone, H, LSC: SimpleLevelSizeChooser, CSB: CollisionSolverBuilder, BS: stats::BuildStatsCollector>(
        map: &HashMap<K, u8, H>,
        conf: MapConf<LSC, CSB, S>,
        stats: &mut BS
    ) -> Self
    {
        let mut keys = Vec::<K>::with_capacity(map.len());
        let mut values = Vec::<u8>::with_capacity(map.len());
        for (k, v) in map {
            keys.push(k.clone());
            values.push(*v);
        }
        Self::with_slices_conf_stats(&mut keys, &mut values, conf, stats)
    }
}

impl Map {
    pub fn with_map<K: Hash + Clone, H, BS: stats::BuildStatsCollector>(map: &HashMap<K, u8, H>, stats: &mut BS) -> Self {
        Self::with_map_conf(map, Default::default(), stats)
    }
}

impl<K: Hash + Clone, H> From<&HashMap<K, u8, H>> for Map {
    fn from(map: &HashMap<K, u8, H>) -> Self {
        Self::with_map(map, &mut ())
    }
}

impl<K: Hash + Clone, H> From<HashMap<K, u8, H>> for Map {
    fn from(map: HashMap<K, u8, H>) -> Self {
        Self::with_map(&map, &mut ())
    }
}


#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use bitm::ceiling_div;
    use maplit::hashmap;

    fn test_read_write(bbmap: &Map) {
        let mut buff = Vec::new();
        bbmap.write(&mut buff).unwrap();
        assert_eq!(buff.len(), bbmap.write_bytes());
        let read = Map::read(&mut &buff[..]).unwrap();
        assert_eq!(bbmap.level_sizes, read.level_sizes);
    }

    fn test_bbmap_invariants(bbmap: &Map) {
        assert_eq!(bbmap.level_sizes.iter().map(|v| *v as usize).sum::<usize>(), bbmap.array.content.len());
        assert_eq!(
            ceiling_div(bbmap.array.content.iter().map(|v|v.count_ones()).sum::<u32>() as usize * bbmap.bits_per_value as usize, 64),
            bbmap.values.len()
        );
    }

    fn test_4pairs(conf: MapConf) {
        let bbmap = Map::with_map_conf(&hashmap!('a'=>1u8, 'b'=>2u8, 'c'=>1u8, 'd'=>3u8), conf, &mut ());
        assert_eq!(bbmap.get(&'a'), Some(1));
        assert_eq!(bbmap.get(&'b'), Some(2));
        assert_eq!(bbmap.get(&'c'), Some(1));
        assert_eq!(bbmap.get(&'d'), Some(3));
        test_bbmap_invariants(&bbmap);
        test_read_write(&bbmap);
    }

    #[test]
    fn with_hashmap_4pairs() {
        test_4pairs(MapConf::default());
    }

    fn test_8pairs<LSC: SimpleLevelSizeChooser>(conf: MapConf<LSC>) {
        let bbmap = Map::with_map_conf(&hashmap!(
            'a' => 1, 'b' => 2, 'c' => 1, 'd' => 3,
            'e' => 4, 'f' => 1, 'g' => 5, 'h' => 6), conf, &mut ());
        assert_eq!(bbmap.get(&'a'), Some(1));
        assert_eq!(bbmap.get(&'b'), Some(2));
        assert_eq!(bbmap.get(&'c'), Some(1));
        assert_eq!(bbmap.get(&'d'), Some(3));
        assert_eq!(bbmap.get(&'e'), Some(4));
        assert_eq!(bbmap.get(&'f'), Some(1));
        assert_eq!(bbmap.get(&'g'), Some(5));
        assert_eq!(bbmap.get(&'h'), Some(6));
        test_bbmap_invariants(&bbmap);
        test_read_write(&bbmap);
    }

    #[test]
    fn with_hashmap_8pairs() {
        test_8pairs(MapConf::default());
    }
}