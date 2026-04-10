use crate::{Conf, Tester};
use mem_dbg::MemSize;
use sux::{
    bits::BitVec,
    rank_sel::{Rank9, SelectAdapt, SelectAdaptConst, SelectZeroAdapt, SelectZeroAdaptConst,
        default_target_inventory_span, DEFAULT_LOG2_WORDS_PER_SUBINVENTORY},
    traits::{BitVecOpsMut, Rank, SelectUnchecked, SelectZeroUnchecked},
};
use sux::rank_small;

pub fn build_bit_vec(conf: &'_ Conf) -> (BitVec, Tester<'_>) {
    let mut content: BitVec = BitVec::new(conf.universe);
    let tester = conf.fill_data(|bit_nr, value| content.set(bit_nr, value));
    (content, tester)
}

fn build_bit_vec_u64(conf: &'_ Conf) -> (BitVec<Vec<u64>>, Tester<'_>) {
    let mut content: BitVec<Vec<u64>> = BitVec::new(conf.universe);
    let tester = conf.fill_data(|bit_nr, value| content.set(bit_nr, value));
    (content, tester)
}

#[cfg(not(target_pointer_width = "64"))]
fn build_bit_vec_u32(conf: &'_ Conf) -> (BitVec<Vec<u32>>, Tester<'_>) {
    let mut content: BitVec<Vec<u32>> = BitVec::new(conf.universe);
    let tester = conf.fill_data(|bit_nr, value| content.set(bit_nr, value));
    (content, tester)
}

pub fn benchmark_rank9(conf: &Conf) {
    println!("sux Rank9:");
    let (content, tester) = build_bit_vec_u64(conf);
    let rs = Rank9::new(content);
    tester.raport_rank("sux Rank9", rs.mem_size(Default::default()),
        |index| rs.rank(index));
}

#[cfg(target_pointer_width = "64")]
pub fn benchmark_rank_small_u64_2(conf: &Conf) {
    println!("sux RankSmall[u64:2]:");
    let (content, tester) = build_bit_vec_u64(conf);
    let rs = rank_small![u64: 2; content];
    tester.raport_rank("sux RankSmall[u64:2]", rs.mem_size(Default::default()),
        |index| rs.rank(index));
}

#[cfg(target_pointer_width = "64")]
pub fn benchmark_rank_small_u64_3(conf: &Conf) {
    println!("sux RankSmall[u64:3]:");
    let (content, tester) = build_bit_vec_u64(conf);
    let rs = rank_small![u64: 3; content];
    tester.raport_rank("sux RankSmall[u64:3]", rs.mem_size(Default::default()),
        |index| rs.rank(index));
}

#[cfg(not(target_pointer_width = "64"))]
pub fn benchmark_rank_small_u32_3(conf: &Conf) {
    println!("sux RankSmall[u32:3]:");
    let (content, tester) = build_bit_vec_u32(conf);
    let rs = rank_small![u32: 3; content];
    tester.raport_rank("sux RankSmall[u32:3]", rs.mem_size(Default::default()),
        |index| rs.rank(index));
}

#[cfg(not(target_pointer_width = "64"))]
pub fn benchmark_rank_small_u32_4(conf: &Conf) {
    println!("sux RankSmall[u32:4]:");
    let (content, tester) = build_bit_vec_u32(conf);
    let rs = rank_small![u32: 4; content];
    tester.raport_rank("sux RankSmall[u32:4]", rs.mem_size(Default::default()),
        |index| rs.rank(index));
}

pub fn benchmark_select_adapt(conf: &Conf) {
    println!("sux SelectAdapt:");

    let (mut content, tester) = build_bit_vec(conf);
    let content_size = content.mem_size(Default::default());

    let rs = SelectAdapt::with_span(content,
        default_target_inventory_span(DEFAULT_LOG2_WORDS_PER_SUBINVENTORY),
        DEFAULT_LOG2_WORDS_PER_SUBINVENTORY);
    tester.raport_select1(
        "sux SelectAdapt",
        rs.mem_size(Default::default()) - content_size,
        |rank| unsafe { rs.select_unchecked(rank) },
    );

    content = rs.into_inner();
    let rs = SelectZeroAdapt::with_span(content,
        default_target_inventory_span(DEFAULT_LOG2_WORDS_PER_SUBINVENTORY),
        DEFAULT_LOG2_WORDS_PER_SUBINVENTORY);
    tester.raport_select0(
        "sux SelectAdapt",
        rs.mem_size(Default::default()) - content_size,
        |rank| unsafe { rs.select_zero_unchecked(rank) },
    );
}

pub fn benchmark_select_adapt_p1(conf: &Conf) {
    println!("sux SelectAdapt (sparser):");

    let (mut content, tester) = build_bit_vec(conf);
    let content_size = content.mem_size(Default::default());

    let rs = SelectAdapt::with_span(content,
        default_target_inventory_span(DEFAULT_LOG2_WORDS_PER_SUBINVENTORY + 1),
        DEFAULT_LOG2_WORDS_PER_SUBINVENTORY);
    tester.raport_select1(
        "sux SelectAdapt (sparser)",
        rs.mem_size(Default::default()) - content_size,
        |rank| unsafe { rs.select_unchecked(rank) },
    );

    content = rs.into_inner();
    let rs = SelectZeroAdapt::with_span(content,
        default_target_inventory_span(DEFAULT_LOG2_WORDS_PER_SUBINVENTORY + 1),
        DEFAULT_LOG2_WORDS_PER_SUBINVENTORY);
    tester.raport_select0(
        "sux SelectAdapt (sparser)",
        rs.mem_size(Default::default()) - content_size,
        |rank| unsafe { rs.select_zero_unchecked(rank) },
    );
}

pub fn benchmark_select_adapt_p2(conf: &Conf) {
    println!("sux SelectAdapt (sparsest):");

    let (mut content, tester) = build_bit_vec(conf);
    let content_size = content.mem_size(Default::default());

    let rs = SelectAdapt::with_span(content,
        default_target_inventory_span(DEFAULT_LOG2_WORDS_PER_SUBINVENTORY + 2),
        DEFAULT_LOG2_WORDS_PER_SUBINVENTORY);
    tester.raport_select1(
        "sux SelectAdapt (sparsest)",
        rs.mem_size(Default::default()) - content_size,
        |rank| unsafe { rs.select_unchecked(rank) },
    );

    content = rs.into_inner();
    let rs = SelectZeroAdapt::with_span(content,
        default_target_inventory_span(DEFAULT_LOG2_WORDS_PER_SUBINVENTORY + 2),
        DEFAULT_LOG2_WORDS_PER_SUBINVENTORY);
    tester.raport_select0(
        "sux SelectAdapt (sparsest)",
        rs.mem_size(Default::default()) - content_size,
        |rank| unsafe { rs.select_zero_unchecked(rank) },
    );
}

pub fn benchmark_select_adapt_const(conf: &Conf) {
    println!("sux SelectAdaptConst:");

    let (mut content, tester) = build_bit_vec(conf);
    let content_size = content.mem_size(Default::default());

    let rs = SelectAdaptConst::<_, _>::new(content);
    //rs.mem_dbg(Default::default()).unwrap();
    tester.raport_select1(
        "sux SelectAdaptConst",
        rs.mem_size(Default::default()) - content_size,
        |rank| unsafe { rs.select_unchecked(rank) },
    );

    content = rs.into_inner();
    let rs = SelectZeroAdaptConst::<_, _>::new(content);
    //rs.mem_dbg(Default::default()).unwrap();
    tester.raport_select0(
        "sux SelectAdaptConst",
        rs.mem_size(Default::default()) - content_size,
        |rank| unsafe { rs.select_zero_unchecked(rank) },
    );
}
