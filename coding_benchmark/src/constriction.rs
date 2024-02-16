use std::{hint::black_box, mem::size_of};

use butils::UnitPrefix;
use constriction::{
    backends::Cursor, symbol::{
        huffman::{DecoderHuffmanTree, EncoderHuffmanTree},
        DefaultQueueEncoder, DefaultStackCoder, EncoderCodebook, QueueDecoder, ReadBitStream, WriteBitStream
    }, UnwrapInfallible};

use crate::{compare_texts, minimum_redundancy::frequencies_u8};

#[inline(always)] fn encode_prefix(text: &Box<[u8]>, encoder_codebook: &EncoderHuffmanTree) -> Vec<u32> {
    let mut encoder = DefaultQueueEncoder::new();
    encoder.encode_iid_symbols(text.iter().map(|v| *v as usize), encoder_codebook).unwrap();
    encoder.into_compressed().unwrap_infallible()
}

#[inline(always)] fn encode_suffix(text: &Box<[u8]>, encoder_codebook: &EncoderHuffmanTree) -> DefaultStackCoder {
    let mut encoder = DefaultStackCoder::new();
    encoder.encode_iid_symbols_reverse(text.iter().map(|v| *v as usize), encoder_codebook).unwrap();
    encoder
}

pub fn benchmark(conf: &super::Conf) {
    //println!("Measuring constriction performance:");
    println!("### constriction ###");

    let text = conf.text();
    let frequencies= frequencies_u8(conf, &text);

    let dec_constr_ns = conf.measure(|| DecoderHuffmanTree::from_probabilities::<usize, _>(&frequencies)).as_nanos();
    let enc_constr_ns = conf.measure(|| EncoderHuffmanTree::from_probabilities::<usize, _>(&frequencies)).as_nanos();
    println!(" Decoder + encoder construction time [ns]: {:.0} + {:.0} = {:.0}", dec_constr_ns, enc_constr_ns, dec_constr_ns+enc_constr_ns);

    let encoder_codebook = EncoderHuffmanTree::from_probabilities::<usize, _>(&frequencies);
    let decoder_codebook = DecoderHuffmanTree::from_probabilities::<usize, _>(&frequencies);
    println!(" Decoder size (lower estimate): {} bytes",
        (decoder_codebook.num_symbols()-1) * size_of::<[usize; 2]>() + size_of::<DecoderHuffmanTree>()
    );

    println!("Encoding:");
    conf.print_speed("  without adding to bit vector (prefix order)", conf.measure(|| { // (symbol prefix order)
        for k in text.iter() {
            let _ = encoder_codebook.encode_symbol_prefix(*k as usize, |bit| {
                black_box(bit); Result::<(), ()>::Ok(())
            });
        }
    }));
    // This order is different from other methods:
    conf.print_speed("  without adding to bit vector (suffix order)", conf.measure(|| {
        for k in text.iter() {
            let _ = encoder_codebook.encode_symbol_suffix(*k as usize, |bit| {
                black_box(bit); Result::<(), ()>::Ok(())
            });
        }
    }));
    conf.print_speed("  + adding to bit vector (prefix order)", conf.measure(|| encode_prefix(&text, &encoder_codebook)));
    conf.print_speed("  + adding to bit vector (suffix order)", conf.measure(|| encode_suffix(&text, &encoder_codebook)));

    println!("Decoding:");
    let cursor = Cursor::new_at_write_beginning(encode_prefix(&text, &encoder_codebook));
    conf.print_speed("  from a queue (prefix order) (without storing)", conf.measure(|| {
        let mut decoder = QueueDecoder::from_compressed(cursor.as_view());
        for sym in decoder.decode_iid_symbols(text.len(), &decoder_codebook) {
            let _ = black_box(sym);
        }
    }));

    if conf.verify {
        print!("  verifying decoding from a queue... ");
        let mut decoder = QueueDecoder::from_compressed(cursor.as_view());
        let reconstructed: Vec<u8> = decoder
            .decode_iid_symbols(text.len(), &decoder_codebook)
            .map(|sym| sym.unwrap() as u8)
            .collect();
        compare_texts(&text, &reconstructed);
        assert!(decoder.maybe_exhausted());
    }
    drop(cursor);

    let mut encoder = encode_suffix(&text, &encoder_codebook);
    conf.print_speed("  from a stack (prefix order) (without storing)", conf.measure(|| {
        let mut decoder = encoder.as_decoder();
        for sym in decoder.decode_iid_symbols(text.len(), &decoder_codebook) {
            let _ = black_box(sym);
        }
    }));

    if conf.verify {
        print!("  verifying decoding from a stack... ");
        //let mut decoder = StackCoder::from_compressed(cursor.as_mut_view()).unwrap();
        let reconstructed: Vec<u8> = encoder/*.into_decoder()*/
            .decode_iid_symbols(text.len(), &decoder_codebook)
            .map(|sym| sym.unwrap() as u8)
            .collect();
        compare_texts(&text, &reconstructed);
    }
}

