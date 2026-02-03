use core::hint::black_box;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use memchr_stuff::memchr_new;
use memchr_stuff::memchr_old;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::Duration;

const RANDOM_SEED: u64 = 4269; //change as needed

const WORD_SIZE: usize = size_of::<usize>();

const SKIP_LESS_THAN_2_WORDS: bool = true;
// CHANGE AS WANTED
const BENCH_MARK_SIZES: &[usize] = &[
    2 * WORD_SIZE,
    4 * WORD_SIZE,
    8 * WORD_SIZE,
    16 * WORD_SIZE,
    24 * WORD_SIZE,
    32 * WORD_SIZE,
    1024,
    4 * 1024,
    8 * 1024,
    64 * 1024,
];

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
    const fn name(self) -> &'static str {
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
            data[0] = needle;
        }
        Placement::Middle => {
            data[size / 2] = needle;
        }
        Placement::End => {
            data[size - 1] = needle;
        }
        Placement::Multiple => {
            data[size / 4] = needle;
            data[size / 2] = needle;
            data[size - 1] = needle;
        }
        Placement::RandomBytes => {
            let mut rng = StdRng::seed_from_u64(RANDOM_SEED ^ size as u64);
            for b in &mut data {
                *b = rng.random::<u8>().wrapping_add(1);
                if *b == needle {
                    *b = needle.wrapping_add(1);
                }
            }

            data[0] = needle;
            data[size / 2] = needle;
            data[size - 1] = needle;
        }
    }

    data
}

fn run_bench(
    c: &mut Criterion,
    group_name: &str,
    old_fn: fn(u8, &[u8]) -> Option<usize>,
    new_fn: fn(u8, &[u8]) -> Option<usize>,
) {
    let mut group = c.benchmark_group(group_name);

    let placements = [
        Placement::Absent,
        Placement::Start,
        Placement::Middle,
        Placement::End,
        Placement::Multiple,
        Placement::RandomBytes,
    ];
    let needle = 1u8;
    for size in BENCH_MARK_SIZES {
        for aligned in [false, true] {
            for placement in placements {
                let label = if aligned { "aligned" } else { "unaligned" };
                let data = make_data(*size, needle, placement);
                // Purposefully unalign it if so.
                let data_slice: &[u8] = if !aligned { &data[1..] } else { &data[..] };
                // Sanity check
                assert_eq!(aligned, data_slice.as_ptr().cast::<usize>().is_aligned());
                let new_size = data_slice.len();

                group.throughput(Throughput::Bytes(new_size as u64));
                if SKIP_LESS_THAN_2_WORDS && new_size < 2 * WORD_SIZE {
                    continue; //No point testing <2 usize
                };

                group.bench_with_input(
                    BenchmarkId::new(format!("std/{}/{}", placement.name(), label), new_size),
                    &data_slice,
                    |b, data| b.iter(|| black_box(old_fn(black_box(needle), black_box(*data)))),
                );

                group.bench_with_input(
                    BenchmarkId::new(format!("new/{}/{}", placement.name(), label), new_size),
                    &data_slice,
                    |b, data| b.iter(|| black_box(new_fn(black_box(needle), black_box(*data)))),
                );
            }
        }
    }

    group.finish();
}

fn bench_memrchr(c: &mut Criterion) {
    run_bench(
        c,
        "memrchr (REVERSED)",
        memchr_old::memrchr,
        memchr_new::memrchr,
    );
}

fn bench_memchr(c: &mut Criterion) {
    run_bench(
        c,
        "memchr (FORWARD)",
        memchr_old::memchr,
        memchr_new::memchr,
    );
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_millis(400))
        .measurement_time(Duration::from_millis(500))
        .sample_size(1000)
        .configure_from_args();
    targets = bench_memchr,bench_memrchr
);
criterion_main!(benches);
