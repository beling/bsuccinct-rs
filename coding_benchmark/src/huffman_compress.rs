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

pub fn benchmark(conf: &super::Conf) {
    let text = conf.rand_text();
    
    println!("Counting symbol occurrences [ns]: {:.0}", conf.measure(||
        <[u32; 256]>::with_occurrences_of(text.iter())
    ).as_nanos());
    let mut frequencies= <[u32; 256]>::with_occurrences_of(text.iter());

    println!("Decoder + encoder construction time [ns]: {:.0}", conf.measure(|| {
        let mut c = CodeBuilder::with_capacity(frequencies.len());
        for (k, w) in frequencies.drain_frequencies() { c.push(k, w) }
        c.finish()
        // above is a bit faster than:
        //CodeBuilder::from_iter(weights.drain_frequencies()).finish()
    }).as_nanos());

    // Construct a Huffman code based on the weights (e.g. counts or relative frequencies).
    let mut c = CodeBuilder::with_capacity(frequencies.len());
    for (k, w) in frequencies.drain_frequencies() { c.push(k, w) }
    let (book, tree) = c.finish();
    //let (book, tree) = CodeBuilder::from_iter(weights.drain_frequencies()).finish();

    println!("Approximate decoder size [bytes]: {}", 24 * (frequencies.number_of_occurring_values() - 1) + 32);

    println!("Encoding time [ns]: {:.0}", conf.measure(|| {
        //let mut compressed_text = BitVec::with_capacity(total_size_bits_u8(&mut frequencies, &book)); //slower
        let mut compressed_text = BitVec::new();
        for k in text.iter() {
            book.encode(&mut compressed_text, k).unwrap();
        }
        compressed_text
    }).as_nanos());

    //let mut compressed_text = BitVec::with_capacity(total_size_bits_u8(&mut frequencies, &book));
    let mut compressed_text = BitVec::new();
    for k in text.iter() {
        book.encode(&mut compressed_text, k).unwrap();
    }

    // Decode the symbols using the tree.
    println!("Decoding time [ns]: {:.0}", conf.measure(|| {
        for sym in tree.unbounded_decoder(compressed_text.iter()) { black_box(sym); };
    }).as_nanos());

    if conf.verify {
        print!("Verification... ");
        let mut decoded_text = Vec::with_capacity(text.len());
        for sym in tree.unbounded_decoder(compressed_text.iter()) {
            decoded_text.push(sym);
        };
        compare_texts(&text, &decoded_text);
    }
}

/*pub fn benchmark(conf: &super::Conf) {
    let weights = conf.frequencies();

    println!("Encoding time per symbol kind [ns]: {:.2}", conf.measure(||
        CodeBuilder::from_iter(&weights).finish()).to_nanos() / weights.len() as f64
    );

    // Construct a Huffman code based on the weights (e.g. counts or relative frequencies).
    let (book, tree) = CodeBuilder::from_iter(&weights).finish();
    println!("Approximate decoder size [bytes]: {}", 24 * (weights.len() - 1) + 32);

    // Encode some symbols using the book.
    let mut compressed_text = BitVec::new();
    let mut total_weight = 0;
    for (k, weight) in weights {
        for _ in 0..weight {
            book.encode(&mut compressed_text, &k).unwrap();
        }
        total_weight += weight;
    }

    // Decode the symbols using the tree.
    println!("Decoding time per symbol in text [ns]: {:.2}", conf.measure(|| {
        for sym in tree.unbounded_decoder(compressed_text.iter()) { black_box(sym); };
    }).to_nanos() / total_weight as f64);
}*/