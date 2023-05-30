use std::hash::Hash;
use minimum_redundancy::{BitsPerFragment, DecodingResult, entropy_to_bpf, Frequencies, ValueSize};
use std::collections::HashMap;
use std::borrow::Borrow;
use std::io::{Read, Write};
use std::io;
use std::convert::TryInto;
use crate::coding::{BuildCoding, Coding, Decoder, SerializableCoding};
use super::U8Code;

impl From<minimum_redundancy::Code> for U8Code {
    fn from(c: minimum_redundancy::Code) -> Self {
        Self { content: c.content.try_into().unwrap(), len: c.len.try_into().unwrap() }
    }
}

impl<'d, DecodedType> Decoder for minimum_redundancy::Decoder<'d, DecodedType, BitsPerFragment> {
    type Value = DecodedType;
    type Decoded = &'d DecodedType;

    #[inline(always)] fn consume_checked(&mut self, fragment: u8) -> DecodingResult<Self::Decoded> {
        Self::consume_checked(self, fragment as _)
    }

    #[inline(always)] fn consume(&mut self, fragment: u8) -> DecodingResult<Self::Decoded> {
        Self::consume(self, fragment as _)
    }
}

impl<Value: Hash + Eq + Clone> Coding for minimum_redundancy::Coding<Value, BitsPerFragment> {
    type Value = Value;
    type Decoder<'d> = minimum_redundancy::Decoder<'d, Value, BitsPerFragment> where Value: 'd;
    type Encoder<'e> = HashMap<&'e Value, U8Code> where Value: 'e;
    type Codeword = U8Code;

    #[inline(always)] fn bits_per_fragment(&self) -> u8 {
        self.degree.0
    }

    #[inline(always)] fn decoder(&self) -> Self::Decoder<'_> {
        Self::decoder(self)
    }

    #[inline(always)] fn encoder(&self) -> Self::Encoder<'_> {
        self.codes().map(|(v, c)| (v, c.into())).collect()
    }

    #[inline(always)] fn len_of(&self, code: U8Code) -> u8 { code.len }

    fn fragment_of(&self, code: Self::Codeword, index: u8) -> u8 {
        self.rev_fragment_of(code, code.len-index-1)
    }

    fn rev_fragment_of(&self, code: Self::Codeword, index: u8) -> u8 {
        let bpf = self.bits_per_fragment();
        code.content.checked_shr(bpf as u32 * index as u32).map_or(0, |v| v & ((1u16 << bpf) - 1) as u8)
    }

    #[inline(always)] fn remove_first_fragment_of(&self, code: &mut U8Code) -> bool {
        code.len -= 1;
        code.len == 0
    }

    fn code_of<'e, Q>(&self, encoder: &Self::Encoder<'e>, to_encode: &Q) -> Self::Codeword where Q: Borrow<Self::Value> {
        encoder[to_encode.borrow()]
    }
}

impl<Value: Hash + Eq + Clone> SerializableCoding for minimum_redundancy::Coding<Value, BitsPerFragment> {
    fn write_bytes(&self, bytes_per_value: usize) -> usize {
        minimum_redundancy::Coding::<Value, BitsPerFragment>::write_size_bytes(self, ValueSize::Const(bytes_per_value))
    }

    fn write<F>(&self, output: &mut dyn Write, write_value: F) -> io::Result<()> where F: FnMut(&mut dyn Write, &Self::Value) -> io::Result<()> {
        minimum_redundancy::Coding::<Value, BitsPerFragment>::write(self, output, write_value)
    }

    fn read<F>(input: &mut dyn Read, read_value: F) -> io::Result<Self> where F: FnMut(&mut dyn Read) -> io::Result<Self::Value>, Self: Sized {
        minimum_redundancy::Coding::read(input, read_value)
    }
}

#[derive(Default, Copy, Clone)]
pub struct BuildMinimumRedundancy {
    pub bits_per_fragment: u8
}

impl<Value: Hash + Eq + Clone> BuildCoding<Value> for BuildMinimumRedundancy {
    type Coding = minimum_redundancy::Coding<Value>;

    fn name(&self) -> String {
        return if self.bits_per_fragment == 0 {
            "minimum_redundancy".to_owned()
        } else {
            format!("minimum_redundancy_b{}", self.bits_per_fragment)
        }
    }

    fn build_from_iter<Iter>(&self, iter: Iter, mut bits_per_fragment: u8) -> Self::Coding
        where Iter: IntoIterator, Iter::Item: Borrow<<Self::Coding as Coding>::Value>
    {
        if bits_per_fragment == 0 { bits_per_fragment = self.bits_per_fragment; }
        let freq = HashMap::<Value, u32>::with_counted_all(iter);
        if bits_per_fragment == 0 { bits_per_fragment = entropy_to_bpf(freq.entropy()-0.2) }
        Self::Coding::from_frequencies(BitsPerFragment(bits_per_fragment), freq)
    }
}
