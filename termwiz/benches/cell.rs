use criterion::{black_box, criterion_group, criterion_main, Criterion};
use termwiz::cell::{Cell, CellAttributes};

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("Cell::blank", |b| b.iter(|| black_box(Cell::blank())));
    c.bench_function("Cell::new", |b| {
        b.iter(|| Cell::new(black_box('a'), CellAttributes::default()))
    });
    c.bench_function("Cell::new_grapheme", |b| {
        b.iter(|| Cell::new_grapheme(black_box("a"), CellAttributes::default(), None))
    });
    c.bench_function("Cell::new_grapheme_with_width", |b| {
        b.iter(|| Cell::new_grapheme_with_width(black_box("a"), 1, CellAttributes::default()))
    });

    c.bench_function("CellAttributes::blank", |b| {
        b.iter(|| black_box(CellAttributes::blank()))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
