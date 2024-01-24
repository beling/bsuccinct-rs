use bitm::{ArrayWithRankSelect101111, BinaryRankSearch, BitAccess, BitVec, CombinedSampling, Rank, Select, Select0};
use crate::UnitPrefix;

fn benchmark_select(conf: &super::Conf, rs: &impl Select) -> f64 {
    conf.num_sampling_measure(1000000, |index| rs.select(index))
}

fn benchmark_select0(conf: &super::Conf, rs: &impl Select0) -> f64 {
    conf.num_complement_sampling_measure(1000000, |index| rs.select0(index))
}

pub fn benchmark_rank_select(conf: &super::Conf) {
    let inserted_values = conf.num * 2 < conf.universe;
    let (mut content, mut to_insert): (Box::<[u64]>, _) = if inserted_values {
        (Box::with_zeroed_bits(conf.universe), conf.num)
    } else {
        (Box::with_filled_bits(conf.universe), conf.universe - conf.num)
    };
    let mut gen = conf.rand_gen();
    while to_insert > 0 {
        let bit_nr = gen.get() as usize % conf.universe;
        if content.get_bit(bit_nr) != inserted_values {
            content.set_bit_to(bit_nr, inserted_values);
            to_insert -= 1;
        }
    }

    let (rs, _) = ArrayWithRankSelect101111::<BinaryRankSearch, BinaryRankSearch>::build(content);
    //assert_eq!(ones, conf.num);

    println!("time/rank [ns]: {:.2}", conf.universe_sampling_measure(1000000, |index| rs.rank(index)).nanos());

    println!("time/select by binary search over ranks [ns]: {:.2}", benchmark_select(conf, &rs).nanos());
    println!("time/select0 by binary search over ranks [ns]: {:.2}", benchmark_select0(conf, &rs).nanos());

    let (rs, _) = ArrayWithRankSelect101111::<CombinedSampling, CombinedSampling>::build(rs.content);
    println!("time/select by combined sampling [ns]: {:.2}", benchmark_select(conf, &rs).nanos());
    println!("time/select0 by combined sampling [ns]: {:.2}", benchmark_select0(conf, &rs).nanos());
}