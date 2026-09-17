#![doc = include_str!("../README.md")]

mod conf;
use crate::conf::{Conf, Method};

mod function;

mod perfect;
use crate::optim::{SumOfLogValuesF, SumOfLogValuesF0, SumOfLogValuesF1};
use crate::perfect::perfect;

mod phast;
use crate::phast::{kphast, nbphast, phast, phast2};

mod partial;

mod optim;

mod benchmark;

use clap::Parser;

use ph::seeds::{Bits8, BitsFast};
use ph::phast::{ProdOfValues, SeedOnly, SeedOnlyK, ShiftOnly, ShiftOnlyWrapped, ShiftOnlyProdWrapped, space_lower_bound, WINDOW_SIZE};

fn main() {
    let conf = Conf::parse();
    //println!("{}", space_lower_bound(conf.k));
    let threads_num = conf.threads();
    let bucket_size = conf.bucket_size();
    if conf.csv && conf.support_csv() {
        if conf.head { println!("{}", conf::CSV_HEADER); }
    } else {
        println!("{} k={}   space lower bound ≈ {:.3} bits/key", conf.method, conf.k, space_lower_bound(conf.k));
        println!("n={} bits/seed={} λ={:.2} slice={} W={WINDOW_SIZE} threads={threads_num}",
        conf.keys_num, conf.bits_per_seed, bucket_size, conf.slice_len);
    }
    match (conf.method, conf.k, conf.bits_per_seed, conf.one || conf.multi_level, conf.is_turbo()) {
        (Method::phast, 1, 8, false, true) => conf.run_turbo(phast, SeedOnly(ProdOfValues)),
        (Method::phast, 1, 8, false, false) => conf.run_phast(phast, Bits8, SeedOnly(ProdOfValues)),
        (Method::phast, 1, b, false, false) => conf.run_phast(phast, BitsFast(b), SeedOnly(ProdOfValues)),
        
        (Method::phast, k, 8, false, true) => conf.run_turbo(kphast, SeedOnlyK::with_evaluator(k, ProdOfValues)),
        (Method::phast, k, 8, false, false) => conf.run_phast(kphast, Bits8, SeedOnlyK::with_evaluator(k, ProdOfValues)),
        (Method::phast, k, b, false, false) => conf.run_phast(kphast, BitsFast(b), SeedOnlyK::with_evaluator(k, ProdOfValues)),

        (Method::phast2, 1, 8, false, true) => conf.run_turbo(phast2, SeedOnly(ProdOfValues)),
        (Method::phast2, 1, 8, false, false) => conf.run_phast(phast2, Bits8, SeedOnly(ProdOfValues)),
        (Method::phast2, 1, b, false, false) => conf.run_phast(phast2, BitsFast(b), SeedOnly(ProdOfValues)),

        (Method::nbphast, 1, b, false, false) => conf.run(|keys| nbphast(keys, conf.params_random(BitsFast(b), conf.bucket_size().into()), threads_num)),

        (Method::perfect, 1, 8, false, false) => conf.run_phast(perfect, Bits8, SeedOnly(ProdOfValues)),
        (Method::perfect, 1, b, false, false) => conf.run_phast(perfect, BitsFast(b), SeedOnly(ProdOfValues)),
        (Method::perfect, k, 8, false, false) => conf.run_phast(perfect, Bits8, SeedOnlyK::with_evaluator(k, ProdOfValues)),
        (Method::perfectlog, k, 8, false, false) =>
            conf.run_phast(perfect, Bits8, SeedOnlyK::with_evaluator(k, SumOfLogValuesF)),
        (Method::perfectlog0, k, 8, false, false) =>
            conf.run_phast(perfect, Bits8, SeedOnlyK::with_evaluator(k, SumOfLogValuesF0)),
        (Method::perfectlog1, k, 8, false, false) =>
            conf.run_phast(perfect, Bits8, SeedOnlyK::with_evaluator(k, SumOfLogValuesF1)),
        (Method::perfect, k, b, false, false) => conf.run_phast(perfect, BitsFast(b), SeedOnlyK::with_evaluator(k, ProdOfValues)),
        (Method::perfectlog, k, b, false, false) =>
            conf.run_phast(perfect, BitsFast(b), SeedOnlyK::with_evaluator(k, SumOfLogValuesF)),
        (Method::perfectlog0, k, b, false, false) =>
            conf.run_phast(perfect, BitsFast(b), SeedOnlyK::with_evaluator(k, SumOfLogValuesF0)),
        (Method::perfectlog1, k, b, false, false) =>
            conf.run_phast(perfect, BitsFast(b), SeedOnlyK::with_evaluator(k, SumOfLogValuesF1)),

        (Method::phast|Method::phast2|Method::perfect, 1, 8, true, false) => conf.run_partial(Bits8, SeedOnly(ProdOfValues)),
        (Method::phast|Method::phast2|Method::perfect, 1, b, true, false) => conf.run_partial(BitsFast(b), SeedOnly(ProdOfValues)),
        (Method::phast|Method::phast2|Method::perfect, k, 8, true, false) => conf.run_partial(Bits8, SeedOnlyK::with_evaluator(k, ProdOfValues)),
        (Method::perfectlog, k, 8, true, false) => conf.run_partial(Bits8, SeedOnlyK::with_evaluator(k, SumOfLogValuesF)),
        (Method::perfectlog0, k, 8, true, false) => conf.run_partial(Bits8, SeedOnlyK::with_evaluator(k, SumOfLogValuesF0)),
        (Method::perfectlog1, k, 8, true, false) => conf.run_partial(Bits8, SeedOnlyK::with_evaluator(k, SumOfLogValuesF1)),
        (Method::phast|Method::phast2|Method::perfect, k, b, true, false) => conf.run_partial(BitsFast(b), SeedOnlyK::with_evaluator(k, ProdOfValues)),
        (Method::perfectlog, k, b, true, false) => conf.run_partial(BitsFast(b), SeedOnlyK::with_evaluator(k, SumOfLogValuesF)),
        (Method::perfectlog0, k, b, true, false) => conf.run_partial(BitsFast(b), SeedOnlyK::with_evaluator(k, SumOfLogValuesF0)),
        (Method::perfectlog1, k, b, true, false) => conf.run_partial(BitsFast(b), SeedOnlyK::with_evaluator(k, SumOfLogValuesF1)),

        (Method::pluswrap { multiplier: 1 }, 1, 8, false, true) => conf.run_turbo(phast, ShiftOnlyWrapped::<1>),
        (Method::pluswrap { multiplier: 2 }, 1, 8, false, true) => conf.run_turbo(phast, ShiftOnlyWrapped::<2>),
        (Method::pluswrap { multiplier: 3 }, 1, 8, false, true) => conf.run_turbo(phast, ShiftOnlyWrapped::<3>),
        (Method::pluswrap { multiplier: 5 }, 1, 8, false, true) => conf.run_turbo(phast, ShiftOnlyWrapped::<5>),
        (Method::pluswrap { multiplier: 7 }, 1, 8, false, true) => conf.run_turbo(phast, ShiftOnlyWrapped::<7>),
        (Method::pluswrap { multiplier: 1 }, 1, 8, false, false) => conf.run_phast(phast, Bits8, ShiftOnlyWrapped::<1>),
        (Method::pluswrap { multiplier: 2 }, 1, 8, false, false) => conf.run_phast(phast, Bits8, ShiftOnlyWrapped::<2>),
        (Method::pluswrap { multiplier: 3 }, 1, 8, false, false) => conf.run_phast(phast, Bits8, ShiftOnlyWrapped::<3>),
        (Method::pluswrap { multiplier: 5 }, 1, 8, false, false) => conf.run_phast(phast, Bits8, ShiftOnlyWrapped::<5>),
        (Method::pluswrap { multiplier: 7 }, 1, 8, false, false) => conf.run_phast(phast, Bits8, ShiftOnlyWrapped::<7>),
        (Method::pluswrap { multiplier: 1 }, 1, b, false, false) => conf.run_phast(phast, BitsFast(b), ShiftOnlyWrapped::<1>),
        (Method::pluswrap { multiplier: 2 }, 1, b, false, false) => conf.run_phast(phast, BitsFast(b), ShiftOnlyWrapped::<2>),
        (Method::pluswrap { multiplier: 3 }, 1, b, false, false) => conf.run_phast(phast, BitsFast(b), ShiftOnlyWrapped::<3>),
        (Method::pluswrap { multiplier: 5 }, 1, b, false, false) => conf.run_phast(phast, BitsFast(b), ShiftOnlyWrapped::<5>),
        (Method::pluswrap { multiplier: 7 }, 1, b, false, false) => conf.run_phast(phast, BitsFast(b), ShiftOnlyWrapped::<7>),

        (Method::pluswrap2 { multiplier: 1 }, 1, 8, false, true) => conf.run_turbo(phast2, ShiftOnlyWrapped::<1>),
        (Method::pluswrap2 { multiplier: 2 }, 1, 8, false, true) => conf.run_turbo(phast2, ShiftOnlyWrapped::<2>),
        (Method::pluswrap2 { multiplier: 3 }, 1, 8, false, true) => conf.run_turbo(phast2, ShiftOnlyWrapped::<3>),
        (Method::pluswrap2 { multiplier: 5 }, 1, 8, false, true) => conf.run_turbo(phast2, ShiftOnlyWrapped::<5>),
        (Method::pluswrap2 { multiplier: 7 }, 1, 8, false, true) => conf.run_turbo(phast2, ShiftOnlyWrapped::<7>),
        (Method::pluswrap2 { multiplier: 1 }, 1, 8, false, false) => conf.run_phast(phast2, Bits8, ShiftOnlyWrapped::<1>),
        (Method::pluswrap2 { multiplier: 2 }, 1, 8, false, false) => conf.run_phast(phast2, Bits8, ShiftOnlyWrapped::<2>),
        (Method::pluswrap2 { multiplier: 3 }, 1, 8, false, false) => conf.run_phast(phast2, Bits8, ShiftOnlyWrapped::<3>),
        (Method::pluswrap2 { multiplier: 5 }, 1, 8, false, false) => conf.run_phast(phast2, Bits8, ShiftOnlyWrapped::<5>),
        (Method::pluswrap2 { multiplier: 7 }, 1, 8, false, false) => conf.run_phast(phast2, Bits8, ShiftOnlyWrapped::<7>),
        (Method::pluswrap2 { multiplier: 1 }, 1, b, false, false) => conf.run_phast(phast2, BitsFast(b), ShiftOnlyWrapped::<1>),
        (Method::pluswrap2 { multiplier: 2 }, 1, b, false, false) => conf.run_phast(phast2, BitsFast(b), ShiftOnlyWrapped::<2>),
        (Method::pluswrap2 { multiplier: 3 }, 1, b, false, false) => conf.run_phast(phast2, BitsFast(b), ShiftOnlyWrapped::<3>),
        (Method::pluswrap2 { multiplier: 5 }, 1, b, false, false) => conf.run_phast(phast2, BitsFast(b), ShiftOnlyWrapped::<5>),
        (Method::pluswrap2 { multiplier: 7 }, 1, b, false, false) => conf.run_phast(phast2, BitsFast(b), ShiftOnlyWrapped::<7>),

        (Method::pluswrap2prod { multiplier: 1 }, 1, 8, false, true) => conf.run_turbo(phast2, ShiftOnlyProdWrapped::<1>),
        (Method::pluswrap2prod { multiplier: 2 }, 1, 8, false, true) => conf.run_turbo(phast2, ShiftOnlyProdWrapped::<2>),
        (Method::pluswrap2prod { multiplier: 3 }, 1, 8, false, true) => conf.run_turbo(phast2, ShiftOnlyProdWrapped::<3>),
        (Method::pluswrap2prod { multiplier: 5 }, 1, 8, false, true) => conf.run_turbo(phast2, ShiftOnlyProdWrapped::<5>),
        (Method::pluswrap2prod { multiplier: 1 }, 1, 8, false, false) => conf.run_phast(phast2, Bits8, ShiftOnlyProdWrapped::<1>),
        (Method::pluswrap2prod { multiplier: 2 }, 1, 8, false, false) => conf.run_phast(phast2, Bits8, ShiftOnlyProdWrapped::<2>),
        (Method::pluswrap2prod { multiplier: 3 }, 1, 8, false, false) => conf.run_phast(phast2, Bits8, ShiftOnlyProdWrapped::<3>),
        (Method::pluswrap2prod { multiplier: 5 }, 1, 8, false, false) => conf.run_phast(phast2, Bits8, ShiftOnlyProdWrapped::<5>),
        (Method::pluswrap2prod { multiplier: 1 }, 1, b, false, false) => conf.run_phast(phast2, BitsFast(b), ShiftOnlyProdWrapped::<1>),
        (Method::pluswrap2prod { multiplier: 2 }, 1, b, false, false) => conf.run_phast(phast2, BitsFast(b), ShiftOnlyProdWrapped::<2>),
        (Method::pluswrap2prod { multiplier: 3 }, 1, b, false, false) => conf.run_phast(phast2, BitsFast(b), ShiftOnlyProdWrapped::<3>),
        (Method::pluswrap2prod { multiplier: 5 }, 1, b, false, false) => conf.run_phast(phast2, BitsFast(b), ShiftOnlyProdWrapped::<5>),

        (Method::pluswrap { multiplier: 1 } | Method::pluswrap2 { multiplier: 1 }, 1, 8, true, false) =>
            conf.run_partial(Bits8, ShiftOnlyWrapped::<1>),
        (Method::pluswrap { multiplier: 2 } | Method::pluswrap2 { multiplier: 2 }, 1, 8, true, false) =>
            conf.run_partial(Bits8, ShiftOnlyWrapped::<2>),
        (Method::pluswrap { multiplier: 3 } | Method::pluswrap2 { multiplier: 3 }, 1, 8, true, false) =>
            conf.run_partial(Bits8, ShiftOnlyWrapped::<3>),
        (Method::pluswrap { multiplier: 5 } | Method::pluswrap2 { multiplier: 5 }, 1, 8, true, false) =>
            conf.run_partial(Bits8, ShiftOnlyWrapped::<5>),
        (Method::pluswrap { multiplier: 7 } | Method::pluswrap2 { multiplier: 7 }, 1, 8, true, false) =>
            conf.run_partial(Bits8, ShiftOnlyWrapped::<7>),

        (Method::pluswrap { multiplier: 1 }| Method::pluswrap2 { multiplier: 1 }, 1, b, true, false) =>
            conf.run_partial(BitsFast(b), ShiftOnlyWrapped::<1>),
        (Method::pluswrap { multiplier: 2 }| Method::pluswrap2 { multiplier: 2 }, 1, b, true, false) =>
            conf.run_partial(BitsFast(b), ShiftOnlyWrapped::<2>),
        (Method::pluswrap { multiplier: 3 }| Method::pluswrap2 { multiplier: 3 }, 1, b, true, false) =>
            conf.run_partial(BitsFast(b), ShiftOnlyWrapped::<3>),
        (Method::pluswrap { multiplier: 5 }| Method::pluswrap2 { multiplier: 5 }, 1, b, true, false) =>
            conf.run_partial(BitsFast(b), ShiftOnlyWrapped::<5>),
        (Method::pluswrap { multiplier: 7 }| Method::pluswrap2 { multiplier: 7 }, 1, b, true, false) =>
            conf.run_partial(BitsFast(b), ShiftOnlyWrapped::<7>),

        (Method::plus, 1, 8, false, true) => conf.run_turbo(phast2, ShiftOnly),
        (Method::plus, 1, 8, false, false) => conf.run_phast(phast2, Bits8, ShiftOnly),
        (Method::plus, 1, b, false, false) => conf.run_phast(phast2, BitsFast(b), ShiftOnly),

        (Method::plus, 1, 8, true, false) => conf.run_partial(Bits8, ShiftOnly),
        (Method::plus, 1, b, true, false) => conf.run_partial(BitsFast(b), ShiftOnly),

        (Method::optphast, 1, _, _, _) => conf.optimize_weights(SeedOnly(ProdOfValues)),
        (Method::optphast, k, _, _, _) => conf.optimize_weights(SeedOnlyK::with_evaluator(k, ProdOfValues)),

        (Method::optphastdelta, 1, _, _, _) => conf.optimize_weights_delta(SeedOnly(ProdOfValues)),
        (Method::optphastdelta, k, _, _, _) => conf.optimize_weights_delta(SeedOnlyK::with_evaluator(k, ProdOfValues)),

        (Method::optphast4, 1, _, _, _) => conf.optimize_weights4(SeedOnly(ProdOfValues)),
        (Method::optphast4, k, _, _, _) => conf.optimize_weights4(SeedOnlyK::with_evaluator(k, ProdOfValues)),

        (Method::optphast6, 1, _, _, _) => conf.optimize_weights6(SeedOnly(ProdOfValues)),
        (Method::optphast6, k, _, _, _) => conf.optimize_weights6(SeedOnlyK::with_evaluator(k, ProdOfValues)),

        (Method::optpluswrap { multiplier: 1 }, 1, _, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<1>),
        (Method::optpluswrap { multiplier: 2 }, 1, _, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<2>),
        (Method::optpluswrap { multiplier: 3 }, 1, _, _, _) => conf.optimize_weights(ShiftOnlyWrapped::<3>),

        (Method::optplusprodwrap { multiplier: 1 }, 1, _, _, _) => conf.optimize_weights(ShiftOnlyProdWrapped::<1>),
        (Method::optplusprodwrap { multiplier: 2 }, 1, _, _, _) => conf.optimize_weights(ShiftOnlyProdWrapped::<2>),
        (Method::optplusprodwrap { multiplier: 3 }, 1, _, _, _) => conf.optimize_weights(ShiftOnlyProdWrapped::<3>),

        (Method::optplusprodwrap6 { multiplier: 1 }, 1, _, _, _) => conf.optimize_weights6(ShiftOnlyProdWrapped::<1>),
        (Method::optplusprodwrap6 { multiplier: 2 }, 1, _, _, _) => conf.optimize_weights6(ShiftOnlyProdWrapped::<2>),
        (Method::optplusprodwrap6 { multiplier: 3 }, 1, _, _, _) => conf.optimize_weights6(ShiftOnlyProdWrapped::<3>),

        (Method::optplus, 1, _, _, _) => conf.optimize_weights(ShiftOnly),
        (Method::optperfectlog, _, _, _, _) => conf.optimize_perfectlog(),
        (Method::optperfectlog0, _, _, _, _) => conf.optimize_perfectlog0(),
        (Method::optperfectlog1, _, _, _, _) => conf.optimize_perfectlog1(),
        (Method::optprod, _, _, _, _) => conf.optimize_prod(),
        (Method::optall, _, _, _, _) => conf.optimize_all(),
        (Method::optfull, _, _, _, _) => conf.optimize_fullk(),
        (Method::optwgenprod, _, _, _, _) => conf.optimize_wgenericprod(),

        (Method::none, _, _, _, _) => {},
        _ => eprintln!("Unsupported configuration.")
    };
}
