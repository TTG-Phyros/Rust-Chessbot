//! The OpenAPI 3.1 document of `/api/v1`, served at `/api/v1/openapi.json`.
//!
//! utoipa describes the routes and the API's own types. The architecture
//! (and its layout and estimate) are described by the JSON Schemas of
//! `chess-nnue-spec` — the ones the server validates with — added to the
//! components afterwards, their `$defs` moved alongside so that every
//! reference is a plain `#/components/schemas/...` one.

use std::borrow::Cow;

use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Map, Value, json};
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::openapi::{Ref, RefOr, Schema};
use utoipa::{Modify, OpenApi, PartialSchema, ToSchema};

use crate::NnueApi;
use crate::pagination::{ArchitecturePage, DatasetPage, JobPage, ModelPage};
use crate::problem::{FieldError, Problem};
use crate::routes::{architectures, datasets, models, runs};

macro_rules! external_schema {
    ($marker:ident, $name:literal, $reference:ident) => {
        /// Stands for a schema of `chess-nnue-spec` in the document (its
        /// component is replaced by the real JSON Schema).
        pub(crate) struct $marker;

        impl PartialSchema for $marker {
            fn schema() -> RefOr<Schema> {
                RefOr::Ref(Ref::from_schema_name($name))
            }
        }

        impl ToSchema for $marker {
            fn name() -> Cow<'static, str> {
                Cow::Borrowed($name)
            }
        }

        /// Reference to the schema, for `schema_with`.
        pub(crate) fn $reference() -> Ref {
            Ref::from_schema_name($name)
        }
    };
}

external_schema!(ArchitectureSchema, "Architecture", architecture_ref);
external_schema!(LayoutSchema, "Layout", layout_ref);
external_schema!(EstimateSchema, "Estimate", estimate_ref);

#[derive(OpenApi)]
#[openapi(
    info(
        title = "chess-bot NNUE lab API",
        version = "1.0.0",
        description = "Declare NNUE architectures, upload or generate datasets, train networks and \
            manage the resulting models.\n\n\
            Authentication: an API key in the `X-API-Key` header. Errors are RFC 9457 problem \
            details (`application/problem+json`); lists page with `limit` and `cursor`; \
            creations accept an `Idempotency-Key` header (kept 24 h). Limits: requests per \
            minute per key (429 with `Retry-After`), size of uploads (413)."
    ),
    paths(
        architectures::validate, architectures::presets, architectures::schema,
        architectures::create, architectures::list, architectures::get, architectures::delete,
        datasets::upload, datasets::selfplay, datasets::list, datasets::get, datasets::stats,
        datasets::delete,
        runs::create_training, runs::list_training, runs::get_training, runs::stop_training_doc,
        runs::resume_training_doc, runs::metrics, runs::events_training,
        runs::list_selfplay, runs::get_selfplay, runs::stop_selfplay_doc, runs::events_selfplay,
        models::list, models::get, models::file, models::activate_doc, models::deactivate,
        models::delete,
    ),
    components(schemas(
        Problem, FieldError, ArchitecturePage, DatasetPage, JobPage, ModelPage,
        ArchitectureSchema, LayoutSchema, EstimateSchema,
    )),
    modifiers(&Security),
    security(("api_key" = [])),
    tags(
        (name = "architectures", description = "Declarative NNUE architectures: validation, presets, saved ones"),
        (name = "datasets", description = "Training positions: uploads, self-play generation, statistics"),
        (name = "training-runs", description = "Training jobs: progress, stop and resume, live events"),
        (name = "selfplay-runs", description = "Self-play jobs generating datasets"),
        (name = "models", description = "Trained networks: provenance, files, the active one"),
    )
)]
struct ApiDoc;

struct Security;

impl Modify for Security {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "api_key",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                "X-API-Key",
                "One of the keys of CHESS_API_KEYS",
            ))),
        );
    }
}

/// The OpenAPI document.
#[must_use]
pub fn openapi() -> Value {
    let mut document = serde_json::to_value(ApiDoc::openapi()).unwrap_or_default();
    let external = [
        ("Architecture", chess_nnue_spec::json_schema()),
        (
            "Layout",
            serde_json::to_value(schemars::schema_for!(chess_nnue_spec::Layout))
                .unwrap_or_default(),
        ),
        (
            "Estimate",
            serde_json::to_value(schemars::schema_for!(chess_nnue_spec::Estimate))
                .unwrap_or_default(),
        ),
    ];
    if let Some(schemas) = document
        .pointer_mut("/components/schemas")
        .and_then(Value::as_object_mut)
    {
        for (name, schema) in external {
            hoist(schemas, name, schema);
        }
    }
    if let Some(paths) = document.get_mut("paths").and_then(Value::as_object_mut) {
        // Every operation needs a key and is rate limited.
        let problem = json!({ "application/problem+json": {
            "schema": { "$ref": "#/components/schemas/Problem" } } });
        for operation in paths
            .values_mut()
            .filter_map(Value::as_object_mut)
            .flat_map(|item| item.values_mut())
            .filter_map(|op| op.get_mut("responses").and_then(Value::as_object_mut))
        {
            operation.insert(
                "401".into(),
                json!({ "description": "Missing or invalid API key", "content": problem }),
            );
            operation.insert(
                "429".into(),
                json!({
                    "description": "Too many requests: wait `Retry-After` seconds",
                    "headers": { "Retry-After": { "schema": { "type": "integer" } } },
                    "content": problem,
                }),
            );
        }
        // Reading the document itself needs no key.
        paths.insert(
            "/api/v1/openapi.json".into(),
            json!({ "get": {
                "tags": ["architectures"],
                "summary": "This document",
                "operationId": "openapi",
                "security": [],
                "responses": { "200": { "description": "OpenAPI 3.1 document",
                    "content": { "application/json": { "schema": {} } } } }
            }}),
        );
    }
    document
}

/// Adds the JSON Schema `schema` as component `name`, its `$defs` as
/// components of their own.
fn hoist(schemas: &mut Map<String, Value>, name: &str, mut schema: Value) {
    if let Some(object) = schema.as_object_mut() {
        if let Some(Value::Object(defs)) = object.remove("$defs") {
            for (def, mut value) in defs {
                rewrite_refs(&mut value);
                schemas.insert(def, value);
            }
        }
        for keyword in ["$schema", "$id"] {
            object.remove(keyword);
        }
    }
    rewrite_refs(&mut schema);
    schemas.insert(name.into(), schema);
}

fn rewrite_refs(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref")
                && let Some(def) = reference.strip_prefix("#/$defs/")
            {
                *reference = format!("#/components/schemas/{def}");
            }
            object.values_mut().for_each(rewrite_refs);
        }
        Value::Array(items) => items.iter_mut().for_each(rewrite_refs),
        _ => {}
    }
}

pub(crate) fn router() -> Router<NnueApi> {
    Router::new().route("/api/v1/openapi.json", get(|| async { Json(openapi()) }))
}
