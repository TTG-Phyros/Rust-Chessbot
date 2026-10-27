//! `chess-reviewer` binary: Chess.com games from the command line.
//!
//! ```text
//! chess-reviewer fetch hikaru -n 20 --time-class blitz   # sync and list games
//! chess-reviewer estimate hikaru -n 10                   # how long, per preset
//! chess-reviewer analyse hikaru -n 10 --preset fast       # analyse them too
//! ```
//!
//! Settings come from `config/review.toml`, `CHESS_REVIEW_*` variables (a
//! `.env` file in the working directory is read first) and flags.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use chess_chesscom::{ApiGame, Client, GameFilter, PlayedGame, TimeClass, parse_date};
use chess_core::Color;
use chess_review::{Budget, MoveClass, PlayerReview, Score, SearchSettings, review_game};
use chess_reviewer::{
    EngineKind, JobRequest, JobState, MonthSource, ReviewConfig, ReviewService, Store, SyncEvent,
    analysis_key, sync_player,
};
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
    Fetch(Selection),
    /// Analyses a player's games (those not analysed yet with these
    /// settings) and prints the evaluations of the most recent one.
    Analyse {
        #[command(flatten)]
        selection: Selection,
        #[command(flatten)]
        search: Search,
    },
    /// How long analysing a player's games would take with each preset.
    Estimate {
        #[command(flatten)]
        selection: Selection,
        #[command(flatten)]
        search: Search,
    },
}

/// Engine and search settings.
#[derive(Debug, clap::Args)]
struct Search {
    /// Engine: `stockfish` or `native` (the configured one by default).
    #[arg(long)]
    engine: Option<EngineKind>,
    /// Preset (see config/review.toml: fast, balanced, deep); the
    /// configured one by default. The options below override it.
    #[arg(long)]
    preset: Option<String>,
    /// Depth of each search.
    #[arg(long, conflicts_with = "movetime")]
    depth: Option<u32>,
    /// Time of each search, in milliseconds.
    #[arg(long)]
    movetime: Option<u64>,
    /// Best lines per position.
    #[arg(long)]
    multipv: Option<usize>,
}

impl Search {
    fn settings(&self, config: &ReviewConfig) -> Result<SearchSettings> {
        let base = match &self.preset {
            Some(name) => config
                .preset(name)
                .with_context(|| format!("unknown preset {name:?}"))?
                .settings(),
            None => config.search_settings(),
        };
        Ok(SearchSettings {
            budget: match (self.depth, self.movetime) {
                (Some(depth), _) => Budget::Depth(depth),
                (None, Some(ms)) => Budget::MoveTimeMs(ms),
                (None, None) => base.budget,
            },
            multipv: self.multipv.unwrap_or(base.multipv),
        })
    }
}

/// Which games of which player.
#[derive(Debug, clap::Args)]
struct Selection {
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
}

impl Selection {
    fn filter(&self) -> Result<GameFilter> {
        let day = |text: &Option<String>| -> Result<Option<i64>> {
            text.as_deref()
                .map(|text| {
                    parse_date(text).with_context(|| format!("invalid date {text:?} (YYYY-MM-DD)"))
                })
                .transpose()
        };
        if self.max_games == 0 {
            bail!("--max-games must be at least 1");
        }
        Ok(GameFilter {
            time_classes: self.time_class.clone(),
            max_games: self.max_games,
            since: day(&self.since)?,
            // Inclusive: up to the end of that day.
            until: day(&self.until)?.map(|start| start + 86_400),
            rated_only: self.rated,
        })
    }
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
        Command::Fetch(selection) => {
            fetch(&config, &selection.username, &selection.filter()?).await
        }
        Command::Analyse { selection, search } => {
            let request = JobRequest {
                username: selection.username.clone(),
                filter: selection.filter()?,
                engine: search.engine,
                settings: Some(search.settings(&config)?),
            };
            analyse(config, request).await
        }
        Command::Estimate { selection, search } => estimate(config, &selection, &search).await,
    }
}

async fn estimate(config: ReviewConfig, selection: &Selection, search: &Search) -> Result<()> {
    let mut candidates: Vec<(String, SearchSettings)> = config
        .analysis
        .presets
        .iter()
        .map(|preset| {
            (
                format!("{} ({})", preset.label, preset.name),
                preset.settings(),
            )
        })
        .collect();
    let own = search.settings(&config)?;
    if candidates.iter().all(|(_, settings)| *settings != own) {
        candidates.push(("custom".to_owned(), own));
    }
    let workers = config.workers();
    let service = ReviewService::new(config)?;
    let settings: Vec<SearchSettings> = candidates.iter().map(|(_, s)| *s).collect();
    eprintln!("timing the settings never used before (a few seconds each)...");
    let estimates = service
        .estimate(
            &selection.username,
            &selection.filter()?,
            search.engine,
            &settings,
        )
        .await?;
    println!("{workers} worker(s)");
    for ((name, settings), estimate) in candidates.iter().zip(&estimates) {
        println!(
            "{name:<24} {:<10} multipv {}  {:>3} game(s) to analyse ({} cached), {:>5} positions, \
             {:.2} s/position ({}): {}",
            settings.budget.to_string(),
            settings.multipv,
            estimate.pending_games,
            estimate.cached_games,
            estimate.positions,
            estimate.seconds_per_position,
            if estimate.measured {
                "measured"
            } else {
                "benchmark"
            },
            duration(estimate.seconds),
        );
    }
    Ok(())
}

/// `1 h 05 min`, `3 min 20 s`, `45 s`.
fn duration(seconds: u64) -> String {
    match seconds {
        0..60 => format!("{seconds} s"),
        60..3600 => format!("{} min {:02} s", seconds / 60, seconds % 60),
        _ => format!("{} h {:02} min", seconds / 3600, seconds / 60 % 60),
    }
}

async fn analyse(config: ReviewConfig, request: JobRequest) -> Result<()> {
    let workers = config.workers();
    let settings = request.settings.unwrap_or_else(|| config.search_settings());
    let service = Arc::new(ReviewService::new(config)?);
    let started = Instant::now();
    let job = service.start(request)?;
    let mut updates = job.subscribe();
    let mut last_line = String::new();
    let progress = loop {
        let progress = updates.borrow_and_update().clone();
        let line = match progress.state {
            JobState::Fetching => format!(
                "fetching: {}/{} month(s)",
                progress.months_done, progress.months_total
            ),
            _ => format!(
                "{:?}: {} game(s), {} cached, {} analysed, {} failed; positions {}/{} ({}), {}",
                progress.state,
                progress.games.len(),
                progress.games_cached,
                progress.games_done,
                progress.games_failed,
                progress.positions_done,
                progress.positions_total,
                progress.engine.as_deref().unwrap_or("-"),
                progress
                    .eta_secs
                    .map_or_else(|| "-".to_owned(), |eta| format!("{} left", duration(eta))),
            ),
        };
        // One line per change of stage or game, not per position.
        if line.split(';').next() != last_line.split(';').next() || progress.state.is_final() {
            eprintln!("{line}");
        }
        last_line = line;
        if progress.state.is_final() {
            break progress;
        }
        if updates.changed().await.is_err() {
            break progress;
        }
    };
    eprintln!(
        "{:.1} s with {workers} worker(s)\n",
        started.elapsed().as_secs_f64()
    );
    if let Some(error) = &progress.error {
        bail!("{error}");
    }
    // The analyses of these settings, by this engine.
    let Some(key) = progress
        .engine
        .as_deref()
        .map(|engine| analysis_key(engine, &settings))
    else {
        return Ok(());
    };
    let store = service.store();
    let mut reviewed = Vec::new();
    for id in &progress.games {
        let (Some(api), Some(analysis)) = (store.game(id)?, store.analysis(id, &key)?) else {
            continue;
        };
        let game = PlayedGame::from_api(&api)?;
        let book = chess_openings::Book::lichess()
            .classify(game.game.start(), game.game.moves())
            .book_plies;
        let review = review_game(
            game.game.start(),
            game.game.moves(),
            &analysis.positions,
            book,
            &service.config().classification,
        )?;
        let accuracy = |color| {
            review
                .player(color)
                .accuracy
                .map_or_else(|| "-".to_owned(), |accuracy| format!("{accuracy:.1}"))
        };
        println!(
            "{}  {} {} - {} {}  {:<7}  accuracy {} / {}{}",
            game.url,
            game.white.username,
            game.white.rating,
            game.black.username,
            game.black.rating,
            game.result.as_str(),
            accuracy(Color::White),
            accuracy(Color::Black),
            game.chesscom_accuracies
                .map_or_else(String::new, |(white, black)| format!(
                    "  (Chess.com {white:.1} / {black:.1})"
                )),
        );
        reviewed.push((game, analysis, review));
    }
    let Some((game, analysis, review)) = reviewed.into_iter().next() else {
        return Ok(());
    };
    println!(
        "\n{}  ({} - {})\n{} | {}",
        game.url,
        game.white.username,
        game.black.username,
        analysis.engine,
        analysis.settings.budget,
    );
    let mut position = game.game.start().clone();
    for (ply, &mv) in game.game.moves().iter().enumerate() {
        let verdict = &review.moves[ply];
        let before = &analysis.positions[ply];
        let after = &analysis.positions[ply + 1];
        let mover = position.side_to_move();
        let san = position.san(mv);
        let best = before
            .best_move()
            .and_then(|uci| position.parse_uci_move(uci).ok())
            .map(|best| position.san(best))
            .unwrap_or_default();
        position.make_move(mv);
        // Evaluations from White's point of view, as on a graph.
        let white = |score: Score, to_move: Color| match to_move {
            Color::White => score,
            Color::Black => score.negate(),
        };
        let number = if mover == Color::White {
            format!("{}.", ply / 2 + 1)
        } else {
            format!("{}...", ply / 2 + 1)
        };
        println!(
            "{number:>6} {san:<8} {:<10} eval {:>8}   best was {best:<8} ({})  accuracy {:>5.1}",
            verdict.class.name(),
            show(white(after.score(), !mover)),
            show(white(before.score(), mover)),
            verdict.accuracy,
        );
    }
    for (color, name) in [
        (Color::White, &game.white.username),
        (Color::Black, &game.black.username),
    ] {
        println!("{name}: {}", player_summary(review.player(color)));
    }
    Ok(())
}

/// Accuracy, ACPL and the number of moves of each class.
fn player_summary(player: &PlayerReview) -> String {
    let mut summary = format!(
        "accuracy {}, ACPL {}",
        player
            .accuracy
            .map_or_else(|| "-".to_owned(), |accuracy| format!("{accuracy:.1}")),
        player
            .acpl
            .map_or_else(|| "-".to_owned(), |acpl| format!("{acpl:.0}")),
    );
    for class in MoveClass::ALL {
        let count = player.counts.get(class);
        if count > 0 {
            summary.push_str(&format!(", {count} {}", class.name()));
        }
    }
    summary
}

fn show(score: Score) -> String {
    match score {
        Score::Cp(cp) => format!("{:+.2}", f64::from(cp) / 100.0),
        Score::Mate(0) => "#".to_owned(),
        Score::Mate(moves) => format!("#{moves}"),
    }
}

async fn fetch(config: &ReviewConfig, username: &str, filter: &GameFilter) -> Result<()> {
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
    let opening = chess_openings::Book::lichess()
        .classify(game.game.start(), game.game.moves())
        .opening
        .map_or_else(|| "-".to_owned(), |o| format!("{} {}", o.eco, o.name));
    let clocks = game.clocks.iter().filter(|clock| clock.is_some()).count();
    let last_clock = game
        .clocks
        .iter()
        .rev()
        .find_map(|clock| *clock)
        .map_or_else(|| "-".to_owned(), format_clock);
    format!(
        "{head} {color:<5}  {} ({}) - {} ({})  {:<7}  {:>3} plies  clocks {clocks}/{} (last {last_clock})  {}  {opening}",
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
