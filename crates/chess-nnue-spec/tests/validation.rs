//! Valid and invalid architecture documents, hashing and layouts.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chess_nnue_spec::{
    Activation, Architecture, Format, SumWidth, ValidationError, parse, presets,
};
use serde_json::{Value, json};

/// The smallest complete document.
fn minimal() -> Value {
    json!({
        "schemaVersion": 1,
        "metadata": { "name": "tiny" },
        "features": { "set": "Chess768" },
        "accumulator": { "size": 64, "activation": "CReLU" },
        "quantization": { "weights": "i8", "qa": 255, "qb": 64, "evalScale": 400 }
    })
}

/// `minimal()` with `patch` merged in (objects recursively, `null` removes).
fn with(patch: Value) -> Value {
    fn merge(target: &mut Value, patch: Value) {
        match (target, patch) {
            (Value::Object(target), Value::Object(patch)) => {
                for (key, value) in patch {
                    if value.is_null() {
                        target.remove(&key);
                    } else {
                        merge(target.entry(key).or_insert(Value::Null), value);
                    }
                }
            }
            (target, patch) => *target = patch,
        }
    }
    let mut doc = minimal();
    merge(&mut doc, patch);
    doc
}

fn check(doc: &Value) -> Result<Architecture, Vec<ValidationError>> {
    chess_nnue_spec::from_value(doc)
}

/// Asserts that `doc` is rejected with an error at `path` whose message
/// contains `fragment`.
#[track_caller]
fn rejected(doc: Value, path: &str, fragment: &str) {
    let errors = check(&doc).expect_err("document should be rejected");
    assert!(
        errors
            .iter()
            .any(|e| e.path == path && e.message.contains(fragment)),
        "expected an error at {path:?} containing {fragment:?}, got {errors:#?}"
    );
}

#[test]
fn minimal_document_is_valid() {
    let arch = check(&minimal()).unwrap();
    assert!(arch.features.perspective);
    assert!(arch.hidden_layers.is_empty());
    let layout = arch.layout();
    assert_eq!(layout.inputs, 768);
    assert_eq!(layout.dense.len(), 1);
    assert_eq!(layout.dense[0].inputs, 128);
}

#[test]
fn yaml_and_json_describe_the_same_network() {
    let yaml = r#"
schemaVersion: 1
metadata: { name: tiny (yaml) }
features:
  set: Chess768
accumulator: { size: 64, activation: CReLU }
quantization:
  weights: i8
  qa: 255
  qb: 64
  evalScale: 400
"#;
    let from_yaml = parse(yaml, Format::Yaml).unwrap();
    let from_json = check(&minimal()).unwrap();
    assert_eq!(from_yaml.hash(), from_json.hash());
}

#[test]
fn hash_ignores_metadata_defaults_and_key_order() {
    let base = check(&minimal()).unwrap().hash();
    let renamed = with(json!({
        "$schema": "./nnue-architecture.v1.json",
        "metadata": { "name": "other", "author": "me", "tags": ["x"] }
    }));
    assert_eq!(check(&renamed).unwrap().hash(), base);

    // The defaults written out.
    let explicit = with(json!({
        "features": { "mirror": "none", "perspective": true, "kingBuckets": vec![0; 64] },
        "hiddenLayers": [],
        "outputBuckets": { "strategy": "none", "count": 1, "scope": "all" },
        "quantization": { "accumulator": "i16", "ftWeightClip": 992.0 / 255.0, "weightClip": 127.0 / 64.0 }
    }));
    assert_eq!(check(&explicit).unwrap().hash(), base);

    let text = serde_json::to_string(&minimal()).unwrap();
    let reordered: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(check(&reordered).unwrap().hash(), base);
    assert_eq!(base.to_string().parse(), Ok(base));
}

#[test]
fn hash_changes_with_the_network() {
    let base = check(&minimal()).unwrap().hash();
    for patch in [
        json!({ "accumulator": { "size": 128 } }),
        json!({ "accumulator": { "activation": "SCReLU" } }),
        json!({ "hiddenLayers": [{ "size": 8, "activation": "CReLU" }] }),
        json!({ "quantization": { "qa": 256 } }),
        json!({ "quantization": { "evalScale": 410 } }),
        json!({ "quantization": { "weights": "i16", "weightClip": 1.0 } }),
        json!({ "features": { "perspective": false } }),
        json!({ "outputBuckets": { "strategy": "material", "count": 4 } }),
    ] {
        let arch = check(&with(patch.clone())).unwrap_or_else(|e| panic!("{patch}: {e:#?}"));
        assert_ne!(arch.hash(), base, "{patch}");
    }
}

#[test]
fn presets_have_the_expected_shapes() {
    let ids: Vec<_> = presets::all().iter().map(|p| p.id).collect();
    assert_eq!(
        ids,
        ["fast-768", "halfka-medium", "multilayer", presets::LEGACY]
    );

    let legacy = presets::get(presets::LEGACY).unwrap().architecture;
    let layout = legacy.layout();
    assert_eq!(
        (layout.inputs, layout.accumulator, layout.perspectives),
        (768, 256, 2)
    );
    assert_eq!((layout.ft_weight_max, layout.weight_max), (765, 127));
    // Same size as a CBNN version 1 file, minus its 28-byte header.
    assert_eq!(legacy.estimate().weights_bytes + 28, 394_784);

    let halfka = presets::get("halfka-medium").unwrap().architecture.layout();
    assert_eq!(
        (halfka.king_buckets, halfka.planes, halfka.inputs),
        (8, 11, 8 * 11 * 64)
    );
    assert_eq!(halfka.dense[0].buckets, 8);
    assert!(halfka.king_refreshes);

    let multi = presets::get("multilayer").unwrap().architecture.layout();
    assert_eq!(multi.ft_outputs, 512);
    let shapes: Vec<_> = multi
        .dense
        .iter()
        .map(|d| (d.inputs, d.outputs, d.buckets))
        .collect();
    assert_eq!(shapes, [(1024, 16, 8), (16, 32, 8), (32, 1, 8)]);
    assert!(multi.dense.iter().all(|d| d.sum == SumWidth::I32));
}

#[test]
fn default_halfka_buckets_are_mirrored() {
    let doc = with(json!({ "features": { "set": "HalfKAv2_hm" } }));
    let layout = check(&doc).unwrap().layout();
    assert_eq!((layout.king_buckets, layout.inputs), (32, 32 * 704));
    let doc = with(json!({ "features": { "set": "HalfKP" } }));
    let layout = check(&doc).unwrap().layout();
    assert_eq!(
        (layout.king_buckets, layout.inputs, layout.max_active),
        (64, 40_960, 30)
    );
}

#[test]
fn wide_sums_switch_to_i64() {
    let doc = with(json!({
        "accumulator": { "size": 1024, "activation": "ReLU" },
        "quantization": { "weights": "i16", "qb": 512, "weightClip": 8.0 }
    }));
    let layout = check(&doc).unwrap().layout();
    assert_eq!(layout.dense[0].sum, SumWidth::I64);
    assert_eq!(layout.dense[0].input_bound, i64::from(i16::MAX));
}

#[test]
fn reports_unparsable_documents() {
    let errors = parse("{ \"schemaVersion\": ", Format::Json).unwrap_err();
    assert_eq!(errors[0].path, "");
    assert!(errors[0].message.contains("not a valid document"));
    assert!(parse("- [", Format::Yaml).is_err());
}

#[test]
fn rejects_schema_violations_with_their_path() {
    rejected(with(json!({ "metadata": null })), "", "metadata");
    rejected(with(json!({ "schemaVersion": 2 })), "/schemaVersion", "1");
    rejected(
        with(json!({ "accumulator": { "sise": 3 } })),
        "/accumulator",
        "sise",
    );
    rejected(
        with(json!({ "accumulator": { "size": 100 } })),
        "/accumulator/size",
        "multiple of 16",
    );
    rejected(
        with(json!({ "accumulator": { "size": 8192 } })),
        "/accumulator/size",
        "4096",
    );
    rejected(
        with(json!({ "accumulator": { "activation": "GELU" } })),
        "/accumulator/activation",
        "GELU",
    );
    rejected(
        with(json!({ "features": { "set": "HalfKA" } })),
        "/features/set",
        "HalfKA",
    );
    rejected(
        with(json!({ "features": { "kingBuckets": vec![0; 63] } })),
        "/features/kingBuckets",
        "64",
    );
    rejected(
        with(json!({ "features": { "kingBuckets": vec![70; 64] } })),
        "/features/kingBuckets/0",
        "63",
    );
    rejected(
        with(json!({ "metadata": { "tags": ["Bad Tag"] } })),
        "/metadata/tags/0",
        "Bad Tag",
    );
    rejected(
        with(json!({ "quantization": { "qa": 0 } })),
        "/quantization/qa",
        "minimum",
    );
    rejected(
        with(json!({ "hiddenLayers": [{ "size": 0, "activation": "ReLU" }] })),
        "/hiddenLayers/0/size",
        "minimum",
    );
}

#[test]
fn rejects_what_the_inference_does_not_support() {
    let mut gap = vec![0_u8; 64];
    gap[10] = 2;
    rejected(
        with(json!({ "features": { "kingBuckets": gap } })),
        "/features/kingBuckets",
        "1 is missing",
    );

    let mut asymmetric: Vec<u8> = vec![0; 64];
    asymmetric[1] = 1;
    rejected(
        with(json!({ "features": { "mirror": "kingFile", "kingBuckets": asymmetric } })),
        "/features/kingBuckets/1",
        "g1",
    );
    rejected(
        with(json!({ "features": { "set": "HalfKAv2_hm", "mirror": "none" } })),
        "/features/mirror",
        "kingFile",
    );
    rejected(
        with(json!({ "features": { "set": "HalfKP", "perspective": false } })),
        "/features/perspective",
        "HalfKP",
    );
    rejected(
        with(json!({ "accumulator": { "size": 48, "activation": "PairwiseCReLU" } })),
        "/accumulator/size",
        "multiple of 32",
    );
    rejected(
        with(json!({ "hiddenLayers": [{ "size": 8, "activation": "PairwiseCReLU" }] })),
        "/hiddenLayers/0/activation",
        "only supported on the accumulator",
    );
    rejected(
        with(json!({ "outputBuckets": { "strategy": "pieceCount", "count": 3 } })),
        "/outputBuckets/count",
        "divisor of 32",
    );
    rejected(
        with(json!({ "outputBuckets": { "strategy": "material" } })),
        "/outputBuckets/count",
        "required",
    );
    rejected(
        with(json!({ "outputBuckets": { "count": 4 } })),
        "/outputBuckets/count",
        "single bucket",
    );
    rejected(
        with(json!({ "quantization": { "ftWeightClip": 4.0 } })),
        "/quantization/ftWeightClip",
        "i16 accumulator",
    );
    rejected(
        with(json!({ "quantization": { "weightClip": 2.5 } })),
        "/quantization/weightClip",
        "i8",
    );
    rejected(
        with(json!({ "quantization": { "weightClip": 0.001 } })),
        "/quantization/weightClip",
        "rounds to 0",
    );
    rejected(
        with(json!({
            "features": { "set": "HalfKP" },
            "accumulator": { "size": 4096 },
            "quantization": { "qa": 32 }
        })),
        "",
        "MiB limit",
    );
}

#[test]
fn reports_every_problem_at_once() {
    let doc = with(json!({
        "accumulator": { "size": 48, "activation": "PairwiseCReLU" },
        "hiddenLayers": [{ "size": 8, "activation": "PairwiseCReLU" }],
        "outputBuckets": { "strategy": "material" }
    }));
    let errors = check(&doc).unwrap_err();
    let paths: Vec<_> = errors.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "/accumulator/size",
            "/hiddenLayers/0/activation",
            "/outputBuckets/count"
        ]
    );
    assert!(errors[0].to_string().starts_with("/accumulator/size: "));
}

#[test]
fn round_trips_through_serde() {
    for preset in presets::all() {
        let text = serde_json::to_string(&preset.architecture).unwrap();
        let back = parse(&text, Format::Json).unwrap();
        assert_eq!(back, preset.architecture);
        let normalized = serde_json::to_string(&preset.architecture.normalized()).unwrap();
        assert_eq!(
            parse(&normalized, Format::Json).unwrap().hash(),
            preset.architecture.hash()
        );
    }
    assert_eq!(
        check(&minimal()).unwrap().accumulator.activation,
        Activation::CReLU
    );
}
