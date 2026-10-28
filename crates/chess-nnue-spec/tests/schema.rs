//! The committed JSON Schema matches the Rust types.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

/// Regenerates `schemas/nnue-architecture.v1.json` when it is out of date
/// (like the ts-rs bindings: the CI fails if the test changed the file).
#[test]
fn committed_schema_is_up_to_date() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/nnue-architecture.v1.json");
    let generated = serde_json::to_string_pretty(&chess_nnue_spec::json_schema()).unwrap() + "\n";
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    if committed.replace("\r\n", "\n") != generated {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, generated).unwrap();
    }
}
