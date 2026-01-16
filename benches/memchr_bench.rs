use core::hint::black_box;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use memchr_stuff::memchr_new;
use memchr_stuff::memchr_old;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::Duration;

const RANDOM_SEED: u64 = 4269; //change as needed

fn create_test_arrays() -> Vec<usize> {
    // no point testing 16 really. doesnt get to the GOOD part.
    let aligned_sizes = [/*16usize,*/ 64, 256, 1024, 8 * 1024, 64 * 1024];
    let mut sizes = Vec::with_capacity(aligned_sizes.len() * 2);
    for &size in &aligned_sizes {
        sizes.push(size);
        sizes.push(size + 7);
    }
    sizes
}

#[derive(Clone, Copy, Debug)]
enum Placement {
    Absent,
    Start,
    Middle,
    End,
    Multiple,
    RandomBytes,
}

impl Placement {
    fn name(self) -> &'static str {
        match self {
            Placement::Absent => "absent",
            Placement::Start => "start",
            Placement::Middle => "middle",
            Placement::End => "end",
            Placement::Multiple => "multiple",
            Placement::RandomBytes => "random",
        }
    }
}

fn make_data(size: usize, needle: u8, placement: Placement) -> Vec<u8> {
    // Choose a base fill byte that is never equal to the needle.
    let fill = if needle == 0 { 0xAA } else { 0x00 };
    let mut data = vec![fill; size];

    match placement {
        Placement::Absent => {
            // Ensure the buffer truly doesn't contain the needle.
            if fill == needle {
                for b in &mut data {
                    *b = needle.wrapping_add(1);
                }
            }
        }
        Placement::Start => {
            if size > 0 {
                data[0] = needle;
            }
        }
        Placement::Middle => {
            if size > 0 {
                data[size / 2] = needle;
            }
        }
        Placement::End => {
            if size > 0 {
                data[size - 1] = needle;
            }
        }
        Placement::Multiple => {
            if size > 0 {
                data[size / 4] = needle;
                data[size / 2] = needle;
                data[size - 1] = needle;
            }
        }
        Placement::RandomBytes => {
            let mut rng = StdRng::seed_from_u64(RANDOM_SEED ^ size as u64);
            for b in &mut data {
                *b = rng.random::<u8>().wrapping_add(1);
                if *b == needle {
                    *b = needle.wrapping_add(1);
                }
            }

            if size > 0 {
                data[0] = needle;
                data[size / 2] = needle;
                data[size - 1] = needle;
            }
        }
    }

    data
}

fn alignment_label(size: usize) -> &'static str {
    if size % 8 == 0 {
        "Aligned"
    } else {
        "Unaligned"
    }
}

fn bench_memrchr(c: &mut Criterion) {
    let mut group = c.benchmark_group("memrchr (REVERSED)");

    let sizes = create_test_arrays();

    let placements = [
        Placement::Absent,
        Placement::Start,
        Placement::Middle,
        Placement::End,
        Placement::Multiple,
        Placement::RandomBytes,
    ];

    for size in sizes {
        let needle = 1u8;

        for placement in placements {
            let data = make_data(size, needle, placement);

            group.bench_with_input(
                BenchmarkId::new(
                    format!("std/{}/{}", placement.name(), alignment_label(size)),
                    size,
                ),
                &data,
                |b, data| {
                    b.iter(|| black_box(memchr_old::memrchr(black_box(needle), black_box(data))))
                },
            );

            group.bench_with_input(
                BenchmarkId::new(
                    format!("new/{}/{}", placement.name(), alignment_label(size)),
                    size,
                ),
                &data,
                |b, data| {
                    b.iter(|| black_box(memchr_new::memrchr(black_box(needle), black_box(data))))
                },
            );
        }
    }

    group.finish();
}

fn bench_memchr(c: &mut Criterion) {
    let mut group = c.benchmark_group("memchr");

    let sizes = create_test_arrays();

    let placements = [
        Placement::Absent,
        Placement::Start,
        Placement::Middle,
        Placement::End,
        Placement::Multiple,
        Placement::RandomBytes,
    ];

    for size in sizes {
        let needle = 1u8;

        for placement in placements {
            let data = make_data(size, needle, placement);

            group.bench_with_input(
                BenchmarkId::new(
                    format!("std/{}/{}", placement.name(), alignment_label(size)),
                    size,
                ),
                &data,
                |b, data| {
                    b.iter(|| black_box(memchr_old::memchr(black_box(needle), black_box(data))))
                },
            );

            group.bench_with_input(
                BenchmarkId::new(
                    format!("new/{}/{}", placement.name(), alignment_label(size)),
                    size,
                ),
                &data,
                |b, data| {
                    b.iter(|| {
                        black_box(memchr_stuff::memchr_new::memchr(
                            black_box(needle),
                            black_box(data),
                        ))
                    })
                },
            );
        }
    }

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_millis(200))
        .measurement_time(Duration::from_millis(1000))
        .sample_size(10000)
        .configure_from_args();
    targets = bench_memrchr, bench_memchr
);
criterion_main!(benches);
