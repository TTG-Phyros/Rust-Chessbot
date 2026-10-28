//! `/api/v1/evaluations`: matches between a model and a baseline in the
//! engine's arena, with an optional SPRT, and comparisons of models over
//! the matches they played.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use chess_jobs::{
    Contestant, EvaluationRequest, Job, JobId, JobKind, JobSpec, MatchResult, ModelId, Progress,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::architectures::created;
use super::runs::{EventsQuery, event_stream, job_of, page, start_after};
use super::{JsonBody, custom_method};
use crate::NnueApi;
use crate::pagination::{JobPage, PageQuery};
use crate::problem::{ApiProblem, ApiResult, Problem};

/// Most players of a comparison.
const MAX_COMPARED: usize = 16;

/// Queues an evaluation match: the candidate model plays pairs of games
/// against the baseline (a model, or the classical evaluation) from the
/// same random opening with colors swapped, at a fixed number of nodes
/// (reproducible) or time per move. With `sprt`, the match stops as soon
/// as the test concludes. The worker first measures the speed (nodes per
/// second) of both engines.
#[utoipa::path(
    post,
    path = "/api/v1/evaluations",
    operation_id = "createEvaluation",
    tag = "evaluations",
    params(("Idempotency-Key" = Option<String>, Header, description = "Makes retries safe")),
    request_body = EvaluationRequest,
    responses(
        (status = 202, description = "Queued", body = Job),
        (status = 422, description = "Invalid request, with every error", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn create(
    State(api): State<NnueApi>,
    JsonBody(request): JsonBody<EvaluationRequest>,
) -> ApiResult<Response> {
    let job = api.lab.create_evaluation(&request)?;
    let mut response = created(&format!("/api/v1/evaluations/{}", job.id), &job);
    *response.status_mut() = StatusCode::ACCEPTED;
    Ok(response)
}

/// Evaluations, newest first.
#[utoipa::path(
    get,
    path = "/api/v1/evaluations",
    operation_id = "listEvaluations",
    tag = "evaluations",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of evaluations", body = JobPage),
        (status = 400, description = "Invalid cursor or limit", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn list(
    State(api): State<NnueApi>,
    Query(query): Query<PageQuery>,
) -> ApiResult<Json<JobPage>> {
    page(&api, JobKind::Evaluation, &query)
}

/// An evaluation, with its latest result: W/D/L, Elo difference ± its
/// 95 % error, likelihood of superiority, SPRT status, and the speed of
/// both engines. Its `spec` describes both contestants (name,
/// architecture, size), even once their models are deleted.
#[utoipa::path(
    get,
    path = "/api/v1/evaluations/{id}",
    operation_id = "getEvaluation",
    tag = "evaluations",
    params(("id" = String, Path, description = "Evaluation id")),
    responses(
        (status = 200, description = "The evaluation", body = Job),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn get(
    State(api): State<NnueApi>,
    Path(id): Path<String>,
) -> ApiResult<Json<Job>> {
    Ok(Json(job_of(&api, JobKind::Evaluation, &id)?))
}

/// Stops an evaluation: a queued one is canceled, a running one stops
/// after its current games and keeps their result.
#[utoipa::path(
    post,
    path = "/api/v1/evaluations/{id}:stop",
    operation_id = "stopEvaluation",
    tag = "evaluations",
    params(("id" = String, Path, description = "Evaluation id")),
    responses(
        (status = 200, description = "`stopping` (or `canceled`)", body = Job),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "Already over", body = Problem, content_type = "application/problem+json"),
    )
)]
#[allow(dead_code)] // Documents a method served by `act`.
pub(crate) async fn stop_doc() {}

/// `POST /evaluations/{id}:stop`.
pub(crate) async fn act(
    State(api): State<NnueApi>,
    Path(segment): Path<String>,
) -> ApiResult<Json<Job>> {
    let (id, method) = custom_method(&segment)?;
    let job = job_of(&api, JobKind::Evaluation, id)?;
    match method {
        "stop" => Ok(Json(api.lab.stop(job.id)?)),
        _ => Err(ApiProblem::not_found(format!(
            "evaluations have no method {method:?} (stop)"
        ))),
    }
}

/// Live events of an evaluation (Server-Sent Events): `status`, and a
/// `progress` after each batch of games. See the training runs' events.
#[utoipa::path(
    get,
    path = "/api/v1/evaluations/{id}/events",
    operation_id = "streamEvaluationEvents",
    tag = "evaluations",
    params(
        ("id" = String, Path, description = "Evaluation id"),
        EventsQuery,
        ("Last-Event-ID" = Option<String>, Header, description = "Resume after this event"),
    ),
    responses(
        (status = 200, description = "Event stream; each `data` is a JobEvent (JSON)", body = String, content_type = "text/event-stream"),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn events(
    State(api): State<NnueApi>,
    Path(id): Path<String>,
    Query(query): Query<EventsQuery>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let job = job_of(&api, JobKind::Evaluation, &id)?;
    Ok(event_stream(api, job.id, start_after(&query, &headers)))
}

/// Players to compare.
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct CompareQuery {
    /// Comma-separated model ids, 1 to 16; `classic` stands for the
    /// classical evaluation. Ratings are relative to the first.
    pub(crate) players: String,
}

/// A player of a comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ComparedPlayer {
    /// Who it is: name, architecture, network size.
    pub(crate) contestant: Contestant,
    /// Elo relative to the first player, fitted over the games of every
    /// pair; absent when no games link it to the first player.
    pub(crate) elo: Option<f64>,
    /// Games it played against the other players.
    pub(crate) games: u32,
    /// Nodes per second measured by its latest evaluation (on that
    /// evaluation's worker: compare speeds measured on the same machine).
    pub(crate) nps: Option<u64>,
}

/// The games between two players of a comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairResult {
    /// Index of the first player in `players`.
    pub(crate) a: usize,
    /// Index of the second player (always greater than `a`).
    pub(crate) b: usize,
    /// Games `a` won.
    pub(crate) wins: u32,
    /// Games drawn.
    pub(crate) draws: u32,
    /// Games `a` lost.
    pub(crate) losses: u32,
    /// Elo difference of `a` over `b`.
    pub(crate) elo: f64,
    /// Half-width of the 95 % confidence interval of `elo`.
    pub(crate) elo_error: f64,
    /// Likelihood that `a` is the stronger.
    pub(crate) los: f64,
    /// Evaluations whose games are counted.
    pub(crate) evaluations: Vec<JobId>,
}

/// Several players compared over the evaluations between them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Comparison {
    /// The players, in the order asked.
    pub(crate) players: Vec<ComparedPlayer>,
    /// Every pair that played, `a` before `b`.
    pub(crate) pairs: Vec<PairResult>,
}

/// Compares models (and the classical evaluation) over every evaluation
/// played between two of them, whatever its settings: network size,
/// speed, results of each pair, and Elo ratings fitted over all the games
/// (Bradley–Terry maximum likelihood, relative to the first player).
#[utoipa::path(
    get,
    path = "/api/v1/evaluations:compare",
    operation_id = "compareModels",
    tag = "evaluations",
    params(CompareQuery),
    responses(
        (status = 200, description = "The comparison", body = Comparison),
        (status = 400, description = "Invalid list of players", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "Unknown model", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn compare(
    State(api): State<NnueApi>,
    Query(query): Query<CompareQuery>,
) -> ApiResult<Json<Comparison>> {
    let keys = parse_players(&query.players)?;
    let mut players = Vec::with_capacity(keys.len());
    for key in &keys {
        let contestant = match key {
            None => Contestant::classic(),
            Some(id) => Contestant::of(&api.lab.model(*id)?),
        };
        players.push(ComparedPlayer {
            contestant,
            elo: None,
            games: 0,
            nps: None,
        });
    }
    let index = |key: Option<ModelId>| keys.iter().position(|k| *k == key);

    let mut pairs: BTreeMap<(usize, usize), (MatchResult, Vec<JobId>)> = BTreeMap::new();
    for job in api.lab.evaluation_results()? {
        let (JobSpec::Evaluation(spec), Some(Progress::Evaluation(progress))) =
            (&job.spec, &job.progress)
        else {
            continue;
        };
        let (candidate, baseline) = (index(spec.candidate.model), index(spec.baseline.model));
        // Latest speeds, whatever the opponent.
        if let (Some(i), Some(nps)) = (candidate, progress.candidate_nps) {
            players[i].nps = Some(nps);
        }
        if let (Some(j), Some(nps)) = (baseline, progress.baseline_nps) {
            players[j].nps = Some(nps);
        }
        let (Some(i), Some(j)) = (candidate, baseline) else {
            continue;
        };
        if i == j || progress.games == 0 {
            continue;
        }
        let (key, result) = if i < j {
            ((i, j), progress.result())
        } else {
            ((j, i), progress.result().reversed())
        };
        let (total, evaluations) = pairs.entry(key).or_default();
        *total += result;
        evaluations.push(job.id);
    }

    let results: Vec<(usize, usize, MatchResult)> = pairs
        .iter()
        .map(|(&(a, b), (result, _))| (a, b, *result))
        .collect();
    for (player, elo) in players
        .iter_mut()
        .zip(chess_jobs::ratings(keys.len(), &results))
    {
        player.elo = elo;
    }
    for &(a, b, result) in &results {
        players[a].games += result.games();
        players[b].games += result.games();
    }
    let pairs = pairs
        .into_iter()
        .map(|((a, b), (result, evaluations))| PairResult {
            a,
            b,
            wins: result.wins,
            draws: result.draws,
            losses: result.losses,
            elo: result.elo(),
            elo_error: result.elo_error(),
            los: result.los(),
            evaluations,
        })
        .collect();
    Ok(Json(Comparison { players, pairs }))
}

/// The players of `players`: model ids, `None` for `classic`.
fn parse_players(players: &str) -> ApiResult<Vec<Option<ModelId>>> {
    let mut keys = Vec::new();
    for item in players.split(',').map(str::trim) {
        let key = match item {
            "classic" => None,
            id => Some(id.parse().map_err(|_| {
                ApiProblem::bad_request(format!(
                    "players: {id:?} is neither a model id nor classic"
                ))
            })?),
        };
        if keys.contains(&key) {
            return Err(ApiProblem::bad_request(format!(
                "players: {item} is listed twice"
            )));
        }
        keys.push(key);
    }
    if keys.len() > MAX_COMPARED {
        return Err(ApiProblem::bad_request(format!(
            "players: at most {MAX_COMPARED} players"
        )));
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn players_are_parsed() {
        let id = ModelId::generate();
        assert_eq!(
            parse_players(&format!("{id}, classic")).unwrap(),
            vec![Some(id), None]
        );
        assert!(parse_players("classic,classic").is_err());
        assert!(parse_players("").is_err());
        assert!(parse_players("../etc").is_err());
        let many: Vec<String> = (0..17).map(|_| ModelId::generate().to_string()).collect();
        assert!(parse_players(&many.join(",")).is_err());
    }
}
