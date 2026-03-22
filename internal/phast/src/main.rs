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
use ph::phast::{SeedOnly, SeedOnlyK, ShiftOnly, ShiftOnlyWrapped, SumOfLogValues, SumOfValues, SumOfWeightedValues};

fn main() {
    let conf = Conf::parse();
    //println!("{}", space_lower_bound(conf.k));
    let threads_num = conf.threads();
    let bucket_size = conf.bucket_size();
    if conf.csv && conf.support_csv() {
        if conf.head { println!("{}", conf::CSV_HEADER); }
    } else {
        println!("{} n={} k={} bits/seed={} lambda={:.2} slice={} threads={threads_num}",
        conf.method, conf.keys_num, conf.k, conf.bits_per_seed, bucket_size, conf.slice_len);
    }
    match (conf.method, conf.k, conf.bits_per_seed, conf.one, bucket_size.into(), conf.is_turbo()) {
        (Method::phast, 1, 8, false, _, true) => conf.run(|keys| phast(&keys, conf.params_turbo(Bits8), threads_num, SeedOnly)),
        (Method::phast, 1, 8, false, bucket_size100, false) => conf.run(|keys| phast(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnly)),
        (Method::phast, 1, b, false, bucket_size100, false) => conf.run(|keys| phast(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnly)),

        (Method::phast2, 1, 8, false, _, true) => conf.run(|keys| phast2(&keys, conf.params_turbo(Bits8), threads_num, SeedOnly)),
        (Method::phast2, 1, 8, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnly)),
        (Method::phast2, 1, b, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnly)),

        //(Method::perfect, 1, 8, false, bucket_size100, true) => conf.run(|keys| perfect(&keys, conf.params_turbo(Bits8, bucket_size100), threads_num, SeedOnly)),
        (Method::perfect, 1, 8, false, bucket_size100, false) => conf.run(|keys| perfect(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnly)),
        (Method::perfect, 1, b, false, bucket_size100, false) => conf.run(|keys| perfect(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnly)),
        (Method::perfect, k, 8, false, bucket_size100, false) => conf.run(|keys| perfect(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnlyK::new(k, SumOfValues))), 
        (Method::perfectw, k, 8, false, bucket_size100, false) => conf.run(|keys| perfect(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnlyK::new(k, SumOfWeightedValues::new(k)))),
        (Method::perfectlog { free_values_weight, value_shift }, k, 8, false, bucket_size100, false) =>
            conf.run(|keys| perfect(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnlyK::new(k, SumOfLogValues { free_values_weight, value_shift }))),
        (Method::perfect, k, b, false, bucket_size100, false) => conf.run(|keys| perfect(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnlyK::new(k, SumOfValues))),
        (Method::perfectw, k, b, false, bucket_size100, false) => conf.run(|keys| perfect(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnlyK::new(k, SumOfWeightedValues::new(k)))),
        (Method::perfectlog { free_values_weight, value_shift }, k, b, false, bucket_size100, false) =>
            conf.run(|keys| perfect(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnlyK::new(k, SumOfLogValues { free_values_weight, value_shift }))),

        (Method::phast|Method::phast2|Method::perfect, 1, 8, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnly)),
        (Method::phast|Method::phast2|Method::perfect, 1, b, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnly)),
        (Method::phast|Method::phast2|Method::perfect, k, 8, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnlyK::new(k, SumOfValues))),
        (Method::perfectw, k, 8, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnlyK::new(k, SumOfWeightedValues::new(k)))),
        (Method::perfectlog { free_values_weight, value_shift }, k, 8, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnlyK::new(k, SumOfLogValues{ free_values_weight, value_shift }))),
        (Method::phast|Method::phast2|Method::perfect, k, b, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnlyK::new(k, SumOfValues))),
        (Method::perfectw, k, b, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnlyK::new(k, SumOfWeightedValues::new(k)))),
        (Method::perfectlog { free_values_weight, value_shift }, k, b, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnlyK::new(k, SumOfLogValues{ free_values_weight, value_shift }))),

        (Method::pluswrap { multiplier: 1 }, 1, 8, false, _, true) => conf.run(|keys| phast(&keys, conf.params_turbo(Bits8), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }, 1, 8, false, _levels, true) => conf.run(|keys| phast(&keys, conf.params_turbo(Bits8), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }, 1, 8, false, _, true) => conf.run(|keys| phast(&keys, conf.params_turbo(Bits8), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap { multiplier: 5 }, 1, 8, false, _, true) => conf.run(|keys| phast(&keys, conf.params_turbo(Bits8), threads_num, ShiftOnlyWrapped::<5>)),
        (Method::pluswrap { multiplier: 7 }, 1, 8, false, _, true) => conf.run(|keys| phast(&keys, conf.params_turbo(Bits8), threads_num, ShiftOnlyWrapped::<7>)),
        (Method::pluswrap { multiplier: 1 }, 1, 8, false, bucket_size100, false) => conf.run(|keys| phast(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }, 1, 8, false, bucket_size100, false) => conf.run(|keys| phast(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }, 1, 8, false, bucket_size100, false) => conf.run(|keys| phast(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap { multiplier: 5 }, 1, 8, false, bucket_size100, false) => conf.run(|keys| phast(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnlyWrapped::<5>)),
        (Method::pluswrap { multiplier: 7 }, 1, 8, false, bucket_size100, false) => conf.run(|keys| phast(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnlyWrapped::<7>)),
        (Method::pluswrap { multiplier: 1 }, 1, b, false, bucket_size100, false) => conf.run(|keys| phast(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }, 1, b, false, bucket_size100, false) => conf.run(|keys| phast(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }, 1, b, false, bucket_size100, false) => conf.run(|keys| phast(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap { multiplier: 5 }, 1, b, false, bucket_size100, false) => conf.run(|keys| phast(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnlyWrapped::<5>)),
        (Method::pluswrap { multiplier: 7 }, 1, b, false, bucket_size100, false) => conf.run(|keys| phast(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnlyWrapped::<7>)),

        (Method::pluswrap2 { multiplier: 1 }, 1, 8, false, _, true) => conf.run(|keys| phast2(&keys, conf.params_turbo(Bits8), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap2 { multiplier: 2 }, 1, 8, false, _, true) => conf.run(|keys| phast2(&keys, conf.params_turbo(Bits8), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap2 { multiplier: 3 }, 1, 8, false, _, true) => conf.run(|keys| phast2(&keys, conf.params_turbo(Bits8), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap2 { multiplier: 5 }, 1, 8, false, _, true) => conf.run(|keys| phast2(&keys, conf.params_turbo(Bits8), threads_num, ShiftOnlyWrapped::<5>)),
        (Method::pluswrap2 { multiplier: 7 }, 1, 8, false, _, true) => conf.run(|keys| phast2(&keys, conf.params_turbo(Bits8), threads_num, ShiftOnlyWrapped::<7>)),
        (Method::pluswrap2 { multiplier: 1 }, 1, 8, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap2 { multiplier: 2 }, 1, 8, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap2 { multiplier: 3 }, 1, 8, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap2 { multiplier: 5 }, 1, 8, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnlyWrapped::<5>)),
        (Method::pluswrap2 { multiplier: 7 }, 1, 8, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnlyWrapped::<7>)),
        (Method::pluswrap2 { multiplier: 1 }, 1, b, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap2 { multiplier: 2 }, 1, b, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap2 { multiplier: 3 }, 1, b, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap2 { multiplier: 5 }, 1, b, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnlyWrapped::<5>)),
        (Method::pluswrap2 { multiplier: 7 }, 1, b, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnlyWrapped::<7>)),

        (Method::pluswrap { multiplier: 1 } | Method::pluswrap2 { multiplier: 1 }, 1, 8, true, bucket_size100, false) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 } | Method::pluswrap2 { multiplier: 2 }, 1, 8, true, bucket_size100, false) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 } | Method::pluswrap2 { multiplier: 3 }, 1, 8, true, bucket_size100, false) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap { multiplier: 5 } | Method::pluswrap2 { multiplier: 5 }, 1, 8, true, bucket_size100, false) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnlyWrapped::<5>)),
        (Method::pluswrap { multiplier: 7 } | Method::pluswrap2 { multiplier: 7 }, 1, 8, true, bucket_size100, false) =>
            conf.runp(|keys| partial(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnlyWrapped::<7>)),

        (Method::pluswrap { multiplier: 1 }| Method::pluswrap2 { multiplier: 1 }, 1, b, true, bucket_size100, false) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnlyWrapped::<1>)),
        (Method::pluswrap { multiplier: 2 }| Method::pluswrap2 { multiplier: 2 }, 1, b, true, bucket_size100, false) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnlyWrapped::<2>)),
        (Method::pluswrap { multiplier: 3 }| Method::pluswrap2 { multiplier: 3 }, 1, b, true, bucket_size100, false) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnlyWrapped::<3>)),
        (Method::pluswrap { multiplier: 5 }| Method::pluswrap2 { multiplier: 5 }, 1, b, true, bucket_size100, false) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnlyWrapped::<5>)),
        (Method::pluswrap { multiplier: 7 }| Method::pluswrap2 { multiplier: 7 }, 1, b, true, bucket_size100, false) =>
            conf.runp(|keys| partial(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnlyWrapped::<7>)),

        (Method::plus, 1, 8, false, _, true) => conf.run(|keys| phast2(&keys, conf.params_turbo(Bits8), threads_num, ShiftOnly)),
        (Method::plus, 1, 8, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnly)),
        (Method::plus, 1, b, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnly)),

        (Method::plus, 1, 8, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(Bits8, bucket_size100), threads_num, ShiftOnly)),
        (Method::plus, 1, b, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, ShiftOnly)),

        (Method::optphast, 1, _, _, _, _) => conf.optimize_weights(SeedOnly),
        (Method::optphast, k, _, _, _, _) => conf.optimize_weights(SeedOnlyK::new(k, SumOfValues)),
        (Method::optperfectw, k, _, _, _, _) => conf.optimize_weights(SeedOnlyK::new(k, SumOfWeightedValues::new(k))),

        (Method::optpluswrap { multiplier: 1 }, 1, _, _, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<1>),
        (Method::optpluswrap { multiplier: 2 }, 1, _, _, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<2>),
        (Method::optpluswrap { multiplier: 3 }, 1, _, _, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<3>),
        (Method::optpluswrap { multiplier: 5 }, 1, _, _, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<5>),
        (Method::optpluswrap { multiplier: 7 }, 1, _, _, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<7>),

        (Method::optplus, 1, _, _, _, _) => conf.optimize_weights(ShiftOnly),
        (Method::optscore, _, _, _, _, _) => conf.optimize_score(),

        (Method::none, _, _, _, _, _) => {},
        _ => eprintln!("Unsupported configuration.")
    };
}
