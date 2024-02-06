use butils::UnitPrefix;
use sucds::{bit_vectors::{Rank9Sel, BitVector, Rank, Select}, Serializable};
use crate::{percent_of, Conf};

fn benchmark_select(conf: &Conf, rs: &impl Select) -> f64 {
    conf.num_queries_measure(|index| rs.select1(index))
}

fn benchmark_select0(conf: &Conf, rs: &impl Select) -> f64 {
    conf.num_complement_queries_measure(|index| rs.select0(index))
}

pub fn benchmark_rank9_select(conf: &Conf) {
    println!("sucds bit vector Rank9Sel:");

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
    let content_size = content.size_in_bytes();
    let mut rs = Rank9Sel::new(content);
    let rs_size_without_hints = rs.size_in_bytes();
    println!("  rank space overhead: {:.4}%", percent_of(rs_size_without_hints - content_size, content_size));
    println!("  time/rank query [ns]: {:.2}", conf.universe_queries_measure(|index| rs.rank1(index)).as_nanos());

    println!(" select without hints (no extra space overhead):");
    println!("  time/select1 query [ns]: {:.2}", benchmark_select(conf, &rs).as_nanos());
    println!("  time/select0 query [ns]: {:.2}", benchmark_select0(conf, &rs).as_nanos());

    println!(" select with hints:");
    rs = rs.select1_hints();
    let rs_select1_size = rs.size_in_bytes() - rs_size_without_hints;
    rs = rs.select0_hints();
    let rs_select0_size = rs.size_in_bytes() - rs_size_without_hints - rs_select1_size;
    println!("  space overhead: select1 {:.4}% select0 {:.4}% (+rank overhead)",
        percent_of(rs_select1_size, content_size),
        percent_of(rs_select0_size, content_size));
    println!("  time/select1 query [ns]: {:.2}", benchmark_select(conf, &rs).as_nanos());
    println!("  time/select0 query [ns]: {:.2}", benchmark_select0(conf, &rs).as_nanos());
}