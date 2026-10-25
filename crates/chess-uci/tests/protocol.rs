//! End-to-end tests driving the UCI binary through its stdin/stdout.

// Test helpers outside `#[test]` functions are not covered by clippy.toml.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Write;
use std::process::{Command, Stdio};

fn run_session(input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_chess-uci"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to start chess-uci");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn handshake() {
    let out = run_session("uci\nisready\nquit\n");
    assert!(out.lines().any(|l| l.starts_with("id name chess-bot")));
    assert!(out.lines().any(|l| l == "uciok"));
    assert!(out.lines().any(|l| l == "readyok"));
}

#[test]
fn go_answers_a_legal_bestmove() {
    let out = run_session("position startpos moves e2e4\ngo depth 1\nquit\n");
    let best = out
        .lines()
        .find_map(|l| l.strip_prefix("bestmove "))
        .unwrap();
    assert_ne!(best, "0000");
    assert!(best.starts_with(|c: char| ('a'..='h').contains(&c)));
}

#[test]
fn checkmated_side_answers_null_move() {
    let fools_mate = "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3";
    let out = run_session(&format!("position fen {fools_mate}\ngo\nquit\n"));
    assert!(out.lines().any(|l| l == "bestmove 0000"));
}

#[test]
fn perft_divide() {
    let out = run_session("position startpos\ngo perft 3\nquit\n");
    assert!(out.lines().any(|l| l == "e2e4: 600"));
    assert_eq!(out.lines().filter(|l| l.contains(": ")).count(), 21);
    assert!(out.lines().any(|l| l == "Nodes searched: 8902"));

    let kiwipete = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
    let out = run_session(&format!("position fen {kiwipete}\ngo perft 2\n"));
    assert!(out.lines().any(|l| l == "Nodes searched: 2039"));
}

#[test]
fn display_and_errors() {
    let out = run_session("position startpos moves g1f3\nd\nposition fen garbage\nd\n");
    let fen = "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQKB1R b KQkq - 1 1";
    // The invalid command is reported and the previous position is kept.
    assert_eq!(out.lines().filter(|l| *l == fen).count(), 2);
    assert!(out.lines().any(|l| l.starts_with("info string error")));
}

#[test]
fn exits_on_eof() {
    let out = run_session("isready\n");
    assert_eq!(out.trim(), "readyok");
}

#[test]
fn finds_mate_and_reports_it() {
    let back_rank = "6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1";
    let out = run_session(&format!("position fen {back_rank}\ngo depth 4\n"));
    assert!(out.lines().any(|l| l.contains("score mate 1")), "{out}");
    assert!(out.lines().any(|l| l.starts_with("bestmove d1d8")), "{out}");
}

#[test]
fn infinite_search_waits_for_stop_and_answers_isready() {
    let out = run_session("position startpos\ngo infinite\nisready\nstop\nquit\n");
    let lines: Vec<&str> = out.lines().collect();
    let ready = lines.iter().position(|l| *l == "readyok").unwrap();
    let best = lines
        .iter()
        .position(|l| l.starts_with("bestmove "))
        .unwrap();
    assert!(ready < best, "{out}");
    assert_eq!(
        lines.iter().filter(|l| l.starts_with("bestmove ")).count(),
        1
    );
}

#[test]
fn movetime_is_respected() {
    let start = std::time::Instant::now();
    let out = run_session("position startpos\ngo movetime 300\n");
    let elapsed = start.elapsed();
    assert!(out.lines().any(|l| l.starts_with("bestmove ")));
    assert!(
        elapsed >= std::time::Duration::from_millis(250),
        "{elapsed:?}"
    );
    assert!(
        elapsed < std::time::Duration::from_millis(1500),
        "{elapsed:?}"
    );
}

#[test]
fn clock_search_plays_quickly() {
    let out = run_session("position startpos\ngo wtime 1000 btime 1000 winc 0 binc 0\n");
    assert!(out.lines().any(|l| l.starts_with("bestmove ")));
}

#[test]
fn bench_reports_nodes() {
    let out = run_session("bench 5\n");
    let nodes: u64 = out
        .lines()
        .find_map(|l| l.strip_prefix("Nodes searched: "))
        .unwrap()
        .parse()
        .unwrap();
    assert!(nodes > 1_000);
    // Deterministic: same count from the command line.
    let output = Command::new(env!("CARGO_BIN_EXE_chess-uci"))
        .args(["bench", "5"])
        .output()
        .unwrap();
    let cli = String::from_utf8(output.stdout).unwrap();
    assert!(cli.contains(&format!("Nodes searched: {nodes}")), "{cli}");
}

#[test]
fn options() {
    let out = run_session("setoption name Hash value 4\nsetoption name Clear Hash\nisready\n");
    assert!(!out.contains("error"), "{out}");
    assert!(out.lines().any(|l| l == "readyok"));
}
