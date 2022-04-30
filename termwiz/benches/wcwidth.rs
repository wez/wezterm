use criterion::{black_box, criterion_group, criterion_main, Criterion};
use termwiz::cell::{grapheme_column_width, UnicodeVersion};

include!("../src/widechar_width.rs");

pub fn criterion_benchmark(c: &mut Criterion) {
    let table = WcLookupTable::new();

    {
        let mut group = c.benchmark_group("Classify ASCII");
        group.bench_function("WcWidth", |b| b.iter(|| WcWidth::from_char(black_box('a'))));
        group.bench_function("WcLookupTable", |b| {
            b.iter(|| table.classify(black_box('a')))
        });
        group.finish();
    }

    {
        let mut group = c.benchmark_group("Classify DoubleWidth");

        group.bench_function("WcWidth", |b| {
            b.iter(|| WcWidth::from_char(black_box('\u{1100}')))
        });
        group.bench_function("WcLookupTable", |b| {
            b.iter(|| table.classify(black_box('\u{1100}')))
        });

        group.finish();
    }

    {
        let mut group = c.benchmark_group("Classify WidenedIn9");

        group.bench_function("WcWidth", |b| {
            b.iter(|| WcWidth::from_char(black_box('\u{231a}')))
        });
        group.bench_function("WcLookupTable", |b| {
            b.iter(|| table.classify(black_box('\u{231a}')))
        });
        group.finish();
    }

    {
        let mut group = c.benchmark_group("Classify Unassigned");

        group.bench_function("WcWidth", |b| {
            b.iter(|| WcWidth::from_char(black_box('\u{fbc9}')))
        });
        group.bench_function("WcLookupTable", |b| {
            b.iter(|| table.classify(black_box('\u{fbc9}')))
        });
        group.finish();
    }

    {
        let mut group = c.benchmark_group("column_width ASCII");
        group.bench_function("grapheme_column_width", |b| {
            b.iter(|| grapheme_column_width(black_box("a"), None))
        });
        group.finish();
    }

    {
        let mut group = c.benchmark_group("column_width variation selector");
        group.bench_function("grapheme_column_width", |b| {
            b.iter(|| grapheme_column_width(black_box("\u{00a9}\u{FE0F}"), None))
        });
        group.finish();
    }

    {
        let mut group = c.benchmark_group("column_width variation selector unicode 14");
        let version = UnicodeVersion {
            version: 14,
            ambiguous_are_wide: false,
        };
        group.bench_function("grapheme_column_width", |b| {
            b.iter(|| grapheme_column_width(black_box("\u{00a9}\u{FE0F}"), Some(version)))
        });
        group.finish();
    }

    {
        let mut group = c.benchmark_group("column_width WidenedIn9");
        group.bench_function("grapheme_column_width", |b| {
            b.iter(|| grapheme_column_width(black_box("\u{231a}"), None))
        });
        group.finish();
    }

    {
        let mut group = c.benchmark_group("column_width Unassigned");
        group.bench_function("grapheme_column_width", |b| {
            b.iter(|| grapheme_column_width(black_box("\u{fbc9}"), None))
        });
        group.finish();
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
