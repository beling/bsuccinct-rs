#![doc = include_str!("../README.md")]

mod elias_fano;

use clap::{Parser, Subcommand};

#[allow(non_camel_case_types)]
//#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[derive(Subcommand)]
pub enum Structure {
    /// Elias-Fano
    EliasFano,
    /// Non-compressed bit vector from bitm library
    BitVec,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
/// Compact sequences benchmark.
pub struct Conf {
    /// Structure to test
    #[command(subcommand)]
    pub structure: Structure,

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
    match conf.structure {
        Structure::EliasFano => elias_fano::benchmark(&conf),
        Structure::BitVec => todo!(),
    }
}