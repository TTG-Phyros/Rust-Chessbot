//! Search benchmarks: `cargo bench -p chess-search` (or `just bench`).

// Benchmark set-up code is not covered by clippy.toml's test exemptions.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use chess_core::Position;
use chess_eval::ClassicEval;
use chess_search::{SearchLimits, Searcher, StopFlag, bench};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn fixed_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");
    group.sample_size(10);
    let positions = [
        (
            "startpos",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            8,
        ),
        (
            "kiwipete",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            7,
        ),
    ];
    for (name, fen, depth) in positions {
        let pos = Position::from_fen(fen).unwrap();
        let mut searcher = Searcher::new(ClassicEval, StopFlag::new());
        group.bench_function(BenchmarkId::new(name, depth), |b| {
            b.iter(|| {
                searcher.clear();
                searcher.search(&pos, &SearchLimits::depth(depth), |_| {})
            });
        });
    }
    group.finish();
}

fn bench_suite(c: &mut Criterion) {
    let mut group = c.benchmark_group("bench");
    group.sample_size(10);
    let mut searcher = Searcher::new(ClassicEval, StopFlag::new());
    group.bench_function("12_positions_depth_7", |b| {
        b.iter(|| bench(&mut searcher, 7))
    });
    group.finish();
}

criterion_group!(benches, fixed_depth, bench_suite);
criterion_main!(benches);
