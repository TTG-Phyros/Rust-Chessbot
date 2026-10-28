//! `/docs`: interactive documentation of `/api/v1` (Scalar, reading
//! `/api/v1/openapi.json`; requests can be tried with an API key).

use axum::Router;
use axum::response::Html;
use axum::routing::get;

use crate::NnueApi;

/// Scalar's API reference, pinned.
const PAGE: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>chess-bot NNUE lab API</title>
  </head>
  <body>
    <script id="api-reference" data-url="/api/v1/openapi.json"></script>
    <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference@1.74.0"></script>
  </body>
</html>
"#;

pub(crate) fn router() -> Router<NnueApi> {
    Router::new().route("/docs", get(|| async { Html(PAGE) }))
}
