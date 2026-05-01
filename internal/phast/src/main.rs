#![doc = include_str!("../README.md")]

mod conf;
use crate::conf::{Conf, Method};

mod function;

mod perfect;
use crate::optim::{SumOfLogValuesF, SumOfLogValuesF0, SumOfLogValuesF1};
use crate::perfect::perfect;

mod phast;
use crate::phast::{kphast, phast, phast2};

mod partial;
use crate::partial::partial;

mod optim;

mod benchmark;

use clap::Parser;

use ph::seeds::{Bits8, BitsFast};
use ph::phast::{ProdOfValues, SeedOnly, SeedOnlyK, ShiftOnly, ShiftOnlyWrapped, space_lower_bound};

fn main() {
    let conf = Conf::parse();
    //println!("{}", space_lower_bound(conf.k));
    let threads_num = conf.threads();
    let bucket_size = conf.bucket_size();
    if conf.csv && conf.support_csv() {
        if conf.head { println!("{}", conf::CSV_HEADER); }
    } else {
        println!("{} k={}   space lower bound ≈ {:.3} bits/key", conf.method, conf.k, space_lower_bound(conf.k));
        println!("n={} bits/seed={} λ={:.2} slice={} threads={threads_num}",
        conf.keys_num, conf.bits_per_seed, bucket_size, conf.slice_len);
    }
    match (conf.method, conf.k, conf.bits_per_seed, conf.one, bucket_size.into(), conf.is_turbo()) {
        (Method::phast, 1, 8, false, _, true) => conf.run(|keys| phast(&keys, conf.params_turbo(Bits8), threads_num, SeedOnly(ProdOfValues))),
        (Method::phast, 1, 8, false, bucket_size100, false) => conf.run(|keys| phast(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnly(ProdOfValues))),
        (Method::phast, 1, b, false, bucket_size100, false) => conf.run(|keys| phast(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnly(ProdOfValues))),
        
        (Method::phast, k, 8, false, _, true) => conf.run(|keys| kphast(&keys, conf.params_turbo(Bits8), threads_num, SeedOnlyK::with_evaluator(k, ProdOfValues))),
        (Method::phast, k, 8, false, bucket_size100, false) => conf.run(|keys| kphast(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, ProdOfValues))),
        (Method::phast, k, b, false, bucket_size100, false) => conf.run(|keys| kphast(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, ProdOfValues))),

        (Method::phast2, 1, 8, false, _, true) => conf.run(|keys| phast2(&keys, conf.params_turbo(Bits8), threads_num, SeedOnly(ProdOfValues))),
        (Method::phast2, 1, 8, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnly(ProdOfValues))),
        (Method::phast2, 1, b, false, bucket_size100, false) => conf.run(|keys| phast2(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnly(ProdOfValues))),

        //(Method::perfect, 1, 8, false, bucket_size100, true) => conf.run(|keys| perfect(&keys, conf.params_turbo(Bits8, bucket_size100), threads_num, SeedOnly)),
        (Method::perfect, 1, 8, false, bucket_size100, false) => conf.run(|keys| perfect(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnly(ProdOfValues))),
        (Method::perfect, 1, b, false, bucket_size100, false) => conf.run(|keys| perfect(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnly(ProdOfValues))),
        (Method::perfect, k, 8, false, bucket_size100, false) => conf.run(|keys| perfect(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, ProdOfValues))), 
        (Method::perfectlog, k, 8, false, bucket_size100, false) =>
            conf.run(|keys| perfect(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, SumOfLogValuesF))),
        (Method::perfectlog0, k, 8, false, bucket_size100, false) =>
            conf.run(|keys| perfect(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, SumOfLogValuesF0))),
        (Method::perfectlog1, k, 8, false, bucket_size100, false) =>
            conf.run(|keys| perfect(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, SumOfLogValuesF1))),
        (Method::perfect, k, b, false, bucket_size100, false) => conf.run(|keys| perfect(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, ProdOfValues))),
        (Method::perfectlog, k, b, false, bucket_size100, false) =>
            conf.run(|keys| perfect(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, SumOfLogValuesF))),
        (Method::perfectlog0, k, b, false, bucket_size100, false) =>
            conf.run(|keys| perfect(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, SumOfLogValuesF0))),
        (Method::perfectlog1, k, b, false, bucket_size100, false) =>
            conf.run(|keys| perfect(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, SumOfLogValuesF1))),

        (Method::phast|Method::phast2|Method::perfect, 1, 8, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnly(ProdOfValues))),
        (Method::phast|Method::phast2|Method::perfect, 1, b, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnly(ProdOfValues))),
        (Method::phast|Method::phast2|Method::perfect, k, 8, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, ProdOfValues))),
        (Method::perfectlog, k, 8, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, SumOfLogValuesF))),
        (Method::perfectlog0, k, 8, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, SumOfLogValuesF0))),
        (Method::perfectlog1, k, 8, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(Bits8, bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, SumOfLogValuesF1))),
        (Method::phast|Method::phast2|Method::perfect, k, b, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, ProdOfValues))),
        (Method::perfectlog, k, b, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, SumOfLogValuesF))),
        (Method::perfectlog0, k, b, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, SumOfLogValuesF0))),
        (Method::perfectlog1, k, b, true, bucket_size100, false) => conf.runp(|keys| partial(&keys, conf.params(BitsFast(b), bucket_size100), threads_num, SeedOnlyK::with_evaluator(k, SumOfLogValuesF1))),

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

        (Method::optphast, 1, _, _, _, _) => conf.optimize_weights(SeedOnly(ProdOfValues)),
        (Method::optphast, k, _, _, _, _) => conf.optimize_weights(SeedOnlyK::with_evaluator(k, ProdOfValues)),

        (Method::optpluswrap { multiplier: 1 }, 1, _, _, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<1>),
        (Method::optpluswrap { multiplier: 2 }, 1, _, _, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<2>),
        (Method::optpluswrap { multiplier: 3 }, 1, _, _, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<3>),
        (Method::optpluswrap { multiplier: 5 }, 1, _, _, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<5>),
        (Method::optpluswrap { multiplier: 7 }, 1, _, _, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<7>),

        (Method::optplus, 1, _, _, _, _) => conf.optimize_weights(ShiftOnly),
        (Method::optperfectlog0, _, _, _, _, _) => conf.optimize_perfectlog0(),
        (Method::optperfectlog1, _, _, _, _, _) => conf.optimize_perfectlog1(),
        (Method::optprod, 1, _, _, _, _) => conf.optimize_genericprod(),
        (Method::optprod, _, _, _, _, _) => conf.optimize_kprod(),
        (Method::optperfectlog, _, _, _, _, _) => conf.optimize_perfectlog(),
        (Method::optwgenprod, _, _, _, _, _) => conf.optimize_wgenericprod(),

        (Method::none, _, _, _, _, _) => {},
        _ => eprintln!("Unsupported configuration.")
    };
}
