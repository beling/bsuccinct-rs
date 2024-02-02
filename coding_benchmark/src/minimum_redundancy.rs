use std::collections::HashMap;
use std::hint::black_box;

use bitm::{BitAccess, BitVec};
use butils::UnitPrefix;
use dyn_size_of::GetSize;
use minimum_redundancy::{BitsPerFragment, Code, Coding};
use minimum_redundancy::Frequencies;

use crate::compare_texts;

#[inline(always)] fn total_size_bits_u8(frequencies: &[u32; 256], book: &[minimum_redundancy::Code; 256]) -> usize {
    frequencies.frequencies().fold(0usize, |acc, (k, w)|
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

#[inline(always)] fn total_size_bits(frequencies: &HashMap<u8, u32>, book: &HashMap<u8, Code>) -> usize {
    frequencies.iter().fold(0usize, |acc, (k, w)|
        acc + book[&k].len as usize * *w as usize
    )
}

#[inline(always)] fn compress(text: &Box<[u8]>, book: &HashMap<u8, Code>, total_size_bits: usize) -> Box<[u64]> {
    let mut compressed_text = Box::<[u64]>::with_zeroed_bits(total_size_bits);
    let mut bit_index = 0usize;
    for k in text.iter() {
        let c = book[k];
        compressed_text.init_bits(bit_index, c.content as u64, c.len.min(32) as u8);
        bit_index += c.len as usize;
    }
    assert_eq!(bit_index, total_size_bits);
    compressed_text
}

#[inline(always)] fn decode(coding: &Coding<u8>, compressed_text: &Box<[u64]>, total_size_bits: usize) {
    //let mut bits = (0..total_size_bits).map(|i| unsafe{compressed_text.get_bit_unchecked(i)});
    let mut bits = unsafe{compressed_text.bit_in_unchecked_range_iter(0..total_size_bits)};
    let mut d = coding.decoder();
    while let Some(b) = bits.next() {
        if let minimum_redundancy::DecodingResult::Value(v) = d.consume(b as u32) {
            black_box(v);
            d.reset();
        }
    }
}

fn verify(text: Box<[u8]>, compressed_text: Box<[u64]>, coding: Coding<u8>, total_size_bits: usize) {
    print!("Verification... ");
    let mut decoded_text = Vec::with_capacity(text.len());
    let mut bits = compressed_text.bit_in_range_iter(0..total_size_bits);
    let mut d = coding.decoder();
    while let Some(b) = bits.next() {
        if let minimum_redundancy::DecodingResult::Value(v) = d.consume(b as u32) {
            decoded_text.push(*v);
            d.reset();
        }
    }
    compare_texts(&text, &decoded_text);
}

pub fn benchmark_u8(conf: &super::Conf) {
    let text = conf.text();

    conf.print_speed("Counting symbol occurrences", conf.measure(||
        <[u32; 256]>::with_occurrences_of(text.iter())
    ));
    let frequencies= <[u32; 256]>::with_occurrences_of(text.iter());

    let dec_constr_ns = conf.measure(|| Coding::from_frequencies_cloned(BitsPerFragment(1), &frequencies)).as_nanos();
    let coding = Coding::from_frequencies_cloned(BitsPerFragment(1), &frequencies);
    let enc_constr_ns = conf.measure(|| coding.reversed_codes_for_values_array()).as_nanos();

    println!("Decoder + encoder construction time [ns]: {:.0} + {:.0} = {:.0}", dec_constr_ns, enc_constr_ns, dec_constr_ns+enc_constr_ns);
    println!("Decoder size: {} bytes", coding.size_bytes());

    let book = coding.reversed_codes_for_values_array();

    conf.print_speed("Encoding without adding to bit vector", conf.measure(|| {
        for k in text.iter() { black_box(book[*k as usize]); }
    }));
    conf.print_speed("Encoding + adding to bit vector", conf.measure(|| {
        compress_u8(&text, &book, total_size_bits_u8(&frequencies, &book))
    }));
    let total_size_bits = total_size_bits_u8(&frequencies, &book);
    let compressed_text = compress_u8(&text, &book, total_size_bits);

    conf.print_speed("Decoding", conf.measure(|| decode(&coding, &compressed_text, total_size_bits)));

    if conf.verify { verify(text, compressed_text, coding, total_size_bits); }
}

pub fn benchmark(conf: &super::Conf) {
    let text = conf.text();

    conf.print_speed("Counting symbol occurrences", conf.measure(||
        HashMap::<u8, u32>::with_occurrences_of(text.iter())
    ));
    let mut frequencies= HashMap::<u8, u32>::with_occurrences_of(text.iter());

    let dec_constr_ns = conf.measure(|| Coding::from_frequencies_cloned(BitsPerFragment(1), &frequencies)).as_nanos();
    let coding = Coding::from_frequencies_cloned(BitsPerFragment(1), &frequencies);
    let enc_constr_ns = conf.measure(|| coding.reversed_codes_for_values_array()).as_nanos();

    println!("Decoder + encoder construction time [ns]: {:.0} + {:.0} = {:.0}", dec_constr_ns, enc_constr_ns, dec_constr_ns+enc_constr_ns);
    println!("Decoder size: {} bytes", coding.size_bytes());

    let book = coding.reversed_codes_for_values();

    conf.print_speed("Encoding without adding to bit vector", conf.measure(|| {
        for k in text.iter() { black_box(book.get(k)); }
    }));
    conf.print_speed("Encoding + adding to bit vector", conf.measure(|| 
        compress(&text, &book, total_size_bits(&frequencies, &book))
    ));
    let total_size_bits = total_size_bits(&mut frequencies, &book);
    let compressed_text = compress(&text, &book, total_size_bits);

    conf.print_speed("Decoding", conf.measure(|| decode(&coding, &compressed_text, total_size_bits)));

    if conf.verify { verify(text, compressed_text, coding, total_size_bits); }
}