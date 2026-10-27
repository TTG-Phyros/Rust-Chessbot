//! Client of the Chess.com public API ([PubAPI]): the monthly game archives
//! of a player, and their games parsed into [`chess_core::pgn::Game`] with
//! ratings, colours, time class and `[%clk]` clocks.
//!
//! Chess.com allows unlimited *serial* access but answers `429 Too Many
//! Requests` to parallel requests: a [`Client`] sends one request at a time
//! (whoever holds it, across tasks), spaced by a minimal interval, and
//! retries rate-limited and failed requests with an exponential backoff.
//! Requests must carry an explicit `User-Agent` with a way to contact the
//! author; it comes from the caller's configuration.
//!
//! This crate performs no caching: conditional requests ([`Validators`])
//! let the caller keep its own cache and revalidate it cheaply.
//!
//! [PubAPI]: https://www.chess.com/news/view/published-data-api

mod client;
mod filter;
mod game;
mod model;

pub use client::{Client, ClientConfig, Error, Fetched, Validators};
pub use filter::{GameFilter, parse_date};
pub use game::{PlayedGame, Player};
pub use model::{Accuracies, ApiGame, ApiPlayer, Month, TimeClass};

/// Whether `username` can be a Chess.com username: 3 to 25 letters, digits,
/// `_` or `-`. Checked before it is put in a URL.
#[must_use]
pub fn is_valid_username(username: &str) -> bool {
    (3..=25).contains(&username.len())
        && username
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames() {
        assert!(is_valid_username("Hikaru"));
        assert!(is_valid_username("magnus_carlsen-2"));
        assert!(!is_valid_username("ab"));
        assert!(!is_valid_username("../../etc"));
        assert!(!is_valid_username("name with spaces"));
    }
}
