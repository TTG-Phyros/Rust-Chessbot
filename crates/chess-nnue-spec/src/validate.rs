//! Validation: the JSON Schema first (types, ranges, unknown fields), then
//! the rules a schema cannot express (cross-field constraints, integer
//! overflow, what the engine's inference supports). Every error points at
//! the offending value with a JSON pointer.

use std::sync::OnceLock;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::layout::{I16_MAX, activation_bound};
use crate::model::{Activation, Architecture, BucketStrategy, FeatureSet, Mirror};

/// Largest quantized network accepted, in bytes.
pub const MAX_WEIGHTS_BYTES: u64 = 256 << 20;

/// One problem in an architecture document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ValidationError {
    /// JSON pointer (RFC 6901) to the offending value; `""` is the document.
    pub path: String,
    /// What is wrong, and usually how to fix it.
    pub message: String,
}

impl ValidationError {
    pub(crate) fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = if self.path.is_empty() {
            "/"
        } else {
            &self.path
        };
        write!(f, "{path}: {}", self.message)
    }
}

/// Validates `value` against the JSON Schema, then deserializes it and
/// checks the semantic rules.
///
/// # Errors
///
/// Every problem found: all schema violations, or, when the document
/// matches the schema, all semantic ones.
pub fn from_value(value: &Value) -> Result<Architecture, Vec<ValidationError>> {
    let errors = structural(value);
    if !errors.is_empty() {
        return Err(errors);
    }
    let arch: Architecture = serde_json::from_value(value.clone())
        .map_err(|err| vec![ValidationError::new("", err.to_string())])?;
    let errors = semantic(&arch);
    if errors.is_empty() {
        Ok(arch)
    } else {
        Err(errors)
    }
}

fn structural(value: &Value) -> Vec<ValidationError> {
    static VALIDATOR: OnceLock<Result<jsonschema::Validator, String>> = OnceLock::new();
    let validator = VALIDATOR.get_or_init(|| {
        jsonschema::validator_for(&crate::json_schema()).map_err(|err| err.to_string())
    });
    match validator {
        Ok(validator) => validator
            .iter_errors(value)
            .map(|err| {
                // The default message names only the first two allowed values.
                let message = match err.kind() {
                    jsonschema::error::ValidationErrorKind::Enum { options } => {
                        let options: Vec<String> = options
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(ToString::to_string)
                            .collect();
                        format!("{} is not one of {}", err.instance(), options.join(", "))
                    }
                    _ => err.to_string(),
                };
                ValidationError::new(err.instance_path().as_str(), message)
            })
            .collect(),
        Err(err) => vec![ValidationError::new(
            "",
            format!("internal schema error: {err}"),
        )],
    }
}

/// Rules the JSON Schema cannot express.
pub(crate) fn semantic(arch: &Architecture) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let mut fail = |path: &str, message: String| errors.push(ValidationError::new(path, message));
    let set = arch.features.set;
    let mirror = arch.features.mirror.unwrap_or(set.default_mirror());

    // Features.
    if let Some(map) = &arch.features.king_buckets {
        let count = map.iter().copied().max().map_or(0, |m| usize::from(m) + 1);
        if let Some(missing) = (0..count).find(|&id| !map.contains(&(id as u8))) {
            fail(
                "/features/kingBuckets",
                format!("bucket ids must be 0..N-1 with every id used: {missing} is missing"),
            );
        }
        if mirror == Mirror::KingFile
            && let Some(square) = (0..64).find(|&s| map[s] != map[s ^ 7])
        {
            fail(
                &format!("/features/kingBuckets/{square}"),
                format!(
                    "with mirror kingFile a king on {} is seen on {}: both squares must share a bucket ({} ≠ {})",
                    square_name(square),
                    square_name(square ^ 7),
                    map[square],
                    map[square ^ 7]
                ),
            );
        }
    }
    if set == FeatureSet::HalfKAv2Hm && mirror == Mirror::None {
        fail(
            "/features/mirror",
            "HalfKAv2_hm is horizontally mirrored by definition: use kingFile".into(),
        );
    }
    if !arch.features.perspective {
        if set != FeatureSet::Chess768 {
            fail(
                "/features/perspective",
                format!(
                    "{} needs the king of each side, hence perspective: true",
                    set.name()
                ),
            );
        }
        if arch
            .features
            .king_buckets
            .as_ref()
            .is_some_and(|map| map.iter().any(|&b| b != 0))
        {
            fail(
                "/features/kingBuckets",
                "king buckets need perspective: true".into(),
            );
        }
        if mirror == Mirror::KingFile {
            fail(
                "/features/mirror",
                "mirroring needs perspective: true".into(),
            );
        }
    }

    // Layers.
    if arch.accumulator.activation == Activation::PairwiseCReLU
        && !arch.accumulator.size.is_multiple_of(32)
    {
        fail(
            "/accumulator/size",
            format!(
                "PairwiseCReLU halves the accumulator: its size must be a multiple of 32, not {}",
                arch.accumulator.size
            ),
        );
    }
    for (i, layer) in arch.hidden_layers.iter().enumerate() {
        if layer.activation == Activation::PairwiseCReLU {
            fail(
                &format!("/hiddenLayers/{i}/activation"),
                "PairwiseCReLU is only supported on the accumulator: use ReLU, CReLU or SCReLU"
                    .into(),
            );
        }
    }

    // Output buckets.
    let buckets = &arch.output_buckets;
    match (buckets.strategy, buckets.count) {
        (BucketStrategy::None, Some(count)) if count != 1 => fail(
            "/outputBuckets/count",
            format!(
                "strategy none means a single bucket, not {count}: pick pieceCount or material"
            ),
        ),
        (BucketStrategy::PieceCount | BucketStrategy::Material, None) => fail(
            "/outputBuckets/count",
            "required when the strategy is not none".into(),
        ),
        (BucketStrategy::PieceCount, Some(count)) if count < 2 || 32 % count != 0 => fail(
            "/outputBuckets/count",
            format!(
                "pieceCount needs a divisor of 32 greater than 1 (2, 4, 8, 16 or 32), not {count}"
            ),
        ),
        (BucketStrategy::Material, Some(count)) if count < 2 => fail(
            "/outputBuckets/count",
            format!("material needs at least 2 buckets, not {count}"),
        ),
        _ => {}
    }

    // Quantization.
    let quant = &arch.quantization;
    let normalized = arch.normalized();
    let qa = i64::from(quant.qa);
    let layout = normalized.layout();
    let ft_clip = normalized.quantization.ft_weight_clip.unwrap_or(0.0);
    if !ft_clip.is_finite() || layout.ft_weight_max < 1 {
        fail(
            "/quantization/ftWeightClip",
            format!(
                "{ft_clip} × qa {qa} rounds to 0: the feature transformer could not learn anything"
            ),
        );
    }
    let accumulator_worst = (layout.max_active as i64 + 1) * layout.ft_weight_max;
    if accumulator_worst > I16_MAX {
        let safe = (I16_MAX / (layout.max_active as i64 + 1)) as f64 / qa as f64;
        fail(
            "/quantization/ftWeightClip",
            format!(
                "{} active features + bias × {} can reach {accumulator_worst}, beyond the i16 accumulator ({I16_MAX}): use at most {:.4}",
                layout.max_active,
                layout.ft_weight_max,
                (safe * 1e4).floor() / 1e4
            ),
        );
    }
    let weight_clip = normalized.quantization.weight_clip.unwrap_or(0.0);
    let type_max = quant.weights.max();
    if !weight_clip.is_finite() || layout.weight_max < 1 {
        fail(
            "/quantization/weightClip",
            format!(
                "{weight_clip} × qb {} rounds to 0: the dense layers could not learn anything",
                quant.qb
            ),
        );
    } else if layout.weight_max > type_max {
        fail(
            "/quantization/weightClip",
            format!(
                "{weight_clip} × qb {} = {} does not fit the {} weights (at most {type_max}): use at most {}",
                quant.qb,
                layout.weight_max,
                if type_max == 127 { "i8" } else { "i16" },
                type_max as f64 / f64::from(quant.qb)
            ),
        );
    }
    // Squared and pairwise products are computed in i32 before rescaling.
    let squared = std::iter::once(arch.accumulator.activation)
        .chain(arch.hidden_layers.iter().map(|l| l.activation))
        .any(|a| matches!(a, Activation::SCReLU | Activation::PairwiseCReLU));
    if squared && activation_bound(Activation::SCReLU, qa).pow(2) > i64::from(i32::MAX) {
        fail(
            "/quantization/qa",
            format!("qa² must fit in i32 for SCReLU/PairwiseCReLU: qa ≤ 46340, not {qa}"),
        );
    }

    let size = normalized.estimate().weights_bytes;
    if errors.is_empty() && size > MAX_WEIGHTS_BYTES {
        errors.push(ValidationError::new(
            "",
            format!(
                "the quantized network would weigh {:.1} MiB, more than the {} MiB limit: shrink the accumulator or the king buckets",
                size as f64 / f64::from(1 << 20),
                MAX_WEIGHTS_BYTES >> 20
            ),
        ));
    }
    errors
}

fn square_name(square: usize) -> String {
    let file = char::from(b'a' + (square % 8) as u8);
    format!("{file}{}", square / 8 + 1)
}
