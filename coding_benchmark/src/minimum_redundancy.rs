use std::hint::black_box;

use bitm::{BitAccess, BitVec};
use butils::UnitPrefix;
use dyn_size_of::GetSize;
use minimum_redundancy::{BitsPerFragment, Coding};
use minimum_redundancy::Frequencies;

use crate::compare_texts;

#[inline(always)] fn total_size_bits_u8(frequencies: &mut [u32; 256], book: &[minimum_redundancy::Code; 256]) -> usize {
    frequencies.drain_frequencies().fold(0usize, |acc, (k, w)|
        acc + book[k as usize].len as usize * w as usize
    )
}

#[inline(always)] fn compress_u8(text: &Box<[u8]>, book: &[minimum_redundancy::Code; 256], total_size_bits: usize) -> Box<[u64]> {
    let mut compressed_text = Box::<[u64]>::with_zeroed_bits(total_size_bits);
    let mut bit_index = 0usize;
    for k in text.iter() {
        let c = book[*k as usize];
        compressed_text.init_bits(bit_index, c.content as u64, c.len.min(32) as u8);
        bit_index += c.len as usize;
    }
    assert_eq!(bit_index, total_size_bits);
    compressed_text
}

pub fn benchmark(conf: &super::Conf) {
    let text = conf.text();

    println!("Counting symbol occurrences [ns]: {:.0}", conf.measure(||
        <[u32; 256]>::with_occurrences_of(text.iter())
    ).as_nanos());
    let mut frequencies= <[u32; 256]>::with_occurrences_of(text.iter());

    let dec_const_ns = conf.measure(|| Coding::from_frequencies(BitsPerFragment(1), frequencies)).as_nanos();
    let coding = Coding::from_frequencies(BitsPerFragment(1), frequencies);
    let enc_constr_ns = conf.measure(|| coding.reversed_codes_for_values_array()).as_nanos();

    println!("Decoder + encoder construction time [ns]: {:.0} + {:.0} = {:.0}", dec_const_ns, enc_constr_ns, dec_const_ns+enc_constr_ns);
    println!("Decoder size [bytes]: {}", coding.size_bytes());

    let book = coding.reversed_codes_for_values_array();

    println!("Encoding time [ns]: {:.0}", conf.measure(|| {
        compress_u8(&text, &book, total_size_bits_u8(&mut frequencies, &book))
    }).as_nanos());
    let total_size_bits = total_size_bits_u8(&mut frequencies, &book);
    let compressed_text = compress_u8(&text, &book, total_size_bits);

    println!("Decoding time [ns]: {:.0}", conf.measure(|| {
        let mut bits = (0..total_size_bits).map(|i| unsafe{compressed_text.get_bit_unchecked(i)});
        let mut d = coding.decoder();
        while let Some(b) = bits.next() {
            if let minimum_redundancy::DecodingResult::Value(v) = d.consume(b as u32) {
                black_box(v);
                d.reset();
            }
        }
    }).as_nanos());

    if conf.verify {
        print!("Verification... ");
        let mut decoded_text = Vec::with_capacity(text.len());
        let mut bits = (0..total_size_bits).map(|i| unsafe{compressed_text.get_bit_unchecked(i)});
        let mut d = coding.decoder();
        while let Some(b) = bits.next() {
            if let minimum_redundancy::DecodingResult::Value(v) = d.consume(b as u32) {
                decoded_text.push(*v);
                d.reset();
            }
        }
        compare_texts(&text, &decoded_text);
    }
}