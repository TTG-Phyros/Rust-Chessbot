//! `/api/v1/architectures`: validation, presets, saved architectures.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chess_jobs::{ArchitectureId, StoredArchitecture};
use chess_nnue_spec::{Architecture, Estimate, Layout};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{Document, parse_id};
use crate::NnueApi;
use crate::openapi::{ArchitectureSchema, architecture_ref, estimate_ref, layout_ref};
use crate::pagination::{ArchitecturePage, PageQuery, next_cursor};
use crate::problem::{ApiResult, FieldError, Problem};

/// Result of a validation: the errors, or what the architecture resolves to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ValidationReport {
    /// Whether the document is a valid architecture.
    pub(crate) valid: bool,
    /// Every problem found (empty when valid).
    pub(crate) errors: Vec<FieldError>,
    /// Hash of the architecture (when valid).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) hash: Option<String>,
    /// The architecture with every default written out (when valid).
    #[schema(schema_with = architecture_ref)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) normalized: Option<Architecture>,
    /// Tensor shapes and integer bounds (when valid).
    #[schema(schema_with = layout_ref)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) layout: Option<Layout>,
    /// Size and cost estimates (when valid).
    #[schema(schema_with = estimate_ref)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) estimate: Option<Estimate>,
}

/// A ready-made architecture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Preset {
    /// Identifier, e.g. `fast-768`.
    pub(crate) id: String,
    /// Hash of the architecture.
    pub(crate) hash: String,
    /// The architecture document.
    #[schema(schema_with = architecture_ref)]
    pub(crate) architecture: Architecture,
    /// Size and cost estimates.
    #[schema(schema_with = estimate_ref)]
    pub(crate) estimate: Estimate,
}

/// Checks an architecture document without saving it.
#[utoipa::path(
    post,
    path = "/api/v1/architectures:validate",
    operation_id = "validateArchitecture",
    tag = "architectures",
    request_body(
        description = "Architecture document",
        content(
            (ArchitectureSchema = "application/json"),
            (ArchitectureSchema = "application/yaml"),
        )
    ),
    responses(
        (status = 200, description = "Validation result, valid or not", body = ValidationReport),
        (status = 400, description = "Not JSON or YAML", body = Problem, content_type = "application/problem+json"),
        (status = 415, description = "Unsupported media type", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn validate(Document(document): Document) -> Json<ValidationReport> {
    Json(match chess_nnue_spec::from_value(&document) {
        Ok(architecture) => ValidationReport {
            valid: true,
            errors: Vec::new(),
            hash: Some(architecture.hash().to_string()),
            layout: Some(architecture.layout()),
            estimate: Some(architecture.estimate()),
            normalized: Some(architecture.normalized()),
        },
        Err(errors) => ValidationReport {
            valid: false,
            errors: errors
                .into_iter()
                .map(|e| FieldError {
                    path: e.path,
                    message: e.message,
                })
                .collect(),
            hash: None,
            normalized: None,
            layout: None,
            estimate: None,
        },
    })
}

/// The ready-made architectures.
#[utoipa::path(
    get,
    path = "/api/v1/architectures/presets",
    operation_id = "listPresets",
    tag = "architectures",
    responses((status = 200, description = "Every preset", body = Vec<Preset>))
)]
pub(crate) async fn presets() -> Json<Vec<Preset>> {
    Json(
        chess_nnue_spec::presets::all()
            .into_iter()
            .map(|preset| Preset {
                id: preset.id.into(),
                hash: preset.architecture.hash().to_string(),
                estimate: preset.architecture.estimate(),
                architecture: preset.architecture,
            })
            .collect(),
    )
}

/// The JSON Schema of architecture documents (for editors: `$schema`).
#[utoipa::path(
    get,
    path = "/api/v1/architectures/schema",
    operation_id = "getArchitectureSchema",
    tag = "architectures",
    responses((status = 200, description = "JSON Schema 2020-12", body = Object, content_type = "application/schema+json"))
)]
pub(crate) async fn schema() -> Response {
    let mut response = Json(chess_nnue_spec::json_schema()).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/schema+json"),
    );
    response
}

/// Saves an architecture for training runs to use (`architectureId`).
#[utoipa::path(
    post,
    path = "/api/v1/architectures",
    operation_id = "createArchitecture",
    tag = "architectures",
    params(("Idempotency-Key" = Option<String>, Header, description = "Makes retries safe")),
    request_body(
        description = "Architecture document",
        content(
            (ArchitectureSchema = "application/json"),
            (ArchitectureSchema = "application/yaml"),
        )
    ),
    responses(
        (status = 201, description = "Saved", body = StoredArchitecture),
        (status = 422, description = "Invalid architecture, with every error", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn create(
    State(api): State<NnueApi>,
    Document(document): Document,
) -> ApiResult<Response> {
    let stored = api.lab.create_architecture(&document)?;
    Ok(created(
        &format!("/api/v1/architectures/{}", stored.id),
        &stored,
    ))
}

/// Saved architectures.
#[utoipa::path(
    get,
    path = "/api/v1/architectures",
    operation_id = "listArchitectures",
    tag = "architectures",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of saved architectures", body = ArchitecturePage),
        (status = 400, description = "Invalid cursor or limit", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn list(
    State(api): State<NnueApi>,
    Query(page): Query<PageQuery>,
) -> ApiResult<Json<ArchitecturePage>> {
    let limit = page.limit()?;
    let items = api.lab.architectures(page.after()?, limit)?;
    Ok(Json(ArchitecturePage {
        next_cursor: next_cursor(items.len(), limit, items.last().map(|a| a.id)),
        items,
    }))
}

/// A saved architecture.
#[utoipa::path(
    get,
    path = "/api/v1/architectures/{id}",
    operation_id = "getArchitecture",
    tag = "architectures",
    params(("id" = String, Path, description = "Architecture id")),
    responses(
        (status = 200, description = "The architecture", body = StoredArchitecture),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn get(
    State(api): State<NnueApi>,
    Path(id): Path<String>,
) -> ApiResult<Json<StoredArchitecture>> {
    let id: ArchitectureId = parse_id(&id, "architecture")?;
    Ok(Json(api.lab.architecture(id)?))
}

/// Deletes a saved architecture (runs and models keep their copy).
#[utoipa::path(
    delete,
    path = "/api/v1/architectures/{id}",
    operation_id = "deleteArchitecture",
    tag = "architectures",
    params(("id" = String, Path, description = "Architecture id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn delete(
    State(api): State<NnueApi>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    api.lab
        .delete_architecture(parse_id(&id, "architecture")?)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `201 Created` with a `Location` header.
pub(crate) fn created(location: &str, body: &impl Serialize) -> Response {
    let mut response = (StatusCode::CREATED, Json(body)).into_response();
    if let Ok(value) = HeaderValue::from_str(location) {
        response.headers_mut().insert(header::LOCATION, value);
    }
    response
}
