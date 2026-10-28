//! Non-regression: legacy networks give exactly the evaluations of the
//! engine before architectures became configurable.
//!
//! `data/legacy-golden.tsv` was produced by the previous engine
//! (`chess-eval` at commit 9703336) on 420 positions: FEN, evaluation by
//! the pseudo-random weights below, evaluation by the trained network
//! `main-sb305.nnue`. That network is not committed: the second test runs
//! when `CHESS_NNUE_LEGACY_FIXTURE` points at it (or when it sits in
//! `data/bots/`), and says so when it is skipped.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use chess_core::Position;
use chess_nnue::legacy::NnueWeights;
use chess_nnue::{Network, inspect};

/// The weights of the previous engine's tests.
fn pseudo_random_weights() -> NnueWeights {
    let mut state = 0x1234_5678_9ABC_DEF0_u64;
    let mut next = move |range: i64| -> i64 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % (2 * range as u64 + 1)) as i64 - range
    };
    let mut weights = NnueWeights::zeros();
    weights
        .ft_weights
        .iter_mut()
        .for_each(|w| *w = next(40) as i16);
    weights
        .ft_bias
        .iter_mut()
        .for_each(|w| *w = next(60) as i16);
    weights
        .out_weights
        .iter_mut()
        .for_each(|w| *w = next(100) as i16);
    weights.out_bias = next(5000) as i32;
    weights
}

struct Golden {
    fen: String,
    random: i32,
    trained: Option<i32>,
}

fn golden() -> Vec<Golden> {
    include_str!("data/legacy-golden.tsv")
        .lines()
        .map(|line| {
            let mut fields = line.split('\t');
            Golden {
                fen: fields.next().unwrap().to_owned(),
                random: fields.next().unwrap().parse().unwrap(),
                trained: fields.next().and_then(|f| f.parse().ok()),
            }
        })
        .collect()
}

/// Every way of loading `v1` gives the golden evaluation `expected` of each
/// position, from scratch and incrementally, with both output kernels.
fn check(v1: &[u8], expected: impl Fn(&Golden) -> i32) {
    let direct = Network::from_bytes(v1).unwrap();
    let converted = Network::from_bytes(&direct.to_bytes()).unwrap();
    assert_eq!(converted.to_bytes(), direct.to_bytes());
    assert_eq!(converted.hash(), direct.hash());
    assert_eq!(
        inspect(v1).unwrap().hash,
        inspect(&direct.to_bytes()).unwrap().hash
    );
    assert_eq!(direct.kernel(), "fixed-update+fused-crelu");
    assert_eq!(direct.clone().generic().kernel(), "generic");
    let generic = converted.clone().generic();

    for network in [&direct, &converted, &generic] {
        for case in golden() {
            let pos = Position::from_fen(&case.fen).unwrap();
            let state = network.new_state(&pos);
            assert_eq!(
                network.evaluate(&pos),
                expected(&case),
                "{} ({})",
                case.fen,
                network.kernel()
            );
            assert_eq!(network.evaluate_incremental(&state, &pos), expected(&case));
        }
    }
}

#[test]
fn pseudo_random_legacy_weights_evaluate_as_before() {
    let weights = pseudo_random_weights();
    // The scalar definition kept in `legacy` agrees too.
    for case in golden() {
        assert_eq!(
            weights.evaluate(&Position::from_fen(&case.fen).unwrap()),
            case.random
        );
    }
    check(&weights.to_bytes(), |case| case.random);
}

#[test]
fn trained_legacy_network_evaluates_as_before() {
    let path = std::env::var_os("CHESS_NNUE_LEGACY_FIXTURE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/bots/main-sb305.nnue")
        });
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!(
            "SKIPPED: {} not found (set CHESS_NNUE_LEGACY_FIXTURE to the main-sb305 network)",
            path.display()
        );
        return;
    };
    assert_eq!(bytes.len(), NnueWeights::FILE_LEN, "not the legacy fixture");
    check(&bytes, |case| case.trained.unwrap());
}
