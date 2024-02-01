use std::{collections::HashMap, hint::black_box};
use bit_vec::BitVec;
use butils::UnitPrefix;
use huffman_compress::CodeBuilder;
use minimum_redundancy::Frequencies;

use crate::compare_texts;

/*#[inline(always)] fn total_size_bits_u8(frequencies: &[u32; 256], book: &huffman_compress::Book::<u8>) -> usize {
    frequencies.frequencies().fold(0usize, |acc, (k, w)|
        acc + book.get(&k).unwrap().len() as usize * w as usize
    )
}*/

#[inline(always)] fn build_coder_u8(frequencies: &[u32; 256]) -> (huffman_compress::Book<u8>, huffman_compress::Tree<u8>) {
    let mut c = CodeBuilder::with_capacity(frequencies.number_of_occurring_values());
    for (k, w) in frequencies.frequencies() { c.push(k, w) }
    c.finish()
    // above is a bit faster than:
    //CodeBuilder::from_iter(weights.frequencies()).finish()
}

#[inline(always)] fn encode(text: &Box<[u8]>, book: &huffman_compress::Book<u8>) -> BitVec {
    //let mut compressed_text = BitVec::with_capacity(total_size_bits_u8(&mut frequencies, &book)); //slower
    let mut compressed_text = BitVec::new();
    for k in text.iter() {
        book.encode(&mut compressed_text, k).unwrap();
    }
    compressed_text
}

pub fn benchmark(conf: &super::Conf) {
    let text = conf.text();
    
    conf.print_speed("Counting symbol occurrences (generic method)", conf.measure(||
        HashMap::<u8, u32>::with_occurrences_of(text.iter())
    ));
    let frequencies= HashMap::<u8, u32>::with_occurrences_of(text.iter());
    println!("Decoder + encoder construction time (generic method): {:.0} ns", conf.measure(||
        CodeBuilder::from_iter(frequencies.iter()).finish()
    ).as_nanos());
    drop(frequencies);

    conf.print_speed("Counting symbol occurrences (u8 specific method)", conf.measure(||
        <[u32; 256]>::with_occurrences_of(text.iter())
    ));
    let frequencies= <[u32; 256]>::with_occurrences_of(text.iter());
    println!("Decoder + encoder construction time (u8 specific method): {:.0} ns", conf.measure(|| build_coder_u8(&frequencies)).as_nanos());
    let (book, tree) = build_coder_u8(&frequencies);

    println!("Approximate decoder size: {} bytes", 24 * (frequencies.number_of_occurring_values() - 1) + 32);

    conf.print_speed("Encoding without adding to bit vector", conf.measure(|| {
        for k in text.iter() { black_box(book.get(k)); }
    }));
    conf.print_speed("Encoding + adding to bit vector", conf.measure(|| encode(&text, &book)));
    let compressed_text = encode(&text, &book);

    conf.print_speed("Decoding", conf.measure(|| {
        for sym in tree.unbounded_decoder(compressed_text.iter()) { black_box(sym); };
    }));

    if conf.verify {
        print!("Verification... ");
        let mut decoded_text = Vec::with_capacity(text.len());
        for sym in tree.unbounded_decoder(compressed_text.iter()) {
            decoded_text.push(sym);
        };
        compare_texts(&text, &decoded_text);
    }
}



