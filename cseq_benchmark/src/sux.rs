use mem_dbg::MemSize;
use sux::{bits::BitVec, rank_sel::{SelectFixed1, SelectFixed2, SelectZeroFixed1, SelectZeroFixed2}, traits::{ConvertTo, Select, SelectZero}};
use crate::{percent_of_diff, Conf, Tester};

/*fn build_bit_vec(conf: &Conf) -> BitVec {
    let inserted_values = conf.num * 2 < conf.universe;
    let mut content = BitVec::new(conf.universe);
    let mut to_insert = if inserted_values { conf.num } else { content.flip(); conf.universe - conf.num };
    let mut gen = conf.rand_gen();
    while to_insert > 0 {
        let bit_nr = gen.get() as usize % conf.universe;
        if content.get(bit_nr) != inserted_values {
            content.set(bit_nr, inserted_values);
            to_insert -= 1;
        }
    }
    content
}*/

pub fn build_bit_vec(conf: &Conf) -> (BitVec, Tester) {
    let mut content = BitVec::new(conf.universe);
    let tester = conf.rand_data(|bit_nr, value| content.set(bit_nr, value));
    (content, tester)
}

pub fn benchmark_select_fixed2(conf: &Conf) {
    println!("sux SelectFixed2:");

    let (mut content, tester) = build_bit_vec(conf);
    let content_size = content.mem_size(Default::default());

    let rs = SelectFixed2::<_, _, 10, 2>::new(content);
    //rs.mem_dbg(Default::default()).unwrap();
    tester.raport_select1("sux SelectFixed2",
        percent_of_diff(rs.mem_size(Default::default()), content_size),
        |rank| unsafe { rs.select_unchecked(rank) });

    content = rs.convert_to().unwrap();
    let rs = SelectZeroFixed2::<_, _, 10, 2>::new(content);
    //rs.mem_dbg(Default::default()).unwrap();
    tester.raport_select0("sux SelectFixed2",
            percent_of_diff(rs.mem_size(Default::default()), content_size),
            |rank| unsafe { rs.select_zero_unchecked(rank) });
}

pub fn benchmark_select_fixed1(conf: &Conf) {
    println!("sux SelectFixed1:");

    let (mut content, tester) = build_bit_vec(conf);
    let content_size = content.mem_size(Default::default());

    let rs = SelectFixed1::<_, _, 8>::new(content, conf.num);
    //rs.mem_dbg(Default::default()).unwrap();
    tester.raport_select1("sux SelectFixed1",
        percent_of_diff(rs.mem_size(Default::default()), content_size),
        |rank| unsafe { rs.select_unchecked(rank) });

    content = rs.convert_to().unwrap();
    let rs = SelectZeroFixed1::<_, _, 8>::new(content);
    //rs.mem_dbg(Default::default()).unwrap();
    tester.raport_select0("sux SelectFixed1",
            percent_of_diff(rs.mem_size(Default::default()), content_size),
            |rank| unsafe { rs.select_zero_unchecked(rank) });
}