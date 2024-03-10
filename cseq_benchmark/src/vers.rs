use vers_vecs::{BitVec, RsVec};

pub fn benchmark_rank_select(conf: &super::Conf) {
    println!("vers:");
    let mut content = BitVec::from_zeros(conf.universe);
    let tester = conf.fill_data(|pos, value| if value { content.flip_bit(pos); });

    let rs = RsVec::from_bit_vec(content);
    tester.raport_rank("vers RsVec", rs.heap_size(), |index| rs.rank1(index));
    tester.raport_select1("vers RsVec", 0, |index| rs.select1(index));
    tester.raport_select0("vers RsVec", 0, |index| rs.select0(index));
}

