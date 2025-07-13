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
use ph::phast::{SeedOnly, SeedOnlyK, ShiftOnly, ShiftOnlyWrapped};

fn main() {
    let conf = Conf::parse();
    let threads_num = conf.threads();
    let bucket_size = conf.bucket_size_100();
    if conf.csv && conf.support_csv() {
        if conf.head { println!("{}", conf::CSV_HEADER); }
    } else {
        println!("{} n={} k={} bits/seed={} lambda={:.2} slice={} threads={threads_num}",
        conf.method, conf.keys_num, conf.k, conf.bits_per_seed, bucket_size as f64/100 as f64, conf.slice_len);
    }
    match (conf.method, conf.k, conf.bits_per_seed, conf.one) {
        (Method::phast, 1, 8, false) => conf.run(|keys| phast(&keys, conf.params(Bits8), threads_num, SeedOnly)),
        (Method::phast, 1, b, false) => conf.run(|keys| phast(&keys, conf.params(BitsFast(b)), threads_num, SeedOnly)),

        (Method::phast2, 1, 8, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8), threads_num, SeedOnly)),
        (Method::phast2, 1, b, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b)), threads_num, SeedOnly)),

        (Method::perfect, 1, 8, false) => conf.run(|keys| perfect(&keys, conf.params(Bits8), threads_num, SeedOnly)),
        (Method::perfect, 1, b, false) => conf.run(|keys| perfect(&keys, conf.params(BitsFast(b)), threads_num, SeedOnly)),
        (Method::perfect, k, 8, false) => conf.run(|keys| perfect(&keys, conf.params(Bits8), threads_num, SeedOnlyK(k))),
        (Method::perfect, k, b, false) => conf.run(|keys| perfect(&keys, conf.params(BitsFast(b)), threads_num, SeedOnlyK(k))),

        (Method::phast|Method::phast2|Method::perfect, 1, 8, true) => conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, SeedOnly)),
        (Method::phast|Method::phast2|Method::perfect, 1, b, true) => conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, SeedOnly)),
        (Method::phast|Method::phast2|Method::perfect, k, 8, true) => conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, SeedOnlyK(k))),
        (Method::phast|Method::phast2|Method::perfect, k, b, true) => conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, SeedOnlyK(k))),

        (Method::pluswrap { multiplier: 1 }, 1, 8, false) => conf.run(|keys| phast(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }, 1, 8, false) => conf.run(|keys| phast(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }, 1, 8, false) => conf.run(|keys| phast(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap { multiplier: 5 }, 1, 8, false) => conf.run(|keys| phast(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<5>)),
        (Method::pluswrap { multiplier: 7 }, 1, 8, false) => conf.run(|keys| phast(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<7>)),
        (Method::pluswrap { multiplier: 9 }, 1, 8, false) => conf.run(|keys| phast(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<9>)),
        (Method::pluswrap { multiplier:11 }, 1, 8, false) => conf.run(|keys| phast(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<11>)),
        (Method::pluswrap { multiplier: 1 }, 1, b, false) => conf.run(|keys| phast(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }, 1, b, false) => conf.run(|keys| phast(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }, 1, b, false) => conf.run(|keys| phast(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap { multiplier: 5 }, 1, b, false) => conf.run(|keys| phast(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<5>)),
        (Method::pluswrap { multiplier: 7 }, 1, b, false) => conf.run(|keys| phast(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<7>)),
        (Method::pluswrap { multiplier: 9 }, 1, b, false) => conf.run(|keys| phast(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<9>)),
        (Method::pluswrap { multiplier:11 }, 1, b, false) => conf.run(|keys| phast(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<11>)),

        (Method::pluswrap2 { multiplier: 1 }, 1, 8, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap2 { multiplier: 2 }, 1, 8, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap2 { multiplier: 3 }, 1, 8, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap2 { multiplier: 5 }, 1, 8, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<5>)),
        (Method::pluswrap2 { multiplier: 7 }, 1, 8, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<7>)),
        (Method::pluswrap2 { multiplier: 9 }, 1, 8, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<9>)),
        (Method::pluswrap2 { multiplier:11 }, 1, 8, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<11>)),
        (Method::pluswrap2 { multiplier: 1 }, 1, b, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap2 { multiplier: 2 }, 1, b, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap2 { multiplier: 3 }, 1, b, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap2 { multiplier: 5 }, 1, b, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<5>)),
        (Method::pluswrap2 { multiplier: 7 }, 1, b, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<7>)),
        (Method::pluswrap2 { multiplier: 9 }, 1, b, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<9>)),
        (Method::pluswrap2 { multiplier:11 }, 1, b, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<11>)),

        (Method::pluswrap { multiplier: 1 } | Method::pluswrap2 { multiplier: 1 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 } | Method::pluswrap2 { multiplier: 2 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 } | Method::pluswrap2 { multiplier: 3 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap { multiplier: 5 } | Method::pluswrap2 { multiplier: 5 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<5>)),
        (Method::pluswrap { multiplier: 7 } | Method::pluswrap2 { multiplier: 7 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<7>)),
        (Method::pluswrap { multiplier: 9 } | Method::pluswrap2 { multiplier: 9 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<9>)),
        (Method::pluswrap { multiplier:11 } | Method::pluswrap2 { multiplier:11 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, ShiftOnlyWrapped::<11>)),

        (Method::pluswrap { multiplier: 1 }| Method::pluswrap2 { multiplier: 1 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }| Method::pluswrap2 { multiplier: 2 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }| Method::pluswrap2 { multiplier: 3 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap { multiplier: 5 }| Method::pluswrap2 { multiplier: 5 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<5>)),
        (Method::pluswrap { multiplier: 7 }| Method::pluswrap2 { multiplier: 7 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<7>)),
        (Method::pluswrap { multiplier: 9 }| Method::pluswrap2 { multiplier: 9 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<9>)),
        (Method::pluswrap { multiplier:11 }| Method::pluswrap2 { multiplier:11 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnlyWrapped::<11>)),

        (Method::plus { multiplier: 1 }, 1, 8, false) =>
            conf.run(|keys| phast2(&keys, conf.params(Bits8), threads_num, ShiftOnly::<1>)),
        (Method::plus { multiplier: 2 }, 1, 8, false) =>
            conf.run(|keys| phast2(&keys, conf.params(Bits8), threads_num, ShiftOnly::<2>)),
        (Method::plus { multiplier: 3 }, 1, 8, false) =>
            conf.run(|keys| phast2(&keys, conf.params(Bits8), threads_num, ShiftOnly::<3>)),
        (Method::plus { multiplier: 1 }, 1, b, false) =>
            conf.run(|keys| phast2(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnly::<1>)),
        (Method::plus { multiplier: 2 }, 1, b, false) =>
            conf.run(|keys| phast2(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnly::<2>)),
        (Method::plus { multiplier: 3 }, 1, b, false) =>
            conf.run(|keys| phast2(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnly::<3>)),

        (Method::plus { multiplier: 1 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, ShiftOnly::<1>)),
        (Method::plus { multiplier: 2 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, ShiftOnly::<2>)),
        (Method::plus { multiplier: 3 }, 1, 8, true) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8), threads_num, ShiftOnly::<3>)),
        (Method::plus { multiplier: 1 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnly::<1>)),
        (Method::plus { multiplier: 2 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnly::<2>)),
        (Method::plus { multiplier: 3 }, 1, b, true) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b)), threads_num, ShiftOnly::<3>)),

        (Method::optphast, 1, _, _) => conf.optimize_weights(SeedOnly),
        (Method::optphast, k, _, _) => conf.optimize_weights(SeedOnlyK(k)),

        (Method::optpluswrap { multiplier: 1 }, 1, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<1>),
        (Method::optpluswrap { multiplier: 2 }, 1, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<2>),
        (Method::optpluswrap { multiplier: 3 }, 1, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<3>),
        (Method::optpluswrap { multiplier: 5 }, 1, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<5>),
        (Method::optpluswrap { multiplier: 7 }, 1, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<7>),
        (Method::optpluswrap { multiplier: 9 }, 1, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<9>),
        (Method::optpluswrap { multiplier:11 }, 1, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<11>),

        (Method::optplus { multiplier: 1 }, 1, _, _) => conf.optimize_weights(ShiftOnly::<1>),
        (Method::optplus { multiplier: 2 }, 1, _, _) => conf.optimize_weights(ShiftOnly::<2>),
        (Method::optplus { multiplier: 3 }, 1, _, _) => conf.optimize_weights(ShiftOnly::<3>),

        (Method::none, _, _, _) => {},
        _ => eprintln!("Unsupported configuration.")
    };
}
