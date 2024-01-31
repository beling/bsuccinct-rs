use std::hint::black_box;
use std::iter::FromIterator;
use bit_vec::BitVec;
use butils::UnitPrefix;
use huffman_compress::CodeBuilder;
use minimum_redundancy::Frequencies;

use crate::compare_texts;

pub fn benchmark(conf: &super::Conf) {
    let text = conf.rand_text();
    
    println!("Counting symbol occurrences [ns]: {:.0}", conf.measure(||
        <[u32; 256]>::with_occurrences_of(text.iter())
    ).as_nanos());
    let mut weights= <[u32; 256]>::with_occurrences_of(text.iter());

    println!("Decoder + encoder construction time [ns]: {:.0}", conf.measure(||
        CodeBuilder::from_iter(weights.drain_frequencies()).finish()).as_nanos()
    );

    // Construct a Huffman code based on the weights (e.g. counts or relative frequencies).
    let (book, tree) = CodeBuilder::from_iter(weights.drain_frequencies()).finish();
    println!("Approximate decoder size [bytes]: {}", 24 * (weights.number_of_occurring_values() - 1) + 32);

    // Encode text using the book.
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