use std::{hint::black_box, mem::size_of};

use butils::UnitPrefix;
use constriction::{
    backends::Cursor, symbol::{huffman::{DecoderHuffmanTree, EncoderHuffmanTree},
        DefaultQueueEncoder, QueueDecoder, ReadBitStream, WriteBitStream},
    UnwrapInfallible};

use crate::{compare_texts, minimum_redundancy::frequencies_u8};

pub fn benchmark(conf: &super::Conf) {
    let text = conf.text();
    let frequencies= frequencies_u8(conf, &text);

    let dec_constr_ns = conf.measure(|| DecoderHuffmanTree::from_probabilities::<u32, _>(&frequencies)).as_nanos();
    let enc_constr_ns = conf.measure(|| EncoderHuffmanTree::from_probabilities::<u32, _>(&frequencies)).as_nanos();
    println!("Decoder + encoder construction time [ns]: {:.0} + {:.0} = {:.0}", dec_constr_ns, enc_constr_ns, dec_constr_ns+enc_constr_ns);

    let encoder_codebook = EncoderHuffmanTree::from_probabilities::<u32, _>(&frequencies);
    let decoder_codebook = DecoderHuffmanTree::from_probabilities::<u32, _>(&frequencies);
    println!("Decoder size (lower estimate): {} bytes",
        (decoder_codebook.num_symbols()-1) * size_of::<[usize; 2]>() + size_of::<DecoderHuffmanTree>()
    );

    conf.print_speed("Encoding + adding to bit vector", conf.measure(|| {
        let mut encoder = DefaultQueueEncoder::new();
        encoder.encode_iid_symbols(text.iter().map(|v| *v as usize), &encoder_codebook).unwrap();
        encoder.into_compressed().unwrap_infallible()
    }));

    let mut encoder = DefaultQueueEncoder::new();
    encoder.encode_iid_symbols(text.iter().map(|v| *v as usize), &encoder_codebook).unwrap();
    let compressed = encoder.into_compressed().unwrap_infallible();
    let cursor = Cursor::new_at_write_beginning(compressed);

    conf.print_speed("Decoding", conf.measure(|| {
        let mut decoder = QueueDecoder::from_compressed(cursor.as_view());
        for sym in decoder.decode_iid_symbols(text.len(), &decoder_codebook) {
            black_box(sym.unwrap());
        }
    }));

    if conf.verify {
        print!("Verification... ");
        let mut decoder = QueueDecoder::from_compressed(cursor.as_view());
        let reconstructed: Vec<u8> = decoder
            .decode_iid_symbols(text.len(), &decoder_codebook)
            .map(|sym| sym.unwrap() as u8)
            .collect();
        compare_texts(&text, &reconstructed);
        assert!(decoder.maybe_exhausted());
    }
}