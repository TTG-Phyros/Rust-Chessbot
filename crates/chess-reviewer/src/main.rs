//! `chess-reviewer` binary: Chess.com games from the command line.
//!
//! ```text
//! chess-reviewer fetch hikaru -n 20 --time-class blitz   # sync and list games
//! chess-reviewer estimate hikaru -n 10                   # how long, per preset
//! chess-reviewer analyse hikaru -n 10 --preset fast       # analyse them too
//! chess-reviewer calibrate collect --seed hikaru --country IS --until 2026-09-30
//! chess-reviewer calibrate refresh                        # after a classification change
//! chess-reviewer calibrate fit                            # config/elo-model.json
//! ```
//!
//! Settings come from `config/review.toml`, `CHESS_REVIEW_*` variables (a
//! `.env` file in the working directory is read first) and flags.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use chess_chesscom::{ApiGame, Client, GameFilter, PlayedGame, TimeClass, parse_date};
use chess_core::Color;
use chess_review::elo::FEATURE_NAMES;
use chess_review::{Budget, MoveClass, PlayerReview, Score, SearchSettings, review_game};
use chess_reviewer::calibration::{self, CollectEvent, CollectOptions, Dataset};
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
    /// Calibrates the Elo estimates: a sample of games, then the model.
    #[command(subcommand)]
    Calibrate(Calibrate),
}

#[derive(Debug, Subcommand)]
enum Calibrate {
    /// Downloads and analyses a sample of rated games, spread over the
    /// rating bands by following the opponents of seed players.
    Collect(Box<CollectArgs>),
    /// Computes the sample's features again from the cached analyses, after
    /// a change of the classification or of the accuracy (nothing is
    /// downloaded or analysed).
    Refresh {
        /// The sample (`calibrate collect`), rewritten in place.
        #[arg(long, default_value = "config/elo-samples.json")]
        data: PathBuf,
    },
    /// Fits the model (a ridge regression per time class) on a sample.
    Fit {
        /// The sample (`calibrate collect`).
        #[arg(long, default_value = "config/elo-samples.json")]
        data: PathBuf,
        /// Where to write the model (`elo_model` in config/review.toml).
        #[arg(long, default_value = "config/elo-model.json")]
        out: PathBuf,
    },
}

#[derive(Debug, clap::Args)]
struct CollectArgs {
    /// Player to start from (repeatable).
    #[arg(long = "seed")]
    seeds: Vec<String>,
    /// Country (ISO code) whose players list gives more seeds (repeatable).
    #[arg(long = "country")]
    countries: Vec<String>,
    /// Seeds drawn from each country's list.
    #[arg(long, default_value_t = 10)]
    country_players: usize,
    /// Time classes to calibrate.
    #[arg(long, value_delimiter = ',', default_value = "bullet,blitz,rapid")]
    time_class: Vec<TimeClass>,
    /// Most recent rated games analysed per player.
    #[arg(long, default_value_t = 4)]
    games_per_player: usize,
    /// Samples wanted per rating band.
    #[arg(long, default_value_t = 25)]
    per_band: usize,
    /// Width of a rating band.
    #[arg(long, default_value_t = 300)]
    band: i32,
    /// Lowest rating sampled.
    #[arg(long, default_value_t = 400)]
    min_rating: i32,
    /// Ratings sampled stay below this one.
    #[arg(long, default_value_t = 2800)]
    max_rating: i32,
    /// Most players visited per time class.
    #[arg(long, default_value_t = 80)]
    max_players: usize,
    /// Games ended on or before this day (YYYY-MM-DD): the same options
    /// then select the same games.
    #[arg(long)]
    until: String,
    #[command(flatten)]
    search: Search,
    /// Where to write the sample.
    #[arg(long, default_value = "config/elo-samples.json")]
    out: PathBuf,
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
        Command::Calibrate(Calibrate::Collect(args)) => collect(config, &args).await,
        Command::Calibrate(Calibrate::Refresh { data }) => refresh(&config, &data),
        Command::Calibrate(Calibrate::Fit { data, out }) => fit(&data, &out),
    }
}

async fn collect(config: ReviewConfig, args: &CollectArgs) -> Result<()> {
    if args.seeds.is_empty() && args.countries.is_empty() {
        bail!("give at least one --seed or --country");
    }
    let until = parse_date(&args.until)
        .with_context(|| format!("invalid date {:?} (YYYY-MM-DD)", args.until))?
        + 86_400;
    let options = CollectOptions {
        seeds: args.seeds.clone(),
        countries: args.countries.clone(),
        country_players: args.country_players,
        time_classes: args.time_class.clone(),
        games_per_player: args.games_per_player,
        per_band: args.per_band,
        band: args.band,
        min_rating: args.min_rating,
        max_rating: args.max_rating,
        max_players: args.max_players,
        until: Some(until),
        engine: args.search.engine.unwrap_or(config.analysis.engine),
        settings: args.search.settings(&config)?,
    };
    let service = Arc::new(ReviewService::new(config)?);
    let started = Instant::now();
    let dataset = calibration::collect(&service, &options, |event| match event {
        CollectEvent::Player {
            time_class,
            username,
            index,
            samples,
        } => eprintln!(
            "[{:>6.0} s] {time_class} #{index} {username} ({samples} samples)",
            started.elapsed().as_secs_f64()
        ),
        CollectEvent::Skipped { username, reason } => eprintln!("  skipped {username}: {reason}"),
    })
    .await?;
    for &time_class in &options.time_classes {
        let mut bands = std::collections::BTreeMap::new();
        for sample in dataset
            .samples
            .iter()
            .filter(|s| s.time_class == time_class)
        {
            let band = options.min_rating
                + (sample.rating - options.min_rating) / options.band * options.band;
            *bands.entry(band).or_insert(0) += 1;
        }
        let text: Vec<String> = bands
            .iter()
            .map(|(band, count)| format!("{band}: {count}"))
            .collect();
        println!("{time_class}: {}", text.join(", "));
    }
    write_json(&args.out, &dataset)?;
    println!(
        "{} samples written to {} in {}",
        dataset.samples.len(),
        args.out.display(),
        duration(started.elapsed().as_secs())
    );
    Ok(())
}

fn write_json(path: &std::path::Path, value: &impl serde::Serialize) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    std::fs::write(path, text).with_context(|| format!("cannot write {}", path.display()))
}

fn read_dataset(data: &std::path::Path) -> Result<Dataset> {
    let text =
        std::fs::read_to_string(data).with_context(|| format!("cannot read {}", data.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("{} is not a calibration sample", data.display()))
}

fn refresh(config: &ReviewConfig, data: &std::path::Path) -> Result<()> {
    let mut dataset = read_dataset(data)?;
    let store = Store::open(&config.cache)?;
    let dropped = calibration::refresh(&store, &config.classification, &mut dataset)?;
    write_json(data, &dataset)?;
    println!(
        "{} samples refreshed in {} ({dropped} dropped: game or analysis not cached)",
        dataset.samples.len(),
        data.display()
    );
    Ok(())
}

fn fit(data: &std::path::Path, out: &std::path::Path) -> Result<()> {
    let dataset = read_dataset(data)?;
    let model = calibration::fit(&dataset);
    if model.models.is_empty() {
        bail!("too few samples to fit any time class");
    }
    println!("{}", model.source);
    println!(
        "{:<7} {:>7} {:>9} {:>11} {:>9} {:>7} {:>9}   per 100 rating points ({})",
        "class",
        "samples",
        "game MAE",
        "player MAE",
        "game sd",
        "offset",
        "coverage",
        FEATURE_NAMES.join(", ")
    );
    for (class, m) in &model.models {
        let slopes: Vec<String> = m
            .slopes
            .iter()
            .map(|s| format!("{:+.2}", s * 100.0))
            .collect();
        println!(
            "{class:<7} {:>7} {:>9.0} {:>6.0} ({:.1}) {:>9.0} {:>7.0} {:>8.0} %   {}",
            m.samples,
            m.game_mae,
            m.player_mae,
            m.player_games,
            (1.0 / m.information).sqrt(),
            m.offset,
            m.coverage * 100.0,
            slopes.join(" ")
        );
    }
    write_json(out, &model)?;
    println!("model written to {}", out.display());
    Ok(())
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
