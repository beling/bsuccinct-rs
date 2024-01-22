#![doc = include_str!("../README.md")]

mod elias_fano;

use clap::Parser;

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

    elias_fano::benchmark(&conf);
}