use criterion::{black_box, criterion_group, criterion_main, Criterion};
use bitm::BitAccess;

pub fn get_bits(c: &mut Criterion) {
    let v = [0x6A_21_55_79_10_90_32_F3; 4];
    c.bench_function("get_bits(30, 20)", |b| b.iter(|| v.get_bits(black_box(30), black_box(20))));
    c.bench_function("get_bits(30, 40)", |b| b.iter(|| v.get_bits(black_box(30), black_box(40))));
    c.bench_function("get_bits(30, 60)", |b| b.iter(|| v.get_bits(black_box(30), black_box(60))));
}

pub fn get_bits_unchecked(c: &mut Criterion) {
    let v = [0x6A_21_55_79_10_90_32_F3; 4];
    unsafe{
    c.bench_function("get_bits_unchecked(30, 20)", |b| b.iter(|| v.get_bits_unchecked(black_box(30), black_box(20))));
    c.bench_function("get_bits_unchecked(30, 40)", |b| b.iter(|| v.get_bits_unchecked(black_box(30), black_box(40))));
    c.bench_function("get_bits_unchecked(30, 60)", |b| b.iter(|| v.get_bits_unchecked(black_box(30), black_box(60))));
    }
}

criterion_group!(bit_vector, get_bits, get_bits_unchecked);
criterion_main!(bit_vector);