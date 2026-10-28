//! The committed OpenAPI document matches the code: the SDKs (`sdk/`) are
//! generated from it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

/// Regenerates `schemas/nnue-api.v1.openapi.json` when it is out of date
/// (the CI fails if the test changed the file).
#[test]
fn committed_document_is_up_to_date() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/nnue-api.v1.openapi.json");
    let generated = serde_json::to_string_pretty(&chess_nnue_api::openapi()).unwrap() + "\n";
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    if committed.replace("\r\n", "\n") != generated {
        std::fs::write(&path, generated).unwrap();
    }
}
