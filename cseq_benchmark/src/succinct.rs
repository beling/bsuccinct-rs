use dyn_size_of::GetSize;
use succinct::{rank, BinSearchSelect, BitRankSupport, select::{Select1Support, Select0Support}, SpaceUsage};
use crate::{bitm::build_bit_vec, percent_of};

fn benchmark(conf: &super::Conf, method_name: &str, space_overhead: f64, rank: impl BitRankSupport) {
    conf.raport_rank(method_name, space_overhead, |index| rank.rank1(index as u64));
    println!(" select by binary search over ranks (no extra space overhead):");
    let select = BinSearchSelect::new(rank);
    conf.raport_select1(method_name, 0.0, |index| select.select1(index as u64));
    conf.raport_select0(method_name, 0.0, |index| select.select0(index as u64));
}

pub fn benchmark_rank9(conf: &super::Conf) {
    println!("succinct Rank9:");
    let content = build_bit_vec(conf);
    let rank = rank::Rank9::new(content.as_ref());
    benchmark(conf, "succinct Rank9", 
        percent_of(rank.total_bytes(), content.size_bytes()),
        rank);
}

pub fn benchmark_jacobson(conf: &super::Conf) {
    println!("succinct JacobsonRank:");
    let content = build_bit_vec(conf);
    let rank = rank::JacobsonRank::new(content.as_ref());
    benchmark(conf, "succinct JacobsonRank", 
        percent_of(rank.total_bytes(), content.size_bytes()),
        rank);
}