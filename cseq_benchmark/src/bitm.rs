use aligned_vec::ABox;
use bitm::{RankSelect101111, BinaryRankSearch, BitAccess, BitVec, CombinedSampling, Rank, Select, Select0};
use dyn_size_of::GetSize;
use crate::{percent_of, percent_of_diff, Conf, Tester};

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

    println!(" select by binary search over ranks (no extra space overhead):");
    tester.raport_select1("bitm RankSelect101111 binary search over ranks",
            0.0, |index| unsafe{rs.select_unchecked(index)});
    tester.raport_select0("bitm RankSelect101111 binary search over ranks",
            0.0, |index| unsafe{rs.select0_unchecked(index)});

    let (rs, _) = RankSelect101111::<CombinedSampling, CombinedSampling, _>::build(rs.content);

    println!(" select by combined sampling:");
    tester.raport_select1("bitm RankSelect101111 combined sampling",
        percent_of(rs.select_support().size_bytes(), rs.content.size_bytes()),
        |index| unsafe{rs.select_unchecked(index)});
    tester.raport_select0("bitm RankSelect101111 combined sampling",
        percent_of(rs.select0_support().size_bytes(), rs.content.size_bytes()),
        |index| unsafe{rs.select0_unchecked(index)});
}

