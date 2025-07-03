#![doc = include_str!("../README.md")]

mod conf;
use crate::conf::{Conf, Method};

mod function;

mod perfect;
use crate::perfect::perfect;

mod phast;
use crate::phast::{phast, phast2};

mod partial;
use crate::partial::partial;

mod optim;

mod benchmark;

use clap::Parser;

use ph::seeds::{Bits8, BitsFast};
use ph::phast::{SeedOnly, SeedOnlyK, ShiftOnlyWrapped};
use rayon::current_num_threads;

fn main() {
    let conf = Conf::parse();
    let threads_num = if conf.multiple_threads { current_num_threads() } else { 1 };
    let bucket_size = conf.bucket_size_100();
    println!("n={} k={} bits/seed={} lambda={:.2} threads={threads_num}", conf.keys_num, conf.k,
        conf.bits_per_seed, bucket_size as f64/100 as f64);
    match (conf.method, conf.k, conf.bits_per_seed, conf.one) {
        (Method::phast, 1, 8, false) =>
            conf.run(|keys| phast(&keys, conf.params(Bits8), threads_num, SeedOnly)),
        (Method::phast, 1, b, false) =>
            conf.run(|keys| phast(&keys, conf.params(BitsFast(b)), threads_num, SeedOnly)),

        (Method::phast2, 1, 8, false) =>
            conf.run(|keys| phast2(&keys, conf.params(Bits8), threads_num, SeedOnly)),
        (Method::phast2, 1, b, false) =>
            conf.run(|keys| phast2(&keys, conf.params(BitsFast(b)), threads_num, SeedOnly)),

        (Method::perfect, 1, 8, false) =>
            conf.run(|keys| perfect(&keys, conf.params(Bits8), threads_num, SeedOnly)),
        (Method::perfect, 1, b, false) =>
            conf.run(|keys| perfect(&keys, conf.params(BitsFast(b)), threads_num, SeedOnly)),
        (Method::perfect, k, 8, false) =>
            conf.run(|keys| perfect(&keys, conf.params(Bits8), threads_num, SeedOnlyK(k))),
        (Method::perfect, k, b, false) =>
            conf.run(|keys| perfect(&keys, conf.params(BitsFast(b)), threads_num, SeedOnlyK(k))),

        (Method::phast|Method::phast2|Method::perfect, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, SeedOnly)),
        (Method::phast|Method::phast2|Method::perfect, 1, b, true) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, SeedOnly)),
        (Method::phast|Method::phast2|Method::perfect, k, 8, true) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, SeedOnlyK(k))),
        (Method::phast|Method::phast2|Method::perfect, k, b, true) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, SeedOnlyK(k))),

        (Method::pluswrap { multiplier: 1 }, 1, 8, false) =>
            conf.run(|keys| phast(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }, 1, 8, false) =>
            conf.run(|keys| phast(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }, 1, 8, false) =>
            conf.run(|keys| phast(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap { multiplier: 1 }, 1, b, false) =>
            conf.run(|keys| phast(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }, 1, b, false) =>
            conf.run(|keys| phast(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }, 1, b, false) =>
            conf.run(|keys| phast(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<3>)),

        (Method::pluswrap2 { multiplier: 1 }, 1, 8, false) =>
            conf.run(|keys| phast2(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap2 { multiplier: 2 }, 1, 8, false) =>
            conf.run(|keys| phast2(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap2 { multiplier: 3 }, 1, 8, false) =>
            conf.run(|keys| phast2(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap2 { multiplier: 1 }, 1, b, false) => 
            conf.run(|keys| phast2(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap2 { multiplier: 2 }, 1, b, false) => 
            conf.run(|keys| phast2(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap2 { multiplier: 3 }, 1, b, false) => 
            conf.run(|keys| phast2(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<3>)),

        (Method::pluswrap { multiplier: 1 } | Method::pluswrap2 { multiplier: 1 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 } | Method::pluswrap2 { multiplier: 2 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 } | Method::pluswrap2 { multiplier: 3 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap { multiplier: 1 }| Method::pluswrap2 { multiplier: 1 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }| Method::pluswrap2 { multiplier: 2 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }| Method::pluswrap2 { multiplier: 3 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<3>)),

        (Method::optphast, 1, _, _) => conf.optimize_weights(SeedOnly),
        (Method::optphast, k, _, _) => conf.optimize_weights(SeedOnlyK(k)),

        (Method::optpluswrap { multiplier: 1 }, 1, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<1>),
        (Method::optpluswrap { multiplier: 2 }, 1, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<2>),
        (Method::optpluswrap { multiplier: 3 }, 1, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<3>),

        _ => eprintln!("Unsupported configuration.")
    };
}
