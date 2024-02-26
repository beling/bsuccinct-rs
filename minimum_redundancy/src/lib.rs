#![doc = include_str!("../README.md")]

use std::collections::HashMap;
use std::hash::Hash;
use co_sort::{co_sort, Permutation};
use frequencies::Weight;

use std::borrow::Borrow;
use binout::{VByte, Serializer};
use dyn_size_of::GetSize;

mod code;
pub use code::{Code, CodeIterator, ReversedCodeIterator};

mod frequencies;
pub use frequencies::Frequencies;
mod degree;
pub use degree::*;
mod decoder;
pub use decoder::Decoder;
mod iterators;
pub use iterators::{CodesIterator, ReversedCodesIterator, LevelIterator};


/// Succinct representation of minimum-redundancy coding
/// (huffman tree of some degree in the canonical form).
pub struct Coding<ValueType, D = BitsPerFragment> {
    /// Values, from the most frequent to the least.
    pub values: Box<[ValueType]>,
    /// Number of the internal nodes of each tree level. The root is not counted.
    /// Contains exactly one zero at the end.
    pub internal_nodes_count: Box<[u32]>,
    /// Size of the fragment given as bits per fragment or the degree of the Huffman tree.
    pub degree: D
}

/// Points the number of bytes needed to store value of the type `ValueType`.
/// This number of bytes can be constant or can depend on the value.
pub enum ValueSize<'v, ValueType> {
    /// Holds constant number of bytes needed to store each value.
    Const(usize),
    /// Holds callback that shows the number of bytes needed to store given value.
    Variable(&'v dyn Fn(&ValueType)->usize)
}

impl<ValueType: GetSize, D> GetSize for Coding<ValueType, D> {
    fn size_bytes_dyn(&self) -> usize {
        self.values.size_bytes_dyn() + self.internal_nodes_count.size_bytes_dyn()
    }
    const USES_DYN_MEM: bool = true;
}

impl<ValueType, D: TreeDegree> Coding<ValueType, D> {

    /// Constructs coding for given `frequencies` of values and `degree` of the Huffman tree.
    pub fn from_frequencies<F: Frequencies<Value=ValueType>>(degree: D, frequencies: F) -> Self {
        let (values, mut freq) = frequencies.into_sorted();
        Self::from_sorted(degree, values, &mut freq)
    }

    /// Constructs coding for given `frequencies` of values and `degree` of the Huffman tree.
    /// Values are cloned from `frequencies`.
    pub fn from_frequencies_cloned<F: Frequencies<Value=ValueType>>(degree: D, frequencies: &F) -> Self
        where F::Value: Clone
    {
        let (values, mut freq) = frequencies.sorted();
        Self::from_sorted(degree, values, &mut freq)
    }

    /// Counts occurrences of all values exposed by `iter` and constructs coding for obtained
    /// frequencies of values and `degree` of the Huffman tree.
    pub fn from_iter<Iter>(degree: D, iter: Iter) -> Self
        where Iter: IntoIterator, Iter::Item: Borrow<ValueType>, ValueType: Hash + Eq + Clone
    {
        Self::from_frequencies(degree, HashMap::<ValueType, usize>::with_occurrences_of(iter))
    }

    /// Returns total (summarized) number of code fragments of all values.
    ///
    /// The algorithm runs in *O(L)* time and *O(1)* memory,
    /// where *L* is the number of fragments in the longest codeword.
    pub fn total_fragments_count(&self) -> usize {
        self.levels().map(|(values, _, fragments)| values.len()*fragments as usize).sum()
    }

    /// Returns decoder that allows for decoding a value.
    #[inline] pub fn decoder(&self) -> Decoder<ValueType, D> {
        return Decoder::<ValueType, D>::new(self);
    }

    /// Construct coding (of given `degree`) for the given `values`, where
    /// `freq` is an array of numbers of occurrences of corresponding values.
    /// `freq` has to be in non-descending order and of the same length as values.
    ///
    /// The algorithm runs in *O(values.len)* time,
    /// in-place (it uses and changes `freq` and move values to the returned `Coding` object).
    pub fn from_sorted<W>(degree: D, mut values: Box<[ValueType]>, freq: &mut [W]) -> Self
        where W: Weight
    {
        assert!(freq.len() <= (1<<32), "minimum_redundancy::Coding does not support more than 2 to the power of 32 different values");
        let len = freq.len();
        let tree_degree = degree.as_u32();
        if len <= tree_degree as usize { // is one-level tree enough?
            values.reverse();
            return Coding {
                values,
                internal_nodes_count: vec![0u32].into_boxed_slice(),
                degree,
            }
        }
        // here: len > tree_degree >= 2

        let reduction_per_step = tree_degree - 1;
        let mut current_tree_degree = (len - 1) as u32 % reduction_per_step; // desired reduction in the first step
        current_tree_degree = if current_tree_degree == 0 { tree_degree } else { current_tree_degree + 1 }; // children of the internal node constructed in the first step
        let mut internals_begin = 0; // first internal node = next parent node to be used
        let mut leafs_begin = 0;     // next leaf to be used
        let internal_nodes_size = (len - 2) as u32 / reduction_per_step + 1; // safe as len > 2
        // = floor of (len+reduction_per_step-2) / reduction_per_step = ceil of (len-1) / reduction_per_step

        for next in 0u32..internal_nodes_size {    // next is next value to be assigned
            // select first item for a pairing
            if leafs_begin >= len || freq[internals_begin] < freq[leafs_begin] {
                freq[next as usize] = freq[internals_begin];
                freq[internals_begin] = Weight::of(next);
                internals_begin += 1;
            } else {
                freq[next as usize] = freq[leafs_begin];
                leafs_begin += 1;
            }

            // add on the second, ... items
            for _ in 1..current_tree_degree {
                if leafs_begin >= len || (internals_begin < next as usize && freq[internals_begin] < freq[leafs_begin]) {
                    freq[next as usize] += freq[internals_begin];
                    freq[internals_begin] = Weight::of(next);
                    internals_begin += 1;
                } else {
                    freq[next as usize] += freq[leafs_begin];
                    leafs_begin += 1;
                }
            }
            current_tree_degree = tree_degree;    // only in first iteration can be: current_tree_degree != tree_degree
        }
        //const CONVERTIBLE_TO_USIZE: &'static str = "symbol weights must be convertible to usize";
        //let convertible_to_usize = |_| { panic!("symbol weights must be convertible to usize") };
        // second pass, right to left, setting internal depths, we also find the maximum depth
        let mut max_depth = 0usize;
        freq[(internal_nodes_size - 1) as usize] = Weight::of(0);    // value for the root
        for next in (0..internal_nodes_size - 1).rev() {
            freq[next as usize] = freq[freq[next as usize].as_usize()] + Weight::of(1);
            let f = freq[next as usize].as_usize();
            if f > max_depth { max_depth = f; }
        }

        values.reverse();
        let mut result = Self {
            values,
            internal_nodes_count: vec![0u32; max_depth + 1].into_boxed_slice(),
            degree
        };
        for i in 0..internal_nodes_size - 1 {
            result.internal_nodes_count[freq[i as usize].as_usize() - 1] += 1;  // only root is at the level 0, we skip it
        }   // no internal nodes at the last level, result.internal_nodes_count[max_depth] is 0

        return result;
    }

    /// Construct coding (of the given `degree`) for the given `values`, where
    /// `freq` has to be of the same length as values and contain number of occurrences of corresponding values.
    ///
    /// The algorithm runs in *O(values.len * log(values.len))* time.
    pub fn from_unsorted<W>(degree: D, mut values: Box<[ValueType]>, freq: &mut [W]) -> Self
        where W: Weight
    {
        co_sort!(freq, values);
        Self::from_sorted(degree, values, freq)
    }

    /// Returns number of bytes which `write_internal_nodes_count` will write.
    pub fn write_internal_nodes_count_bytes(&self) -> usize {
        VByte::array_size(&self.internal_nodes_count[..self.internal_nodes_count.len()-1])
    }

    /// Writes `internal_nodes_count` to `output` as the following `internal_nodes_count.len()`, VByte values:
    /// `internal_nodes_count.len()-1` (=l), `internal_nodes_count[0]`, `internal_nodes_count[1]`, ..., `internal_nodes_count[l]`
    pub fn write_internal_nodes_count(&self, output: &mut dyn std::io::Write) -> std::io::Result<()> {
        VByte::write_array(output, &self.internal_nodes_count[..self.internal_nodes_count.len()-1])
        //<VByte as Serializer::<u32>>::write_all(output, &self.internal_nodes_count[..l])
        //self.internal_nodes_count[..l].iter().try_for_each(|v| vbyte_write(output, *v as u64))
    }

    /// Reads (written by `write_internal_nodes_count`) `internal_nodes_count` from `input`.
    pub fn read_internal_nodes_count(input: &mut dyn std::io::Read) -> std::io::Result<Box<[u32]>> {
        let s: usize = VByte::read(input)?;
        let mut v = Vec::<u32>::with_capacity(s + 1);
        for _ in 0..s { v.push(VByte::read(input)?); }
        v.push(0);
        Ok(v.into_boxed_slice())
    }

    /// Returns number of bytes which `write_values` will write,
    /// assuming that each call to `write_value` writes the number of bytes pointed by `value_size`.
    pub fn write_values_size_bytes(&self, value_size: ValueSize<ValueType>) -> usize {
        VByte::size(self.values.len()) +
            match value_size {
                ValueSize::Const(bytes_per_value) => { bytes_per_value*self.values.len() }
                ValueSize::Variable(f) => { self.values.iter().map(f).sum::<usize>() }
            }
    }

    /// Writes `values` to the given `output`, using `write_value` to write each value.
    pub fn write_values<F>(&self, output: &mut dyn std::io::Write, mut write_value: F) -> std::io::Result<()>
        where F: FnMut(&mut dyn std::io::Write, &ValueType) -> std::io::Result<()>
    {
        VByte::write(output, self.values.len())?;
        self.values.iter().try_for_each(|v| { write_value(output, v) })
    }

    /// Reads `values` from the given `input`, using `read_value` to read each value.
    pub fn read_values<F>(input: &mut dyn std::io::Read, mut read_value: F) -> std::io::Result<Box<[ValueType]>>
        where F: FnMut(&mut dyn std::io::Read) -> std::io::Result<ValueType>
    {
        let s = VByte::read(input)?;
        let mut v = Vec::with_capacity(s);
        for _ in 0..s { v.push(read_value(input)?); }
        Ok(v.into_boxed_slice())
    }

    /// Returns number of bytes which `write` will write,
    /// assuming that each call to `write_value` writes the number of bytes pointed by `value_size`.
    pub fn write_size_bytes(&self, value_size: ValueSize<ValueType>) -> usize {
        self.degree.write_size_bytes() + self.write_internal_nodes_count_bytes()
            + self.write_values_size_bytes(value_size)
    }

    /// Writes `self` to the given `output`, using `write_value` to write each value.
    pub fn write<F>(&self, output: &mut dyn std::io::Write, write_value: F) -> std::io::Result<()>
        where F: FnMut(&mut dyn std::io::Write, &ValueType) -> std::io::Result<()>
    {
        self.degree.write(output)?;
        self.write_internal_nodes_count(output)?;
        self.write_values(output, write_value)
    }

    /// Reads `Coding` from the given `input`, using `read_value` to read each value.
    pub fn read<F>(input: &mut dyn std::io::Read, read_value: F) -> std::io::Result<Self>
        where F: FnMut(&mut dyn std::io::Read) -> std::io::Result<ValueType>
    {
        let fragment_size = D::read(input)?;
        let internal_nodes_count = Self::read_internal_nodes_count(input)?;
        Ok(Self {
            values: Self::read_values(input, read_value)?,
            internal_nodes_count,
            degree: fragment_size
        })
    }

    /// Returns iterator over the levels of the huffman tree.
    #[inline] pub fn levels(&self) -> LevelIterator<'_, ValueType, D> {
        LevelIterator::<'_, ValueType, D>::new(&self)
    }

    /// Reverse the `codeword`.
    #[inline(always)] pub fn reverse_code(&self, codeword: &mut Code) {
        codeword.content = self.degree.reverse_code(codeword.content, codeword.len);
    }

    /// Returns reversed copy of the given `codeword`.
    #[inline(always)] pub fn reversed_code(&self, codeword: Code) -> Code {
        Code {
            content: self.degree.reverse_code(codeword.content, codeword.len),
            len: codeword.len
        }
    }

    /// Returns iterator over value-codeword pairs.
    #[inline] pub fn codes(&self) -> CodesIterator<'_, ValueType, D> {
        CodesIterator::<'_, ValueType, D>::new(&self)
    }

    /// Returns iterator over value-codeword pairs with reversed codewords.
    #[inline] pub fn reversed_codes(&self) -> ReversedCodesIterator<'_, ValueType, D> {
        ReversedCodesIterator::<'_, ValueType, D>::new(&self)
    }

    /*pub fn codes(&self) -> impl Iterator<Item=(&ValueType, Code)> {
        self.levels().flat_map(|(values, first_code_bits, fragments)|
            values.iter().enumerate().map(move |(i, v)| {
                (v, Code{ bits: first_code_bits + i as u32, fragments })
            })
        )
    }*/
}

impl<ValueType: Hash + Eq, D: TreeDegree> Coding<ValueType, D> {

    /// Returns a map from (references to) values to the lengths of their codes.
    pub fn code_lengths_ref(&self) -> HashMap<&ValueType, u32> {
        self.codes().map(|(v, c)| (v, c.len)).collect()
    }

    /// Returns a map from (references to) values to their codes.
    pub fn codes_for_values_ref(&self) -> HashMap<&ValueType, Code> {
        self.codes().collect()
    }

    /// Returns a map from (references to) values to their reversed codes.
    pub fn reversed_codes_for_values_ref(&self) -> HashMap<&ValueType, Code> {
        self.reversed_codes().collect()
    }
}

impl<ValueType: Hash + Eq + Clone, D: TreeDegree> Coding<ValueType, D> {
    /// Returns a map from (clones of) values to the lengths of their codes.
    pub fn code_lengths(&self) -> HashMap<ValueType, u32> {
        let mut result = HashMap::<ValueType, u32>::with_capacity(self.values.len());
        for (value, code) in self.codes() {
            result.insert(value.clone(), code.len);
        }
        result
    }

    /// Returns a map from (clones of) values to their codes.
    pub fn codes_for_values(&self) -> HashMap<ValueType, Code> {
        let mut result = HashMap::<ValueType, Code>::with_capacity(self.values.len());
        for (value, code) in self.codes() {
            result.insert(value.clone(), code);
        };
        result
    }

    /// Returns a map from (clones of) values to their reversed codes.
    pub fn reversed_codes_for_values(&self) -> HashMap<ValueType, Code> {
        let mut result = HashMap::<ValueType, Code>::with_capacity(self.values.len());
        for (value, code) in self.reversed_codes() {
            result.insert(value.clone(), code);
        };
        result
    }
}

impl<D: TreeDegree> Coding<u8, D> {
    /// Returns array indexed by values that contains the lengths of their codes.
    pub fn code_lengths_array(&self) -> [u32; 256] {
        let mut result = [0; 256];
        for (value, code) in self.codes() {
            result[*value as usize] = code.len;
        }
        result
    }

    /// Returns array indexed by values that contains their codes.
    pub fn codes_for_values_array(&self) -> [Code; 256] {
        let mut result = [Default::default(); 256];
        for (value, code) in self.codes() {
            result[*value as usize] = code;
        };
        return result;
    }

    /// Returns array indexed by values that contains their reversed codes.
    pub fn reversed_codes_for_values_array(&self) -> [Code; 256] {
        let mut result = [Default::default(); 256];
        for (value, code) in self.reversed_codes() {
            result[*value as usize] = code;
        };
        return result;
    }
}

/// Result of fragment decoding returned be `consume` method of `Decoder`.
#[derive(PartialOrd, Ord, PartialEq, Eq, Debug, Clone, Hash)]
pub enum DecodingResult<T> {
    /// Completed value that has been successfully decoded.
    Value(T),
    /// The codeword is incomplete and the next fragment is needed.
    Incomplete,
    /// The codeword is invalid (possible only for bits per fragment > 1).
    Invalid
}

impl<T> From<Option<T>> for DecodingResult<T> {
    #[inline(always)] fn from(option: Option<T>) -> Self {
        if let Some(v) = option { DecodingResult::Value(v) } else { DecodingResult::Invalid }
    }
}    // Note: Brodnik describes also faster decoder that runs in expected loglog(length of the longest code) expected time, but requires all codeword bits in advance.

/// Heuristically calculates bits per fragment that gives about constant length average code size.
/// `entropy` should equals to entropy or a bit less, e.g. entropy minus `0.2`
pub fn entropy_to_bpf(entropy: f64) -> u8 {
    (1f64.max(entropy).ceil() as u64 - 1).min(8) as u8
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use maplit::hashmap;
    use binout::*;

    fn test_read_write<FS: TreeDegree>(huffman: &Coding<char, FS>) {
        let mut buff = Vec::new();
        huffman.write_values(&mut buff, |b, v| AsIs::write(b, *v as u8)).unwrap();
        assert_eq!(buff.len(), huffman.write_values_size_bytes(ValueSize::Const(1)));
        assert_eq!(Coding::<_, FS>::read_values(&mut &buff[..], |b| AsIs::read(b).map(|v: u8| v as char)).unwrap(), huffman.values);
        buff.clear();
        huffman.write_internal_nodes_count(&mut buff).unwrap();
        assert_eq!(buff.len(), huffman.write_internal_nodes_count_bytes());
        assert_eq!(Coding::<char, FS>::read_internal_nodes_count(&mut &buff[..]).unwrap(), huffman.internal_nodes_count);
        buff.clear();
        huffman.write(&mut buff, |b, v| AsIs::write(b, *v as u8)).unwrap();
        assert_eq!(buff.len(), huffman.write_size_bytes(ValueSize::Const(1)));
        assert_eq!(buff.len(), huffman.write_size_bytes(ValueSize::Variable(&|_| 1)));
        let read = Coding::<_, FS>::read(&mut &buff[..], |b| AsIs::read(b).map(|v: u8| v as char)).unwrap();
        assert_eq!(huffman.degree.as_u32(), read.degree.as_u32());
        assert_eq!(huffman.values, read.values);
        assert_eq!(huffman.internal_nodes_count, read.internal_nodes_count);
    }

    #[test]
    fn coding_3sym_1bit() {
        //  /  \
        // /\  a
        // bc
        let huffman = Coding::from_frequencies(BitsPerFragment(1),
                                               hashmap!('a' => 100u32, 'b' => 50, 'c' => 10));
        assert_eq!(huffman.total_fragments_count(), 5);
        assert_eq!(huffman.values.as_ref(), ['a', 'b', 'c']);
        assert_eq!(huffman.internal_nodes_count.as_ref(), [1, 0]);
        assert_eq!(huffman.codes_for_values(), hashmap!(
                'a' => Code{ content: 0b1, len: 1 },
                'b' => Code{ content: 0b00, len: 2 },
                'c' => Code{ content: 0b01, len: 2 }
               ));
        assert_eq!(huffman.reversed_codes_for_values(), hashmap!(
                'a' => Code{ content: 0b1, len: 1 },
                'b' => Code{ content: 0b00, len: 2 },
                'c' => Code{ content: 0b10, len: 2 }
               ));
        let mut decoder_for_a = huffman.decoder();
        assert_eq!(decoder_for_a.consume(1), DecodingResult::Value(&'a'));
        let mut decoder_for_b = huffman.decoder();
        assert_eq!(decoder_for_b.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_b.consume(0), DecodingResult::Value(&'b'));
        let mut decoder_for_c = huffman.decoder();
        assert_eq!(decoder_for_c.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_c.consume(1), DecodingResult::Value(&'c'));
        assert_eq!(huffman.codes().len(), 3);
        assert_eq!(huffman.levels().len(), 2);
        assert_eq!(huffman.levels().map(|(v, _, _)| v.len()).collect::<Vec<_>>(), &[1, 2]);
        test_read_write(&huffman);
    }

    #[test]
    fn coding_3sym_2bits() {
        //  /|\
        //  abc
        let huffman = Coding::from_frequencies(BitsPerFragment(2),
                                               hashmap!('a' => 100u32, 'b' => 50, 'c' => 10));
        assert_eq!(huffman.total_fragments_count(), 3);
        assert_eq!(huffman.values.as_ref(), ['a', 'b', 'c']);
        assert_eq!(huffman.internal_nodes_count.as_ref(), [0]);
        assert_eq!(huffman.codes_for_values(), hashmap!(
                'a' => Code{ content: 0, len: 1 },
                'b' => Code{ content: 1, len: 1 },
                'c' => Code{ content: 2, len: 1 }
               ));
        assert_eq!(huffman.reversed_codes_for_values(), hashmap!(
                'a' => Code{ content: 0, len: 1 },
                'b' => Code{ content: 1, len: 1 },
                'c' => Code{ content: 2, len: 1 }
        ));
        let mut decoder_for_a = huffman.decoder();
        assert_eq!(decoder_for_a.consume(0), DecodingResult::Value(&'a'));
        let mut decoder_for_b = huffman.decoder();
        assert_eq!(decoder_for_b.consume(1), DecodingResult::Value(&'b'));
        let mut decoder_for_c = huffman.decoder();
        assert_eq!(decoder_for_c.consume(2), DecodingResult::Value(&'c'));
        let mut decoder_for_invalid = huffman.decoder();
        assert_eq!(decoder_for_invalid.consume(3), DecodingResult::Invalid);
        assert_eq!(huffman.codes().len(), 3);
        assert_eq!(huffman.levels().len(), 1);
        assert_eq!(huffman.levels().map(|(v, _, _)| v.len()).collect::<Vec<_>>(), &[3]);
        test_read_write(&huffman);
    }

    #[test]
    fn coding_6sym_1bit() {
        //     /   \
        //   /  \  /\
        //  / \ d  ef
        // /\ a
        // bc
        let frequencies = hashmap!('d' => 12u32, 'e' => 11, 'f' => 10, 'a' => 3, 'b' => 2, 'c' => 1);
        let huffman = Coding::from_frequencies(BitsPerFragment(1), frequencies);
        assert_eq!(huffman.total_fragments_count(), 17);
        assert_eq!(huffman.values.as_ref(), ['d', 'e', 'f', 'a', 'b', 'c']);
        assert_eq!(huffman.internal_nodes_count.as_ref(), [2, 1, 1, 0]);
        assert_eq!(huffman.codes_for_values(), hashmap!(
                'a' => Code{content: 0b001, len: 3 },
                'b' => Code{content: 0b0000, len: 4 },
                'c' => Code{content: 0b0001, len: 4 },
                'd' => Code{content: 0b01, len: 2 },
                'e' => Code{content: 0b10, len: 2 },
                'f' => Code{content: 0b11, len: 2 }
               ));
        assert_eq!(huffman.reversed_codes_for_values(), hashmap!(
                'a' => Code{content: 0b100, len: 3 },
                'b' => Code{content: 0b0000, len: 4 },
                'c' => Code{content: 0b1000, len: 4 },
                'd' => Code{content: 0b10, len: 2 },
                'e' => Code{content: 0b01, len: 2 },
                'f' => Code{content: 0b11, len: 2 }
               ));
        let mut decoder_for_a = huffman.decoder();
        assert_eq!(decoder_for_a.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_a.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_a.consume(1), DecodingResult::Value(&'a'));
        let mut decoder_for_b = huffman.decoder();
        assert_eq!(decoder_for_b.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_b.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_b.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_b.consume(0), DecodingResult::Value(&'b'));
        let mut decoder_for_c = huffman.decoder();
        assert_eq!(decoder_for_c.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_c.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_c.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_c.consume(1), DecodingResult::Value(&'c'));
        let mut decoder_for_d = huffman.decoder();
        assert_eq!(decoder_for_d.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_d.consume(1), DecodingResult::Value(&'d'));
        let mut decoder_for_e = huffman.decoder();
        assert_eq!(decoder_for_e.consume(1), DecodingResult::Incomplete);
        assert_eq!(decoder_for_e.consume(0), DecodingResult::Value(&'e'));
        let mut decoder_for_f = huffman.decoder();
        assert_eq!(decoder_for_f.consume(1), DecodingResult::Incomplete);
        assert_eq!(decoder_for_f.consume(1), DecodingResult::Value(&'f'));
        assert_eq!(huffman.codes().len(), 6);
        assert_eq!(huffman.levels().len(), 4);
        assert_eq!(huffman.levels().map(|(v, _, _)| v.len()).collect::<Vec<_>>(), &[0, 3, 1, 2]);
        test_read_write(&huffman);
    }

    #[test]
    fn coding_6sym_2bits() {
        //  /   |  \  \
        // /\\  d  e  f
        // abc 12 11 10
        // 321
        let frequencies = hashmap!('d' => 12u32, 'e' => 11, 'f' => 10, 'a' => 3, 'b' => 2, 'c' => 1);
        let huffman = Coding::from_frequencies(BitsPerFragment(2), frequencies);
        assert_eq!(huffman.total_fragments_count(), 9);
        assert_eq!(huffman.values.as_ref(), ['d', 'e', 'f', 'a', 'b', 'c']);
        assert_eq!(huffman.internal_nodes_count.as_ref(), [1, 0]);
        assert_eq!(huffman.codes_for_values(), hashmap!(
                'a' => Code{content: 0b00_00, len: 2 },
                'b' => Code{content: 0b00_01, len: 2 },
                'c' => Code{content: 0b00_10, len: 2 },
                'd' => Code{content: 0b01, len: 1 },
                'e' => Code{content: 0b10, len: 1 },
                'f' => Code{content: 0b11, len: 1 }
               ));
        assert_eq!(huffman.reversed_codes_for_values(), hashmap!(
                'a' => Code{content: 0b00_00, len: 2 },
                'b' => Code{content: 0b01_00, len: 2 },
                'c' => Code{content: 0b10_00, len: 2 },
                'd' => Code{content: 0b01, len: 1 },
                'e' => Code{content: 0b10, len: 1 },
                'f' => Code{content: 0b11, len: 1 }
               ));
        let mut decoder_for_a = huffman.decoder();
        assert_eq!(decoder_for_a.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_a.consume(0), DecodingResult::Value(&'a'));
        let mut decoder_for_b = huffman.decoder();
        assert_eq!(decoder_for_b.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_b.consume(1), DecodingResult::Value(&'b'));
        let mut decoder_for_c = huffman.decoder();
        assert_eq!(decoder_for_c.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_c.consume(2), DecodingResult::Value(&'c'));
        let mut decoder_for_invalid = huffman.decoder();
        assert_eq!(decoder_for_invalid.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_invalid.consume(3), DecodingResult::Invalid);
        let mut decoder_for_d = huffman.decoder();
        assert_eq!(decoder_for_d.consume(1), DecodingResult::Value(&'d'));
        let mut decoder_for_e = huffman.decoder();
        assert_eq!(decoder_for_e.consume(2), DecodingResult::Value(&'e'));
        let mut decoder_for_f = huffman.decoder();
        assert_eq!(decoder_for_f.consume(3), DecodingResult::Value(&'f'));
        assert_eq!(huffman.codes().len(), 6);
        assert_eq!(huffman.levels().len(), 2);
        assert_eq!(huffman.levels().map(|(v, _, _)| v.len()).collect::<Vec<_>>(), &[3, 3]);
        test_read_write(&huffman);
    }

    #[test]
    fn coding_5sym_tree_degree3() {
        //  /   |  \
        // /\\  d  e
        // abc 12 11
        // 321
        let frequencies = hashmap!('d' => 12u32, 'e' => 11, 'a' => 3, 'b' => 2, 'c' => 1);
        let huffman = Coding::from_frequencies(Degree(3), frequencies);
        assert_eq!(huffman.total_fragments_count(), 8);
        assert_eq!(huffman.values.as_ref(), ['d', 'e', 'a', 'b', 'c']);
        assert_eq!(huffman.internal_nodes_count.as_ref(), [1, 0]);
        assert_eq!(huffman.codes_for_values(), hashmap!(
                'a' => Code{content: 0+0, len: 2 },
                'b' => Code{content: 0+1, len: 2 },
                'c' => Code{content: 0+2, len: 2 },
                'd' => Code{content: 1, len: 1 },
                'e' => Code{content: 2, len: 1 }
               ));
        assert_eq!(huffman.reversed_codes_for_values(), hashmap!(
                'a' => Code{content: 0+0, len: 2 },
                'b' => Code{content: 0+3*1, len: 2 },
                'c' => Code{content: 0+3*2, len: 2 },
                'd' => Code{content: 1, len: 1 },
                'e' => Code{content: 2, len: 1 }
               ));
        let mut decoder_for_a = huffman.decoder();
        assert_eq!(decoder_for_a.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_a.consume(0), DecodingResult::Value(&'a'));
        let mut decoder_for_b = huffman.decoder();
        assert_eq!(decoder_for_b.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_b.consume(1), DecodingResult::Value(&'b'));
        let mut decoder_for_c = huffman.decoder();
        assert_eq!(decoder_for_c.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_c.consume(2), DecodingResult::Value(&'c'));
        let mut decoder_for_invalid = huffman.decoder();
        assert_eq!(decoder_for_invalid.consume(0), DecodingResult::Incomplete);
        assert_eq!(decoder_for_invalid.consume(3), DecodingResult::Invalid);
        let mut decoder_for_d = huffman.decoder();
        assert_eq!(decoder_for_d.consume(1), DecodingResult::Value(&'d'));
        let mut decoder_for_e = huffman.decoder();
        assert_eq!(decoder_for_e.consume(2), DecodingResult::Value(&'e'));
        assert_eq!(huffman.codes().len(), 5);
        assert_eq!(huffman.levels().len(), 2);
        assert_eq!(huffman.levels().map(|(v, _, _)| v.len()).collect::<Vec<_>>(), &[2, 3]);
        test_read_write(&huffman);
    }
}
