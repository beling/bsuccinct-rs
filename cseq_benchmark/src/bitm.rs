use aligned_vec::ABox;
use bitm::{RankSelect101111, BinaryRankSearch, BitAccess, BitVec, CombinedSampling, Rank, Select, Select0};
use dyn_size_of::GetSize;
use crate::{percent_of, percent_of_diff, Conf, Tester};

/*pub fn build_bit_vec(conf: &Conf) -> Box<[u64]> {
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
}*/

/*pub fn build_bit_vec(conf: &Conf) -> (Box<[u64]>, Tester) {
    let mut content = Box::with_zeroed_bits(conf.universe);
    let tester = conf.rand_data(|bit_nr, value| if value {content.init_bit(bit_nr, value)});
    (content, tester)
}*/

pub fn build_bit_vec(conf: &Conf) -> (ABox<[u64]>, Tester) {
    let mut content =  ABox::with_zeroed_bits(conf.universe);
    //let mut content = AVec::from_iter(64, (0..ceiling_div(conf.universe, 64)).map(|_| 0)).into_boxed_slice();
    let tester = conf.rand_data(|bit_nr, value| if value {content.init_bit(bit_nr, value)});
    (content, tester)
}

pub fn benchmark_rank_select(conf: &super::Conf) {
    println!("bitm RankSelect101111:");
    let (content, tester) = build_bit_vec(conf);

    let (rs, _) = RankSelect101111::<BinaryRankSearch, BinaryRankSearch, _>::build(content);
    //assert_eq!(ones, conf.num);

    tester.raport_rank("bitm RankSelect101111",
        percent_of_diff(rs.size_bytes(), rs.content.size_bytes()),
        |index| unsafe{rs.rank_unchecked(index)});

    /*println!(" checked select by binary search over ranks (no extra space overhead):");
    tester.raport_select1("bitm RankSelect101111 binary search over ranks checked",
            0.0, |index| rs.select(index));
    tester.raport_select0("bitm RankSelect101111 binary search over ranks checked",
            0.0, |index| rs.select0(index));*/

    println!(" select by binary search over ranks (no extra space overhead):");
    tester.raport_select1("bitm RankSelect101111 binary search over ranks",
            0.0, |index| unsafe{rs.select_unchecked(index)});
    tester.raport_select0("bitm RankSelect101111 binary search over ranks",
            0.0, |index| unsafe{rs.select0_unchecked(index)});

    let (rs, _) = RankSelect101111::<CombinedSampling, CombinedSampling, _>::build(rs.content);

    /*println!(" checked select by combined sampling:");
    tester.raport_select1("bitm RankSelect101111 combined sampling checked",
            percent_of(rs.select_support().size_bytes(), rs.content.size_bytes()),
            |index| rs.select(index));
    tester.raport_select0("bitm RankSelect101111 combined sampling checked",
            percent_of(rs.select0_support().size_bytes(), rs.content.size_bytes()),
            |index| rs.select0(index));*/

    println!(" select by combined sampling:");
    tester.raport_select1("bitm RankSelect101111 combined sampling",
        percent_of(rs.select_support().size_bytes(), rs.content.size_bytes()),
        |index| unsafe{rs.select_unchecked(index)});
    tester.raport_select0("bitm RankSelect101111 combined sampling",
        percent_of(rs.select0_support().size_bytes(), rs.content.size_bytes()),
        |index| unsafe{rs.select0_unchecked(index)});
}



/*pub fn benchmark_rank_select(conf: &super::Conf) {
    println!("bitm bit vector:");
    let content = build_bit_vec(conf);

    let (rs, _) = RankSelect101111::<BinaryRankSearch, BinaryRankSearch>::build(content);
    //assert_eq!(ones, conf.num);

    /*conf.raport_rank("bitm RankSelect101111",
        percent_of_diff(rs.size_bytes(), rs.content.size_bytes()),
        |index| unsafe{rs.rank_unchecked(index)});

    println!(" select by binary search over ranks (no extra space overhead):");
        conf.raport_select1("bitm RankSelect101111 binary search over ranks",
            0.0, |index| unsafe{rs.select_unchecked(index)});
        conf.raport_select0("bitm RankSelect101111 binary search over ranks",
            0.0, |index| unsafe{rs.select0_unchecked(index)});*/

    //type Cs = ConstCombinedSamplingDensity<10>;
    //type Cs = AdaptiveCombinedSamplingDensity<14>;
    let (rs, _) = RankSelect101111::<CombinedSampling, CombinedSampling>::build(rs.content);

    /*println!(" checked select by combined sampling:");
        conf.raport_select1("bitm checked RankSelect101111 combined sampling",
            percent_of(rs.select_support().size_bytes(), rs.content.size_bytes()),
            |index| rs.select(index));
        conf.raport_select0("bitm checked RankSelect101111 combined sampling",
            percent_of(rs.select0_support().size_bytes(), rs.content.size_bytes()),
            |index| rs.select0(index));*/

    println!(" select by combined sampling:");
    conf.raport_select1("bitm RankSelect101111 combined sampling",
        percent_of(rs.select_support().size_bytes(), rs.content.size_bytes()),
        |index| unsafe{rs.select_unchecked(index)});
    conf.raport_select0("bitm RankSelect101111 combined sampling",
        percent_of(rs.select0_support().size_bytes(), rs.content.size_bytes()),
        |index| unsafe{rs.select0_unchecked(index)});
}*/

