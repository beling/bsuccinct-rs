//! Encoding of values of any type to/from a sequence of code words of fixed bit length.

use std::io;
pub use minimum_redundancy::DecodingResult;
use std::borrow::Borrow;
use std::iter::FusedIterator;

pub use minimum_redundancy;

mod mr;
pub use mr::*;
mod geom;
pub use geom::*;

#[derive(Default, Copy, Clone)]
pub struct U8Code {
    pub content: u8,
    pub len: u8
}

/// Decoder that decodes a value for codeword given fragment by fragment.
pub trait Decoder {
    /// Type of value.
    type Value;

    /// Type returned by decoder. Usually equals `Self::Value` or `&Self::Value`.
    type Decoded: Borrow<Self::Value>;

    /// Consumes a `fragment` of the code and returns a value if the given `fragment` finishes the valid code.
    /// Returns `DecodingResult::Invalid` if fragments given so far do not constitute a valid code.
    fn consume_checked(&mut self, fragment: u8) -> DecodingResult<Self::Decoded>;

    /// Consumes a `fragment` of the code and returns a value if the given `fragment` finishes the valid code.
    /// The result is undefined if the fragments given so far do not constitute a valid code.
    ///
    /// Default implementation just calls `self.consume_checked(fragment)`.
    #[inline(always)] fn consume(&mut self, fragment: u8) -> DecodingResult<Self::Decoded> {
        self.consume_checked(fragment)
    }
}

/// A bijection between values and codewords.
/// Codewords are sequences of fragments.
/// Each fragment occupies constant number of bits.
pub trait Coding {
    /// Type of value.
    type Value;

    /// Type of decoder. Decoder decodes a value for code given fragment by fragment.
    type Decoder<'d>: Decoder<Value=Self::Value> where Self: 'd;

    /// Type of encoder. Encoder maps values to their codes.
    type Encoder<'e> where Self: 'e;

    /// Type of codeword.
    type Codeword: Copy + Sized + Sync;

    /// Number of bits needed to store codeword fragment.
    fn bits_per_fragment(&self) -> u8;

    /// Maximum value of fragment.
    fn max_fragment_value(&self) -> u8 {
        1u8.checked_shl(self.bits_per_fragment() as u32).map_or(u8::MAX, |v| v-1)
    }

    /// Returns decoder that allows for decoding a value.
    fn decoder(&self) -> Self::Decoder<'_>;

    /// Returns a map from values to their codewords.
    fn encoder(&self) -> Self::Encoder<'_>;

    /// Returns the length of `code` in fragments.
    fn len_of(&self, code: Self::Codeword) -> u8;

    /// Returns `index`-th fragment of `code`.
    fn fragment_of(&self, code: Self::Codeword, index: u8) -> u8;

    /// Returns last `index`-th fragment of `code`.
    #[inline] fn rev_fragment_of(&self, code: Self::Codeword, index: u8) -> u8 {
        self.fragment_of(code, self.len_of(code)-index-1)
    }

    /// Returns iterator over `code` fragments.
    #[inline] fn fragments_of(&self, code: Self::Codeword) -> FragmentsIterator<'_, Self> {
        FragmentsIterator { coding: &self, code }
    }

    /// Returns whether the `code` is empty (has zero fragments).
    #[inline] fn is_code_empty(&self, code: Self::Codeword) -> bool { self.len_of(code) == 0 }

    /// Returns the first fragment of the `code`.
    #[inline] fn first_fragment_of(&self, code: Self::Codeword) -> u8 { self.fragment_of(code, 0) }

    /// Extracts and returns the first fragment of the `code` or [`None`] if the `code` is already empty.
    fn extract_first_fragment_of(&self, code: &mut Self::Codeword) -> Option<u8> {
        (!self.is_code_empty(*code)).then(|| {
            let result = self.first_fragment_of(*code);
            self.remove_first_fragment_of(code);
            result
        })
    }

    /// Removes the first fragment of the `code` and returns whether it is empty now.
    fn remove_first_fragment_of(&self, code: &mut Self::Codeword) -> bool;

    /// Returns code of the value `to_encode`.
    fn code_of<'e, Q>(&self, encoder: &Self::Encoder<'e>, to_encode: &Q) -> Self::Codeword where Q: Borrow<Self::Value>;

    /// Returns the length (number of fragments) of code of the value `to_encode`.
    /// (this is the same value as `code(to_encode).fragments`, but `code_len` is faster for some encoders)
    #[inline(always)] fn len_of_encoded<'e, Q>(&self, encoder: &Self::Encoder<'e>, to_encode: &Q) -> u8 where Q: Borrow<Self::Value> {
        self.len_of(self.code_of(encoder, to_encode))
    }

    /// Returns iterator over `code` fragments.
    #[inline] fn fragments_of_encoded<'e, Q>(&self, encoder: &Self::Encoder<'e>, to_encode: &Q) -> FragmentsIterator<'_, Self>
        where Q: Borrow<Self::Value>
    {
        self.fragments_of(self.code_of(encoder, to_encode))
    }
}

/// Iterator over codeword fragments.
pub struct FragmentsIterator<'c, C: Coding + ?Sized> {
    coding: &'c C,
    code: C::Codeword
}

impl<'c, C: Coding> FusedIterator for FragmentsIterator<'c, C> {}

impl<'c, C: Coding> ExactSizeIterator for FragmentsIterator<'c, C> {
    #[inline] fn len(&self) -> usize {
        self.coding.len_of(self.code) as usize
    }
} 

impl<'c, C: Coding> Iterator for FragmentsIterator<'c, C> {
    type Item = u8;

    #[inline] fn next(&mut self) -> Option<Self::Item> {
        self.coding.extract_first_fragment_of(&mut self.code)
    }

    #[inline] fn size_hint(&self) -> (usize, Option<usize>) {
        let l = self.len(); (l, Some(l))
    }
}

/// Codings that implement `SerializableCoding` can be serialized/deserialized.
pub trait SerializableCoding: Coding {
    /// Returns the number of bytes which `write` will write
    /// assuming that each call to `write_value` writes `bytes_per_value` bytes.
    fn write_bytes(&self, bytes_per_value: usize) -> usize;

    /// Writes `self` to the `output`, using `write_value` to write values.
    fn write<F>(&self, output: &mut dyn io::Write, write_value: F) -> io::Result<()>
        where F: FnMut(&mut dyn io::Write, &Self::Value) -> io::Result<()>;

    /// Reads `self` from the input using `read_value` to read values.
    fn read<F>(input: &mut dyn io::Read, read_value: F) -> io::Result<Self>
        where F: FnMut(&mut dyn io::Read) -> io::Result<Self::Value>, Self: Sized;
}

/// Coding builder.
pub trait BuildCoding<V> {
    /// Type of coding that `self` builds.
    type Coding: Coding<Value=V>;

    /// Returns the name of `self`.
    ///
    /// The name can be used by some cache systems to check
    /// if coding built by `self` is already in cache.
    fn name(&self) -> String;

    /// Build coding that uses given number of `bits_per_fragment` and is optimal for data provided by `iter`.
    /// If `bits_per_fragment` is 0, it is set automatically.
    fn build_from_iter<Iter>(&self, iter: Iter, bits_per_fragment: u8) -> Self::Coding
        where Iter: IntoIterator, Iter::Item: Borrow<<Self::Coding as Coding>::Value>;
}

// Returns `fragment_nr`-th `bits_per_fragment`-bits fragment of `bits`.
/*#[inline(always)] pub fn get_u32_fragment(bits: u32, bits_per_fragment: u8, fragment_nr: u8) -> u32 {
    bits.checked_shr(bits_per_fragment as u32 * fragment_nr as u32).map_or(0, |v| v & ((1u32 << bits_per_fragment) - 1))
}*/
