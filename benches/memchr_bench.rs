use core::hint::black_box;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use memchr_stuff::memchr_new;
use memchr_stuff::memchr_old;
use std::time::Duration;

fn create_test_arrays() -> Vec<usize> {
    let aligned_sizes = [16usize, 64, 256, 1024, 8 * 1024, 64 * 1024];
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

// Simple deterministic PRNG (no extra deps). Good enough?
fn xorshift64(mut x: u64) -> impl FnMut() -> u64 {
    move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
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
            let mut next = xorshift64(0x6eed_0e9d_5a15_5eed_u64 ^ size as u64);
            for b in &mut data {
                *b = (next() as u8).wrapping_add(1);
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

    // For memrchr, placement affects how far from the end the *last* match is.
    // (e.g., "start" forces a full scan when there are no later matches.)
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
                //let is_aligned: &str = if size % 8 == 0 { "Aligned" } else { "Unaligned" };
                BenchmarkId::new(
                    format!("old/{}/{}", placement.name(), alignment_label(size)),
                    size,
                ),
                &data,
                |b, data| {
                    b.iter(|| {
                        black_box(memchr_old::memrchr_old(black_box(needle), black_box(data)))
                    })
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
                    format!("old/{}/{}", placement.name(), alignment_label(size)),
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
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5))
        .sample_size(200);
    targets = bench_memrchr, bench_memchr
);
criterion_main!(benches);
