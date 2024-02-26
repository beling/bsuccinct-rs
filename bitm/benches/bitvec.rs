use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use bitm::BitAccess;

pub fn get_bits(c: &mut Criterion) {
    let v = [0x6A_21_55_79_10_90_32_F3; 4];

    c.bench_function("get_bit (checked)", |b| b.iter(|| v.get_bit(black_box(30))));
    c.bench_function("get_bit (unchecked)", |b| b.iter(|| unsafe{v.get_bit_unchecked(black_box(30))}));

    let mut group = c.benchmark_group("get_bits (checked)");
    for size in [20, 40, 60].iter() {
        //group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| v.get_bits(black_box(30), size))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("get_bits_unchecked");
    for size in [20, 40, 60].iter() {
        //group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| unsafe{ v.get_bits_unchecked(black_box(30), size) })
        });
    }
    group.finish();
}

pub fn set_bits(c: &mut Criterion) {
    let mut v = [0x6A_21_55_79_10_90_32_F3; 4];

    c.bench_function("set_bit (checked)", |b| b.iter(|| v.set_bit(black_box(30))));
    c.bench_function("set_bit (unchecked)", |b| b.iter(|| unsafe{v.set_bit_unchecked(black_box(30))}));

    let mut group = c.benchmark_group("set_bit_to (checked)");
    for value in [false, true] {
        group.bench_with_input(BenchmarkId::from_parameter(value), &value, |b, value| {
            b.iter(|| v.set_bit_to(black_box(30), black_box(*value)))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("set_bit_to (unchecked)");
    for value in [false, true] {
        group.bench_with_input(BenchmarkId::from_parameter(value), &value, |b, value| {
            b.iter(|| unsafe{v.set_bit_to_unchecked(black_box(30), black_box(*value))})
        });
    }
    group.finish();

    c.bench_function("set_bit_to (checked)", |b| b.iter(|| v.set_bit_to(black_box(30), true)));
    c.bench_function("set_bit_to (unchecked)", |b| b.iter(|| unsafe{v.set_bit_unchecked(black_box(30))}));

    let mut group = c.benchmark_group("set_bits (checked)");
    for size in [20, 40, 60].iter() {
        //group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| v.set_bits(black_box(30), black_box(0), size))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("set_bits_unchecked");
    for size in [20, 40, 60].iter() {
        //group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| unsafe{v.set_bits_unchecked(black_box(30), black_box(0), size)})
        });
    }
    group.finish();
}

criterion_group!(bit_vector, get_bits, set_bits);
criterion_main!(bit_vector);