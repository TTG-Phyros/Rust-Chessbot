//! The trainable network (burn) and its conversions to checkpoint and
//! quantized formats. The architecture is the one of `chess_eval::nnue`.

use anyhow::{Context, Result, ensure};
use burn::module::{Param, ParamId};
use burn::nn::{Linear, LinearConfig};
use burn::prelude::*;
use burn::tensor::IndexingUpdateOp;
use chess_eval::nnue::{HIDDEN, INPUTS, NnueWeights, QA, QB};

/// Largest output weight representable after quantization (`i8`-safe, as
/// most NNUE inference code stores them in 8 bits).
pub(crate) const MAX_OUTPUT_WEIGHT: f32 = 127.0 / QB as f32;

/// Bound on feature transformer weights and biases: 33 · 3.0 · QA < 32 767.
pub(crate) const MAX_FT_VALUE: f32 = 3.0;

#[derive(Module, Debug)]
pub(crate) struct Network {
    /// Feature transformer, shared by both perspectives: `[INPUTS, HIDDEN]`.
    ft: Linear,
    /// Output layer: `[2 × HIDDEN, 1]`.
    out: Linear,
}

impl Network {
    pub(crate) fn new(device: &Device) -> Self {
        Self {
            ft: LinearConfig::new(INPUTS, HIDDEN).init(device),
            out: LinearConfig::new(2 * HIDDEN, 1).init(device),
        }
    }

    /// `[batch, INPUTS]` × 2 perspectives → `[batch, 1]`, in units of
    /// `SCALE` centipawns (so that `sigmoid(output)` is a win probability).
    pub(crate) fn forward(&self, stm: Tensor<2>, nstm: Tensor<2>) -> Tensor<2> {
        let us = self.ft.forward(stm).clamp(0.0, 1.0);
        let them = self.ft.forward(nstm).clamp(0.0, 1.0);
        self.out.forward(Tensor::cat(vec![us, them], 1))
    }

    /// Keeps weights within the quantizable range (called after each step):
    /// output weights fit 8 bits, and feature transformer values can never
    /// overflow the `i16` accumulators of `chess-eval` (32 pieces + bias).
    pub(crate) fn clip(mut self) -> Self {
        let clamp = |param: &Param<Tensor<2>>, limit: f32| {
            let clipped = param.val().clamp(-limit, limit).detach();
            self::param(param.id.val(), clipped)
        };
        self.out.weight = clamp(&self.out.weight, MAX_OUTPUT_WEIGHT);
        self.ft.weight = clamp(&self.ft.weight, MAX_FT_VALUE);
        if let Some(bias) = &self.ft.bias {
            let clipped = bias.val().clamp(-MAX_FT_VALUE, MAX_FT_VALUE).detach();
            self.ft.bias = Some(param(bias.id.val(), clipped));
        }
        self
    }

    pub(crate) fn to_float(&self) -> Result<FloatWeights> {
        let values = |tensor: Tensor<2>| tensor.into_data().try_to_vec::<f32>();
        let bias = |linear: &Linear| -> Result<(u64, Vec<f32>)> {
            let bias = linear.bias.as_ref().context("missing bias")?;
            let data = bias.val().into_data().try_to_vec::<f32>();
            Ok((bias.id.val(), data.map_err(|e| anyhow::anyhow!("{e:?}"))?))
        };
        let (ft_bias_id, ft_bias) = bias(&self.ft)?;
        let (out_bias_id, out_bias) = bias(&self.out)?;
        Ok(FloatWeights {
            ids: [
                self.ft.weight.id.val(),
                ft_bias_id,
                self.out.weight.id.val(),
                out_bias_id,
            ],
            ft_weights: values(self.ft.weight.val()).map_err(|e| anyhow::anyhow!("{e:?}"))?,
            ft_bias,
            out_weights: values(self.out.weight.val()).map_err(|e| anyhow::anyhow!("{e:?}"))?,
            out_bias: out_bias.first().copied().context("empty output bias")?,
        })
    }

    /// Rebuilds the network, keeping the parameter ids so that a saved
    /// optimizer state still matches its parameters.
    pub(crate) fn from_float(weights: &FloatWeights, device: &Device) -> Self {
        let matrix = |data: &[f32], rows: usize, cols: usize| {
            Tensor::<2>::from_data(TensorData::new(data.to_vec(), [rows, cols]), device)
        };
        let vector = |data: &[f32]| {
            Tensor::<1>::from_data(TensorData::new(data.to_vec(), [data.len()]), device)
        };
        let [ft_w, ft_b, out_w, out_b] = weights.ids;
        let mut network = Self::new(device);
        network.ft.weight = param(ft_w, matrix(&weights.ft_weights, INPUTS, HIDDEN));
        network.ft.bias = Some(param(ft_b, vector(&weights.ft_bias)));
        network.out.weight = param(out_w, matrix(&weights.out_weights, 2 * HIDDEN, 1));
        network.out.bias = Some(param(out_b, vector(&[weights.out_bias])));
        network
    }
}

/// Dense one-hot inputs `[batch, INPUTS]` from the sparse ones of a
/// [`Batch`](crate::batch::Batch): `indices` and `mask` are `[batch, MAX_FEATURES]`.
/// Built on the inputs' device, where it costs a fill and a scatter.
pub(crate) fn one_hot(indices: Tensor<2, Int>, mask: Tensor<2>) -> Tensor<2> {
    let [rows, _] = indices.dims();
    Tensor::<2>::zeros([rows, INPUTS], &indices.device()).scatter(
        1,
        indices,
        mask,
        IndexingUpdateOp::Add,
    )
}

fn param<const D: usize>(id: u64, tensor: Tensor<D>) -> Param<Tensor<D>> {
    let mut param = Param::from_tensor(tensor);
    param.id = ParamId::from(id);
    param
}

/// Float weights: the checkpoint format, and the source of quantization.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FloatWeights {
    /// Parameter ids of ft weight, ft bias, out weight, out bias.
    pub(crate) ids: [u64; 4],
    /// `INPUTS × HIDDEN`, feature-major (same layout as `NnueWeights`).
    pub(crate) ft_weights: Vec<f32>,
    pub(crate) ft_bias: Vec<f32>,
    pub(crate) out_weights: Vec<f32>,
    pub(crate) out_bias: f32,
}

const FLOAT_MAGIC: &[u8; 4] = b"CBFW";
const FLOAT_VERSION: u32 = 1;

impl FloatWeights {
    /// `CBFW` file: magic, version (`u32`), the 4 parameter ids (`u64`),
    /// then every value as little-endian `f32`, in field order.
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8 + 32 + 4 * (INPUTS * HIDDEN + 3 * HIDDEN + 1));
        bytes.extend_from_slice(FLOAT_MAGIC);
        bytes.extend_from_slice(&FLOAT_VERSION.to_le_bytes());
        for id in self.ids {
            bytes.extend_from_slice(&id.to_le_bytes());
        }
        let values = self
            .ft_weights
            .iter()
            .chain(&self.ft_bias)
            .chain(&self.out_weights);
        for value in values.chain(std::iter::once(&self.out_bias)) {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    pub(crate) fn from_bytes(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.get(..4) == Some(FLOAT_MAGIC.as_slice()),
            "not a float weights file"
        );
        let expected = 8 + 32 + 4 * (INPUTS * HIDDEN + 3 * HIDDEN + 1);
        ensure!(
            bytes.len() == expected,
            "float weights: {} bytes, expected {expected}",
            bytes.len()
        );
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        ensure!(
            version == FLOAT_VERSION,
            "unsupported float weights version {version}"
        );
        let mut ids = [0_u64; 4];
        for (i, id) in ids.iter_mut().enumerate() {
            let start = 8 + i * 8;
            *id = u64::from_le_bytes(bytes[start..start + 8].try_into()?);
        }
        let mut values = bytes[40..]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b));
        let ft_weights = values.by_ref().take(INPUTS * HIDDEN).collect();
        let ft_bias = values.by_ref().take(HIDDEN).collect();
        let out_weights = values.by_ref().take(2 * HIDDEN).collect();
        let out_bias = values.next().context("truncated float weights")?;
        Ok(Self {
            ids,
            ft_weights,
            ft_bias,
            out_weights,
            out_bias,
        })
    }

    /// Quantizes for `chess-eval` (see `chess_eval::nnue` for the scheme).
    pub(crate) fn quantize(&self) -> NnueWeights {
        let round = |x: f32, scale: i32| -> i16 {
            (x * scale as f32)
                .round()
                .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
        };
        NnueWeights {
            ft_weights: self.ft_weights.iter().map(|&w| round(w, QA)).collect(),
            ft_bias: self.ft_bias.iter().map(|&b| round(b, QA)).collect(),
            out_weights: self.out_weights.iter().map(|&w| round(w, QB)).collect(),
            out_bias: (self.out_bias * (QA * QB) as f32).round() as i32,
        }
    }

    /// Float evaluation in centipawns for the side to move (reference for
    /// checking the quantization error).
    #[cfg(test)]
    pub(crate) fn evaluate(&self, pos: &chess_core::Position) -> f32 {
        use chess_eval::nnue::active_features;
        let us = pos.side_to_move();
        let mut output = self.out_bias;
        for (half, perspective) in [us, !us].into_iter().enumerate() {
            let mut acc = self.ft_bias.clone();
            for feature in active_features(pos, perspective) {
                for (a, w) in acc.iter_mut().zip(&self.ft_weights[feature * HIDDEN..]) {
                    *a += w;
                }
            }
            for (a, w) in acc.iter().zip(&self.out_weights[half * HIDDEN..]) {
                output += a.clamp(0.0, 1.0) * w;
            }
        }
        output * chess_eval::nnue::SCALE as f32
    }
}

#[cfg(test)]
mod tests {
    use chess_core::Position;

    use super::*;
    use crate::batch::MAX_FEATURES;

    fn cpu() -> Device {
        Device::flex()
    }

    #[test]
    fn float_weights_round_trip_with_ids() {
        let network = Network::new(&cpu());
        let weights = network.to_float().unwrap();
        assert_eq!(weights.ft_weights.len(), INPUTS * HIDDEN);
        let decoded = FloatWeights::from_bytes(&weights.to_bytes()).unwrap();
        assert_eq!(decoded, weights);
        let rebuilt = Network::from_float(&decoded, &cpu());
        assert_eq!(rebuilt.to_float().unwrap(), weights);
        assert_eq!(rebuilt.ft.weight.id, network.ft.weight.id);
        assert!(FloatWeights::from_bytes(b"junk").is_err());
    }

    #[test]
    fn forward_matches_the_float_reference() {
        let device = cpu();
        let network = Network::new(&device);
        let weights = network.to_float().unwrap();
        let pos = Position::from_fen(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 0 1",
        )
        .unwrap();
        let dense = |perspective| {
            let mut input = vec![0.0_f32; INPUTS];
            for f in chess_eval::nnue::active_features(&pos, perspective) {
                input[f] = 1.0;
            }
            Tensor::<2>::from_data(TensorData::new(input, [1, INPUTS]), &device)
        };
        // The sparse batch path builds the same inputs.
        let record = crate::dataset::Record::new(&pos, 0, crate::dataset::Outcome::Draw);
        let batch = crate::batch::make_batch(&[record], 0.0);
        let sparse = one_hot(
            Tensor::from_data(TensorData::new(batch.stm, [1, MAX_FEATURES]), &device),
            Tensor::from_data(TensorData::new(batch.mask, [1, MAX_FEATURES]), &device),
        );
        assert_eq!(
            sparse.into_data().try_to_vec::<f32>().unwrap(),
            dense(pos.side_to_move())
                .into_data()
                .try_to_vec::<f32>()
                .unwrap()
        );
        let us = pos.side_to_move();
        let output = network
            .forward(dense(us), dense(!us))
            .into_data()
            .try_to_vec::<f32>()
            .unwrap();
        let reference = weights.evaluate(&pos) / chess_eval::nnue::SCALE as f32;
        assert!(
            (output[0] - reference).abs() < 1e-4,
            "{} vs {reference}",
            output[0]
        );
    }

    #[test]
    fn quantized_eval_stays_within_the_rounding_bound() {
        // Deterministic weights in the ranges of a trained network.
        let mut rng = crate::rng::Rng::new(3);
        let mut uniform = |scale: f32| (rng.below(20_001) as f32 / 10_000.0 - 1.0) * scale;
        let weights = FloatWeights {
            ids: [1, 2, 3, 4],
            ft_weights: (0..INPUTS * HIDDEN).map(|_| uniform(0.3)).collect(),
            ft_bias: (0..HIDDEN).map(|_| uniform(0.3)).collect(),
            out_weights: (0..2 * HIDDEN).map(|_| uniform(1.5)).collect(),
            out_bias: uniform(0.2),
        };
        let quantized = NnueWeights::from_bytes(&weights.quantize().to_bytes()).unwrap();

        for fen in [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 b - - 0 10",
        ] {
            let pos = Position::from_fen(fen).unwrap();
            // Worst case of the rounding errors: each output weight is off by
            // at most 0.5/QB, each activation by (features + bias) · 0.5/QA.
            let us = pos.side_to_move();
            let mut bound = 0.5 / (QA * QB) as f32;
            for (half, perspective) in [us, !us].into_iter().enumerate() {
                let features: Vec<usize> =
                    chess_eval::nnue::active_features(&pos, perspective).collect();
                let activation_error = (features.len() + 1) as f32 * 0.5 / QA as f32;
                for h in 0..HIDDEN {
                    let acc = weights.ft_bias[h]
                        + features
                            .iter()
                            .map(|&f| weights.ft_weights[f * HIDDEN + h])
                            .sum::<f32>();
                    let w = weights.out_weights[half * HIDDEN + h];
                    bound += acc.clamp(0.0, 1.0) * 0.5 / QB as f32 + w.abs() * activation_error;
                }
            }
            let bound = bound * chess_eval::nnue::SCALE as f32 + 1.0;
            let float = weights.evaluate(&pos);
            let quant = quantized.evaluate(&pos) as f32;
            assert!(
                (float - quant).abs() <= bound,
                "{fen}: {float} vs {quant} (bound {bound})"
            );
            // And in practice the errors largely cancel out.
            assert!(
                (float - quant).abs() <= bound / 4.0,
                "{fen}: {float} vs {quant}"
            );
        }
    }

    #[test]
    fn clipping_bounds_output_weights() {
        let mut weights = Network::new(&cpu()).to_float().unwrap();
        weights.out_weights.iter_mut().for_each(|w| *w *= 1000.0);
        let clipped = Network::from_float(&weights, &cpu())
            .clip()
            .to_float()
            .unwrap();
        assert!(
            clipped
                .out_weights
                .iter()
                .all(|w| w.abs() <= MAX_OUTPUT_WEIGHT + 1e-6)
        );
        assert_eq!(clipped.ids, weights.ids);
    }
}
