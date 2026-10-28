//! Calibration of the Elo model, in two reproducible steps.
//!
//! 1. [`collect`]: a sample of Chess.com games, analysed like any review.
//!    It starts from seed players (named, or drawn from a country's list
//!    with a fixed hash), then follows their opponents (snowball sampling),
//!    picking each time the player whose rating band has the fewest
//!    samples, until every band holds its quota. Each analysed game gives
//!    one sample per side: how they played, and their rating. Games after
//!    `until` are left out, so that the same options select the same games.
//! 2. [`fit`]: one model per time class on that sample (see
//!    [`chess_review::elo`]), saved as the model the reviews use.
//!
//! [`refresh`] computes the sample's features again from the cache when the
//! classification or the accuracy change, before fitting again.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use chess_chesscom::{ApiGame, GameFilter, PlayedGame, TimeClass};
use chess_core::Color;
use chess_openings::Book;
use chess_review::elo::{self, EloFeatures, EloModel, Sample};
use chess_review::{GameReview, SearchSettings, Thresholds, review_game};
use serde::{Deserialize, Serialize};

use crate::config::EngineKind;
use crate::error::Error;
use crate::jobs::{JobRequest, ReviewService};
use crate::store::{Store, analysis_key};

/// What [`collect`] samples.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectOptions {
    /// Players to start from.
    pub seeds: Vec<String>,
    /// Countries (ISO codes) whose players lists give more seeds.
    pub countries: Vec<String>,
    /// Seeds drawn from each country.
    pub country_players: usize,
    /// Time classes to calibrate, one after the other.
    pub time_classes: Vec<TimeClass>,
    /// Most recent rated games analysed per player.
    pub games_per_player: usize,
    /// Samples wanted in each rating band.
    pub per_band: usize,
    /// Width of a rating band.
    pub band: i32,
    /// Lowest rating sampled.
    pub min_rating: i32,
    /// Ratings sampled stay below this one.
    pub max_rating: i32,
    /// Most players whose games are downloaded, per time class.
    pub max_players: usize,
    /// Only games ended before this Unix time.
    pub until: Option<i64>,
    /// Engine of the analyses.
    pub engine: EngineKind,
    /// Settings of the analyses.
    pub settings: SearchSettings,
}

/// One calibration sample: one side of one game.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetSample {
    /// Time class of the game.
    pub time_class: TimeClass,
    /// The game.
    pub game_id: String,
    /// `white` or `black`.
    pub color: String,
    /// The player.
    pub username: String,
    /// Their rating in that game.
    pub rating: i32,
    /// How they played.
    pub features: EloFeatures,
}

/// The sample, with how it was drawn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dataset {
    /// The options of [`collect`].
    pub options: CollectOptions,
    /// Analysis key of the games (engine and settings).
    pub analysis: String,
    /// The samples, in the order they were drawn.
    pub samples: Vec<DatasetSample>,
}

/// What [`collect`] reports while it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectEvent {
    /// A player's games are being downloaded and analysed.
    Player {
        /// Time class being sampled.
        time_class: TimeClass,
        /// The player.
        username: String,
        /// Players done so far in this time class.
        index: usize,
        /// Samples so far in this time class.
        samples: usize,
    },
    /// A player was skipped (unknown, no games, failed analysis).
    Skipped {
        /// The player.
        username: String,
        /// Why.
        reason: String,
    },
}

/// FNV-1a, for orders that do not change between runs.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Collects a calibration sample (see the module documentation).
///
/// # Errors
///
/// Without a `User-Agent`, or when Chess.com or the cache fail on a
/// country list; failing players are skipped and reported instead.
pub async fn collect(
    service: &Arc<ReviewService>,
    options: &CollectOptions,
    mut on_event: impl FnMut(CollectEvent),
) -> Result<Dataset, Error> {
    let client = service.client().ok_or_else(|| {
        Error::Unavailable("calibration needs a User-Agent: set CHESS_REVIEW_USER_AGENT".into())
    })?;
    let mut seeds: Vec<String> = options.seeds.iter().map(|s| s.to_lowercase()).collect();
    for country in &options.countries {
        let mut players = client.country_players(country).await?;
        players.sort_by_key(|name| fnv(&format!("{country}/{name}")));
        seeds.extend(
            players
                .into_iter()
                .take(options.country_players)
                .map(|name| name.to_lowercase()),
        );
    }

    let mut samples = Vec::new();
    let mut analysis = String::new();
    for &time_class in &options.time_classes {
        let mut sampler = Sampler::new(options, &seeds);
        let mut players = 0;
        while players < options.max_players && !sampler.full() {
            let Some(username) = sampler.next() else {
                break;
            };
            players += 1;
            on_event(CollectEvent::Player {
                time_class,
                username: username.clone(),
                index: players,
                samples: sampler.samples.len(),
            });
            match sample_player(service, options, time_class, &username, &mut sampler).await {
                Ok(key) => analysis = key,
                Err(reason) => on_event(CollectEvent::Skipped { username, reason }),
            }
        }
        samples.extend(sampler.samples);
    }
    Ok(Dataset {
        options: options.clone(),
        analysis,
        samples,
    })
}

/// Downloads and analyses the games of `username`, and samples them; the
/// analysis key, or why nothing came of it.
async fn sample_player(
    service: &Arc<ReviewService>,
    options: &CollectOptions,
    time_class: TimeClass,
    username: &str,
    sampler: &mut Sampler,
) -> Result<String, String> {
    let filter = GameFilter {
        time_classes: vec![time_class],
        max_games: options.games_per_player,
        since: None,
        until: options.until,
        rated_only: true,
    };
    let job = service
        .start(JobRequest {
            username: username.to_owned(),
            filter: filter.clone(),
            engine: Some(options.engine),
            settings: Some(options.settings),
        })
        .map_err(|err| err.to_string())?;
    let mut updates = job.subscribe();
    let progress = loop {
        let progress = updates.borrow_and_update().clone();
        if progress.state.is_final() {
            break progress;
        }
        if updates.changed().await.is_err() {
            break job.progress();
        }
    };
    if let Some(error) = progress.error {
        return Err(error);
    }
    let engine = progress.engine.ok_or_else(|| "no games".to_owned())?;
    let key = analysis_key(&engine, &options.settings);
    let store = service.store();
    let games = store
        .games(username, &filter)
        .map_err(|err| err.to_string())?;
    for api in games {
        let Some((game, review)) = review(store, &api, &key, &service.config().classification)
        else {
            continue;
        };
        for color in [Color::White, Color::Black] {
            let player = game.player(color);
            sampler.meet(&player.username, player.rating);
            if let Some(features) = EloFeatures::of(review.player(color)) {
                sampler.offer(DatasetSample {
                    time_class,
                    game_id: game.id.clone(),
                    color: match color {
                        Color::White => "white",
                        Color::Black => "black",
                    }
                    .into(),
                    username: player.username.to_lowercase(),
                    rating: player.rating,
                    features,
                });
            }
        }
    }
    Ok(key)
}

/// The review of a cached game from its analysis `key`, as the reviews
/// make it; `None` when the analysis is missing or does not fit the game.
fn review(
    store: &Store,
    api: &ApiGame,
    key: &str,
    thresholds: &Thresholds,
) -> Option<(PlayedGame, GameReview)> {
    let game = PlayedGame::from_api(api).ok()?;
    let analysis = store.analysis(api.id(), key).ok()??;
    let book = Book::lichess()
        .classify(game.game.start(), game.game.moves())
        .book_plies;
    let review = review_game(
        game.game.start(),
        game.game.moves(),
        &analysis.positions,
        book,
        thresholds,
    )
    .ok()?;
    Some((game, review))
}

/// Computes the features of `dataset`'s samples again from the cached
/// analyses of their games, with `thresholds` and the current accuracy:
/// after a change of the classification, without downloading or analysing
/// anything. Samples whose game or analysis is not in the cache, or that
/// no longer have enough moves, are dropped; returns how many.
///
/// # Errors
///
/// When the cache fails.
pub fn refresh(
    store: &Store,
    thresholds: &Thresholds,
    dataset: &mut Dataset,
) -> Result<usize, Error> {
    let before = dataset.samples.len();
    let mut reviews: HashMap<String, Option<GameReview>> = HashMap::new();
    let mut kept = Vec::with_capacity(before);
    for mut sample in std::mem::take(&mut dataset.samples) {
        if !reviews.contains_key(&sample.game_id) {
            let review = store.game(&sample.game_id)?.and_then(|api| {
                review(store, &api, &dataset.analysis, thresholds).map(|(_, review)| review)
            });
            reviews.insert(sample.game_id.clone(), review);
        }
        let color = if sample.color == "white" {
            Color::White
        } else {
            Color::Black
        };
        let features = reviews[&sample.game_id]
            .as_ref()
            .and_then(|review| EloFeatures::of(review.player(color)));
        if let Some(features) = features {
            sample.features = features;
            kept.push(sample);
        }
    }
    dataset.samples = kept;
    Ok(before - dataset.samples.len())
}

/// The players met and the samples kept, for one time class.
struct Sampler {
    seeds: Vec<String>,
    /// Rating of each player met (lowercase), first seen.
    met: BTreeMap<String, i32>,
    visited: HashSet<String>,
    bands: HashMap<i32, usize>,
    taken: HashSet<(String, String)>,
    samples: Vec<DatasetSample>,
    per_band: usize,
    band: i32,
    min_rating: i32,
    max_rating: i32,
}

impl Sampler {
    fn new(options: &CollectOptions, seeds: &[String]) -> Self {
        Self {
            seeds: seeds.to_vec(),
            met: BTreeMap::new(),
            visited: HashSet::new(),
            bands: HashMap::new(),
            taken: HashSet::new(),
            samples: Vec::new(),
            per_band: options.per_band,
            band: options.band.max(1),
            min_rating: options.min_rating,
            max_rating: options.max_rating,
        }
    }

    /// Band of `rating`, if sampled.
    fn band_of(&self, rating: i32) -> Option<i32> {
        (self.min_rating..self.max_rating)
            .contains(&rating)
            .then(|| (rating - self.min_rating) / self.band)
    }

    fn bands(&self) -> i32 {
        (self.max_rating - self.min_rating + self.band - 1) / self.band
    }

    fn count(&self, band: i32) -> usize {
        self.bands.get(&band).copied().unwrap_or(0)
    }

    fn full(&self) -> bool {
        (0..self.bands()).all(|band| self.count(band) >= self.per_band)
    }

    fn meet(&mut self, username: &str, rating: i32) {
        self.met.entry(username.to_lowercase()).or_insert(rating);
    }

    fn offer(&mut self, sample: DatasetSample) {
        let Some(band) = self.band_of(sample.rating) else {
            return;
        };
        if self.count(band) >= self.per_band
            || !self
                .taken
                .insert((sample.game_id.clone(), sample.color.clone()))
        {
            return;
        }
        *self.bands.entry(band).or_default() += 1;
        self.samples.push(sample);
    }

    /// The next player: the seeds first, then the player met whose band
    /// has the fewest samples (the lowest rating, then the name, on ties).
    /// When no one met is in a band still short of samples, the player
    /// nearest to such a band: their opponents lead there.
    fn next(&mut self) -> Option<String> {
        if let Some(seed) = self.seeds.iter().find(|seed| !self.visited.contains(*seed)) {
            let seed = seed.clone();
            self.visited.insert(seed.clone());
            return Some(seed);
        }
        let unvisited = || {
            self.met
                .iter()
                .filter(|(name, _)| !self.visited.contains(*name))
        };
        let direct = unvisited()
            .filter_map(|(name, &rating)| {
                let band = self.band_of(rating)?;
                (self.count(band) < self.per_band).then_some((name, (self.count(band), rating)))
            })
            .min_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(b.0)))
            .map(|(name, _)| name);
        let short: Vec<i32> = (0..self.bands())
            .filter(|&band| self.count(band) < self.per_band)
            .map(|band| self.min_rating + band * self.band + self.band / 2)
            .collect();
        let bridge = || {
            unvisited()
                .filter_map(|(name, &rating)| {
                    let distance = short.iter().map(|centre| (rating - centre).abs()).min()?;
                    Some((name, distance))
                })
                .min_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(b.0)))
                .map(|(name, _)| name)
        };
        let name = direct.or_else(bridge)?.clone();
        self.visited.insert(name.clone());
        Some(name)
    }
}

/// Cross-validation folds of [`fit`].
pub const FOLDS: usize = 5;

/// Fits one model per time class of `dataset` (classes with too few
/// samples are left out).
#[must_use]
pub fn fit(dataset: &Dataset) -> EloModel {
    let mut by_class: BTreeMap<TimeClass, Vec<Sample>> = BTreeMap::new();
    for sample in &dataset.samples {
        by_class.entry(sample.time_class).or_default().push(Sample {
            features: sample.features,
            rating: f64::from(sample.rating),
            player: sample.username.clone(),
        });
    }
    let models = by_class
        .into_iter()
        .filter_map(|(time_class, samples)| {
            elo::fit(&samples, FOLDS).map(|model| (time_class.as_str().to_owned(), model))
        })
        .collect();
    EloModel {
        source: format!(
            "{} samples, analysis {}, games until {}",
            dataset.samples.len(),
            dataset.analysis,
            dataset
                .options
                .until
                .map_or_else(|| "the collection".to_owned(), |until| until.to_string()),
        ),
        models,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_review::Budget;

    fn options() -> CollectOptions {
        CollectOptions {
            seeds: vec!["seed".into()],
            countries: Vec::new(),
            country_players: 0,
            time_classes: vec![TimeClass::Blitz],
            games_per_player: 4,
            per_band: 2,
            band: 500,
            min_rating: 1000,
            max_rating: 2000,
            max_players: 10,
            until: None,
            engine: EngineKind::Stockfish,
            settings: SearchSettings {
                budget: Budget::Depth(12),
                multipv: 2,
            },
        }
    }

    fn sample(game: &str, rating: i32) -> DatasetSample {
        DatasetSample {
            time_class: TimeClass::Blitz,
            game_id: game.into(),
            color: "white".into(),
            username: format!("p{rating}"),
            rating,
            features: EloFeatures {
                accuracy: 80.0,
                acpl: 40.0,
                blunders: 2.0,
                mistakes: 2.0,
                inaccuracies: 4.0,
            },
        }
    }

    #[test]
    fn the_sampler_fills_the_emptiest_band_first() {
        let mut sampler = Sampler::new(&options(), &["seed".into()]);
        assert_eq!(sampler.next().as_deref(), Some("seed"));
        sampler.meet("Low", 1100);
        sampler.meet("high", 1700);
        sampler.meet("out", 2400);
        sampler.offer(sample("a", 1150));
        // The 1000-1500 band has a sample, the 1500-2000 one none.
        assert_eq!(sampler.next().as_deref(), Some("high"));
        assert_eq!(sampler.next().as_deref(), Some("low"));
        // Out of range, but the way to other players.
        assert_eq!(sampler.next().as_deref(), Some("out"));
        assert_eq!(sampler.next(), None);
        assert!(!sampler.full());
        sampler.offer(sample("b", 1200));
        sampler.offer(sample("c", 1250));
        sampler.offer(sample("a", 1150));
        assert_eq!(sampler.samples.len(), 2);
        sampler.offer(sample("d", 1600));
        sampler.offer(sample("e", 1999));
        sampler.offer(sample("f", 2000));
        assert!(sampler.full());
        assert_eq!(sampler.samples.len(), 4);
    }

    #[test]
    fn fit_makes_one_model_per_time_class() {
        let mut samples = Vec::new();
        for i in 0..60 {
            let mut s = sample(&format!("g{i}"), 1000 + 15 * i);
            s.features.accuracy = 70.0 + f64::from(i) / 3.0 + f64::from(i % 7) / 2.0;
            s.features.acpl = 80.0 - f64::from(i) + f64::from(i % 5);
            s.features.blunders = 6.0 - f64::from(i) / 12.0 + f64::from(i % 3) / 3.0;
            s.features.mistakes = 4.0 - f64::from(i) / 20.0 + f64::from(i % 4) / 4.0;
            samples.push(s);
        }
        let dataset = Dataset {
            options: options(),
            analysis: "test".into(),
            samples,
        };
        let model = fit(&dataset);
        assert_eq!(model.models.keys().collect::<Vec<_>>(), ["blitz"]);
        assert!(model.source.starts_with("60 samples"));
        let blitz = model.get("blitz").unwrap();
        assert!(blitz.slopes[0] > 0.0, "{blitz:?}");
        assert_eq!(model, fit(&dataset));
    }
}
