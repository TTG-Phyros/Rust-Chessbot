//! Move generation benchmarks: `cargo bench -p chess-core` (or `just bench`).

// Benchmark set-up code is not covered by clippy.toml's test exemptions.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::hint::black_box;

use chess_core::{Position, perft};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

const POSITIONS: [(&str, &str); 3] = [
    (
        "startpos",
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    ),
    (
        "kiwipete",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    ),
    ("endgame", "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1"),
];

fn legal_moves(c: &mut Criterion) {
    let mut group = c.benchmark_group("legal_moves");
    for (name, fen) in POSITIONS {
        let pos = Position::from_fen(fen).unwrap();
        group.bench_with_input(BenchmarkId::from_parameter(name), &pos, |b, pos| {
            b.iter(|| black_box(pos).legal_moves().len());
        });
    }
    group.finish();
}

fn make_unmake(c: &mut Criterion) {
    let mut pos = Position::from_fen(POSITIONS[1].1).unwrap();
    let moves = pos.legal_moves();
    c.bench_function("make_unmake/kiwipete_all_moves", |b| {
        b.iter(|| {
            for &mv in &moves {
                pos.make_move(black_box(mv));
                pos.unmake_move();
            }
        });
    });
}

fn perft_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("perft");
    group.sample_size(10);
    for (name, fen, depth, nodes) in [
        ("startpos", POSITIONS[0].1, 5, 4_865_609),
        ("kiwipete", POSITIONS[1].1, 4, 4_085_603),
    ] {
        let mut pos = Position::from_fen(fen).unwrap();
        group.throughput(Throughput::Elements(nodes));
        group.bench_function(BenchmarkId::new(name, depth), |b| {
            b.iter(|| assert_eq!(perft(&mut pos, black_box(depth)), nodes));
        });
    }
    group.finish();
}

criterion_group!(benches, legal_moves, make_unmake, perft_bench);
criterion_main!(benches);
