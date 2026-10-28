//! What an architecture resolves to: defaults filled in, tensor shapes,
//! integer bounds, and size and cost estimates.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::{
    Activation, Architecture, BucketScope, BucketStrategy, FeatureSet, Mirror, WeightType,
};

/// Largest i16 value: the bound of accumulators and of quantized activations.
pub(crate) const I16_MAX: i64 = i16::MAX as i64;

/// Dot products of dense layers whose worst case stays below this are
/// summed in i32; the other half of the i32 range is left to the bias.
pub const I32_DOT_LIMIT: i64 = 1 << 30;

/// Integer type in which a dense layer sums its inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SumWidth {
    /// The fast path.
    I32,
    /// Needed when inputs × activation × weight can exceed 2³⁰.
    I64,
}

/// A dense layer as stored: `buckets` sets of `outputs × inputs` weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DenseShape {
    /// Inputs of one neuron.
    pub inputs: usize,
    /// Neurons.
    pub outputs: usize,
    /// Sets of weights (1 when the layer is not bucketed).
    pub buckets: usize,
    /// Activation after the layer; `None` for the output layer.
    pub activation: Option<Activation>,
    /// Largest quantized input value.
    pub input_bound: i64,
    /// Integer type of the dot products.
    pub sum: SumWidth,
}

/// Resolved shapes of a (normalized) architecture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// Feature encoding.
    pub feature_set: FeatureSet,
    /// Planes of 64 squares per king bucket.
    pub planes: usize,
    /// Distinct king buckets.
    pub king_buckets: usize,
    /// Input features per perspective (`king_buckets × planes × 64`).
    pub inputs: usize,
    /// Most active features per perspective.
    pub max_active: usize,
    /// 2 with perspective, 1 without.
    pub perspectives: usize,
    /// Accumulator neurons per perspective.
    pub accumulator: usize,
    /// Accumulator activation.
    pub accumulator_activation: Activation,
    /// Accumulator outputs per perspective after its activation.
    pub ft_outputs: usize,
    /// Hidden layers, then the output layer.
    pub dense: Vec<DenseShape>,
    /// Output buckets.
    pub output_buckets: usize,
    /// Largest quantized feature transformer weight or bias.
    pub ft_weight_max: i64,
    /// Largest quantized dense weight.
    pub weight_max: i64,
    /// Whether a king move can force a full refresh of its perspective.
    pub king_refreshes: bool,
}

/// Size and cost of an architecture, to compare designs before training.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Estimate {
    /// Input features per perspective.
    pub input_features: u64,
    /// Trainable parameters.
    pub parameters: u64,
    /// Size of the quantized weights, in bytes (the network file adds a
    /// header of a few hundred bytes).
    pub weights_bytes: u64,
    /// Memory of one accumulator (both perspectives), copied at every ply.
    pub accumulator_bytes: u64,
    /// i16 additions to update the accumulators after a quiet move.
    pub update_ops: u64,
    /// i16 additions to rebuild one perspective from scratch (king bucket
    /// or mirror change).
    pub refresh_ops: u64,
    /// Multiply-adds of the dense layers per evaluation.
    pub dense_macs: u64,
    /// Whether king moves can force refreshes.
    pub king_refreshes: bool,
    /// `(update_ops + dense_macs)` relative to `legacy-768x256`: a rough
    /// proxy of the evaluation cost, not a measurement.
    pub cost_vs_legacy: f64,
}

/// `update_ops + dense_macs` of `legacy-768x256`.
const LEGACY_COST: f64 = (2 * 2 * 256 + 512) as f64;

impl Architecture {
    /// The same architecture with every default written out, and without
    /// the editor-only `$schema`. Equivalent documents normalize to the
    /// same value (and hash).
    #[must_use]
    pub fn normalized(&self) -> Self {
        let mut arch = self.clone();
        arch.schema = None;
        let set = arch.features.set;
        arch.features
            .king_buckets
            .get_or_insert_with(|| set.default_king_buckets());
        arch.features.mirror.get_or_insert(set.default_mirror());

        let buckets = &mut arch.output_buckets;
        if buckets.strategy == BucketStrategy::None {
            buckets.count = Some(1);
        }
        if buckets.count.unwrap_or(1) <= 1 {
            // A single bucket: where it applies does not matter.
            buckets.scope = Some(BucketScope::Output);
        }
        buckets.scope.get_or_insert(BucketScope::Output);

        let quant = &mut arch.quantization;
        let (qa, qb) = (f64::from(quant.qa.max(1)), f64::from(quant.qb.max(1)));
        quant
            .ft_weight_clip
            .get_or_insert_with(|| (I16_MAX / (set.max_active() as i64 + 1)) as f64 / qa);
        quant
            .weight_clip
            .get_or_insert_with(|| quant.weights.max() as f64 / qb);
        arch
    }

    /// Shapes and bounds. Meaningful for valid architectures only.
    #[must_use]
    pub fn layout(&self) -> Layout {
        let arch = self.normalized();
        let set = arch.features.set;
        let king_map = arch.features.king_buckets.unwrap_or_default();
        let king_buckets = king_map
            .iter()
            .copied()
            .max()
            .map_or(1, |m| usize::from(m) + 1);
        let perspectives = if arch.features.perspective { 2 } else { 1 };
        let accumulator = arch.accumulator.size as usize;
        let ft_outputs = match arch.accumulator.activation {
            Activation::PairwiseCReLU => accumulator / 2,
            _ => accumulator,
        };
        let quant = &arch.quantization;
        let qa = i64::from(quant.qa);
        let quantize =
            |clip: Option<f64>, scale: i64| (clip.unwrap_or(0.0) * scale as f64).round() as i64;
        let ft_weight_max = quantize(quant.ft_weight_clip, qa);
        let weight_max = quantize(quant.weight_clip, i64::from(quant.qb));

        let output_buckets = arch.output_buckets.count.unwrap_or(1).max(1) as usize;
        let all_bucketed = arch.output_buckets.scope == Some(BucketScope::All);
        let mut dense = Vec::with_capacity(arch.hidden_layers.len() + 1);
        let mut inputs = perspectives * ft_outputs;
        let mut input_bound = activation_bound(arch.accumulator.activation, qa);
        let layers = arch
            .hidden_layers
            .iter()
            .map(|layer| (layer.size as usize, Some(layer.activation)))
            .chain(std::iter::once((1, None)));
        for (outputs, activation) in layers {
            let worst = (inputs as i64)
                .saturating_mul(input_bound)
                .saturating_mul(weight_max);
            dense.push(DenseShape {
                inputs,
                outputs,
                buckets: if activation.is_none() || all_bucketed {
                    output_buckets
                } else {
                    1
                },
                activation,
                input_bound,
                sum: if worst <= I32_DOT_LIMIT {
                    SumWidth::I32
                } else {
                    SumWidth::I64
                },
            });
            if let Some(activation) = activation {
                input_bound = activation_bound(activation, qa);
            }
            inputs = outputs;
        }

        Layout {
            feature_set: set,
            planes: set.planes(),
            king_buckets,
            inputs: king_buckets * set.planes() * 64,
            max_active: set.max_active(),
            perspectives,
            accumulator,
            accumulator_activation: arch.accumulator.activation,
            ft_outputs,
            dense,
            output_buckets,
            ft_weight_max,
            weight_max,
            king_refreshes: arch.features.perspective
                && (king_buckets > 1 || arch.features.mirror == Some(Mirror::KingFile)),
        }
    }

    /// Size and cost estimates. Meaningful for valid architectures only.
    #[must_use]
    pub fn estimate(&self) -> Estimate {
        self.layout().estimate(self.quantization.weights)
    }
}

impl Layout {
    fn estimate(&self, weights: WeightType) -> Estimate {
        let (inputs, acc) = (self.inputs as u64, self.accumulator as u64);
        let ft_parameters = inputs * acc + acc;
        let dense_parameters: u64 = self
            .dense
            .iter()
            .map(|d| (d.buckets * (d.inputs * d.outputs + d.outputs)) as u64)
            .sum();
        let dense_bytes: u64 = self
            .dense
            .iter()
            .map(|d| {
                (d.buckets as u64)
                    * ((d.inputs * d.outputs) as u64 * weights.bytes() + d.outputs as u64 * 4)
            })
            .sum();
        let perspectives = self.perspectives as u64;
        let update_ops = 2 * perspectives * acc;
        let dense_macs: u64 = self
            .dense
            .iter()
            .map(|d| (d.inputs * d.outputs) as u64)
            .sum();
        Estimate {
            input_features: inputs,
            parameters: ft_parameters + dense_parameters,
            weights_bytes: ft_parameters * 2 + dense_bytes,
            accumulator_bytes: perspectives * acc * 2,
            update_ops,
            refresh_ops: self.max_active as u64 * acc,
            dense_macs,
            king_refreshes: self.king_refreshes,
            cost_vs_legacy: (update_ops + dense_macs) as f64 / LEGACY_COST,
        }
    }
}

/// Largest quantized value after `activation` (every activation brings its
/// output back to the `qa` scale; ReLU saturates at the i16 range).
#[must_use]
pub const fn activation_bound(activation: Activation, qa: i64) -> i64 {
    match activation {
        Activation::ReLU => I16_MAX,
        Activation::CReLU | Activation::SCReLU | Activation::PairwiseCReLU => qa,
    }
}
