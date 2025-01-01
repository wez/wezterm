use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rangeset::RangeSet;

fn build_contig_rangeset(size: usize) -> RangeSet<usize> {
    let mut set = RangeSet::new();
    for i in 0..size {
        set.add(i);
    }
    set
}

fn build_sparse_rangeset(size: usize) -> RangeSet<usize> {
    let mut set = RangeSet::new();
    for i in (0..size).step_by(2) {
        set.add(i);
    }
    set
}

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("Contig 100", |b| {
        b.iter(|| black_box(build_contig_rangeset(100)))
    });
    c.bench_function("Contig 10000", |b| {
        b.iter(|| black_box(build_contig_rangeset(10000)))
    });
    c.bench_function("Contig 1000000", |b| {
        b.iter(|| black_box(build_contig_rangeset(1000000)))
    });

    c.bench_function("Sparse 100", |b| {
        b.iter(|| black_box(build_sparse_rangeset(100)))
    });
    c.bench_function("Sparse 10000", |b| {
        b.iter(|| black_box(build_sparse_rangeset(10000)))
    });
    c.bench_function("Sparse 1000000", |b| {
        b.iter(|| black_box(build_sparse_rangeset(1000000)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
