use iai_callgrind::{black_box, main, library_benchmark_group, library_benchmark};
use bitm::BitAccess;

#[library_benchmark]
#[bench::short(&[0x6A_21_55_79_10_90_32_F3; 4], 10, 30)]
#[bench::long(&[0x6A_21_55_79_10_90_32_F3; 4], 10, 60)]
fn get_bits(tab: &[u64], index: usize, v_size: u8) -> u64 {
    black_box(tab.get_bits(index, v_size))
}

#[library_benchmark]
#[bench::short(&[0x6A_21_55_79_10_90_32_F3; 4], 10, 30)]
#[bench::long(&[0x6A_21_55_79_10_90_32_F3; 4], 10, 60)]
fn get_bits_unchecked(tab: &[u64], index: usize, v_size: u8) -> u64 {
    black_box(unsafe{tab.get_bits_unchecked(index, v_size)})
}

library_benchmark_group!(
    name = bitvec;
    benchmarks = get_bits, get_bits_unchecked
);

main!(library_benchmark_groups = bitvec);