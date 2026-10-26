//! NNUE inference benchmarks: `cargo bench -p chess-eval` (or `just bench`).

// Benchmark set-up code is not covered by clippy.toml's test exemptions.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::hint::black_box;

use chess_core::{Move, Position};
use chess_eval::Evaluator;
use chess_eval::nnue::{NnueEval, NnueWeights};
use criterion::{Criterion, criterion_group, criterion_main};

const KIWIPETE: &str = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";

/// Deterministic weights within the accumulator range; their values do not
/// change the cost of inference.
fn weights() -> NnueWeights {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = move |range: u64| -> i16 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        ((state % (2 * range + 1)) as i64 - range as i64) as i16
    };
    let mut weights = NnueWeights::zeros();
    weights.ft_weights.iter_mut().for_each(|w| *w = next(300));
    weights.ft_bias.iter_mut().for_each(|w| *w = next(100));
    weights.out_weights.iter_mut().for_each(|w| *w = next(127));
    weights.out_bias = 1_000;
    weights
}

fn find_move(pos: &Position, uci: &str) -> Move {
    pos.legal_moves()
        .iter()
        .copied()
        .find(|mv| mv.to_string() == uci)
        .unwrap()
}

fn inference(c: &mut Criterion) {
    let eval = NnueEval::new(weights());
    let pos = Position::from_fen(KIWIPETE).unwrap();
    let mut state = eval.new_state(&pos);
    let mut group = c.benchmark_group("nnue");

    group.bench_function("refresh", |b| b.iter(|| eval.new_state(black_box(&pos))));
    group.bench_function("output", |b| {
        b.iter(|| eval.evaluate_incremental(black_box(&state), black_box(&pos)))
    });
    group.bench_function("full_evaluate", |b| {
        b.iter(|| eval.evaluate(black_box(&pos)))
    });
    group.bench_function("make_unmake_null", |b| {
        b.iter(|| {
            eval.on_make(&mut state, black_box(&pos), black_box(Move::NULL));
            eval.on_unmake(&mut state);
        })
    });
    for (name, uci) in [("quiet", "a2a3"), ("capture", "e5f7"), ("castle", "e1g1")] {
        let mv = find_move(&pos, uci);
        group.bench_function(format!("make_unmake_{name}"), |b| {
            b.iter(|| {
                eval.on_make(&mut state, black_box(&pos), black_box(mv));
                eval.on_unmake(&mut state);
            })
        });
    }
    group.finish();
}

criterion_group!(benches, inference);
criterion_main!(benches);
