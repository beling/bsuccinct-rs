use sucds::{bit_vectors::{Rank9Sel, BitVector, Rank, Select}, Serializable};
use crate::Conf;

pub fn benchmark_rank9_select(conf: &Conf) {
    println!("sucds Rank9Sel:");

    let mut content = BitVector::from_bit(false, conf.universe);
    let tester = conf.fill_data(|pos, value|
        if value { content.set_bit(pos, value).unwrap(); }
    );

    let mut rs = Rank9Sel::new(content);
    let rs_size_without_hints = rs.size_in_bytes();
    tester.raport_rank("sucds Rank9Sel", rs_size_without_hints,
        |index| rs.rank1(index));

    println!(" select without hints (no extra space overhead):");
    tester.raport_select1("sucds Rank9Sel", 0, |index| rs.select1(index));
    tester.raport_select0("sucds Rank9Sel", 0, |index| rs.select0(index));

    println!(" select with hints:");
    rs = rs.select1_hints();
    let rs_select1_size = rs.size_in_bytes() - rs_size_without_hints;
    rs = rs.select0_hints();
    let rs_select0_size = rs.size_in_bytes() - rs_size_without_hints - rs_select1_size;
    tester.raport_select1("sucds Rank9Sel + hints", rs_select1_size,
        |index| rs.select1(index));
    tester.raport_select0("sucds Rank9Sel + hints", rs_select0_size,
        |index| rs.select0(index));
}