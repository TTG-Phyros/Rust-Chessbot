//! Training and engine-info endpoints, with a fake control daemon.

// Test helpers outside `#[test]` functions are not covered by clippy.toml.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum::routing::{get, post};
use chess_eval::nnue::NnueWeights;
use chess_server::AppState;
use chess_server::api::router;
use chess_server::api::types::{ControllerState, EngineInfo, TrainingView};
use chess_server::engine::EngineService;
use serde::de::DeserializeOwned;
use tower::ServiceExt;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("chess-server-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("metrics")).unwrap();
    dir
}

/// Pretends to be `chess-train serve`.
async fn fake_daemon(start_status: StatusCode) -> String {
    let app = Router::new()
        .route(
            "/status",
            get(|| async { r#"{"state":"running","run":"main","error":null}"# }),
        )
        .route("/start", post(move || async move { start_status }))
        .route("/stop", post(|| async { StatusCode::ACCEPTED }))
        .route("/timer", post(|| async { StatusCode::ACCEPTED }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

fn state(data_dir: PathBuf, trainer_url: Option<String>, eval_file: Option<PathBuf>) -> AppState {
    AppState {
        engine: EngineService::new(2, 1, eval_file),
        data_dir,
        trainer_url,
        http: reqwest::Client::new(),
        review: None,
        nnue: None,
    }
}

async fn request<T: DeserializeOwned>(
    state: AppState,
    method: &str,
    uri: &str,
) -> (StatusCode, Option<T>) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(if method == "POST" { "{}" } else { "" }))
        .unwrap();
    let response = router(state).oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).ok())
}

#[tokio::test]
async fn progress_without_a_daemon() {
    let dir = temp_dir("no-daemon");
    let (status, view) =
        request::<TrainingView>(state(dir.clone(), None, None), "GET", "/api/training").await;
    assert_eq!(status, StatusCode::OK);
    let view = view.unwrap();
    assert_eq!(view.controller.state, ControllerState::Unavailable);
    assert!(view.status.is_none());
    assert_eq!(view.engine.evaluator, "classic");

    let (status, _) =
        request::<serde_json::Value>(state(dir, None, None), "POST", "/api/training/start").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn unreachable_daemon_is_reported() {
    let dir = temp_dir("unreachable");
    // Nothing listens on this port.
    let url = Some("http://127.0.0.1:9".to_owned());
    let (_, view) = request::<TrainingView>(
        state(dir.clone(), url.clone(), None),
        "GET",
        "/api/training",
    )
    .await;
    let view = view.unwrap();
    assert_eq!(view.controller.state, ControllerState::Unavailable);
    assert!(view.controller.error.unwrap().contains("--profile train"));
    let (status, _) =
        request::<serde_json::Value>(state(dir, url, None), "POST", "/api/training/stop").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn progress_and_control_with_a_daemon() {
    let dir = temp_dir("daemon");
    std::fs::write(
        dir.join("metrics/status.json"),
        r#"{"run":"main","phase":"pipeline","device":"GPU NVIDIA GeForce RTX 4060","games":120,"positions":9000,"superbatch":2,"loss":0.03,"elo":-80.0,"elo_error":50.0}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("metrics/events.jsonl"),
        [
            r#"{"kind":"superbatch","run":"main","superbatch":1,"loss":0.05,"learning_rate":0.001,"positions_per_second":500000}"#,
            r#"{"kind":"superbatch","run":"main","superbatch":2,"loss":0.03,"learning_rate":0.001,"positions_per_second":510000}"#,
            r#"{"kind":"match","run":"main","superbatch":2,"elo":-80.0,"elo_error":50.0,"games":64}"#,
        ]
        .join("\n"),
    )
    .unwrap();
    let url = Some(fake_daemon(StatusCode::ACCEPTED).await);

    let (_, view) = request::<TrainingView>(
        state(dir.clone(), url.clone(), None),
        "GET",
        "/api/training",
    )
    .await;
    let view = view.unwrap();
    assert_eq!(view.controller.state, ControllerState::Running);
    let status = view.status.unwrap();
    assert_eq!((status.games, status.superbatch), (120, 2));
    assert_eq!(view.loss.len(), 2);
    assert_eq!(view.elo.len(), 1);

    let (code, _) = request::<serde_json::Value>(
        state(dir.clone(), url.clone(), None),
        "POST",
        "/api/training/start",
    )
    .await;
    assert_eq!(code, StatusCode::ACCEPTED);
    let (code, _) = request::<serde_json::Value>(
        state(dir.clone(), url.clone(), None),
        "POST",
        "/api/training/timer",
    )
    .await;
    assert_eq!(code, StatusCode::ACCEPTED);
    let (code, _) =
        request::<serde_json::Value>(state(dir.clone(), url, None), "POST", "/api/training/stop")
            .await;
    assert_eq!(code, StatusCode::ACCEPTED);

    let busy = Some(fake_daemon(StatusCode::CONFLICT).await);
    let (code, _) =
        request::<serde_json::Value>(state(dir, busy, None), "POST", "/api/training/start").await;
    assert_eq!(code, StatusCode::CONFLICT);
}

#[tokio::test]
async fn engine_uses_and_reloads_the_network() {
    let dir = temp_dir("engine");
    let net = dir.join("main.nnue");
    let shared = state(dir.clone(), None, Some(net.clone()));

    let (_, info) = request::<EngineInfo>(shared.clone(), "GET", "/api/engine").await;
    assert_eq!(info.unwrap().evaluator, "classic");

    std::fs::write(&net, NnueWeights::zeros().to_bytes()).unwrap();
    let (_, info) = request::<EngineInfo>(shared.clone(), "GET", "/api/engine").await;
    let info = info.unwrap();
    assert_eq!(info.evaluator, "nnue");
    assert!(info.weights_file.unwrap().ends_with("main.nnue"));

    // A broken file keeps the last good network.
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&net, b"half-written").unwrap();
    let (_, info) = request::<EngineInfo>(shared.clone(), "GET", "/api/engine").await;
    assert_eq!(info.unwrap().evaluator, "nnue");

    std::fs::remove_file(&net).unwrap();
    let (_, info) = request::<EngineInfo>(shared, "GET", "/api/engine").await;
    assert_eq!(info.unwrap().evaluator, "classic");
}

#[tokio::test]
async fn activating_a_lab_model_switches_the_engine() {
    use chess_jobs::{Broker, Device, Hyperparameters, JobKind, TrainingSummary, WorkerInfo};

    let dir = temp_dir("lab");
    let configured = dir.join("main.nnue");
    std::fs::write(&configured, NnueWeights::zeros().to_bytes()).unwrap();
    let lab = std::sync::Arc::new(
        chess_jobs::Lab::open(
            &dir.join("lab/lab.sqlite"),
            chess_jobs::LabPaths::under(&dir.join("lab")),
        )
        .unwrap(),
    );
    // A model, trained by a pretend worker.
    let dataset = {
        let path = dir.join("data.bin");
        let mut writer = chess_dataset::DatasetWriter::create(&path).unwrap();
        let record = chess_dataset::Record::new(
            &chess_core::Position::startpos(),
            0,
            chess_dataset::Outcome::Draw,
        );
        for _ in 0..64 {
            writer.write(&record).unwrap();
        }
        writer.finish().unwrap();
        lab.import_dataset("d", std::fs::File::open(path).unwrap(), 1 << 20)
            .unwrap()
    };
    let architecture = chess_nnue::spec::presets::get("fast-768")
        .unwrap()
        .architecture;
    lab.create_training(&chess_jobs::TrainingRequest {
        name: None,
        architecture: Some(serde_json::to_value(&architecture).unwrap()),
        architecture_id: None,
        datasets: vec![dataset.id],
        hyperparameters: Hyperparameters {
            batch_size: 64,
            shuffle_records: 64,
            ..Hyperparameters::default()
        },
        seed: None,
        device: Device::Cpu,
        trainer: "burn".into(),
    })
    .unwrap();
    let claim = lab
        .claim(&WorkerInfo {
            id: "w".into(),
            gpu: false,
            kinds: vec![JobKind::Training],
            trainers: vec!["burn".into()],
        })
        .unwrap()
        .unwrap();
    let network = chess_nnue::Network::pseudo_random(&architecture, 1)
        .unwrap()
        .to_bytes();
    let summary = TrainingSummary {
        superbatches: 1,
        positions: 64,
        final_loss: None,
        device: "CPU".into(),
    };
    let model = lab
        .complete_training(claim.token(), &network, &summary)
        .unwrap();

    let api = chess_nnue_api::NnueApi::new(
        std::sync::Arc::clone(&lab),
        chess_nnue_api::ApiConfig {
            auth: chess_nnue_api::Auth::Disabled,
            ..chess_nnue_api::ApiConfig::default()
        },
    );
    let shared = state(dir.clone(), None, Some(configured)).with_nnue(api);
    let weights = |state: AppState| async move {
        let (_, info) = request::<EngineInfo>(state, "GET", "/api/engine").await;
        info.unwrap().weights_file.unwrap()
    };
    assert!(weights(shared.clone()).await.ends_with("main.nnue"));

    let (status, _) = request::<serde_json::Value>(
        shared.clone(),
        "POST",
        &format!("/api/v1/models/{}:activate", model.id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        weights(shared.clone())
            .await
            .ends_with(&format!("{}.nnue", model.id))
    );

    // The activation survives a restart of the server.
    let restarted = state(dir.clone(), None, Some(dir.join("main.nnue"))).with_nnue(
        chess_nnue_api::NnueApi::new(lab, chess_nnue_api::ApiConfig::default()),
    );
    assert!(
        weights(restarted)
            .await
            .ends_with(&format!("{}.nnue", model.id))
    );

    let response = router(shared.clone())
        .oneshot(
            Request::post("/api/v1/models:deactivate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(weights(shared).await.ends_with("main.nnue"));
}
