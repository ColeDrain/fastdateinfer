//! Benchmarks for fastdateinfer

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use fastdateinfer::{infer, infer_with_options, InferOptions};

fn generate_dates_dmy(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| format!("{:02}/{:02}/2025", (i % 28) + 1, (i % 12) + 1))
        .collect()
}

/// Generate ambiguous dates with a single disambiguating date at a non-sampled index.
fn generate_dates_prescan(n: usize, disambig_value: &str, disambig_index: usize) -> Vec<String> {
    let mut dates: Vec<String> = (0..n)
        .map(|i| format!("{:02}/{:02}/2025", (i % 12) + 1, (i % 12) + 1))
        .collect();
    if disambig_index < n {
        dates[disambig_index] = disambig_value.to_string();
    }
    dates
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

fn bench_prescan(c: &mut Criterion) {
    let mut group = c.benchmark_group("prescan");

    // Disambiguating DD/MM date at non-sampled index
    for size in [1000, 10000, 100000] {
        let dates = generate_dates_prescan(size, "25/06/2025", 7);
        group.bench_with_input(
            BenchmarkId::new("ddmm_disambig", size),
            &dates,
            |b, dates| {
                b.iter(|| infer(black_box(dates)))
            },
        );
    }

    // Disambiguating MM/DD date at non-sampled index
    for size in [1000, 10000, 100000] {
        let dates = generate_dates_prescan(size, "06/25/2025", 7);
        group.bench_with_input(
            BenchmarkId::new("mmdd_disambig", size),
            &dates,
            |b, dates| {
                b.iter(|| infer(black_box(dates)))
            },
        );
    }

    // No disambiguation — all ambiguous
    for size in [1000, 10000, 100000] {
        let dates: Vec<String> = (0..size)
            .map(|i| format!("{:02}/{:02}/2025", (i % 12) + 1, (i % 12) + 1))
            .collect();
        group.bench_with_input(
            BenchmarkId::new("all_ambiguous", size),
            &dates,
            |b, dates| {
                b.iter(|| infer(black_box(dates)))
            },
        );
    }

    group.finish();
}

fn bench_strict(c: &mut Criterion) {
    let mut group = c.benchmark_group("strict");

    let options_strict = InferOptions {
        strict: true,
        ..Default::default()
    };
    let options_default = InferOptions::default();

    for size in [100, 1000, 10000] {
        let dates = generate_dates_dmy(size);

        group.bench_with_input(
            BenchmarkId::new("strict_on", size),
            &dates,
            |b, dates| {
                b.iter(|| infer_with_options(black_box(dates), &options_strict))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("strict_off", size),
            &dates,
            |b, dates| {
                b.iter(|| infer_with_options(black_box(dates), &options_default))
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_inference, bench_tokenization, bench_prescan, bench_strict);
criterion_main!(benches);
