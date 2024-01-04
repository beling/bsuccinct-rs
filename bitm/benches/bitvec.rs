use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use bitm::BitAccess;

pub fn get_bits(c: &mut Criterion) {
    let v = [0x6A_21_55_79_10_90_32_F3; 4];

    let mut group = c.benchmark_group("get_bits (checked)");
    for size in [20, 40, 60].iter() {
        //group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| v.get_bits(black_box(30), size))
        });
    }
    group.finish();
}

pub fn get_bits_unchecked(c: &mut Criterion) {
    let v = [0x6A_21_55_79_10_90_32_F3; 4];

    let mut group = c.benchmark_group("get_bits_unchecked");
    for size in [20, 40, 60].iter() {
        //group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| unsafe{ v.get_bits_unchecked(black_box(30), size) })
        });
    }
    group.finish();
}

criterion_group!(bit_vector, get_bits, get_bits_unchecked);
criterion_main!(bit_vector);