//! Stockfish as a tournament opponent (see the `chess-stockfish` crate):
//! the `stockfish-1` to `stockfish-10` bots.
//!
//! The binary is `[arena] stockfish` in the configuration (`stockfish` on
//! the `PATH` by default; the Docker images ship Stockfish 19).

pub(crate) use chess_stockfish::{LEVELS, Stockfish, UciScore, probe};

/// Bot name of a level: `stockfish-1` to `stockfish-10`.
pub(crate) fn bot_name(level: u8) -> String {
    format!("stockfish-{level}")
}

/// The level a bot name designates, if it is a Stockfish one.
pub(crate) fn level_of(name: &str) -> Option<u8> {
    let digits = name.strip_prefix("stockfish-")?;
    if digits.starts_with('0') {
        return None;
    }
    digits.parse().ok().filter(|level| LEVELS.contains(level))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_levels() {
        assert_eq!(level_of("stockfish-1"), Some(1));
        assert_eq!(level_of("stockfish-10"), Some(10));
        for name in [
            "stockfish-0",
            "stockfish-11",
            "stockfish-01",
            "stockfish",
            "sf-3",
        ] {
            assert_eq!(level_of(name), None, "{name}");
        }
        assert_eq!(bot_name(7), "stockfish-7");
    }
}
