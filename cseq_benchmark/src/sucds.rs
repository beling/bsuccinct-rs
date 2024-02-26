use sucds::{bit_vectors::{Rank9Sel, BitVector, Rank, Select}, Serializable};
use crate::{percent_of, percent_of_diff, Conf};

pub fn benchmark_rank9_select(conf: &Conf) {
    println!("sucds Rank9Sel:");

    let mut content = BitVector::from_bit(false, conf.universe);
    let tester = conf.rand_data(|pos, value|
        if value { content.set_bit(pos, value).unwrap(); }
    );

    let content_size = content.size_in_bytes();
    let mut rs = Rank9Sel::new(content);
    let rs_size_without_hints = rs.size_in_bytes();
    tester.raport_rank("sucds Rank9Sel",
        percent_of_diff(rs_size_without_hints, content_size),
        |index| rs.rank1(index));

    println!(" select without hints (no extra space overhead):");
    tester.raport_select1("sucds Rank9Sel", 0.0, |index| rs.select1(index));
    tester.raport_select0("sucds Rank9Sel", 0.0, |index| rs.select0(index));

    println!(" select with hints:");
    rs = rs.select1_hints();
    let rs_select1_size = rs.size_in_bytes() - rs_size_without_hints;
    rs = rs.select0_hints();
    let rs_select0_size = rs.size_in_bytes() - rs_size_without_hints - rs_select1_size;
    tester.raport_select1("sucds Rank9Sel + hints",
        percent_of(rs_select1_size, content_size),
        |index| rs.select1(index));
    tester.raport_select0("sucds Rank9Sel + hints",
        percent_of(rs_select0_size, content_size),
        |index| rs.select0(index));
}