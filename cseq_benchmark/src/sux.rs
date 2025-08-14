use mem_dbg::MemSize;
use sux::{bits::BitVec, rank_sel::{SelectAdapt, SelectAdaptConst, SelectZeroAdapt, SelectZeroAdaptConst},
    traits::{SelectUnchecked, SelectZeroUnchecked}};
use crate::{Conf, Tester};

pub fn build_bit_vec(conf: &'_ Conf) -> (BitVec, Tester<'_>) {
    let mut content = BitVec::new(conf.universe);
    let tester = conf.fill_data(|bit_nr, value| content.set(bit_nr, value));
    (content, tester)
}

pub fn benchmark_select_adapt(conf: &Conf) {
    println!("sux SelectAdapt:");

    let (mut content, tester) = build_bit_vec(conf);
    let content_size = content.mem_size(Default::default());

    let rs = SelectAdapt::new(content, 3);
    //rs.mem_dbg(Default::default()).unwrap();
    tester.raport_select1("sux SelectAdapt",
        rs.mem_size(Default::default()) - content_size,
        |rank| unsafe { rs.select_unchecked(rank) });

    content = rs.into_inner();
    let rs = SelectZeroAdapt::new(content, 3);
    //rs.mem_dbg(Default::default()).unwrap();
    tester.raport_select0("sux SelectAdapt",
            rs.mem_size(Default::default()) - content_size,
            |rank| unsafe { rs.select_zero_unchecked(rank) });
}

pub fn benchmark_select_adapt_const(conf: &Conf) {
    println!("sux SelectAdaptConst:");

    let (mut content, tester) = build_bit_vec(conf);
    let content_size = content.mem_size(Default::default());

    let rs = SelectAdaptConst::<_,_>::new(content);
    //rs.mem_dbg(Default::default()).unwrap();
    tester.raport_select1("sux SelectAdaptConst",
        rs.mem_size(Default::default()) - content_size,
        |rank| unsafe { rs.select_unchecked(rank) });

    content = rs.into_inner();
    let rs = SelectZeroAdaptConst::<_,_>::new(content);
    //rs.mem_dbg(Default::default()).unwrap();
    tester.raport_select0("sux SelectAdaptConst",
            rs.mem_size(Default::default()) - content_size,
            |rank| unsafe { rs.select_zero_unchecked(rank) });
}