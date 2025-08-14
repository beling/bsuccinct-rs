use succinct::{rank, select::{Select0Support, Select1Support}, BinSearchSelect, BitRankSupport, BitVecMut, BitVector, SpaceUsage};
use crate::{Conf, Tester};

pub fn build_bit_vec(conf: &'_ Conf) -> (BitVector::<u64>, Tester<'_>) {
    let mut content = BitVector::with_fill(conf.universe as u64, false);
    let tester = conf.fill_data(|bit_nr, value| {content.set_bit(bit_nr as u64, value)});
    (content, tester)
}

fn benchmark(mut tester: super::Tester, method_name: &str, rank: impl BitRankSupport+SpaceUsage) {
    tester.rank_includes_current = true;
    tester.raport_rank(method_name, rank.total_bytes(),
        |index| rank.rank1(index as u64) as usize);
    println!(" select by binary search over ranks (no extra space overhead):");
    let select = BinSearchSelect::new(rank);
    tester.raport_select1(method_name, 0, |index| select.select1(index as u64).map(|v| v as usize));
    tester.raport_select0(method_name, 0, |index| select.select0(index as u64).map(|v| v as usize));
}

pub fn benchmark_rank9(conf: &super::Conf) {
    println!("succinct Rank9:");
    let (content, tester) = build_bit_vec(conf);
    benchmark(tester, "succinct Rank9", rank::Rank9::new(content));
}

pub fn benchmark_jacobson(conf: &super::Conf) {
    println!("succinct JacobsonRank:");
    let (content, tester) = build_bit_vec(conf);
    benchmark(tester, "succinct JacobsonRank", rank::JacobsonRank::new(content));
}