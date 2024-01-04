use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use ph::fmph;

pub fn get(c: &mut Criterion) {
    let keys = (0u16..2048).step_by(2).collect::<Vec<_>>();
    let f = fmph::Function::from(keys);
    let mut group = c.benchmark_group("get");
    for key in [2, 1032, 2040].iter() {
        //group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(key), key, |b, &key| {
            b.iter(|| f.get(&key))
        });
    }
    group.finish();
}

criterion_group!(fmph, get);
criterion_main!(fmph);