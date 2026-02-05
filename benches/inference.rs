//! Benchmarks for fastdateinfer

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use fastdateinfer::infer;

fn generate_dates_dmy(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| format!("{:02}/{:02}/2025", (i % 28) + 1, (i % 12) + 1))
        .collect()
}

fn generate_dates_mixed(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            if i % 3 == 0 {
                format!("{:02}/{:02}/2025", (i % 28) + 1, (i % 12) + 1)
            } else if i % 3 == 1 {
                format!("2025-{:02}-{:02}", (i % 12) + 1, (i % 28) + 1)
            } else {
                format!("{} Jan 2025", (i % 28) + 1)
            }
        })
        .collect()
}

fn bench_inference(c: &mut Criterion) {
    let mut group = c.benchmark_group("inference");

    for size in [100, 1000, 10000, 100000] {
        let dates = generate_dates_dmy(size);
        group.bench_with_input(
            BenchmarkId::new("dmy_slash", size),
            &dates,
            |b, dates| {
                b.iter(|| infer(black_box(dates)))
            },
        );
    }

    group.finish();
}

fn bench_tokenization(c: &mut Criterion) {
    let dates = generate_dates_dmy(1000);
    c.bench_function("tokenize_1000", |b| {
        b.iter(|| {
            for date in &dates {
                let _ = fastdateinfer::infer(&[black_box(date)]);
            }
        })
    });
}

criterion_group!(benches, bench_inference, bench_tokenization);
criterion_main!(benches);
