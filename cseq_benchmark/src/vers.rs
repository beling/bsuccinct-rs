use vers_vecs::{BitVec, RsVec};
use crate::percent_of_diff;

pub fn benchmark_rank_select(conf: &super::Conf) {
    println!("vers:");

    let inserted_values = conf.num * 2 < conf.universe;
    let (mut content, mut to_insert) = if inserted_values {
        (BitVec::from_zeros(conf.universe), conf.num)
    } else {
        (BitVec::from_ones(conf.universe), conf.universe - conf.num)
    };
    let mut gen = conf.rand_gen();
    while to_insert > 0 {
        let bit_nr = gen.get() as usize % conf.universe;
        if content.is_bit_set_unchecked(bit_nr) != inserted_values {
            content.flip_bit(bit_nr);
            to_insert -= 1;
        }
    }

    let raw_size = content.heap_size();
    let rs = RsVec::from_bit_vec(content);
    conf.raport_rank("vers RsVec", percent_of_diff(rs.heap_size(), raw_size), |index| rs.rank1(index));
    conf.raport_select1("vers RsVec", 0.0, |index| rs.select1(index));
    conf.raport_select0("vers RsVec", 0.0, |index| rs.select0(index));

    /*println!(" rank and select space overhead: {:.4}%", percent_of(rs.heap_size()-raw_size, raw_size));
    println!(" time/rank query [ns]: {:.2}", conf.universe_queries_measure(|index| rs.rank1(index)).as_nanos());
    println!(" time/select1 query [ns]: {:.2}", conf.num_queries_measure(|index| rs.select1(index)).as_nanos());
    println!(" time/select0 query [ns]: {:.2}", conf.num_complement_queries_measure(|index| rs.select0(index)).as_nanos());*/
}

