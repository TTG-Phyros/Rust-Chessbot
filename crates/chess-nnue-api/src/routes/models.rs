//! `/api/v1/models`: trained networks, their files, the active one.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chess_jobs::{Model, ModelId};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{custom_method, parse_id};
use crate::NnueApi;
use crate::pagination::{ModelPage, PageQuery, next_cursor};
use crate::problem::{ApiProblem, ApiResult, Problem};

/// A model, and whether the engine plays with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelView {
    /// The model.
    #[serde(flatten)]
    pub(crate) model: Model,
    /// Whether it is the active model.
    pub(crate) active: bool,
}

fn view(api: &NnueApi, model: Model) -> ApiResult<ModelView> {
    let active = api.lab.active_model()? == Some(model.id);
    Ok(ModelView { model, active })
}

/// Models, newest first.
#[utoipa::path(
    get,
    path = "/api/v1/models",
    operation_id = "listModels",
    tag = "models",
    params(PageQuery),
    responses(
        (status = 200, description = "A page of models", body = ModelPage),
        (status = 400, description = "Invalid cursor or limit", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn list(
    State(api): State<NnueApi>,
    Query(page): Query<PageQuery>,
) -> ApiResult<Json<ModelPage>> {
    let limit = page.limit()?;
    let active = api.lab.active_model()?;
    let models = api.lab.models(page.after()?, limit)?;
    Ok(Json(ModelPage {
        next_cursor: next_cursor(models.len(), limit, models.last().map(|m| m.id)),
        items: models
            .into_iter()
            .map(|model| ModelView {
                active: active == Some(model.id),
                model,
            })
            .collect(),
    }))
}

/// A model, with how it was made.
#[utoipa::path(
    get,
    path = "/api/v1/models/{id}",
    operation_id = "getModel",
    tag = "models",
    params(("id" = String, Path, description = "Model id")),
    responses(
        (status = 200, description = "The model", body = ModelView),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn get(
    State(api): State<NnueApi>,
    Path(id): Path<String>,
) -> ApiResult<Json<ModelView>> {
    let model = api.lab.model(parse_id(&id, "model")?)?;
    Ok(Json(view(&api, model)?))
}

/// The network file (`CBNN` version 2), loadable by the engine
/// (`setoption name EvalFile`).
#[utoipa::path(
    get,
    path = "/api/v1/models/{id}/file",
    operation_id = "downloadModel",
    tag = "models",
    params(("id" = String, Path, description = "Model id")),
    responses(
        (status = 200, description = "The network", body = inline(crate::openapi::Binary), content_type = "application/octet-stream"),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn file(
    State(api): State<NnueApi>,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let id: ModelId = parse_id(&id, "model")?;
    let model = api.lab.model(id)?;
    let path = api.lab.paths().model_file(id);
    let bytes = crate::blocking(move || std::fs::read(path).map_err(ApiProblem::internal)).await?;
    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{id}.nnue\"")) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    if let Ok(value) = HeaderValue::from_str(&format!("\"{}\"", model.file_sha256)) {
        headers.insert(header::ETAG, value);
    }
    Ok(response)
}

/// Makes the engine play (and analyse) with this model, from the next search on.
#[utoipa::path(
    post,
    path = "/api/v1/models/{id}:activate",
    operation_id = "activateModel",
    tag = "models",
    params(("id" = String, Path, description = "Model id")),
    responses(
        (status = 200, description = "Active", body = ModelView),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
    )
)]
#[allow(dead_code)] // Documents a method served by `act`.
pub(crate) async fn activate_doc() {}

/// `POST /models/{id}:activate`.
pub(crate) async fn act(
    State(api): State<NnueApi>,
    Path(segment): Path<String>,
) -> ApiResult<Json<ModelView>> {
    let (id, method) = custom_method(&segment)?;
    if method != "activate" {
        return Err(ApiProblem::not_found(format!(
            "models have no method {method:?} (activate)"
        )));
    }
    let model = api.lab.model(parse_id(id, "model")?)?;
    api.lab.set_active_model(Some(model.id))?;
    api.apply_active_model()?;
    tracing::info!(model = %model.id, name = %model.name, "model activated");
    Ok(Json(view(&api, model)?))
}

/// Goes back to the server's configured network.
#[utoipa::path(
    post,
    path = "/api/v1/models:deactivate",
    operation_id = "deactivateModel",
    tag = "models",
    responses((status = 204, description = "No model is active any more"))
)]
pub(crate) async fn deactivate(State(api): State<NnueApi>) -> ApiResult<StatusCode> {
    api.lab.set_active_model(None)?;
    api.apply_active_model()?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a model and its file.
#[utoipa::path(
    delete,
    path = "/api/v1/models/{id}",
    operation_id = "deleteModel",
    tag = "models",
    params(("id" = String, Path, description = "Model id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Unknown id", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "It is the active model", body = Problem, content_type = "application/problem+json"),
    )
)]
pub(crate) async fn delete(
    State(api): State<NnueApi>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    api.lab.delete_model(parse_id(&id, "model")?)?;
    Ok(StatusCode::NO_CONTENT)
}
