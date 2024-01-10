use iai_callgrind::{black_box, main, library_benchmark_group, library_benchmark};
use bitm::BitAccess;

#[library_benchmark]
#[bench::short(&[0x6A_21_55_79_10_90_32_F3; 2], 10, 30)]
#[bench::long(&[0x6A_21_55_79_10_90_32_F3; 2], 10, 60)]
fn get_bits(tab: &[u64], index: usize, v_size: u8) -> u64 {
    black_box(tab.get_bits(bitm::Bits::index_len(index, v_size)))
}

#[library_benchmark]
#[bench::short(&[0x6A_21_55_79_10_90_32_F3; 2], 10, 30)]
#[bench::long(&[0x6A_21_55_79_10_90_32_F3; 2], 10, 60)]
fn get_bits_unchecked(tab: &[u64], index: usize, v_size: u8) -> u64 {
    black_box(unsafe{tab.get_bits_unchecked(bitm::Bits::index_len(index, v_size))})
}

#[library_benchmark]
#[bench::short(&mut [0x6A_21_55_79_10_90_32_F3; 2], 10, 0xaa_aa_aa, 30)]
#[bench::long(&mut [0x6A_21_55_79_10_90_32_F3; 2], 10, 0xaa_aa_aa_aa_aa_aa, 60)]
fn set_bits(tab: &mut [u64], index: usize, v: u64, v_size: u8) {
    tab.set_bits(index, v, v_size)
}

#[library_benchmark]
#[bench::short(&mut [0x6A_21_55_79_10_90_32_F3; 2], 10, 0xaa_aa_aa, 30)]
#[bench::long(&mut [0x6A_21_55_79_10_90_32_F3; 2], 10, 0xaa_aa_aa_aa_aa_aa, 60)]
fn set_bits_unchecked(tab: &mut [u64], index: usize, v: u64, v_size: u8) {
    unsafe { tab.set_bits_unchecked(index, v, v_size) }
}

#[library_benchmark]
#[bench::zero(&mut [0x6A_21_55_79_10_90_32_F3; 2], 10, false)]
#[bench::one(&mut [0x6A_21_55_79_10_90_32_F3; 2], 10, true)]
fn set_bit_to(tab: &mut [u64], index: usize, value: bool) {
    tab.set_bit_to(index, value)
}

#[library_benchmark]
#[bench::zero(&mut [0x6A_21_55_79_10_90_32_F3; 2], 10, false)]
#[bench::one(&mut [0x6A_21_55_79_10_90_32_F3; 2], 10, true)]
fn set_bit_to_unchecked(tab: &mut [u64], index: usize, value: bool) {
    unsafe{tab.set_bit_to_unchecked(index, value)}
}

#[library_benchmark]
#[bench::short(&mut [0x6A_21_55_79_10_90_32_F3; 2], 10, 30)]
#[bench::long(&mut [0x6A_21_55_79_10_90_32_F3; 2], 10, 60)]
fn conditionally_change_bits(tab: &mut [u64], index: usize, v_size: u8) -> u64 {
    black_box(tab.conditionally_change_bits(|v| (v>2).then(|| v-1), index, v_size))
}

library_benchmark_group!(
    name = bitvec;
    benchmarks =
        get_bits, get_bits_unchecked,
        set_bits, set_bits_unchecked,
        set_bit_to, set_bit_to_unchecked,
        conditionally_change_bits
);

main!(library_benchmark_groups = bitvec);