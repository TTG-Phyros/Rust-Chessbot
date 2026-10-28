//! The REST API against an in-memory router. Every JSON response is checked
//! against the schema the OpenAPI document gives for its route and status.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use chess_core::Position;
use chess_dataset::{DatasetWriter, Outcome, Record};
use chess_jobs::{Lab, LabPaths};
use chess_nnue::Network;
use chess_nnue::spec::presets;
use chess_nnue_api::{ApiConfig, Auth, NnueApi, openapi};
use serde_json::{Value, json};
use tower::ServiceExt;

const KEY: &str = "test-key";
const WORKER: &str = "worker-token";

struct Harness {
    router: Router,
    lab: Arc<Lab>,
    activated: Arc<Mutex<Vec<Option<PathBuf>>>>,
    dir: PathBuf,
}

fn harness(name: &str, config: ApiConfig) -> Harness {
    let dir = std::env::temp_dir()
        .join("chess-nnue-api-tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let lab = Arc::new(Lab::open(&dir.join("lab.sqlite"), LabPaths::under(&dir)).unwrap());
    let activated = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&activated);
    let api = NnueApi::new(Arc::clone(&lab), config)
        .on_activate(Arc::new(move |file| log.lock().unwrap().push(file)));
    Harness {
        router: api.router(),
        lab,
        activated,
        dir,
    }
}

fn config() -> ApiConfig {
    ApiConfig {
        auth: Auth::Keys(vec!["other-key".into(), KEY.into()]),
        rate_limit: 0,
        max_upload_bytes: 1 << 20,
        worker_token: Some(WORKER.into()),
    }
}

/// A response: status, headers, body.
struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|err| panic!("{err}: {}", String::from_utf8_lossy(&self.body)))
    }

    fn content_type(&self) -> &str {
        self.headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
    }

    fn problem_type(&self) -> String {
        assert_eq!(self.content_type(), "application/problem+json");
        self.json()["type"].as_str().unwrap().to_owned()
    }
}

impl Harness {
    async fn send(&self, request: Request<Body>) -> Reply {
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec();
        Reply {
            status,
            headers,
            body,
        }
    }

    /// An authenticated request, checked against the contract.
    async fn call(&self, method: Method, uri: &str, body: Option<Value>) -> Reply {
        let mut request = Request::builder()
            .method(method.clone())
            .uri(uri)
            .header("x-api-key", KEY);
        let body = match body {
            Some(json) => {
                request = request.header(header::CONTENT_TYPE, "application/json");
                Body::from(json.to_string())
            }
            None => Body::empty(),
        };
        let reply = self.send(request.body(body).unwrap()).await;
        check_contract(&method, uri, &reply);
        reply
    }

    async fn upload(&self, name: &str, bytes: Vec<u8>) -> Reply {
        let request = Request::post(format!("/api/v1/datasets?name={name}"))
            .header("x-api-key", KEY)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(bytes))
            .unwrap();
        let reply = self.send(request).await;
        check_contract(&Method::POST, "/api/v1/datasets", &reply);
        reply
    }

    /// A worker's request to the internal API.
    async fn internal(&self, path: &str, body: Body, content_type: &str) -> Reply {
        let request = Request::post(path)
            .header(header::AUTHORIZATION, format!("Bearer {WORKER}"))
            .header(header::CONTENT_TYPE, content_type)
            .body(body)
            .unwrap();
        self.send(request).await
    }
}

/// The documented path template matching `uri` (custom methods included).
fn template(document: &Value, method: &Method, uri: &str) -> Option<String> {
    let path = uri.split('?').next().unwrap_or(uri);
    let parts: Vec<&str> = path.split('/').collect();
    document["paths"]
        .as_object()?
        .iter()
        .find_map(|(template, item)| {
            item.get(method.as_str().to_ascii_lowercase())?;
            let pattern: Vec<&str> = template.split('/').collect();
            let matches = pattern.len() == parts.len()
                && pattern.iter().zip(&parts).all(|(p, s)| {
                    if let Some(rest) = p.strip_prefix("{id}") {
                        // `{id}` or `{id}:method`
                        s.ends_with(rest) && s.len() > rest.len()
                    } else {
                        p == s
                    }
                });
            matches.then(|| template.clone())
        })
}

/// Checks that `reply` is documented for this route and matches its schema.
fn check_contract(method: &Method, uri: &str, reply: &Reply) {
    let document = openapi();
    let template = template(&document, method, uri)
        .unwrap_or_else(|| panic!("{method} {uri} is not documented"));
    let operation = &document["paths"][&template][method.as_str().to_ascii_lowercase()];
    let response = &operation["responses"][reply.status.as_str()];
    assert!(
        response.is_object(),
        "{method} {template}: status {} not documented ({})",
        reply.status,
        String::from_utf8_lossy(&reply.body)
    );
    let content_type = reply
        .content_type()
        .split(';')
        .next()
        .unwrap_or("")
        .to_owned();
    if !content_type.contains("json") {
        return;
    }
    let schema = &response["content"][&content_type]["schema"];
    assert!(
        schema.is_object(),
        "{method} {template} {}: {content_type} not documented",
        reply.status
    );
    let root = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "components": document["components"],
        "allOf": [schema],
    });
    let validator = jsonschema::validator_for(&root).unwrap();
    let body = reply.json();
    let errors: Vec<String> = validator
        .iter_errors(&body)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "{method} {template} {}: {errors:#?}\n{body:#}",
        reply.status
    );
}

fn dataset_bytes(dir: &Path, positions: usize) -> Vec<u8> {
    let path = dir.join("source.bin");
    let mut writer = DatasetWriter::create(&path).unwrap();
    let pos = Position::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1").unwrap();
    for i in 0..positions {
        writer
            .write(&Record::new(&pos, i as i16, Outcome::WhiteWins))
            .unwrap();
    }
    writer.finish().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    bytes
}

fn architecture(id: &str) -> Value {
    serde_json::to_value(&presets::get(id).unwrap().architecture).unwrap()
}

#[tokio::test]
async fn the_document_resolves_and_describes_architectures_exactly() {
    let document = openapi();
    assert_eq!(document["openapi"], "3.1.0");
    // Every reference points at a component.
    fn refs(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Object(o) => {
                if let Some(Value::String(r)) = o.get("$ref") {
                    out.push(r.clone());
                }
                o.values().for_each(|v| refs(v, out));
            }
            Value::Array(a) => a.iter().for_each(|v| refs(v, out)),
            _ => {}
        }
    }
    let mut all = Vec::new();
    refs(&document, &mut all);
    for reference in &all {
        assert!(
            document
                .pointer(reference.trim_start_matches('#'))
                .is_some(),
            "dangling {reference}"
        );
    }
    // The architecture component is the spec's JSON Schema: it accepts the
    // presets and rejects what the server rejects structurally.
    let root = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "components": document["components"],
        "$ref": "#/components/schemas/Architecture",
    });
    let validator = jsonschema::validator_for(&root).unwrap();
    for preset in presets::all() {
        assert!(validator.is_valid(&serde_json::to_value(&preset.architecture).unwrap()));
    }
    let mut bad = architecture("fast-768");
    bad["accumulator"]["activation"] = json!("tanh");
    assert!(!validator.is_valid(&bad));
    assert!(document["components"]["securitySchemes"]["api_key"].is_object());
}

#[tokio::test]
async fn keys_are_required_except_for_the_documentation() {
    let h = harness("auth", config());
    let anonymous = h
        .send(Request::get("/api/v1/models").body(Body::empty()).unwrap())
        .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        anonymous.problem_type(),
        "urn:chess-bot:problem:unauthorized"
    );
    assert!(anonymous.headers.contains_key(header::WWW_AUTHENTICATE));
    let wrong = h
        .send(
            Request::get("/api/v1/models")
                .header("x-api-key", "nope")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    let bearer = h
        .send(
            Request::get("/api/v1/models")
                .header(header::AUTHORIZATION, format!("Bearer {KEY}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(bearer.status, StatusCode::OK);

    for public in ["/api/v1/openapi.json", "/docs"] {
        let reply = h
            .send(Request::get(public).body(Body::empty()).unwrap())
            .await;
        assert_eq!(reply.status, StatusCode::OK, "{public}");
    }
    let docs = h
        .send(Request::get("/docs").body(Body::empty()).unwrap())
        .await;
    assert!(String::from_utf8_lossy(&docs.body).contains("/api/v1/openapi.json"));

    let open = harness(
        "auth-disabled",
        ApiConfig {
            auth: Auth::Disabled,
            ..config()
        },
    );
    let reply = open
        .send(Request::get("/api/v1/models").body(Body::empty()).unwrap())
        .await;
    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn requests_are_rate_limited_per_key() {
    let h = harness(
        "rate",
        ApiConfig {
            rate_limit: 3,
            ..config()
        },
    );
    for _ in 0..3 {
        assert_eq!(
            h.call(Method::GET, "/api/v1/models", None).await.status,
            StatusCode::OK
        );
    }
    let limited = h.call(Method::GET, "/api/v1/models", None).await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.problem_type(), "urn:chess-bot:problem:rate-limited");
    let wait: u64 = limited.headers[header::RETRY_AFTER]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=20).contains(&wait), "{wait}");
}

#[tokio::test]
async fn architectures_are_validated_saved_and_paged() {
    let h = harness("architectures", config());
    let valid = h
        .call(
            Method::POST,
            "/api/v1/architectures:validate",
            Some(architecture("halfka-medium")),
        )
        .await
        .json();
    assert_eq!(valid["valid"], true);
    assert_eq!(
        valid["hash"],
        presets::get("halfka-medium")
            .unwrap()
            .architecture
            .hash()
            .to_string()
    );
    assert!(valid["estimate"]["parameters"].as_u64().unwrap() > 0);

    let mut bad = architecture("fast-768");
    bad["accumulator"]["size"] = json!(100);
    bad["quantization"]["qa"] = json!(0);
    let invalid = h
        .call(
            Method::POST,
            "/api/v1/architectures:validate",
            Some(bad.clone()),
        )
        .await
        .json();
    assert_eq!(invalid["valid"], false);
    let paths: Vec<&str> = invalid["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"/accumulator/size"), "{paths:?}");
    assert!(paths.contains(&"/quantization/qa"), "{paths:?}");

    // YAML works too.
    let yaml = "schemaVersion: 1\nmetadata: {name: tiny}\nfeatures: {set: Chess768}\n\
        accumulator: {size: 32, activation: CReLU}\n\
        quantization: {weights: i8, qa: 255, qb: 64, evalScale: 400}\n";
    let reply = h
        .send(
            Request::post("/api/v1/architectures:validate")
                .header("x-api-key", KEY)
                .header(header::CONTENT_TYPE, "application/yaml")
                .body(Body::from(yaml))
                .unwrap(),
        )
        .await;
    assert_eq!(reply.json()["valid"], true);
    let reply = h
        .send(
            Request::post("/api/v1/architectures:validate")
                .header("x-api-key", KEY)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from(yaml))
                .unwrap(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let presets = h
        .call(Method::GET, "/api/v1/architectures/presets", None)
        .await
        .json();
    assert_eq!(presets.as_array().unwrap().len(), presets::all().len());
    let schema = h
        .call(Method::GET, "/api/v1/architectures/schema", None)
        .await;
    assert_eq!(schema.content_type(), "application/schema+json");
    assert_eq!(schema.json(), chess_nnue_spec::json_schema());

    // Saved architectures page newest first.
    let mut ids = Vec::new();
    for id in ["fast-768", "halfka-medium", "multilayer"] {
        let reply = h
            .call(
                Method::POST,
                "/api/v1/architectures",
                Some(architecture(id)),
            )
            .await;
        assert_eq!(reply.status, StatusCode::CREATED);
        let stored = reply.json();
        assert_eq!(
            reply.headers[header::LOCATION],
            format!("/api/v1/architectures/{}", stored["id"].as_str().unwrap())
        );
        ids.push(stored["id"].as_str().unwrap().to_owned());
    }
    let refused = h
        .call(Method::POST, "/api/v1/architectures", Some(bad))
        .await;
    assert_eq!(refused.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        refused.problem_type(),
        "urn:chess-bot:problem:validation-error"
    );
    assert!(refused.json()["errors"].as_array().unwrap().len() >= 2);

    let first = h
        .call(Method::GET, "/api/v1/architectures?limit=2", None)
        .await
        .json();
    assert_eq!(first["items"][0]["id"], ids[2]);
    let cursor = first["nextCursor"].as_str().unwrap();
    let second = h
        .call(
            Method::GET,
            &format!("/api/v1/architectures?limit=2&cursor={cursor}"),
            None,
        )
        .await
        .json();
    assert_eq!(second["items"].as_array().unwrap().len(), 1);
    assert_eq!(second["items"][0]["id"], ids[0]);
    assert!(second.get("nextCursor").is_none());
    let bad_cursor = h
        .call(Method::GET, "/api/v1/architectures?cursor=zz", None)
        .await;
    assert_eq!(bad_cursor.status, StatusCode::BAD_REQUEST);

    let path = format!("/api/v1/architectures/{}", ids[1]);
    assert_eq!(
        h.call(Method::GET, &path, None).await.status,
        StatusCode::OK
    );
    assert_eq!(
        h.call(Method::DELETE, &path, None).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        h.call(Method::GET, &path, None).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        h.call(Method::GET, "/api/v1/architectures/not-an-id", None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn idempotency_keys_make_creations_safe_to_retry() {
    let h = harness("idempotency", config());
    let post = |key: &'static str, body: Value| {
        Request::post("/api/v1/architectures")
            .header("x-api-key", KEY)
            .header("idempotency-key", key)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let first = h.send(post("create-1", architecture("fast-768"))).await;
    assert_eq!(first.status, StatusCode::CREATED);
    let retry = h.send(post("create-1", architecture("fast-768"))).await;
    assert_eq!(retry.status, StatusCode::CREATED);
    assert_eq!(retry.body, first.body);
    assert_eq!(retry.headers["idempotent-replayed"], "true");
    assert_eq!(h.lab.architectures(None, 10).unwrap().len(), 1);

    let reused = h.send(post("create-1", architecture("multilayer"))).await;
    assert_eq!(reused.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        reused.problem_type(),
        "urn:chess-bot:problem:idempotency-key-reused"
    );
    // Without a key, each request creates.
    let mut plain = post("x", architecture("fast-768"));
    plain.headers_mut().remove("idempotency-key");
    assert_eq!(h.send(plain).await.status, StatusCode::CREATED);
    assert_eq!(h.lab.architectures(None, 10).unwrap().len(), 2);
}

#[tokio::test]
async fn datasets_are_uploaded_checked_and_described() {
    let h = harness("datasets", config());
    let created = h.upload("queens", dataset_bytes(&h.dir, 100)).await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.json());
    let dataset = created.json();
    assert_eq!(dataset["records"], 100);
    assert_eq!(dataset["status"], "ready");
    let id = dataset["id"].as_str().unwrap().to_owned();
    let stats = h
        .call(Method::GET, &format!("/api/v1/datasets/{id}/stats"), None)
        .await
        .json();
    assert_eq!(stats["outcomes"]["whiteWins"], 100);
    assert_eq!(stats["pieces"][3], 100);

    let mut corrupt = dataset_bytes(&h.dir, 10);
    corrupt[16 + 5 * 32 + 26] = 7;
    let rejected = h.upload("corrupt", corrupt).await;
    assert_eq!(rejected.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        rejected.problem_type(),
        "urn:chess-bot:problem:invalid-dataset"
    );
    assert!(
        rejected.json()["detail"]
            .as_str()
            .unwrap()
            .contains("record 5")
    );

    // Over the limit, whether announced or found while streaming.
    let big = dataset_bytes(&h.dir, 40_000);
    let declared = h
        .send(
            Request::post("/api/v1/datasets?name=big")
                .header("x-api-key", KEY)
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .header(header::CONTENT_LENGTH, big.len())
                .body(Body::from(big.clone()))
                .unwrap(),
        )
        .await;
    assert_eq!(declared.status, StatusCode::PAYLOAD_TOO_LARGE);
    let streamed = h.upload("big", big).await;
    assert_eq!(streamed.status, StatusCode::PAYLOAD_TOO_LARGE);
    let bad_name = h.upload("", dataset_bytes(&h.dir, 10)).await;
    assert_eq!(bad_name.status, StatusCode::UNPROCESSABLE_ENTITY);
    // Nothing was kept of the rejected uploads.
    assert_eq!(h.lab.datasets(None, 10).unwrap().len(), 1);
    assert_eq!(
        std::fs::read_dir(&h.lab.paths().datasets).unwrap().count(),
        1
    );

    let started = h
        .call(
            Method::POST,
            "/api/v1/datasets:selfplay",
            Some(json!({ "games": 2, "nodes": 100 })),
        )
        .await;
    assert_eq!(started.status, StatusCode::ACCEPTED);
    let run = started.json()["run"]["id"].as_str().unwrap().to_owned();
    let generating = started.json()["dataset"]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        h.call(
            Method::GET,
            &format!("/api/v1/datasets/{generating}/stats"),
            None
        )
        .await
        .status,
        StatusCode::CONFLICT
    );
    assert_eq!(
        h.call(
            Method::DELETE,
            &format!("/api/v1/datasets/{generating}"),
            None
        )
        .await
        .status,
        StatusCode::CONFLICT
    );
    let stopped = h
        .call(
            Method::POST,
            &format!("/api/v1/selfplay-runs/{run}:stop"),
            None,
        )
        .await
        .json();
    assert_eq!(stopped["status"], "canceled");
    assert_eq!(
        h.call(Method::DELETE, &format!("/api/v1/datasets/{id}"), None)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let invalid = h
        .call(
            Method::POST,
            "/api/v1/datasets:selfplay",
            Some(json!({ "games": 0, "unknown": 1 })),
        )
        .await;
    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);
}

/// Creates a training run on a fresh dataset and returns its id.
async fn training_run(h: &Harness) -> String {
    let dataset = h.upload("data", dataset_bytes(&h.dir, 100)).await.json();
    let reply = h
        .call(
            Method::POST,
            "/api/v1/training-runs",
            Some(json!({
                "architecture": architecture("fast-768"),
                "datasets": [dataset["id"]],
                "hyperparameters": { "superbatches": 2, "batchSize": 64, "shuffleRecords": 64 },
                "seed": 5,
            })),
        )
        .await;
    assert_eq!(reply.status, StatusCode::ACCEPTED, "{}", reply.json());
    reply.json()["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn a_training_run_from_request_to_active_model() {
    let h = harness("training", config());
    let run = training_run(&h).await;
    let job = h
        .call(Method::GET, &format!("/api/v1/training-runs/{run}"), None)
        .await
        .json();
    assert_eq!(job["status"], "queued");
    assert_eq!(job["spec"]["seed"], 5);
    assert_eq!(job["spec"]["kind"], "training");
    let invalid = h
        .call(
            Method::POST,
            "/api/v1/training-runs",
            Some(json!({ "datasets": [], "hyperparameters": { "wdl": 3 } })),
        )
        .await;
    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);
    let paths: Vec<String> = invalid.json()["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        paths,
        ["/architecture", "/hyperparameters/wdl", "/datasets"]
    );

    // A worker takes it over the internal API.
    let worker = json!({ "id": "w1", "gpu": false, "kinds": ["training"], "trainers": ["burn"] });
    let claim = h
        .internal(
            "/internal/v1/claims",
            Body::from(worker.to_string()),
            "application/json",
        )
        .await;
    assert_eq!(claim.status, StatusCode::OK);
    let attempt = format!("/internal/v1/jobs/{run}/attempts/1");
    let progress = json!({ "kind": "training", "superbatch": 1, "superbatches": 2, "step": 8,
        "positions": 512, "loss": 0.07, "learningRate": 0.001, "positionsPerSecond": 1e6,
        "vramUsedMb": null });
    let directive = h
        .internal(
            &format!("{attempt}/progress"),
            Body::from(progress.to_string()),
            "application/json",
        )
        .await;
    assert_eq!(directive.json(), json!("continue"));
    let metrics = h
        .call(
            Method::GET,
            &format!("/api/v1/training-runs/{run}/metrics"),
            None,
        )
        .await
        .json();
    assert_eq!(metrics["points"][0]["loss"], 0.07);
    assert_eq!(metrics["points"][0]["superbatch"], 1);

    // The network goes up with the summary; a model is created.
    let network = Network::pseudo_random(&presets::get("fast-768").unwrap().architecture, 3)
        .unwrap()
        .to_bytes();
    let stored = h
        .internal(
            &format!("{attempt}/network?superbatches=2&positions=1024&finalLoss=0.05&device=CPU"),
            Body::from(network.clone()),
            "application/octet-stream",
        )
        .await;
    assert_eq!(
        stored.status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&stored.body)
    );
    let model = stored.json()["id"].as_str().unwrap().to_owned();
    let lost = h
        .internal(
            &format!("{attempt}/heartbeat"),
            Body::empty(),
            "application/json",
        )
        .await;
    assert_eq!(lost.status, StatusCode::CONFLICT);
    assert_eq!(lost.problem_type(), "urn:chess-bot:problem:lost-claim");

    // The event stream replays the history and ends with the run.
    let events = h
        .call(
            Method::GET,
            &format!("/api/v1/training-runs/{run}/events"),
            None,
        )
        .await;
    assert_eq!(events.content_type(), "text/event-stream");
    let text = String::from_utf8(events.body).unwrap();
    let kinds: Vec<&str> = text
        .lines()
        .filter_map(|l| l.strip_prefix("event: "))
        .collect();
    assert_eq!(kinds, ["status", "status", "progress", "model", "status"]);
    let last_id: i64 = text
        .lines()
        .rev()
        .find_map(|l| l.strip_prefix("id: "))
        .unwrap()
        .parse()
        .unwrap();
    let resumed = h
        .send(
            Request::get(format!("/api/v1/training-runs/{run}/events"))
                .header("x-api-key", KEY)
                .header("last-event-id", (last_id - 1).to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    let resumed = String::from_utf8(resumed.body).unwrap();
    assert_eq!(resumed.matches("event: ").count(), 1, "{resumed}");

    // The model: provenance, file, activation.
    let view = h
        .call(Method::GET, &format!("/api/v1/models/{model}"), None)
        .await
        .json();
    assert_eq!(view["active"], false);
    assert_eq!(view["provenance"]["seed"], 5);
    assert_eq!(view["provenance"]["finalLoss"], 0.05);
    let file = h
        .call(Method::GET, &format!("/api/v1/models/{model}/file"), None)
        .await;
    assert_eq!(file.body, network);
    assert_eq!(file.content_type(), "application/octet-stream");
    let active = h
        .call(
            Method::POST,
            &format!("/api/v1/models/{model}:activate"),
            None,
        )
        .await
        .json();
    assert_eq!(active["active"], true);
    let model_id: chess_jobs::ModelId = model.parse().unwrap();
    assert_eq!(
        h.activated.lock().unwrap().last().unwrap(),
        &Some(h.lab.paths().model_file(model_id))
    );
    assert_eq!(
        h.call(Method::DELETE, &format!("/api/v1/models/{model}"), None)
            .await
            .status,
        StatusCode::CONFLICT
    );
    assert_eq!(
        h.call(Method::POST, "/api/v1/models:deactivate", None)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(h.activated.lock().unwrap().last().unwrap(), &None);
    let models = h.call(Method::GET, "/api/v1/models", None).await.json();
    assert_eq!(models["items"][0]["active"], false);
    assert_eq!(
        h.call(Method::DELETE, &format!("/api/v1/models/{model}"), None)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn runs_stop_and_resume_through_custom_methods() {
    let h = harness("stop-resume", config());
    let run = training_run(&h).await;
    let worker = json!({ "id": "w1", "gpu": false, "kinds": ["training"], "trainers": ["burn"] });
    h.internal(
        "/internal/v1/claims",
        Body::from(worker.to_string()),
        "application/json",
    )
    .await;
    let stopping = h
        .call(
            Method::POST,
            &format!("/api/v1/training-runs/{run}:stop"),
            None,
        )
        .await
        .json();
    assert_eq!(stopping["status"], "stopping");
    let attempt = format!("/internal/v1/jobs/{run}/attempts/1");
    let directive = h
        .internal(
            &format!("{attempt}/heartbeat"),
            Body::empty(),
            "application/json",
        )
        .await;
    assert_eq!(directive.json(), json!("stop"));
    let acknowledged = h
        .internal(
            &format!("{attempt}/stopped"),
            Body::empty(),
            "application/json",
        )
        .await;
    assert_eq!(acknowledged.status, StatusCode::NO_CONTENT);
    let resumed = h
        .call(
            Method::POST,
            &format!("/api/v1/training-runs/{run}:resume"),
            None,
        )
        .await
        .json();
    assert_eq!(resumed["status"], "queued");
    let again = h
        .call(
            Method::POST,
            &format!("/api/v1/training-runs/{run}:resume"),
            None,
        )
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    let unknown = h
        .send(
            Request::post(format!("/api/v1/training-runs/{run}:explode"))
                .header("x-api-key", KEY)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    // A training run is not a self-play run.
    assert_eq!(
        h.call(Method::GET, &format!("/api/v1/selfplay-runs/{run}"), None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let list = h
        .call(Method::GET, "/api/v1/training-runs", None)
        .await
        .json();
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn the_internal_api_needs_the_worker_token() {
    let h = harness("internal", config());
    let anonymous = h
        .send(
            Request::post("/internal/v1/claims")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    // An API key is not a worker token.
    let with_key = h
        .send(
            Request::post("/internal/v1/claims")
                .header(header::AUTHORIZATION, format!("Bearer {KEY}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
    assert_eq!(with_key.status, StatusCode::UNAUTHORIZED);
    let empty = h
        .internal(
            "/internal/v1/claims",
            Body::from(
                json!({ "id": "w", "gpu": true, "kinds": ["training"], "trainers": ["burn"] })
                    .to_string(),
            ),
            "application/json",
        )
        .await;
    assert_eq!(empty.status, StatusCode::NO_CONTENT);

    // Without a worker token, the internal API does not exist.
    let closed = harness(
        "internal-closed",
        ApiConfig {
            worker_token: None,
            ..config()
        },
    );
    let reply = closed
        .send(
            Request::post("/internal/v1/claims")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}

/// A model in the lab, as a finished training job leaves it.
fn model(h: &Harness, seed: u64) -> String {
    use chess_jobs::Broker;
    let dataset = h
        .lab
        .import_dataset("data", dataset_bytes(&h.dir, 100).as_slice(), 1 << 20)
        .unwrap();
    h.lab
        .create_training(&chess_jobs::TrainingRequest {
            name: Some(format!("net {seed}")),
            architecture: Some(architecture("fast-768")),
            architecture_id: None,
            datasets: vec![dataset.id],
            hyperparameters: chess_jobs::Hyperparameters {
                batch_size: 64,
                shuffle_records: 64,
                ..chess_jobs::Hyperparameters::default()
            },
            seed: Some(seed),
            device: chess_jobs::Device::Cpu,
            trainer: "burn".into(),
        })
        .unwrap();
    let worker = chess_jobs::WorkerInfo {
        id: "trainer".into(),
        gpu: false,
        kinds: vec![chess_jobs::JobKind::Training],
        trainers: vec!["burn".into()],
    };
    let claim = h.lab.claim(&worker).unwrap().unwrap();
    let network = Network::pseudo_random(&presets::get("fast-768").unwrap().architecture, seed)
        .unwrap()
        .to_bytes();
    let summary = chess_jobs::TrainingSummary {
        superbatches: 1,
        positions: 64,
        final_loss: None,
        device: "CPU".into(),
    };
    h.lab
        .complete_training(claim.token(), &network, &summary)
        .unwrap()
        .id
        .to_string()
}

/// A worker plays the evaluation `id` over the internal API: claims it,
/// reports a batch, then the final result.
async fn play(h: &Harness, id: &str, wins: u32, draws: u32, losses: u32) {
    let worker = json!({ "id": "w", "gpu": false, "kinds": ["evaluation"], "trainers": [] });
    let claim = h
        .internal(
            "/internal/v1/claims",
            Body::from(worker.to_string()),
            "application/json",
        )
        .await;
    assert_eq!(claim.json()["job"]["id"], id);
    let attempt = format!("/internal/v1/jobs/{id}/attempts/1");
    let result = |w: u32, d: u32, l: u32| {
        serde_json::to_value(chess_jobs::EvaluationProgress {
            candidate_nps: Some(900_000),
            baseline_nps: Some(2_000_000),
            ..chess_jobs::EvaluationProgress::new(
                chess_jobs::MatchResult {
                    wins: w,
                    draws: d,
                    losses: l,
                },
                100,
                Some(&chess_jobs::Sprt::default()),
            )
        })
        .unwrap()
    };
    let mut report = result(1, 0, 1);
    report["kind"] = "evaluation".into();
    let directive = h
        .internal(
            &format!("{attempt}/progress"),
            Body::from(report.to_string()),
            "application/json",
        )
        .await;
    assert_eq!(directive.json(), json!("continue"));
    let done = h
        .internal(
            &format!("{attempt}/evaluation"),
            Body::from(result(wins, draws, losses).to_string()),
            "application/json",
        )
        .await;
    assert_eq!(
        done.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&done.body)
    );
    assert_eq!(done.json()["status"], "succeeded");
}

#[tokio::test]
async fn evaluations_report_matches_and_compare_models() {
    let h = harness("evaluations", config());
    let (a, b) = (model(&h, 1), model(&h, 2));

    let invalid = h
        .call(
            Method::POST,
            "/api/v1/evaluations",
            Some(json!({ "candidate": a, "baseline": a, "games": 1, "sprt": { "alpha": 0.7 } })),
        )
        .await;
    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);
    let paths: Vec<String> = invalid.json()["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(paths, ["/games", "/baseline", "/sprt/alpha"]);

    // a against the classical evaluation, then b against a.
    let first = h
        .call(
            Method::POST,
            "/api/v1/evaluations",
            Some(json!({ "candidate": a, "games": 100, "nodes": 1000, "sprt": {}, "seed": 9 })),
        )
        .await;
    assert_eq!(first.status, StatusCode::ACCEPTED, "{}", first.json());
    let first = first.json();
    assert_eq!(first["spec"]["kind"], "evaluation");
    assert_eq!(first["spec"]["baseline"]["name"], "classical evaluation");
    assert!(first["spec"]["candidate"]["parameters"].as_u64().unwrap() > 100_000);
    assert_eq!(first["spec"]["sprt"]["elo1"], 10.0);
    let first = first["id"].as_str().unwrap().to_owned();
    play(&h, &first, 60, 20, 20).await;

    let second = h
        .call(
            Method::POST,
            "/api/v1/evaluations",
            Some(json!({ "candidate": b, "baseline": a, "games": 100 })),
        )
        .await
        .json();
    let second = second["id"].as_str().unwrap().to_owned();
    play(&h, &second, 30, 40, 30).await;

    let done = h
        .call(Method::GET, &format!("/api/v1/evaluations/{first}"), None)
        .await
        .json();
    let progress = &done["progress"];
    assert_eq!(
        (progress["wins"].as_u64(), progress["games"].as_u64()),
        (Some(60), Some(100))
    );
    // +147 Elo, but 100 games do not yet prove +10 at 5 % risk.
    assert_eq!(progress["sprt"]["verdict"], "inconclusive");
    let llr = progress["sprt"]["llr"].as_f64().unwrap();
    assert!((1.7..1.8).contains(&llr), "{llr}");
    assert!(progress["eloError"].as_f64().unwrap() > 0.0);
    assert_eq!(progress["candidateNps"], 900_000);

    // The stream replays the match: queued, running, two reports, succeeded.
    let events = h
        .call(
            Method::GET,
            &format!("/api/v1/evaluations/{first}/events"),
            None,
        )
        .await;
    let text = String::from_utf8(events.body).unwrap();
    let kinds: Vec<&str> = text
        .lines()
        .filter_map(|l| l.strip_prefix("event: "))
        .collect();
    assert_eq!(
        kinds,
        ["status", "status", "progress", "progress", "status"]
    );

    // Compared: b is as strong as a, a well above the classical evaluation.
    let comparison = h
        .call(
            Method::GET,
            &format!("/api/v1/evaluations:compare?players=classic,{a},{b}"),
            None,
        )
        .await;
    assert_eq!(comparison.status, StatusCode::OK);
    let comparison = comparison.json();
    let players = comparison["players"].as_array().unwrap();
    assert_eq!(players[0]["elo"], 0.0);
    assert_eq!(players[0]["nps"], 2_000_000);
    let (elo_a, elo_b) = (
        players[1]["elo"].as_f64().unwrap(),
        players[2]["elo"].as_f64().unwrap(),
    );
    assert!(
        elo_a > 100.0 && (elo_a - elo_b).abs() < 1.0,
        "{comparison:#}"
    );
    assert_eq!(players[1]["games"], 200);
    assert_eq!(players[2]["contestant"]["name"], "net 2");
    let pairs = comparison["pairs"].as_array().unwrap();
    assert_eq!(pairs.len(), 2);
    // Pairs read from the first player listed: the classical evaluation lost 20-20-60.
    assert_eq!(
        (pairs[0]["a"].as_u64(), pairs[0]["b"].as_u64()),
        (Some(0), Some(1))
    );
    assert_eq!(pairs[0]["wins"], 20);
    assert_eq!(pairs[0]["evaluations"], json!([first]));
    let bad = h
        .call(
            Method::GET,
            "/api/v1/evaluations:compare?players=classic,classic",
            None,
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    let unknown = chess_jobs::ModelId::generate();
    let missing = h
        .call(
            Method::GET,
            &format!("/api/v1/evaluations:compare?players={unknown}"),
            None,
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    // A queued evaluation is canceled by :stop.
    let third = h
        .call(
            Method::POST,
            "/api/v1/evaluations",
            Some(json!({ "candidate": b })),
        )
        .await
        .json();
    let third = third["id"].as_str().unwrap();
    let canceled = h
        .call(
            Method::POST,
            &format!("/api/v1/evaluations/{third}:stop"),
            None,
        )
        .await
        .json();
    assert_eq!(canceled["status"], "canceled");
    let list = h
        .call(Method::GET, "/api/v1/evaluations", None)
        .await
        .json();
    assert_eq!(list["items"].as_array().unwrap().len(), 3);
    // An evaluation is not a training run.
    assert_eq!(
        h.call(Method::GET, &format!("/api/v1/training-runs/{first}"), None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}
