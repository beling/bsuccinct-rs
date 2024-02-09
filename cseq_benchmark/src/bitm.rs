use bitm::{ArrayWithRankSelect101111, BinaryRankSearch, BitAccess, BitVec, CombinedSampling, Rank, Select, Select0};
use dyn_size_of::GetSize;
use crate::{percent_of, percent_of_diff, Conf};

pub fn build_bit_vec(conf: &Conf) -> Box<[u64]> {
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
    content
}

pub fn benchmark_rank_select(conf: &super::Conf) {
    println!("bitm bit vector:");
    let content = build_bit_vec(conf);

    let (rs, _) = ArrayWithRankSelect101111::<BinaryRankSearch, BinaryRankSearch>::build(content);
    //assert_eq!(ones, conf.num);

    conf.raport_rank("bitm ArrayWithRankSelect101111",
        percent_of_diff(rs.size_bytes(), rs.content.size_bytes()),
        |index| unsafe{rs.rank_unchecked(index)});
    //println!("  rank space overhead: {:.4}%", percent_of(rs.size_bytes()-rs.content.size_bytes(), rs.content.size_bytes()));
    //println!("  time/rank query [ns]: {:.2}", conf.universe_queries_measure(|index| unsafe{rs.rank_unchecked(index)}).as_nanos());
    //println!("  time/rank query [ns]: {:.2}", conf.universe_queries_measure(|index| rs.try_rank(index)).as_nanos());

    println!(" select by binary search over ranks (no extra space overhead):");
    conf.raport_select1("bitm ArrayWithRankSelect101111 binary search over ranks",
        0.0, |index| rs.select(index));
    conf.raport_select0("bitm ArrayWithRankSelect101111 binary search over ranks",
        0.0, |index| rs.select0(index));
    //println!("  time/select1 query [ns]: {:.2}", benchmark_select(conf, &rs).as_nanos());
    //println!("  time/select0 query [ns]: {:.2}", benchmark_select0(conf, &rs).as_nanos());

    let (rs, _) = ArrayWithRankSelect101111::<CombinedSampling, CombinedSampling>::build(rs.content);
    println!(" select by combined sampling:");
    conf.raport_select1("bitm ArrayWithRankSelect101111 combined sampling",
        percent_of(rs.select_support().size_bytes(), rs.content.size_bytes()),
        |index| rs.select(index));
    conf.raport_select0("bitm ArrayWithRankSelect101111 combined sampling",
        percent_of(rs.select0_support().size_bytes(), rs.content.size_bytes()),
        |index| rs.select0(index));
    /*println!("  space overhead: select1 {:.4}% select0 {:.4}% (+rank overhead)",
        percent_of(rs.select_support().size_bytes(), rs.content.size_bytes()),
        percent_of(rs.select0_support().size_bytes(), rs.content.size_bytes()));
    println!("  time/select1 query [ns]: {:.2}", benchmark_select(conf, &rs).as_nanos());
    println!("  time/select0 query [ns]: {:.2}", benchmark_select0(conf, &rs).as_nanos());*/
}

