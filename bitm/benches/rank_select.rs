use criterion::{black_box, criterion_group, criterion_main, Criterion};
use bitm::{RankSelect101111, Rank};

pub fn rank(c: &mut Criterion) {
    let bitmap = vec![0x6A_21_55_79_10_90_32_F3; 16].into_boxed_slice();
    let r: RankSelect101111 = bitmap.into();

    c.bench_function("rank (checked)", |b| b.iter(|| r.rank(black_box(18*7))));
    c.bench_function("try_rank", |b| b.iter(|| r.try_rank(black_box(18*7))));
    c.bench_function("rank_unchecked", |b| b.iter(|| unsafe{r.rank_unchecked(black_box(18*7))}));
}

criterion_group!(rank_select, rank);
criterion_main!(rank_select);