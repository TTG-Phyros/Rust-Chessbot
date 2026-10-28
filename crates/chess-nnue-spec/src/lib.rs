//! Declarative NNUE architectures.
//!
//! An architecture is a JSON (or YAML) document validated by a versioned
//! JSON Schema ([`json_schema`], committed as
//! `schemas/nnue-architecture.v1.json`) and by semantic rules the schema
//! cannot express. It describes the input features, the accumulator, the
//! dense layers, the output buckets and the quantization of a network; the
//! engine's inference and the trainers are driven by it.
//!
//! ```
//! use chess_nnue_spec::{Format, parse};
//!
//! let arch = parse(
//!     r#"{
//!         "schemaVersion": 1,
//!         "metadata": { "name": "tiny" },
//!         "features": { "set": "Chess768" },
//!         "accumulator": { "size": 64, "activation": "CReLU" },
//!         "quantization": { "weights": "i8", "qa": 255, "qb": 64, "evalScale": 400 }
//!     }"#,
//!     Format::Json,
//! )
//! .unwrap();
//! assert_eq!(arch.estimate().input_features, 768);
//!
//! let errors = parse(r#"{ "schemaVersion": 1 }"#, Format::Json).unwrap_err();
//! assert!(errors.iter().any(|e| e.message.contains("metadata")));
//! ```

mod hash;
mod layout;
mod model;
pub mod presets;
mod validate;

pub use hash::ArchitectureHash;
pub use layout::{DenseShape, Estimate, I32_DOT_LIMIT, Layout, SumWidth, activation_bound};
pub use model::{
    Accumulator, AccumulatorType, Activation, Architecture, BucketScope, BucketStrategy,
    DenseLayer, FeatureSet, Features, Metadata, Mirror, OutputBuckets, Quantization, WeightType,
};
pub use validate::{MAX_WEIGHTS_BYTES, ValidationError, from_value};

/// Version of the architecture schema implemented by this crate.
pub const SCHEMA_VERSION: u32 = 1;

/// `$id` of the JSON Schema.
pub const SCHEMA_ID: &str = "urn:chess-bot:schema:nnue-architecture:v1";

/// Syntax of an architecture document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// JSON.
    Json,
    /// YAML (a superset of JSON).
    Yaml,
}

/// Parses and fully validates an architecture document.
///
/// # Errors
///
/// Every problem found, each with the JSON pointer of the offending value.
pub fn parse(text: &str, format: Format) -> Result<Architecture, Vec<ValidationError>> {
    let value: serde_json::Value = match format {
        Format::Json => serde_json::from_str(text).map_err(|err| err.to_string()),
        Format::Yaml => serde_norway::from_str(text).map_err(|err| err.to_string()),
    }
    .map_err(|message| {
        vec![ValidationError::new(
            "",
            format!("not a valid document: {message}"),
        )]
    })?;
    from_value(&value)
}

/// The JSON Schema (draft 2020-12) of architecture documents.
#[must_use]
pub fn json_schema() -> serde_json::Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(Architecture)).unwrap_or_default();
    if let Some(object) = schema.as_object_mut() {
        object.insert("$id".into(), SCHEMA_ID.into());
    }
    if let Some(defs) = schema.get_mut("$defs").and_then(|d| d.as_object_mut()) {
        defs.values_mut().for_each(enum_of_consts);
    }
    schema
}

/// schemars writes documented unit enums as `oneOf` of `const`s; a plain
/// `enum` (each value's description moved into the definition's) makes
/// validators say which values are allowed, and code generators emit enums.
fn enum_of_consts(def: &mut serde_json::Value) {
    let Some(variants) = def.get("oneOf").and_then(|v| v.as_array()) else {
        return;
    };
    let Some(values) = variants
        .iter()
        .map(|v| v.get("const").cloned())
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let mut description = def
        .get("description")
        .and_then(|d| d.as_str())
        .unwrap_or_default()
        .to_owned();
    for variant in variants {
        if let (Some(value), Some(text)) = (variant.get("const"), variant.get("description")) {
            let text = text.as_str().unwrap_or_default().replace('\n', " ");
            description.push_str(&format!(
                "\n- `{}`: {text}",
                value.as_str().unwrap_or_default()
            ));
        }
    }
    *def = serde_json::json!({ "type": "string", "enum": values, "description": description });
}
