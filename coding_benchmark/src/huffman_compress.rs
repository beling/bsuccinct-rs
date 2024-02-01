use std::hint::black_box;
use bit_vec::BitVec;
use butils::UnitPrefix;
use huffman_compress::CodeBuilder;
use minimum_redundancy::Frequencies;

use crate::compare_texts;

/*#[inline(always)] fn total_size_bits_u8(frequencies: &mut [u32; 256], book: &huffman_compress::Book::<u8>) -> usize {
    frequencies.drain_frequencies().fold(0usize, |acc, (k, w)|
        acc + book.get(&k).unwrap().len() as usize * w as usize
    )
}*/

#[inline(always)] fn build_coder(frequencies: &[u32; 256]) -> (huffman_compress::Book<u8>, huffman_compress::Tree<u8>) {
    let mut c = CodeBuilder::with_capacity(frequencies.len());
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
    
    conf.print_speed("Counting symbol occurrences", conf.measure(||
        <[u32; 256]>::with_occurrences_of(text.iter())
    ));
    let mut frequencies= <[u32; 256]>::with_occurrences_of(text.iter());

    println!("Decoder + encoder construction time [ns]: {:.0}", conf.measure(|| build_coder(&mut frequencies)).as_nanos());
    let (book, tree) = build_coder(&mut frequencies);

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



