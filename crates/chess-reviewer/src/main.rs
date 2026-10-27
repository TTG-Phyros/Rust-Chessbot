//! `chess-reviewer` binary: Chess.com games from the command line.
//!
//! ```text
//! chess-reviewer fetch hikaru -n 20 --time-class blitz   # sync and list games
//! ```
//!
//! Settings come from `config/review.toml`, `CHESS_REVIEW_*` variables (a
//! `.env` file in the working directory is read first) and flags.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use chess_chesscom::{ApiGame, Client, GameFilter, PlayedGame, TimeClass, parse_date};
use chess_core::Color;
use chess_reviewer::{MonthSource, ReviewConfig, Store, SyncEvent, sync_player};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Brings a player's games into the cache and lists them.
    Fetch {
        /// Chess.com username.
        username: String,
        /// Number of games, most recent first.
        #[arg(short = 'n', long, default_value_t = 20)]
        max_games: usize,
        /// Time classes to keep (all by default), e.g. `blitz,rapid`.
        #[arg(long, value_delimiter = ',')]
        time_class: Vec<TimeClass>,
        /// Games that ended on or after this day (YYYY-MM-DD, UTC).
        #[arg(long)]
        since: Option<String>,
        /// Games that ended on or before this day (YYYY-MM-DD, UTC).
        #[arg(long)]
        until: Option<String>,
        /// Rated games only.
        #[arg(long)]
        rated: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // A missing `.env` is fine: the variables may come from the environment.
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()))
        .with_writer(std::io::stderr)
        .init();
    let config = ReviewConfig::load().context("invalid review configuration")?;

    match Cli::parse().command {
        Command::Fetch {
            username,
            max_games,
            time_class,
            since,
            until,
            rated,
        } => {
            let day = |text: Option<String>| -> Result<Option<i64>> {
                text.map(|text| {
                    parse_date(&text).with_context(|| format!("invalid date {text:?} (YYYY-MM-DD)"))
                })
                .transpose()
            };
            let filter = GameFilter {
                time_classes: time_class,
                max_games,
                since: day(since)?,
                // Inclusive: up to the end of that day.
                until: day(until)?.map(|start| start + 86_400),
                rated_only: rated,
            };
            fetch(&config, &username, &filter).await
        }
    }
}

async fn fetch(config: &ReviewConfig, username: &str, filter: &GameFilter) -> Result<()> {
    if filter.max_games == 0 {
        bail!("--max-games must be at least 1");
    }
    let client = Client::new(config.client_config())?;
    let store = Store::open(&config.cache)
        .with_context(|| format!("cannot open the cache {}", config.cache.display()))?;
    let mut downloaded = 0;
    let mut revalidated = 0;
    let mut cached = 0;
    let games = sync_player(
        &client,
        &store,
        username,
        filter,
        config.recent_ttl(),
        |event| match event {
            SyncEvent::Months { total, cached } => {
                eprintln!(
                    "{total} month(s) in the period{}",
                    if cached { " (list cached)" } else { "" }
                );
            }
            SyncEvent::Month {
                month,
                index,
                total,
                source,
                found,
            } => {
                let label = match source {
                    MonthSource::Cache => {
                        cached += 1;
                        "cached"
                    }
                    MonthSource::Revalidated => {
                        revalidated += 1;
                        "revalidated"
                    }
                    MonthSource::Downloaded => {
                        downloaded += 1;
                        "downloaded"
                    }
                    MonthSource::Unavailable => "not served by Chess.com, skipped",
                };
                eprintln!("[{index}/{total}] {month} {label}, {found} game(s) so far");
            }
        },
    )
    .await?;
    eprintln!(
        "{} game(s); months: {downloaded} downloaded, {revalidated} revalidated, {cached} from the cache ({})\n",
        games.len(),
        config.cache.display()
    );
    for game in &games {
        println!("{}", describe(game, username));
    }
    Ok(())
}

/// One line per game: date, class, colour, players, result, moves, clocks.
fn describe(api: &ApiGame, username: &str) -> String {
    let date = chess_chesscom::Month::containing(api.end_time);
    let day = (api.end_time - date.start()) / 86_400 + 1;
    let head = format!(
        "{date}-{day:02}  {:<6} {:<8}",
        api.time_class.as_str(),
        api.time_control
    );
    let game = match PlayedGame::from_api(api) {
        Ok(game) => game,
        Err(err) => return format!("{head} unreadable PGN: {err}"),
    };
    let color = match game.color_of(username) {
        Some(Color::White) => "white",
        Some(Color::Black) => "black",
        None => "?",
    };
    let clocks = game.clocks.iter().filter(|clock| clock.is_some()).count();
    let last_clock = game
        .clocks
        .iter()
        .rev()
        .find_map(|clock| *clock)
        .map_or_else(|| "-".to_owned(), format_clock);
    format!(
        "{head} {color:<5}  {} ({}) - {} ({})  {:<7}  {:>3} plies  clocks {clocks}/{} (last {last_clock})  {}",
        game.white.username,
        game.white.rating,
        game.black.username,
        game.black.rating,
        game.result.as_str(),
        game.game.moves().len(),
        game.clocks.len(),
        game.url,
    )
}

fn format_clock(clock: Duration) -> String {
    let seconds = clock.as_secs();
    format!(
        "{}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}
