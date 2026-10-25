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
fn go_answers_bestmove() {
    let out = run_session("position startpos\ngo depth 1\nquit\n");
    assert!(out.lines().any(|l| l.starts_with("bestmove ")));
}

#[test]
fn exits_on_eof() {
    let out = run_session("isready\n");
    assert_eq!(out.trim(), "readyok");
}
