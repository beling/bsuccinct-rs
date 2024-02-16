use std::{hint::black_box, mem::size_of};
use bit_vec::BitVec;
use butils::UnitPrefix;
use huffman_compress::{CodeBuilder, Tree};
use minimum_redundancy::Frequencies;

use crate::{compare_texts, minimum_redundancy::{frequencies, frequencies_u8}};

/*#[inline(always)] fn total_size_bits_u8(frequencies: &[u32; 256], book: &huffman_compress::Book::<u8>) -> usize {
    frequencies.frequencies().fold(0usize, |acc, (k, w)|
        acc + book.get(&k).unwrap().len() as usize * w as usize
    )
}*/

#[inline(always)] fn build_coder_u8(frequencies: &[usize; 256]) -> (huffman_compress::Book<u8>, huffman_compress::Tree<u8>) {
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
    //println!("Measuring huffman_compress performance:");
    println!("### huffman_compress ###");

    let text = conf.text();
    let frequencies= frequencies(conf, &text);
    println!(" Decoder + encoder construction time (generic method): {:.0} ns", conf.measure(||
        CodeBuilder::from_iter(frequencies.iter()).finish()
    ).as_nanos());
    drop(frequencies);

    let frequencies= frequencies_u8(conf, &text);
    println!(" Decoder + encoder construction time (u8 specific method): {:.0} ns", conf.measure(|| build_coder_u8(&frequencies)).as_nanos());
    let (book, tree) = build_coder_u8(&frequencies);

    println!(" Decoder size (lower estimate): {} bytes",
        (2*size_of::<usize>() + size_of::<Option<usize>>()) * (2*frequencies.number_of_occurring_values() - 1) + size_of::<Tree<u8>>()
    );  // on heap it allocates: (2 usizes + Option<usize>) per node of Huffman tree + maybe some paddings (uncounted)

    println!(" Encoding:");
    conf.print_speed("  without adding to bit vector", conf.measure(|| {
        for k in text.iter() { black_box(book.get(k)); }
    }));
    conf.print_speed("  + adding to bit vector", conf.measure(|| encode(&text, &book)));
    let compressed_text = encode(&text, &book);
    conf.print_compressed_size(compressed_text.len());

    conf.print_speed(" Decoding (without storing)", conf.measure(|| {
        for sym in tree.unbounded_decoder(compressed_text.iter()) { black_box(sym); };
    }));

    if conf.verify {
        print!(" Verification... ");
        let mut decoded_text = Vec::with_capacity(text.len());
        for sym in tree.unbounded_decoder(compressed_text.iter()) {
            decoded_text.push(sym);
        };
        compare_texts(&text, &decoded_text);
    }
}



