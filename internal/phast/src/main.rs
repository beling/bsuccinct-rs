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
            conf.run(|keys| phast(&keys, bucket_size, threads_num, Bits8, SeedOnly)),
        (Method::phast, 1, b, false) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, BitsFast(b), SeedOnly)),

        (Method::phast2, 1, 8, false) =>
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, Bits8, SeedOnly)),
        (Method::phast2, 1, b, false) =>
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, BitsFast(b), SeedOnly)),

        (Method::perfect, 1, 8, false) =>
            conf.run(|keys| perfect(&keys, bucket_size, threads_num, Bits8, SeedOnly)),
        (Method::perfect, 1, b, false) =>
            conf.run(|keys| perfect(&keys, bucket_size, threads_num, BitsFast(b), SeedOnly)),
        (Method::perfect, k, 8, false) =>
            conf.run(|keys| perfect(&keys, bucket_size, threads_num, Bits8, SeedOnlyK(k))),
        (Method::perfect, k, b, false) =>
            conf.run(|keys| perfect(&keys, bucket_size, threads_num, BitsFast(b), SeedOnlyK(k))),

        (Method::phast|Method::phast2|Method::perfect, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, Bits8, SeedOnly)),
        (Method::phast|Method::phast2|Method::perfect, 1, b, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, BitsFast(b), SeedOnly)),
        (Method::phast|Method::phast2|Method::perfect, k, 8, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, Bits8, SeedOnlyK(k))),
        (Method::phast|Method::phast2|Method::perfect, k, b, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, BitsFast(b), SeedOnlyK(k))),

        (Method::pluswrap { multiplier: 1 }, 1, 8, false) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }, 1, 8, false) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }, 1, 8, false) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap { multiplier: 1 }, 1, b, false) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }, 1, b, false) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }, 1, b, false) =>
            conf.run(|keys| phast(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<3>)),

        (Method::pluswrap2 { multiplier: 1 }, 1, 8, false) =>
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap2 { multiplier: 2 }, 1, 8, false) =>
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap2 { multiplier: 3 }, 1, 8, false) =>
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap2 { multiplier: 1 }, 1, b, false) => 
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<1>)),
        (Method::pluswrap2 { multiplier: 2 }, 1, b, false) => 
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<2>)),
        (Method::pluswrap2 { multiplier: 3 }, 1, b, false) => 
            conf.run(|keys| phast2(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<3>)),

        (Method::pluswrap { multiplier: 1 } | Method::pluswrap2 { multiplier: 1 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 } | Method::pluswrap2 { multiplier: 2 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 } | Method::pluswrap2 { multiplier: 3 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, Bits8, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap { multiplier: 1 }| Method::pluswrap2 { multiplier: 1 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }| Method::pluswrap2 { multiplier: 2 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }| Method::pluswrap2 { multiplier: 3 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, bucket_size, threads_num, BitsFast(b), ShiftOnlyWrapped::<3>)),

        _ => eprintln!("Unsupported configuration.")
    };
}
