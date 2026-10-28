//! The architecture document. Doc comments here become the descriptions of
//! the JSON Schema, so they are written for API users.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A complete NNUE architecture: input features, feature transformer
/// (accumulator), dense layers, output buckets and quantization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(title = "NNUE architecture")]
pub struct Architecture {
    /// Optional link to this schema, for editors. Ignored.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// Version of the architecture schema. Always 1.
    #[schemars(extend("const" = 1))]
    pub schema_version: u32,
    /// Descriptive information. Not part of the architecture hash.
    pub metadata: Metadata,
    /// Input features of the network.
    pub features: Features,
    /// Feature transformer: the first, sparse layer, updated incrementally.
    pub accumulator: Accumulator,
    /// Dense layers between the accumulator and the output, in order. The
    /// output layer (one neuron, no activation) is implicit.
    #[serde(default)]
    #[schemars(length(max = 16))]
    pub hidden_layers: Vec<DenseLayer>,
    /// Several sets of weights for the layers after the accumulator, one of
    /// which is chosen by the position.
    #[serde(default)]
    pub output_buckets: OutputBuckets,
    /// How weights and activations are stored as integers.
    pub quantization: Quantization,
}

/// Descriptive information about a network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Metadata {
    /// Display name.
    #[schemars(length(min = 1, max = 64))]
    pub name: String,
    /// Free-form version, e.g. `1.2.0`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 32))]
    pub version: Option<String>,
    /// Who designed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 64))]
    pub author: Option<String>,
    /// What it is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 2000))]
    pub description: Option<String>,
    /// Lowercase labels: letters, digits and dashes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(length(max = 16), inner(regex(pattern = r"^[a-z0-9][a-z0-9-]{0,31}$")))]
    pub tags: Vec<String>,
}

/// Input features.
///
/// Each perspective sees the board from its own side: squares are flipped
/// vertically for Black, and pieces are "own" or "opponent". With king
/// buckets, the features are repeated once per bucket and the bucket of the
/// perspective's own king selects which copy is active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Features {
    /// Piece encoding:
    /// `Chess768`: 12 planes (own/opponent × 6 kinds, kings included) × 64 squares;
    /// `HalfKP`: 10 planes (kings excluded) × 64, one copy per king square by default;
    /// `HalfKAv2_hm`: 11 planes (both kings share one) × 64, mirrored, 32 king buckets by default.
    pub set: FeatureSet,
    /// Bucket of each square of the perspective's own king, 64 entries from
    /// a1 (index 0, own back rank) to h8 (index 63). Ids are 0..N-1, all
    /// used. Default: one bucket for `Chess768`, one per square for
    /// `HalfKP`, one per square of files a–d for `HalfKAv2_hm`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 64, max = 64), inner(range(max = 63)))]
    pub king_buckets: Option<Vec<u8>>,
    /// Horizontal mirroring: with `kingFile`, a perspective whose own king
    /// stands on files e–h sees the board mirrored (a↔h), so its king is
    /// always on files a–d. Default: `kingFile` for `HalfKAv2_hm`, `none`
    /// otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror: Option<Mirror>,
    /// Two accumulators, one per side, the side to move's first in the
    /// next layer's input. Without it, a single accumulator sees the board
    /// as White and the result is negated when Black is to move.
    #[serde(default = "yes")]
    pub perspective: bool,
}

const fn yes() -> bool {
    true
}

/// Predefined feature sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum FeatureSet {
    /// Own/opponent × 6 piece kinds × 64 squares.
    Chess768,
    /// Own/opponent × 5 non-king kinds × 64 squares, per king bucket.
    HalfKP,
    /// Own/opponent × 5 non-king kinds + kings × 64 squares, per king
    /// bucket, horizontally mirrored.
    #[serde(rename = "HalfKAv2_hm")]
    #[schemars(rename = "HalfKAv2_hm")]
    HalfKAv2Hm,
}

/// Horizontal mirroring of the features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Mirror {
    /// Never mirrored.
    None,
    /// Mirrored when the perspective's own king is on files e–h.
    KingFile,
}

/// Activation functions, on values where 1.0 is the quantized `qa`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum Activation {
    /// `max(x, 0)` (saturated at the i16 range once quantized).
    ReLU,
    /// Clipped ReLU: `clamp(x, 0, 1)`.
    CReLU,
    /// Squared clipped ReLU: `clamp(x, 0, 1)²`.
    SCReLU,
    /// Accumulator only: each half of a perspective multiplied by the other,
    /// `clamp(a, 0, 1) · clamp(b, 0, 1)`; halves the accumulator's output.
    PairwiseCReLU,
}

/// The feature transformer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Accumulator {
    /// Neurons per perspective; a multiple of 16 (32 with `PairwiseCReLU`).
    #[schemars(range(min = 16, max = 4096), extend("multipleOf" = 16))]
    pub size: u32,
    /// Applied to each perspective before the dense layers.
    pub activation: Activation,
}

/// A dense (fully connected) layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DenseLayer {
    /// Number of neurons.
    #[schemars(range(min = 1, max = 4096))]
    pub size: u32,
    /// `ReLU`, `CReLU` or `SCReLU`.
    pub activation: Activation,
}

/// Output buckets.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutputBuckets {
    /// How a position picks its bucket.
    #[serde(default)]
    pub strategy: BucketStrategy,
    /// Number of buckets: required unless the strategy is `none`. With
    /// `pieceCount`, a divisor of 32.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 32))]
    pub count: Option<u32>,
    /// Bucketed layers: the output layer only (default) or every layer
    /// after the accumulator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<BucketScope>,
}

/// Bucket selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum BucketStrategy {
    /// A single set of weights.
    #[default]
    None,
    /// `(pieces − 2) / (32 / count)`, pieces counted with the kings.
    PieceCount,
    /// `min(count − 1, material · count / 79)`, material counted P=1, N=B=3,
    /// R=5, Q=9 for both sides (78 at the start).
    Material,
}

/// Layers with one set of weights per bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum BucketScope {
    /// The output layer only.
    Output,
    /// Every layer after the accumulator ("layer stacks").
    All,
}

/// Integer storage of the network.
///
/// Accumulator values are stored ×`qa` (1.0 ↔ `qa`), dense weights ×`qb`,
/// dense biases ×`qa·qb` in i32. Every activation brings values back to the
/// `qa` scale, and the network output is `sum · evalScale / (qa · qb)`
/// centipawns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Quantization {
    /// Accumulator (and feature transformer weights) type.
    #[serde(default)]
    pub accumulator: AccumulatorType,
    /// Type of the dense layers' weights.
    pub weights: WeightType,
    /// Scale of the accumulator: an activation of 1.0 is `qa`.
    #[schemars(range(min = 1, max = 32767))]
    pub qa: u32,
    /// Scale of the dense weights.
    #[schemars(range(min = 1, max = 32767))]
    pub qb: u32,
    /// Centipawns per unit of network output (the trainer's sigmoid scale).
    #[schemars(range(min = 1, max = 10000))]
    pub eval_scale: u32,
    /// Bound on feature transformer weights and biases during training, so
    /// that the i16 accumulator never overflows. Default: the largest safe
    /// value, `⌊32767 / (max active features + 1)⌋ / qa`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub ft_weight_clip: Option<f64>,
    /// Bound on dense weights during training. Default: the largest value
    /// the weight type holds, `127 / qb` (i8) or `32767 / qb` (i16).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub weight_clip: Option<f64>,
}

/// Accumulator integer type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AccumulatorType {
    /// 16-bit accumulators.
    #[default]
    I16,
}

/// Dense weights integer type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum WeightType {
    /// 8-bit weights.
    I8,
    /// 16-bit weights.
    I16,
}

impl WeightType {
    /// Largest stored magnitude.
    #[must_use]
    pub const fn max(self) -> i64 {
        match self {
            Self::I8 => i8::MAX as i64,
            Self::I16 => i16::MAX as i64,
        }
    }

    /// Bytes per weight.
    #[must_use]
    pub const fn bytes(self) -> u64 {
        match self {
            Self::I8 => 1,
            Self::I16 => 2,
        }
    }
}

impl FeatureSet {
    /// Feature planes of 64 squares per king bucket.
    #[must_use]
    pub const fn planes(self) -> usize {
        match self {
            Self::Chess768 => 12,
            Self::HalfKP => 10,
            Self::HalfKAv2Hm => 11,
        }
    }

    /// Most features active at once in a legal position, per perspective.
    #[must_use]
    pub const fn max_active(self) -> usize {
        match self {
            Self::Chess768 | Self::HalfKAv2Hm => 32,
            Self::HalfKP => 30,
        }
    }

    /// Default king bucket of each square.
    #[must_use]
    pub fn default_king_buckets(self) -> Vec<u8> {
        (0..64_u8)
            .map(|square| match self {
                Self::Chess768 => 0,
                Self::HalfKP => square,
                // Files e–h are mirrored onto d–a.
                Self::HalfKAv2Hm => {
                    let (rank, file) = (square / 8, square % 8);
                    rank * 4 + file.min(7 - file)
                }
            })
            .collect()
    }

    /// Default mirroring.
    #[must_use]
    pub const fn default_mirror(self) -> Mirror {
        match self {
            Self::HalfKAv2Hm => Mirror::KingFile,
            Self::Chess768 | Self::HalfKP => Mirror::None,
        }
    }

    /// Name in architecture documents.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Chess768 => "Chess768",
            Self::HalfKP => "HalfKP",
            Self::HalfKAv2Hm => "HalfKAv2_hm",
        }
    }
}

impl Activation {
    /// Name in architecture documents.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ReLU => "ReLU",
            Self::CReLU => "CReLU",
            Self::SCReLU => "SCReLU",
            Self::PairwiseCReLU => "PairwiseCReLU",
        }
    }
}
