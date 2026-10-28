//! A quantized network of any valid architecture, and its evaluation.
//!
//! Integer semantics (the trainers' export must match them):
//!
//! - accumulators are `i16`: `ft_bias + Σ ft_weights[feature]`;
//! - accumulator activations, on the `qa` scale: ReLU `max(x, 0)`, CReLU
//!   `clamp(x, 0, qa)`, SCReLU `clamp(x, 0, qa)² / qa`, pairwise CReLU
//!   `clamp(a, 0, qa) · clamp(b, 0, qa) / qa` (first half × second half);
//! - dense layer: `sum = bias + Σ input · weight` (scale `qa · qb`), then for
//!   a hidden layer `h = sum / qb` and the same activations on `h` (ReLU
//!   saturating at 32767);
//! - output: `sum · evalScale / (qa · qb)` centipawns for the side to move.
//!
//! Divisions truncate toward zero.

use std::cell::RefCell;

use chess_core::{Color, Position};
use chess_nnue_spec::{
    Activation, Architecture, ArchitectureHash, I32_DOT_LIMIT, Layout, SumWidth,
};

use crate::divisor::Divisor;
use crate::features::{Context, Encoder, pieces};
use crate::{NnueError, Score, simd};

/// Weights and biases of one dense layer: `buckets × outputs × inputs`
/// weights (row-major: one row of inputs per neuron) and `buckets ×
/// outputs` biases, scaled by `qa · qb`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenseWeights {
    /// Weights, widened to `i16` whatever the stored type.
    pub weights: Vec<i16>,
    /// Biases.
    pub bias: Vec<i32>,
}

/// How the output is computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Output {
    /// No hidden layer and a clipped ReLU: the activation is fused into the
    /// output layer's dot products (vectorized).
    FusedCReLU,
    /// Any architecture: activations into a buffer, then each layer.
    Generic,
}

/// A loaded network.
#[derive(Debug, Clone)]
pub struct Network {
    architecture: Architecture,
    hash: ArchitectureHash,
    layout: Layout,
    encoder: Encoder,
    /// `inputs × accumulator`, feature-major.
    ft_weights: Vec<i16>,
    ft_bias: Vec<i16>,
    dense: Vec<DenseWeights>,
    qa: i16,
    qa_divisor: Divisor,
    /// Brings a dense layer's sum back to the `qa` scale.
    qb: Divisor,
    /// Output scale: `qa · qb`.
    output_scale: Divisor,
    eval_scale: i64,
    output: Output,
    /// Accumulator size when moves update it with the kernels compiled for
    /// that size (two perspectives, no king refreshes, a common size).
    fixed_update: Option<usize>,
}

impl Network {
    /// A network of `architecture` with the given weights.
    ///
    /// # Errors
    ///
    /// [`NnueError::Architecture`] if the architecture is invalid,
    /// [`NnueError::Shape`] if a tensor has the wrong size and
    /// [`NnueError::Range`] if a value exceeds the bounds the architecture's
    /// quantization guarantees (which would overflow the integer sums).
    pub fn from_parts(
        architecture: &Architecture,
        ft_weights: Vec<i16>,
        ft_bias: Vec<i16>,
        dense: Vec<DenseWeights>,
    ) -> Result<Self, NnueError> {
        let value = serde_json::to_value(architecture.normalized())
            .map_err(|err| NnueError::Architecture(err.to_string()))?;
        let architecture = chess_nnue_spec::from_value(&value).map_err(|errors| {
            NnueError::Architecture(
                errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        })?;
        let layout = architecture.layout();
        check_len(
            "feature transformer weights",
            ft_weights.len(),
            layout.inputs * layout.accumulator,
        )?;
        check_len(
            "feature transformer biases",
            ft_bias.len(),
            layout.accumulator,
        )?;
        check_len("dense layers", dense.len(), layout.dense.len())?;
        for (i, (layer, shape)) in dense.iter().zip(&layout.dense).enumerate() {
            let outputs = shape.buckets * shape.outputs;
            check_len(
                &format!("dense layer {i} weights"),
                layer.weights.len(),
                outputs * shape.inputs,
            )?;
            check_len(
                &format!("dense layer {i} biases"),
                layer.bias.len(),
                outputs,
            )?;
        }

        check_range(
            "feature transformer",
            ft_weights.iter().chain(&ft_bias).map(|&v| i64::from(v)),
            layout.ft_weight_max,
        )?;
        for (i, layer) in dense.iter().enumerate() {
            check_range(
                &format!("dense layer {i} weights"),
                layer.weights.iter().map(|&v| i64::from(v)),
                layout.weight_max,
            )?;
            check_range(
                &format!("dense layer {i} biases"),
                layer.bias.iter().map(|&v| i64::from(v)),
                I32_DOT_LIMIT,
            )?;
        }

        let quant = &architecture.quantization;
        let output = if layout.dense.len() == 1
            && layout.accumulator_activation == Activation::CReLU
            && layout.dense[0].sum == SumWidth::I32
        {
            Output::FusedCReLU
        } else {
            Output::Generic
        };
        let fixed_update = (!layout.king_refreshes
            && layout.perspectives == 2
            && [256, 512, 768, 1024, 1536, 2048].contains(&layout.accumulator))
        .then_some(layout.accumulator);
        Ok(Self {
            fixed_update,
            hash: architecture.hash(),
            encoder: Encoder::new(&architecture),
            qa: quant.qa as i16,
            qa_divisor: Divisor::new(u64::from(quant.qa)),
            qb: Divisor::new(u64::from(quant.qb)),
            output_scale: Divisor::new(u64::from(quant.qa) * u64::from(quant.qb)),
            eval_scale: i64::from(quant.eval_scale),
            architecture,
            layout,
            ft_weights,
            ft_bias,
            dense,
            output,
        })
    }

    /// A network of `architecture` whose weights are all zero.
    ///
    /// # Errors
    ///
    /// [`NnueError::Architecture`] if the architecture is invalid.
    pub fn zeros(architecture: &Architecture) -> Result<Self, NnueError> {
        let layout = architecture.layout();
        let dense = layout
            .dense
            .iter()
            .map(|shape| DenseWeights {
                weights: vec![0; shape.buckets * shape.outputs * shape.inputs],
                bias: vec![0; shape.buckets * shape.outputs],
            })
            .collect();
        Self::from_parts(
            architecture,
            vec![0; layout.inputs * layout.accumulator],
            vec![0; layout.accumulator],
            dense,
        )
    }

    /// A network of `architecture` with deterministic pseudo-random weights
    /// within its quantization bounds, for tests and benchmarks.
    ///
    /// # Errors
    ///
    /// [`NnueError::Architecture`] if the architecture is invalid.
    pub fn pseudo_random(architecture: &Architecture, seed: u64) -> Result<Self, NnueError> {
        let layout = architecture.layout();
        let mut state = seed | 1;
        let mut next = move |bound: i64| -> i64 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % (2 * bound as u64 + 1)) as i64 - bound
        };
        let ft_max = layout.ft_weight_max;
        let ft_weights = (0..layout.inputs * layout.accumulator)
            .map(|_| next(ft_max / 2) as i16)
            .collect();
        let ft_bias = (0..layout.accumulator)
            .map(|_| next(ft_max) as i16)
            .collect();
        let dense = layout
            .dense
            .iter()
            .map(|shape| {
                let neurons = shape.buckets * shape.outputs;
                DenseWeights {
                    weights: (0..neurons * shape.inputs)
                        .map(|_| next(layout.weight_max) as i16)
                        .collect(),
                    bias: (0..neurons).map(|_| next(10_000) as i32).collect(),
                }
            })
            .collect();
        Self::from_parts(architecture, ft_weights, ft_bias, dense)
    }

    /// The same network without specialized kernels: every evaluation goes
    /// through the generic path (for tests and benchmarks).
    #[must_use]
    pub fn generic(mut self) -> Self {
        self.output = Output::Generic;
        self.fixed_update = None;
        self
    }

    /// Kernels in use: `generic`, or the specialized ones
    /// (`fixed-update`, `fused-crelu`, joined with `+`).
    #[must_use]
    pub fn kernel(&self) -> String {
        let mut kernels = Vec::new();
        if self.fixed_update.is_some() {
            kernels.push("fixed-update");
        }
        if self.output == Output::FusedCReLU {
            kernels.push("fused-crelu");
        }
        if kernels.is_empty() {
            "generic".into()
        } else {
            kernels.join("+")
        }
    }

    /// The architecture, normalized (every default written out).
    #[must_use]
    pub fn architecture(&self) -> &Architecture {
        &self.architecture
    }

    /// Hash of the architecture.
    #[must_use]
    pub fn hash(&self) -> ArchitectureHash {
        self.hash
    }

    /// Resolved shapes.
    #[must_use]
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Checks that this network has the architecture `expected`.
    ///
    /// # Errors
    ///
    /// [`NnueError::HashMismatch`] otherwise.
    pub fn expect_hash(&self, expected: ArchitectureHash) -> Result<(), NnueError> {
        if self.hash == expected {
            Ok(())
        } else {
            Err(NnueError::HashMismatch {
                expected: expected.to_string(),
                found: self.hash.to_string(),
            })
        }
    }

    pub(crate) fn fixed_update(&self) -> Option<usize> {
        self.fixed_update
    }

    pub(crate) fn ft_weights(&self) -> &[i16] {
        &self.ft_weights
    }

    pub(crate) fn ft_bias(&self) -> &[i16] {
        &self.ft_bias
    }

    pub(crate) fn dense(&self) -> &[DenseWeights] {
        &self.dense
    }

    pub(crate) fn encoder(&self) -> &Encoder {
        &self.encoder
    }

    /// Row of the feature transformer for `feature`.
    pub(crate) fn row(&self, feature: usize) -> &[i16] {
        let n = self.layout.accumulator;
        self.ft_weights
            .get(feature * n..(feature + 1) * n)
            .unwrap_or_default()
    }

    /// Accumulator values of `perspective` for `pieces`, from scratch.
    pub(crate) fn refresh(
        &self,
        out: &mut [i16],
        perspective: Color,
        context: Context,
        pieces: impl Iterator<Item = (chess_core::Piece, chess_core::Square)>,
    ) {
        let mut rows: [&[i16]; 32] = [&[]; 32];
        let mut count = 0;
        for feature in self.encoder.features(context, perspective, pieces) {
            if let Some(row) = rows.get_mut(count) {
                *row = self.row(feature);
                count += 1;
            }
        }
        simd::update(out, &self.ft_bias, &rows[..count], &[]);
    }

    /// Accumulators of `pos` (one per perspective, side by side).
    pub(crate) fn accumulators(&self, pos: &Position) -> Vec<i16> {
        let n = self.layout.accumulator;
        let mut acc = vec![0; n * self.encoder.perspectives().len()];
        for (slot, &perspective) in acc.chunks_exact_mut(n).zip(self.encoder.perspectives()) {
            let context = self.encoder.context_in(pos, perspective);
            self.refresh(slot, perspective, context, pieces(pos));
        }
        acc
    }

    /// Evaluation of `pos` in centipawns for the side to move, from scratch.
    #[must_use]
    pub fn evaluate(&self, pos: &Position) -> Score {
        self.output(&self.accumulators(pos), pos, &RefCell::default())
    }

    /// What each piece of `pos` brings to its evaluation, in centipawns for
    /// the side to move: the evaluation minus the evaluation with that
    /// piece's features removed (kings' buckets and mirroring unchanged).
    #[must_use]
    pub fn piece_contributions(
        &self,
        pos: &Position,
    ) -> Vec<(chess_core::Square, chess_core::Piece, Score)> {
        let n = self.layout.accumulator;
        let acc = self.accumulators(pos);
        let scratch = RefCell::default();
        let full = self.output(&acc, pos, &scratch);
        pieces(pos)
            .map(|(piece, square)| {
                let mut without = acc.clone();
                for (slot, &perspective) in
                    without.chunks_exact_mut(n).zip(self.encoder.perspectives())
                {
                    let context = self.encoder.context_in(pos, perspective);
                    if let Some(feature) = self.encoder.index(context, perspective, piece, square) {
                        let before = slot.to_vec();
                        simd::update(slot, &before, &[], &[self.row(feature)]);
                    }
                }
                (square, piece, full - self.output(&without, pos, &scratch))
            })
            .collect()
    }

    /// The network's output for accumulators `acc` (one slot per
    /// perspective, `Color::index` order) of position `pos`. `scratch` holds
    /// the activations of the generic path.
    pub(crate) fn output(&self, acc: &[i16], pos: &Position, scratch: &RefCell<Vec<i16>>) -> Score {
        let n = self.layout.accumulator;
        let stm = pos.side_to_move();
        let perspective = self.encoder.perspectives().len() == 2;
        let order = if perspective {
            [stm.index(), (!stm).index()]
        } else {
            [0, 0]
        };
        let halves = &order[..self.encoder.perspectives().len()];
        let bucket = self.encoder.output_bucket_in(pos);
        let slot = |p: usize| acc.get(p * n..(p + 1) * n).unwrap_or_default();

        let sum = match (self.output, self.fixed_update) {
            (Output::FusedCReLU, Some(256)) => self.fused_fixed::<256>(acc, stm, bucket),
            (Output::FusedCReLU, Some(512)) => self.fused_fixed::<512>(acc, stm, bucket),
            (Output::FusedCReLU, Some(768)) => self.fused_fixed::<768>(acc, stm, bucket),
            (Output::FusedCReLU, Some(1024)) => self.fused_fixed::<1024>(acc, stm, bucket),
            (Output::FusedCReLU, _) => {
                let layer = &self.dense[0];
                let inputs = self.layout.dense[0].inputs;
                let row = layer
                    .weights
                    .get(bucket * inputs..(bucket + 1) * inputs)
                    .unwrap_or_default();
                halves.iter().enumerate().fold(
                    i64::from(layer.bias.get(bucket).copied().unwrap_or(0)),
                    |sum, (k, &p)| {
                        let weights = row.get(k * n..(k + 1) * n).unwrap_or_default();
                        sum + i64::from(simd::crelu_dot(slot(p), weights, self.qa))
                    },
                )
            }
            (Output::Generic, _) => {
                self.generic_output(halves, &slot, bucket, &mut scratch.borrow_mut())
            }
        };
        let cp = self.output_scale.divide(sum * self.eval_scale) as Score;
        if !perspective && stm == Color::Black {
            -cp
        } else {
            cp
        }
    }

    /// The fused output of a two-perspective network of `N` neurons: both
    /// halves and their weights addressed at compile-time size.
    fn fused_fixed<const N: usize>(&self, acc: &[i16], stm: Color, bucket: usize) -> i64 {
        let slots = acc.as_chunks::<N>().0;
        let layer = &self.dense[0];
        let rows = layer.weights.as_chunks::<N>().0;
        let (Some(us), Some(them), Some(w_us), Some(w_them), Some(&bias)) = (
            slots.get(stm.index()),
            slots.get((!stm).index()),
            rows.get(2 * bucket),
            rows.get(2 * bucket + 1),
            layer.bias.get(bucket),
        ) else {
            return 0;
        };
        i64::from(bias)
            + i64::from(simd::crelu_dot_arrays(us, w_us, self.qa))
            + i64::from(simd::crelu_dot_arrays(them, w_them, self.qa))
    }

    fn generic_output<'a>(
        &self,
        halves: &[usize],
        slot: &dyn Fn(usize) -> &'a [i16],
        bucket: usize,
        scratch: &mut Vec<i16>,
    ) -> i64 {
        let qa = Activations {
            qa: self.qa,
            divisor: self.qa_divisor,
        };
        let ft_outputs = self.layout.ft_outputs;
        let width = self
            .layout
            .dense
            .iter()
            .map(|d| d.inputs.max(d.outputs))
            .max()
            .unwrap_or(0);
        scratch.resize(2 * width, 0);
        let (mut input, mut next) = scratch.split_at_mut(width);

        for (k, &p) in halves.iter().enumerate() {
            let out = &mut input[k * ft_outputs..(k + 1) * ft_outputs];
            qa.accumulator(slot(p), out, self.layout.accumulator_activation);
        }
        for (layer, shape) in self.dense.iter().zip(&self.layout.dense) {
            let b = if shape.buckets > 1 { bucket } else { 0 };
            let inputs = &input[..shape.inputs];
            for (o, out) in next[..shape.outputs].iter_mut().enumerate() {
                let neuron = b * shape.outputs + o;
                let row = layer
                    .weights
                    .get(neuron * shape.inputs..(neuron + 1) * shape.inputs)
                    .unwrap_or_default();
                let dot = match shape.sum {
                    SumWidth::I32 => i64::from(simd::dot(inputs, row)),
                    SumWidth::I64 => inputs
                        .iter()
                        .zip(row)
                        .map(|(&a, &w)| i64::from(a) * i64::from(w))
                        .sum(),
                };
                let sum = dot + i64::from(layer.bias.get(neuron).copied().unwrap_or(0));
                match shape.activation {
                    None => return sum,
                    Some(activation) => *out = qa.apply(self.qb.divide(sum), activation),
                }
            }
            std::mem::swap(&mut input, &mut next);
        }
        0
    }
}

/// Activation functions on the `qa` scale.
#[derive(Clone, Copy)]
struct Activations {
    qa: i16,
    /// Division by `qa`, after squares and products.
    divisor: Divisor,
}

impl Activations {
    /// `value` (on the `qa` scale) through `activation`; the result is on
    /// the `qa` scale too, and fits in an `i16`.
    fn apply(self, value: i64, activation: Activation) -> i16 {
        let clipped = value.clamp(0, i64::from(self.qa));
        (match activation {
            Activation::ReLU => value.clamp(0, i64::from(i16::MAX)),
            Activation::CReLU | Activation::PairwiseCReLU => clipped,
            Activation::SCReLU => self.divisor.divide(clipped * clipped),
        }) as i16
    }

    /// One perspective of the accumulator through `activation`.
    fn accumulator(self, acc: &[i16], out: &mut [i16], activation: Activation) {
        let qa = self.qa;
        match activation {
            Activation::PairwiseCReLU => {
                let (first, second) = acc.split_at(acc.len() / 2);
                for ((o, &a), &b) in out.iter_mut().zip(first).zip(second) {
                    let product = i64::from(a.clamp(0, qa)) * i64::from(b.clamp(0, qa));
                    *o = self.divisor.divide(product) as i16;
                }
            }
            _ => {
                for (o, &a) in out.iter_mut().zip(acc) {
                    *o = self.apply(i64::from(a), activation);
                }
            }
        }
    }
}

fn check_len(what: &str, actual: usize, expected: usize) -> Result<(), NnueError> {
    if actual == expected {
        Ok(())
    } else {
        Err(NnueError::Shape(format!(
            "{what}: {actual} values, the architecture needs {expected}"
        )))
    }
}

fn check_range(what: &str, values: impl Iterator<Item = i64>, bound: i64) -> Result<(), NnueError> {
    match values.enumerate().find(|(_, v)| v.abs() > bound) {
        None => Ok(()),
        Some((index, value)) => Err(NnueError::Range(format!(
            "{what}: value {value} at index {index} exceeds ±{bound}, the bound of the architecture's quantization"
        ))),
    }
}
