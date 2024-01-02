#![doc = include_str!("../README.md")]

use std::hint::black_box;

use clap::Parser;
use cpu_time::ThreadTime;
use cseq::elias_fano;
use dyn_size_of::GetSize;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
/// Compact sequences benchmark.
pub struct Conf {
    /// The number of items to use
    #[arg(short = 'n', long, default_value_t = 1_000_000)]
    pub num: usize,

    /// Item universe.
    #[arg(short = 'u', long, default_value_t = 1024*1024*1024)]
    pub universe: usize,

    /// Whether to check the validity of built sequence
    #[arg(short='v', long, default_value_t = true)]
    pub verify: bool,
}

fn main() {
    let conf: Conf = Conf::parse();

    let data: Vec<u64> = (1..=conf.num).map(|i| (i*conf.universe/conf.num) as u64).collect();
    println!("data contains {} items in the range [0, {})", data.len(), data.last().unwrap()+1);

    let start_moment = ThreadTime::now();
    //let ef: elias_fano::Sequence<bitm::BinaryRankSearch, bitm::BinaryRankSearch> = elias_fano::Sequence::with_items_from_slice_s(&data);
    let ef = elias_fano::Sequence::with_items_from_slice(&data);
    let build_time_seconds = start_moment.elapsed().as_secs_f64();
    println!("size [bits/item]: {:.2}, construction time [μs]: {:.0}", 8.0*ef.size_bytes_dyn() as f64/data.len() as f64, build_time_seconds*1_000_000.0);

    let start_moment = ThreadTime::now();
    for index in 0..data.len() {
        black_box(ef.get(index));
    }
    let get_time_nanos = start_moment.elapsed().as_nanos();
    print!("time/item to [ns]: get {:.2}", get_time_nanos as f64 / data.len() as f64);

    let start_moment = ThreadTime::now();
    for v in data.iter() {
        black_box(ef.index_of(*v));
    }
    let index_time_nanos = start_moment.elapsed().as_nanos();
    println!(", index {:.2}", index_time_nanos as f64 / data.len() as f64);

    if conf.verify {
        print!("verification: ");
        for (index, v) in data.iter().copied().enumerate() {
            assert_eq!(ef.get(index), Some(v), "wrong value for index {index}");
            assert_eq!(ef.index_of(v), Some(index), "wrong index for value {v}");
        }
        println!("DONE");
    }
}