use criterion::{black_box, criterion_group, criterion_main, Criterion};
use termwiz::cell::grapheme_column_width;

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("grapheme_column_width a", |b| {
        b.iter(|| grapheme_column_width(black_box("a"), None))
    });

    c.bench_function("grapheme_column_width emoji with variation selector", |b| {
        b.iter(|| grapheme_column_width(black_box("\u{00a9}\u{FE0F}"), None))
    });

    c.bench_function("grapheme_column_width WidenedIn9", |b| {
        b.iter(|| grapheme_column_width(black_box("\u{231a}"), None))
    });

    c.bench_function("grapheme_column_width Unassigned", |b| {
        b.iter(|| grapheme_column_width(black_box("\u{fbc9}"), None))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
