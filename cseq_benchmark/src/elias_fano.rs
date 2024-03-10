use cseq::elias_fano;
use dyn_size_of::GetSize;
use aligned_vec::ABox;
use elias_fano::Builder;
use bitm::{Rank, Select};

pub fn benchmark(conf: &super::Conf) {
    println!("cseq Elias-Fano");

    let mut builder = Builder::<ABox<[u64], _>>::new_b(conf.num(), conf.universe as u64);
    let tester = conf.add_data(|v| builder.push(v as u64));

    //let start_moment = Instant::now();
    //let ef = Sequence::<S, S, ABox<[u64], _>>::with_items_from_slice_s(&data);
    //let build_time_seconds = start_moment.elapsed().as_secs_f64();
    let ef = builder.finish();
    println!("  size: {:.2} bits/item   {} bits/lo entry", 8.0*ef.size_bytes() as f64/tester.number_of_ones as f64, ef.bits_per_lo());

    tester.raport_rank("cseq Elias-Fano", ef.size_bytes(), |i| ef.rank(i));
    tester.raport_select1("cseq Elias-Fano", 0, |i| ef.select(i));

    /*let start_moment = Instant::now();
    for index in 0..data.len() {
        black_box(ef.get(index));
    }
    let get_time_nanos = start_moment.elapsed().as_nanos();
    print!("time/item to [ns]: get {:.2}", get_time_nanos as f64 / data.len() as f64);

    let start_moment = Instant::now();
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
    }*/
}