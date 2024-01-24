use sucds::bit_vectors::{Rank9Sel, BitVector, Rank, Select};
use crate::UnitPrefix;

fn benchmark_select(conf: &super::Conf, rs: &impl Select) -> f64 {
    conf.num_sampling_measure(1000000, |index| rs.select1(index))
}

fn benchmark_select0(conf: &super::Conf, rs: &impl Select) -> f64 {
    conf.num_complement_sampling_measure(1000000, |index| rs.select0(index))
}

pub fn benchmark_rank9_select(conf: &super::Conf) {
    let inserted_values = conf.num * 2 < conf.universe;
    let mut content = BitVector::from_bit(!inserted_values, conf.universe);
    let mut to_insert = if inserted_values { conf.num } else { conf.universe - conf.num };
    let mut gen = conf.rand_gen();
    while to_insert > 0 {
        let bit_nr = gen.get() as usize % conf.universe;
        if content.get_bit(bit_nr).unwrap() != inserted_values {
            content.set_bit(bit_nr, inserted_values).unwrap();
            to_insert -= 1;
        }
    }
    let mut rs = Rank9Sel::new(content);

    println!("time/rank [ns]: {:.2}", conf.universe_sampling_measure(1000000, |index| rs.rank1(index)).nanos());

    println!("time/select without hints [ns]: {:.2}", benchmark_select(conf, &rs).nanos());
    println!("time/select0 without hints [ns]: {:.2}", benchmark_select0(conf, &rs).nanos());

    rs = rs.select0_hints().select1_hints();
    println!("time/select with hints [ns]: {:.2}", benchmark_select(conf, &rs).nanos());
    println!("time/select0 with hints [ns]: {:.2}", benchmark_select0(conf, &rs).nanos());
}