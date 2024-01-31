use std::mem::size_of_val;
use std::io::{Read, Write};
use std::io;
use minimum_redundancy::{DecodingResult, entropy_to_bpf, Frequencies};
use std::borrow::Borrow;
use std::collections::HashMap;
use dyn_size_of::GetSize;
use crate::coding::{BuildCoding, Coding, Decoder, SerializableCoding};
use super::U8Code;

#[derive(Copy, Clone)]
pub struct GeometricUnlimitedDecoder {
    threshold: u8,
    value: u16
}

impl GeometricUnlimitedDecoder {    // <V: Default>
    pub fn new(threshold: u8) -> Self { Self{ threshold, value: Default::default()  } }
}

impl Decoder for GeometricUnlimitedDecoder //where V: Clone + AddAssign, u16: TryInto<V>
{
    type Value = u16;
    type Decoded = u16;

    #[inline] fn consume_checked(&mut self, fragment: u8) -> DecodingResult<Self::Decoded> {
        //self.value += unsafe { fragment.try_into().unwrap_unchecked() };
        self.value += fragment as u16;
        if fragment == self.threshold {
            DecodingResult::Incomplete
        } else {
            DecodingResult::Value(self.value)
            //DecodingResult::Value(self.value.clone())
        }
    }
}

#[derive(Copy, Clone)]
pub struct GeometricUnlimited //<V = u16>
{
    threshold: u8,
    bits_per_fragment: u8,
    //v_type: PhantomData<V>
}

impl GeometricUnlimited {
    #[inline] pub fn new(bits_per_fragment: u8) -> Self {
        Self { threshold: (1<<bits_per_fragment)-1, bits_per_fragment }
    }
}

impl Coding for GeometricUnlimited
//where V: Clone + Into<u16> /*, u16: TryInto<V>*/
{
    type Value = u16;
    //type Decoder<'d> where V: 'd = GeometricUnlimitedDecoder;
    type Decoder<'d> = GeometricUnlimitedDecoder;
    type Encoder<'e> = ();
    //type Code = u16;
    type Codeword = U8Code;

    #[inline] fn bits_per_fragment(&self) -> u8 {
        self.bits_per_fragment
    }

    #[inline] fn decoder(&self) -> Self::Decoder<'_> {
        GeometricUnlimitedDecoder::new(self.threshold)
    }

    fn encoder(&self) -> Self::Encoder<'_> { () }

    #[inline] fn len_of(&self, code: Self::Codeword) -> u8 {
        //(code / self.threshold) as u8 + 1
        code.len
    }

    fn fragment_of(&self, code: Self::Codeword, index: u8) -> u8 {
        if index+1 == code.len {
            code.content
        } else {
            self.threshold
        }
    }

    fn remove_first_fragment_of(&self, code: &mut Self::Codeword) -> bool {
        code.len -= 1;
        code.len == 0
    }

    fn code_of<'e, Q>(&self, _encoder: &Self::Encoder<'e>, to_encode: &Q) -> Self::Codeword where Q: Borrow<Self::Value> {
        let v = *to_encode.borrow();
        Self::Codeword { content: v as u8 & self.threshold, len: (v >> self.bits_per_fragment) as u8 + 1 }
        //to_encode.borrow()
    }

    /*fn fragment_of(&self, code: Self::Code, index: u8) -> u8 {
        (if index * self.threshold < code {
            self.threshold
        } else {
            code % self.threshold
        }) as u8
    }

    fn remove_first_fragment_of(&self, code: &mut Self::Code) -> bool {
        if *code <= self.threshold {
            false
        } else {
            *code -= self.threshold;
            true
        }
    }

    fn value_of<Q>(&self, _encoder: &Self::Encoder, to_encode: &Q) -> Self::Code where Q: Borrow<Self::Value> {
        to_encode.borrow().clone().into()
    }*/
}

impl SerializableCoding for GeometricUnlimited
    //where V: Clone + Into<u16>
    //Default + Clone + AddAssign + Into<u32>, u32: TryInto<V>
{
    fn write_bytes(&self, _bytes_per_value: usize) -> usize {
        size_of_val(&self.bits_per_fragment)
    }

    fn write<F>(&self, output: &mut dyn Write, _write_value: F) -> io::Result<()> where F: FnMut(&mut dyn Write, &Self::Value) -> io::Result<()> {
        output.write_all(std::slice::from_ref(&self.bits_per_fragment)).map(|_| ())
    }

    fn read<F>(input: &mut dyn Read, _read_value: F) -> io::Result<Self> where F: FnMut(&mut dyn Read) -> io::Result<Self::Value>, Self: Sized {
        let mut bits_per_fragment = 0u8;
        input.read_exact(std::slice::from_mut(&mut bits_per_fragment))?;
        Ok(Self::new(bits_per_fragment))
    }
}

impl GetSize for GeometricUnlimited {}

#[derive(Default, Copy, Clone)]
pub struct BuildGeometricUnlimited {
    pub bits_per_fragment: u8
}

impl BuildCoding<u16> for BuildGeometricUnlimited
    //where Value: Default + Clone + AddAssign + Into<u16> + Hash + Eq, u16: TryInto<Value>
{
    //type Coding = GeometricUnlimited<Value>;
    type Coding = GeometricUnlimited;

    fn name(&self) -> String {
        return if self.bits_per_fragment == 0 {
            "geometric_unlimited".to_owned()
        } else {
            format!("geometric_unlimited_b{}", self.bits_per_fragment)
        }
    }

    fn build_from_iter<Iter>(&self, iter: Iter, mut bits_per_fragment: u8) -> Self::Coding
        where Iter: IntoIterator, Iter::Item: Borrow<<Self::Coding as Coding>::Value>
    {
        if bits_per_fragment == 0 { bits_per_fragment = self.bits_per_fragment; }
        if bits_per_fragment == 0 {
            //bits_per_fragment = entropy_to_bpf(HashMap::<Value, u32>::with_counted_all(iter).entropy()-0.2);
            bits_per_fragment = entropy_to_bpf(HashMap::<u16, u32>::with_occurrences_of(iter).entropy()-0.2);
            // old: we use 1 bit less than sound maximum... probably better heuristic exists
            //bits_per_fragment = bits_to_store!(iter.into_iter().map(|v|Into::<u32>::into(v.borrow().clone())).max().unwrap_or(0));
            //if bits_per_fragment > 1 { bits_per_fragment -= 1; }
        }
        Self::Coding::new(bits_per_fragment)
    }
}
