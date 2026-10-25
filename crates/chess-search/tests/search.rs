//! Behavioural tests of the search with the classical evaluation.

// Test helpers outside `#[test]` functions are not covered by clippy.toml.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant};

use chess_core::{GameStatus, Position};
use chess_eval::ClassicEval;
use chess_search::{Score, SearchLimits, SearchResult, Searcher, StopFlag, bench};

fn searcher() -> Searcher<ClassicEval> {
    Searcher::new(ClassicEval, StopFlag::new())
}

fn search(fen: &str, limits: &SearchLimits) -> SearchResult {
    let pos = Position::from_fen(fen).unwrap();
    searcher().search(&pos, limits, |_| {})
}

fn best_uci(result: &SearchResult) -> String {
    result
        .best_move
        .map(|mv| mv.to_string())
        .unwrap_or_default()
}

#[test]
fn finds_back_rank_mate_in_one() {
    let result = search(
        "6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1",
        &SearchLimits::depth(4),
    );
    assert_eq!(best_uci(&result), "d1d8");
    assert_eq!(result.score, Score::Mate(1));
}

#[test]
fn finds_mate_in_two() {
    // 1. Kb6 Kb8 2. Rh8#.
    let result = search("k7/8/2K5/8/8/8/8/7R w - - 0 1", &SearchLimits::depth(6));
    assert_eq!(result.score, Score::Mate(2), "{result:?}");
}

#[test]
fn sees_being_mated() {
    // Black must play ...Kg8, then Qg7# follows.
    let result = search("7k/8/6KQ/8/8/8/8/8 b - - 0 1", &SearchLimits::depth(5));
    assert_eq!(result.score, Score::Mate(-1), "{result:?}");
    assert_eq!(best_uci(&result), "h8g8");
}

#[test]
fn wins_a_hanging_queen() {
    let result = search("4k3/8/8/3q4/8/8/3R4/3K4 w - - 0 1", &SearchLimits::depth(4));
    assert_eq!(best_uci(&result), "d2d5");
}

#[test]
fn does_not_take_a_poisoned_pawn() {
    // Qxb7?? loses the queen to ...Rb8xb7... the rook guards b7 along the rank.
    let result = search(
        "1r2k3/1p6/8/8/8/8/8/1Q2K3 w - - 0 1",
        &SearchLimits::depth(5),
    );
    assert_ne!(best_uci(&result), "b1b7");
}

#[test]
fn avoids_stalemating_a_lone_king() {
    let fen = "k7/8/1Q6/8/8/8/8/6K1 w - - 0 1";
    let result = search(fen, &SearchLimits::depth(6));
    let mut pos = Position::from_fen(fen).unwrap();
    pos.make_move(result.best_move.unwrap());
    assert_ne!(pos.status(), GameStatus::Stalemate);
    assert!(
        matches!(result.score, Score::Mate(n) if n > 0)
            || matches!(result.score, Score::Centipawns(cp) if cp > 500)
    );
}

#[test]
fn reports_draw_score_in_dead_positions() {
    let result = search("4k3/8/8/8/8/8/8/4KB2 w - - 0 1", &SearchLimits::depth(6));
    assert_eq!(result.score, Score::Centipawns(0));
}

#[test]
fn no_legal_move_means_no_best_move() {
    let mated = "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3";
    let result = search(mated, &SearchLimits::depth(3));
    assert_eq!(result.best_move, None);
    assert_eq!(result.score, Score::Mate(0));
}

#[test]
fn respects_depth_and_reports_each_iteration() {
    let pos = Position::startpos();
    let mut depths = Vec::new();
    let result = searcher().search(&pos, &SearchLimits::depth(5), |info| {
        assert!(!info.pv.is_empty());
        assert!(pos.is_legal(info.pv[0]));
        depths.push(info.depth);
    });
    assert_eq!(depths, [1, 2, 3, 4, 5]);
    assert_eq!(result.depth, 5);
    assert!(result.ponder.is_some());
}

#[test]
fn principal_variation_is_legal() {
    let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
    let result = search(fen, &SearchLimits::depth(6));
    let mut pos = Position::from_fen(fen).unwrap();
    for mv in &result.pv {
        pos.play(*mv).unwrap();
    }
    assert_eq!(result.pv.first().copied(), result.best_move);
}

#[test]
fn respects_node_limit() {
    let result = search(chess_core::STARTPOS_FEN, &SearchLimits::nodes(20_000));
    assert!(result.nodes <= 20_000, "{}", result.nodes);
    assert!(result.best_move.is_some());
}

#[test]
fn respects_movetime() {
    let start = Instant::now();
    let fen = "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10";
    let result = search(fen, &SearchLimits::movetime(Duration::from_millis(200)));
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_millis(600), "{elapsed:?}");
    assert!(result.best_move.is_some());
}

#[test]
fn stops_when_the_flag_is_raised() {
    let stop = StopFlag::new();
    let mut searcher = Searcher::new(ClassicEval, stop.clone());
    let handle = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        stop.stop();
    });
    let start = Instant::now();
    let infinite = SearchLimits {
        infinite: true,
        ..SearchLimits::default()
    };
    let result = searcher.search(&Position::startpos(), &infinite, |_| {});
    handle.join().unwrap();
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(result.best_move.is_some());
}

#[test]
fn clock_based_search_uses_a_fraction_of_the_time() {
    let limits = SearchLimits {
        white_time: Some(Duration::from_millis(2_000)),
        black_time: Some(Duration::from_millis(2_000)),
        ..SearchLimits::default()
    };
    let start = Instant::now();
    let result = search(chess_core::STARTPOS_FEN, &limits);
    assert!(
        start.elapsed() < Duration::from_millis(1_000),
        "{:?}",
        start.elapsed()
    );
    assert!(result.depth >= 3);
}

#[test]
fn detects_repetition_in_the_game_history() {
    // White is a rook down; repeating the position is its best outcome, and
    // after two knight shuffles the position already occurred twice.
    let mut pos = Position::from_fen("r3k3/8/8/8/8/8/8/1N2K3 w - - 0 1").unwrap();
    for uci in ["b1c3", "a8b8", "c3b1", "b8a8", "b1c3", "a8b8", "c3b1"] {
        let mv = pos.parse_uci_move(uci).unwrap();
        pos.make_move(mv);
    }
    // Black to move: b8a8 repeats the start position a third time. Black is
    // winning, so it must avoid it.
    let result = searcher().search(&pos, &SearchLimits::depth(6), |_| {});
    assert_ne!(best_uci(&result), "b8a8");
}

#[test]
fn deterministic_after_clear() {
    let mut searcher = searcher();
    let first = bench(&mut searcher, 4);
    let second = bench(&mut searcher, 4);
    assert_eq!(first.nodes, second.nodes);
    assert!(first.nodes > 1_000);
}

#[test]
fn plays_a_complete_legal_game_against_itself() {
    let mut searcher = searcher();
    let mut pos = Position::startpos();
    for _ in 0..300 {
        if pos.status().is_over() {
            break;
        }
        let result = searcher.search(&pos, &SearchLimits::depth(3), |_| {});
        let mv = result.best_move.expect("a move while the game is not over");
        pos.play(mv).unwrap();
    }
    assert!(pos.ply() > 10);
}
