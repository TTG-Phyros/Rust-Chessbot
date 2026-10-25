//! Parsing of UCI command arguments.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use chess_core::Position;
use chess_search::SearchLimits;

/// What `go` asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Go {
    /// Search with these limits.
    Search(SearchLimits),
    /// Count leaf nodes to this depth (debugging extension).
    Perft(u32),
}

/// Parses the arguments of `go`. Unknown tokens are ignored, as the
/// protocol recommends.
pub(crate) fn parse_go(args: &str, move_overhead: Duration) -> Result<Go> {
    let mut limits = SearchLimits {
        move_overhead,
        ..SearchLimits::default()
    };
    let mut tokens = args.split_whitespace();
    while let Some(token) = tokens.next() {
        let mut value = || {
            tokens
                .next()
                .with_context(|| format!("`{token}` needs a value"))
        };
        match token {
            "perft" => return Ok(Go::Perft(value()?.parse().context("perft depth")?)),
            "wtime" => limits.white_time = Some(millis(value()?)?),
            "btime" => limits.black_time = Some(millis(value()?)?),
            "winc" => limits.white_increment = Some(millis(value()?)?),
            "binc" => limits.black_increment = Some(millis(value()?)?),
            "movetime" => limits.movetime = Some(millis(value()?)?),
            "movestogo" => limits.moves_to_go = Some(value()?.parse().context("movestogo")?),
            "depth" => limits.depth = Some(value()?.parse().context("depth")?),
            "nodes" => limits.nodes = Some(value()?.parse().context("nodes")?),
            // Pondering is not advertised; treat it like an infinite search.
            "infinite" | "ponder" => limits.infinite = true,
            _ => {}
        }
    }
    Ok(Go::Search(limits))
}

/// Milliseconds; GUIs may send negative clock values once a flag has fallen.
fn millis(text: &str) -> Result<Duration> {
    let ms: i64 = text
        .parse()
        .with_context(|| format!("invalid time {text:?}"))?;
    Ok(Duration::from_millis(ms.max(0).unsigned_abs()))
}

/// Parses the arguments of `position`: `startpos | fen <FEN>` then optionally
/// `moves <uci>...`.
pub(crate) fn parse_position(args: &str) -> Result<Position> {
    let (setup, moves) = match args.split_once("moves") {
        Some((setup, moves)) => (setup.trim(), Some(moves)),
        None => (args.trim(), None),
    };
    let mut position = if setup == "startpos" {
        Position::startpos()
    } else if let Some(fen) = setup.strip_prefix("fen") {
        Position::from_fen(fen.trim()).context("invalid FEN")?
    } else {
        bail!("expected `startpos` or `fen <FEN>`, got {setup:?}");
    };
    for text in moves.unwrap_or_default().split_whitespace() {
        let mv = position
            .parse_uci_move(text)
            .with_context(|| format!("move {text}"))?;
        position.make_move(mv);
    }
    Ok(position)
}

/// Parses `setoption name <name> [value <value>]`; option names may contain spaces.
pub(crate) fn parse_setoption(args: &str) -> Result<(String, Option<String>)> {
    let rest = args
        .trim()
        .strip_prefix("name")
        .context("expected `name <option>`")?;
    let (name, value) = match rest.split_once(" value ") {
        Some((name, value)) => (name, Some(value.trim().to_owned())),
        None => (rest, None),
    };
    let name = name.trim();
    if name.is_empty() {
        bail!("missing option name");
    }
    Ok((name.to_owned(), value))
}

#[cfg(test)]
mod tests {
    use super::*;

    const OVERHEAD: Duration = Duration::from_millis(30);
    const MS: fn(u64) -> Duration = Duration::from_millis;

    fn limits(args: &str) -> SearchLimits {
        match parse_go(args, OVERHEAD).unwrap() {
            Go::Search(limits) => limits,
            Go::Perft(_) => panic!("unexpected perft"),
        }
    }

    #[test]
    fn go_with_clocks() {
        let limits = limits("wtime 60000 btime -50 winc 1000 binc 0 movestogo 20");
        assert_eq!(limits.white_time, Some(MS(60_000)));
        assert_eq!(limits.black_time, Some(MS(0)));
        assert_eq!(limits.white_increment, Some(MS(1_000)));
        assert_eq!(limits.moves_to_go, Some(20));
        assert_eq!(limits.move_overhead, OVERHEAD);
        assert!(!limits.infinite);
    }

    #[test]
    fn go_with_fixed_limits() {
        assert_eq!(limits("depth 7").depth, Some(7));
        assert_eq!(limits("nodes 5000").nodes, Some(5_000));
        assert_eq!(limits("movetime 250").movetime, Some(MS(250)));
        assert!(limits("infinite").infinite);
        assert!(limits("ponder wtime 1000").infinite);
        assert_eq!(limits("searchmoves e2e4 depth 3").depth, Some(3));
        assert_eq!(
            limits(""),
            SearchLimits {
                move_overhead: OVERHEAD,
                ..SearchLimits::default()
            }
        );
    }

    #[test]
    fn go_perft_and_errors() {
        assert_eq!(parse_go("perft 4", OVERHEAD).unwrap(), Go::Perft(4));
        assert!(parse_go("perft", OVERHEAD).is_err());
        assert!(parse_go("depth x", OVERHEAD).is_err());
        assert!(parse_go("wtime", OVERHEAD).is_err());
    }

    #[test]
    fn positions() {
        let pos = parse_position("startpos moves e2e4 c7c5 g1f3").unwrap();
        assert_eq!(
            pos.to_fen(),
            "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2"
        );
        assert_eq!(pos.ply(), 3);
        let fen = "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1";
        assert_eq!(parse_position(&format!("fen {fen}")).unwrap().to_fen(), fen);
        let pos = parse_position(&format!("fen {fen} moves e2e4")).unwrap();
        assert_eq!(pos.to_fen(), "4k3/8/8/8/4P3/8/8/4K3 b - - 0 1");
        assert!(parse_position("").is_err());
        assert!(parse_position("fen nonsense").is_err());
        assert!(parse_position("startpos moves e2e5").is_err());
    }

    #[test]
    fn setoption() {
        assert_eq!(
            parse_setoption("name Hash value 128").unwrap(),
            ("Hash".into(), Some("128".into()))
        );
        assert_eq!(
            parse_setoption("name Move Overhead value 50").unwrap(),
            ("Move Overhead".into(), Some("50".into()))
        );
        assert_eq!(
            parse_setoption("name Clear Hash").unwrap(),
            ("Clear Hash".into(), None)
        );
        assert!(parse_setoption("Hash 128").is_err());
        assert!(parse_setoption("name ").is_err());
    }
}
