use core::hint::black_box;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use memchr_stuff::memchr_new::memrchr;
use memchr_stuff::memchr_old::memrchr_old;
use std::time::Duration;

fn bench_memrchr(c: &mut Criterion) {
    let mut group = c.benchmark_group("memrchr (REVERSED)");

    let sizes = [16usize, 64, 256, 1024, 8 * 1024, 64 * 1024];

    for size in sizes {
        let mut data = vec![0u8; size];
        data[size / 2] = 1;
        data[size - 1] = 1;

        let needle = 1u8;

        group.bench_with_input(BenchmarkId::new("old", size), &data, |b, data| {
            b.iter(|| black_box(memrchr_old(black_box(needle), black_box(data))))
        });

        group.bench_with_input(BenchmarkId::new("new", size), &data, |b, data| {
            b.iter(|| black_box(memrchr(black_box(needle), black_box(data))))
        });
    }

    group.finish();
}

fn bench_memchr(c: &mut Criterion) {
    let mut group = c.benchmark_group("memchr");

    let sizes = [16usize, 64, 256, 1024, 8 * 1024, 64 * 1024];

    for size in sizes {
        let mut data = vec![0u8; size];
        data[size / 2] = 1;
        data[size - 1] = 1;

        let needle = 1u8;

        group.bench_with_input(BenchmarkId::new("old", size), &data, |b, data| {
            b.iter(|| {
                black_box(memchr_stuff::memchr_old::memchr(
                    black_box(needle),
                    black_box(data),
                ))
            })
        });

        group.bench_with_input(BenchmarkId::new("new", size), &data, |b, data| {
            b.iter(|| {
                black_box(memchr_stuff::memchr_new::memchr(
                    black_box(needle),
                    black_box(data),
                ))
            })
        });
    }

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10))
        .sample_size(200);
    targets = bench_memrchr, bench_memchr
);
criterion_main!(benches);
