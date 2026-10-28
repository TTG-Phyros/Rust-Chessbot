//! Inference cost of each preset: `cargo bench -p chess-nnue`.
//!
//! `legacy-768x256/generic` is the legacy network forced through the
//! generic path, to measure what the specialized kernels save.

// Benchmark set-up code is not covered by clippy.toml's test exemptions.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::hint::black_box;

use chess_core::{Move, Position};
use chess_nnue::Network;
use chess_nnue::spec::presets;
use criterion::{Criterion, criterion_group, criterion_main};

const KIWIPETE: &str = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";

fn find_move(pos: &Position, uci: &str) -> Move {
    pos.legal_moves()
        .iter()
        .copied()
        .find(|mv| mv.to_string() == uci)
        .unwrap()
}

fn inference(c: &mut Criterion) {
    let pos = Position::from_fen(KIWIPETE).unwrap();
    let mut networks: Vec<(String, Network)> = presets::all()
        .into_iter()
        .map(|p| {
            let network = Network::pseudo_random(&p.architecture, 7).unwrap();
            (p.id.to_owned(), network)
        })
        .collect();
    let legacy = Network::pseudo_random(&presets::get(presets::LEGACY).unwrap().architecture, 7)
        .unwrap()
        .generic();
    networks.push(("legacy-768x256/generic".into(), legacy));

    let quiet = find_move(&pos, "a2a3");
    let king = find_move(&pos, "e1f1");
    for (name, network) in &networks {
        let mut group = c.benchmark_group(name.as_str());
        let mut state = network.new_state(&pos);
        group.bench_function("output", |b| {
            b.iter(|| network.evaluate_incremental(black_box(&state), black_box(&pos)))
        });
        group.bench_function("make_unmake_quiet", |b| {
            b.iter(|| {
                network.on_make(&mut state, black_box(&pos), black_box(quiet));
                network.on_unmake(&mut state);
            })
        });
        group.bench_function("make_unmake_king", |b| {
            b.iter(|| {
                network.on_make(&mut state, black_box(&pos), black_box(king));
                network.on_unmake(&mut state);
            })
        });
        group.finish();
    }
}

criterion_group!(benches, inference);
criterion_main!(benches);
