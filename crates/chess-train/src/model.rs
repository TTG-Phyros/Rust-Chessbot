//! The trainable network of any architecture (burn), its float weights
//! (checkpoint format) and their quantization into a `chess_nnue::Network`.
//!
//! The float network computes exactly what the engine computes, divided by
//! the quantization scales: the accumulator and activations are on the
//! scale 1 instead of `qa`, a dense layer's output on the scale 1 instead
//! of `qa · qb`, and the output `y` gives `y · evalScale` centipawns.

use anyhow::{Context, Result, ensure};
use burn::module::{Param, ParamId};
use burn::prelude::*;
use burn::tensor::IndexingUpdateOp;
#[cfg(test)]
use chess_core::{Color, Piece, Position, Square};
#[cfg(test)]
use chess_nnue::Encoder;
use chess_nnue::spec::{Activation, Architecture, I32_DOT_LIMIT, Layout};
use chess_nnue::{DenseWeights, Network};

use crate::rng::Rng;

/// Shapes and bounds the float network follows.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    pub(crate) layout: Layout,
    /// One-hot matrix product instead of a gather: the accumulator either
    /// multiplies `[batch, inputs]` one-hot rows by the feature
    /// transformer, or gathers `[batch, max_active, accumulator]` rows and
    /// sums them. Both are materialized (with their gradients): the
    /// smaller one is used — one-hot for every preset, the gather only for
    /// many king buckets with a small accumulator.
    dense_inputs: bool,
    /// Bounds of the weights, so that they quantize without overflow.
    ft_clip: f32,
    weight_clip: f32,
    bias_clip: f32,
    /// Saturation of ReLU (32767 on the `qa` scale).
    relu_max: f32,
}

impl Plan {
    pub(crate) fn new(architecture: &Architecture) -> Self {
        let arch = architecture.normalized();
        let layout = arch.layout();
        let quant = &arch.quantization;
        let (qa, qb) = (f64::from(quant.qa), f64::from(quant.qb));
        Self {
            dense_inputs: layout.inputs <= layout.max_active * layout.accumulator,
            ft_clip: quant.ft_weight_clip.unwrap_or(0.0) as f32,
            weight_clip: quant.weight_clip.unwrap_or(0.0) as f32,
            bias_clip: (I32_DOT_LIMIT as f64 / (qa * qb)) as f32,
            relu_max: (f64::from(i16::MAX) / qa) as f32,
            layout,
        }
    }

    fn activate(&self, x: Tensor<2>, activation: Activation) -> Tensor<2> {
        match activation {
            Activation::ReLU => x.clamp(0.0, self.relu_max),
            Activation::CReLU => x.clamp(0.0, 1.0),
            Activation::SCReLU => x.clamp(0.0, 1.0).powi_scalar(2),
            Activation::PairwiseCReLU => {
                let [_, n] = x.dims();
                let first = x.clone().narrow(1, 0, n / 2).clamp(0.0, 1.0);
                let second = x.narrow(1, n / 2, n / 2).clamp(0.0, 1.0);
                first * second
            }
        }
    }
}

/// One dense layer: `buckets × outputs` neurons of `inputs` weights, laid
/// out as in the network file (one row per neuron).
#[derive(Module, Debug)]
pub(crate) struct Dense {
    weight: Param<Tensor<2>>,
    bias: Param<Tensor<1>>,
}

#[derive(Module, Debug)]
pub(crate) struct Model {
    /// Feature transformer, shared by the perspectives: `[inputs, accumulator]`.
    ft_weight: Param<Tensor<2>>,
    ft_bias: Param<Tensor<1>>,
    /// Hidden layers, then the output layer.
    layers: Vec<Dense>,
    #[module(skip)]
    plan: Plan,
}

/// The inputs of a batch, on the device.
pub(crate) struct Inputs {
    /// `[batch, max_active]` features of the side to move (of White
    /// without perspective), padded with feature 0.
    pub(crate) stm: Tensor<2, Int>,
    /// `[batch, max_active]`: 1 for a feature, 0 for padding.
    pub(crate) stm_mask: Tensor<2>,
    /// The opponent's, with perspective.
    pub(crate) nstm: Option<(Tensor<2, Int>, Tensor<2>)>,
    /// `[batch, 1]` output bucket.
    pub(crate) bucket: Tensor<2, Int>,
}

impl Model {
    /// A network of `architecture` with weights drawn from `seed`: uniform
    /// in ±1/√fan-in (fan-in of the feature transformer: one king bucket's
    /// features), the usual initialization of linear layers.
    pub(crate) fn new(architecture: &Architecture, seed: u64, device: &Device) -> Self {
        let plan = Plan::new(architecture);
        let layout = &plan.layout;
        let mut rng = Rng::new(seed);
        let mut uniform = |len: usize, fan_in: usize| -> Vec<f32> {
            let bound = 1.0 / (fan_in.max(1) as f32).sqrt();
            (0..len).map(|_| rng.uniform(bound)).collect()
        };
        let ft_fan_in = layout.planes * 64;
        let mut ids = 0_u64..;
        let mut next_id = || ids.next().unwrap_or_default();
        let ft_weight = param(
            next_id(),
            matrix(
                uniform(layout.inputs * layout.accumulator, ft_fan_in),
                [layout.inputs, layout.accumulator],
                device,
            ),
        );
        let ft_bias = param(
            next_id(),
            vector(uniform(layout.accumulator, ft_fan_in), device),
        );
        let layers = layout
            .dense
            .iter()
            .map(|shape| {
                let neurons = shape.buckets * shape.outputs;
                Dense {
                    weight: param(
                        next_id(),
                        matrix(
                            uniform(neurons * shape.inputs, shape.inputs),
                            [neurons, shape.inputs],
                            device,
                        ),
                    ),
                    bias: param(next_id(), vector(uniform(neurons, shape.inputs), device)),
                }
            })
            .collect();
        Self {
            ft_weight,
            ft_bias,
            layers,
            plan,
        }
        .clip()
    }

    pub(crate) fn plan(&self) -> &Plan {
        &self.plan
    }

    /// Accumulator of one perspective, before its activation: `[batch, accumulator]`.
    fn accumulate(&self, features: Tensor<2, Int>, mask: Tensor<2>) -> Tensor<2> {
        let layout = &self.plan.layout;
        let [rows, active] = features.dims();
        let bias = self.ft_bias.val().unsqueeze_dim::<2>(0);
        if self.plan.dense_inputs {
            let one_hot = Tensor::<2>::zeros([rows, layout.inputs], &features.device()).scatter(
                1,
                features,
                mask,
                IndexingUpdateOp::Add,
            );
            one_hot.matmul(self.ft_weight.val()) + bias
        } else {
            // Gathered rows, masked and summed; the backward pass scatters
            // the gradients into the active rows only.
            let rows_of = self
                .ft_weight
                .val()
                .select(0, features.reshape([rows * active]))
                .reshape([rows, active, layout.accumulator]);
            let masked = rows_of * mask.reshape([rows, active, 1]);
            masked.sum_dim(1).reshape([rows, layout.accumulator]) + bias
        }
    }

    /// `[batch, 1]`: the output of the network for each position, on the
    /// scale where `y · evalScale` is centipawns (side to move with
    /// perspective, White without).
    pub(crate) fn forward(&self, inputs: Inputs) -> Tensor<2> {
        let plan = &self.plan;
        let layout = &plan.layout;
        let activation = layout.accumulator_activation;
        let us = plan.activate(self.accumulate(inputs.stm, inputs.stm_mask), activation);
        let mut x = match inputs.nstm {
            Some((features, mask)) => {
                let them = plan.activate(self.accumulate(features, mask), activation);
                Tensor::cat(vec![us, them], 1)
            }
            None => us,
        };
        let [rows, _] = x.dims();
        for (layer, shape) in self.layers.iter().zip(&layout.dense) {
            let mut y =
                x.matmul(layer.weight.val().transpose()) + layer.bias.val().unsqueeze_dim::<2>(0);
            if shape.buckets > 1 {
                let index = inputs
                    .bucket
                    .clone()
                    .reshape([rows, 1, 1])
                    .repeat_dim(2, shape.outputs);
                y = y
                    .reshape([rows, shape.buckets, shape.outputs])
                    .gather(1, index)
                    .reshape([rows, shape.outputs]);
            }
            x = match shape.activation {
                Some(activation) => plan.activate(y, activation),
                None => y,
            };
        }
        x
    }

    /// Keeps weights within the quantizable range (called after each
    /// step): accumulators can never overflow `i16`, dense weights fit
    /// their integer type, biases the `i32` sums.
    pub(crate) fn clip(mut self) -> Self {
        fn clamp<const D: usize>(param: &Param<Tensor<D>>, limit: f32) -> Param<Tensor<D>> {
            self::param(param.id.val(), param.val().clamp(-limit, limit).detach())
        }
        let plan = &self.plan;
        self.ft_weight = clamp(&self.ft_weight, plan.ft_clip);
        self.ft_bias = clamp(&self.ft_bias, plan.ft_clip);
        for layer in &mut self.layers {
            layer.weight = clamp(&layer.weight, plan.weight_clip);
            layer.bias = clamp(&layer.bias, plan.bias_clip);
        }
        self
    }

    pub(crate) fn to_float(&self, architecture: &Architecture) -> Result<FloatWeights> {
        fn values<const D: usize>(param: &Param<Tensor<D>>) -> Result<(u64, Vec<f32>)> {
            let data = param.val().into_data().try_to_vec::<f32>();
            Ok((
                param.id.val(),
                data.map_err(|err| anyhow::anyhow!("{err:?}"))?,
            ))
        }
        let mut tensors = vec![values(&self.ft_weight)?, values(&self.ft_bias)?];
        for layer in &self.layers {
            tensors.push(values(&layer.weight)?);
            tensors.push(values(&layer.bias)?);
        }
        Ok(FloatWeights {
            architecture: architecture.normalized(),
            ids: tensors.iter().map(|(id, _)| *id).collect(),
            tensors: tensors.into_iter().map(|(_, values)| values).collect(),
        })
    }

    /// Rebuilds the network, keeping the parameter ids so that a saved
    /// optimizer state still matches its parameters.
    pub(crate) fn from_float(weights: &FloatWeights, device: &Device) -> Result<Self> {
        weights.check()?;
        let plan = Plan::new(&weights.architecture);
        let layout = &plan.layout;
        let tensor = |i: usize| (weights.ids[i], weights.tensors[i].clone());
        let (id, values) = tensor(0);
        let ft_weight = param(
            id,
            matrix(values, [layout.inputs, layout.accumulator], device),
        );
        let (id, values) = tensor(1);
        let ft_bias = param(id, vector(values, device));
        let layers = layout
            .dense
            .iter()
            .enumerate()
            .map(|(i, shape)| {
                let neurons = shape.buckets * shape.outputs;
                let (weight_id, weight) = tensor(2 + 2 * i);
                let (bias_id, bias) = tensor(3 + 2 * i);
                Dense {
                    weight: param(weight_id, matrix(weight, [neurons, shape.inputs], device)),
                    bias: param(bias_id, vector(bias, device)),
                }
            })
            .collect();
        Ok(Self {
            ft_weight,
            ft_bias,
            layers,
            plan,
        })
    }
}

fn matrix(values: Vec<f32>, shape: [usize; 2], device: &Device) -> Tensor<2> {
    Tensor::from_data(TensorData::new(values, shape), device)
}

fn vector(values: Vec<f32>, device: &Device) -> Tensor<1> {
    let len = values.len();
    Tensor::from_data(TensorData::new(values, [len]), device)
}

fn param<const D: usize>(id: u64, tensor: Tensor<D>) -> Param<Tensor<D>> {
    let mut param = Param::from_tensor(tensor);
    param.id = ParamId::from(id);
    param
}

/// Float weights: the checkpoint format, and the source of quantization.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FloatWeights {
    /// Normalized architecture.
    pub(crate) architecture: Architecture,
    /// Parameter id of each tensor.
    pub(crate) ids: Vec<u64>,
    /// Feature transformer weights (`inputs × accumulator`, feature-major)
    /// and biases, then the weights (one row per neuron) and biases of
    /// each dense layer: the order and layout of the network file.
    pub(crate) tensors: Vec<Vec<f32>>,
}

const FLOAT_MAGIC: &[u8; 4] = b"CBFW";
/// Version 2: any architecture. Version 1 (the legacy network, without its
/// architecture) is still read.
const FLOAT_VERSION: u32 = 2;

impl FloatWeights {
    /// Expected tensor sizes of the architecture.
    fn sizes(layout: &Layout) -> Vec<usize> {
        let mut sizes = vec![layout.inputs * layout.accumulator, layout.accumulator];
        for shape in &layout.dense {
            sizes.push(shape.buckets * shape.outputs * shape.inputs);
            sizes.push(shape.buckets * shape.outputs);
        }
        sizes
    }

    fn check(&self) -> Result<()> {
        let sizes = Self::sizes(&self.architecture.layout());
        let actual: Vec<usize> = self.tensors.iter().map(Vec::len).collect();
        ensure!(
            actual == sizes && self.ids.len() == sizes.len(),
            "float weights do not fit the architecture: tensors of {actual:?} values, expected {sizes:?}"
        );
        Ok(())
    }

    /// `CBFW` version 2: magic, version (`u32`), architecture length
    /// (`u32`) and JSON, tensor count (`u32`), then per tensor its
    /// parameter id (`u64`), length (`u64`) and values (`f32`), all
    /// little-endian.
    pub(crate) fn to_bytes(&self) -> Result<Vec<u8>> {
        let spec = serde_json::to_vec(&self.architecture)?;
        let values: usize = self.tensors.iter().map(Vec::len).sum();
        let mut bytes = Vec::with_capacity(16 + spec.len() + 16 * self.tensors.len() + 4 * values);
        bytes.extend_from_slice(FLOAT_MAGIC);
        bytes.extend_from_slice(&FLOAT_VERSION.to_le_bytes());
        bytes.extend_from_slice(&(spec.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&spec);
        bytes.extend_from_slice(&(self.tensors.len() as u32).to_le_bytes());
        for (id, tensor) in self.ids.iter().zip(&self.tensors) {
            bytes.extend_from_slice(&id.to_le_bytes());
            bytes.extend_from_slice(&(tensor.len() as u64).to_le_bytes());
            for value in tensor {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        Ok(bytes)
    }

    pub(crate) fn from_bytes(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.get(..4) == Some(FLOAT_MAGIC.as_slice()),
            "not a float weights file"
        );
        let mut reader = Reader { bytes, at: 4 };
        let weights = match reader.u32()? {
            1 => Self::from_v1(&mut reader)?,
            2 => {
                let len = reader.u32()? as usize;
                let architecture: Architecture = serde_json::from_slice(reader.take(len)?)
                    .context("float weights: invalid architecture")?;
                let count = reader.u32()? as usize;
                ensure!(count <= 64, "float weights: {count} tensors");
                let mut ids = Vec::with_capacity(count);
                let mut tensors = Vec::with_capacity(count);
                for _ in 0..count {
                    ids.push(reader.u64()?);
                    let len = usize::try_from(reader.u64()?)?;
                    tensors.push(reader.f32s(len)?);
                }
                Self {
                    architecture: architecture.normalized(),
                    ids,
                    tensors,
                }
            }
            version => anyhow::bail!("unsupported float weights version {version}"),
        };
        ensure!(reader.at == bytes.len(), "float weights: trailing bytes");
        weights.check()?;
        Ok(weights)
    }

    /// Version 1: the legacy network — 4 parameter ids, then the feature
    /// transformer weights and biases, the output weights and its bias.
    fn from_v1(reader: &mut Reader<'_>) -> Result<Self> {
        let architecture = chess_nnue::legacy_architecture()?;
        let ids = (0..4).map(|_| reader.u64()).collect::<Result<Vec<_>>>()?;
        let tensors = Self::sizes(&architecture.layout())
            .into_iter()
            .map(|len| reader.f32s(len))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            architecture: architecture.normalized(),
            ids,
            tensors,
        })
    }

    /// The quantized network (see the module documentation of
    /// `chess_nnue::network` for the integer semantics).
    pub(crate) fn quantize(&self) -> Result<Network> {
        let layout = self.architecture.layout();
        let quant = &self.architecture.quantization;
        let (qa, qb) = (f64::from(quant.qa), f64::from(quant.qb));
        let round = |values: &[f32], scale: f64, bound: i64| -> Vec<i64> {
            values
                .iter()
                .map(|&v| ((f64::from(v) * scale).round() as i64).clamp(-bound, bound))
                .collect()
        };
        let i16s = |values: Vec<i64>| values.into_iter().map(|v| v as i16).collect();
        let ft_weights = i16s(round(&self.tensors[0], qa, layout.ft_weight_max));
        let ft_bias = i16s(round(&self.tensors[1], qa, layout.ft_weight_max));
        let dense = (0..layout.dense.len())
            .map(|i| DenseWeights {
                weights: i16s(round(&self.tensors[2 + 2 * i], qb, layout.weight_max)),
                bias: round(&self.tensors[3 + 2 * i], qa * qb, I32_DOT_LIMIT)
                    .into_iter()
                    .map(|v| v as i32)
                    .collect(),
            })
            .collect();
        Ok(Network::from_parts(
            &self.architecture,
            ft_weights,
            ft_bias,
            dense,
        )?)
    }

    /// Float evaluation in centipawns for the side to move: the reference
    /// the quantized network approximates.
    #[cfg(test)]
    pub(crate) fn evaluate(&self, pos: &Position) -> f32 {
        let layout = self.architecture.layout();
        let plan = FloatPlan::new(&self.architecture);
        let encoder = Encoder::new(&self.architecture);
        let pieces: Vec<(Piece, Square)> = pos
            .occupied()
            .filter_map(|square| pos.piece_at(square).map(|piece| (piece, square)))
            .collect();
        let n = layout.accumulator;
        let stm = pos.side_to_move();
        let order: Vec<Color> = if layout.perspectives == 2 {
            vec![stm, !stm]
        } else {
            vec![Color::White]
        };
        let mut x = Vec::new();
        for perspective in order {
            let mut acc = self.tensors[1].clone();
            let mut features = Vec::new();
            encoder.active(perspective, &pieces, &mut features);
            for feature in features {
                for (a, w) in acc.iter_mut().zip(&self.tensors[0][feature * n..]) {
                    *a += w;
                }
            }
            x.extend(plan.activate(&acc, layout.accumulator_activation));
        }
        let bucket = encoder.output_bucket(&pieces);
        for (i, shape) in layout.dense.iter().enumerate() {
            let b = if shape.buckets > 1 { bucket } else { 0 };
            let (weights, bias) = (&self.tensors[2 + 2 * i], &self.tensors[3 + 2 * i]);
            let y: Vec<f32> = (0..shape.outputs)
                .map(|o| {
                    let neuron = b * shape.outputs + o;
                    let row = &weights[neuron * shape.inputs..(neuron + 1) * shape.inputs];
                    bias[neuron] + row.iter().zip(&x).map(|(w, v)| w * v).sum::<f32>()
                })
                .collect();
            x = match shape.activation {
                Some(activation) => plan.activate(&y, activation),
                None => y,
            };
        }
        let cp = x[0] * self.architecture.quantization.eval_scale as f32;
        if layout.perspectives == 1 && stm == Color::Black {
            -cp
        } else {
            cp
        }
    }
}

/// The activations of [`Plan`], on plain vectors (float reference).
#[cfg(test)]
struct FloatPlan {
    relu_max: f32,
}

#[cfg(test)]
impl FloatPlan {
    fn new(architecture: &Architecture) -> Self {
        Self {
            relu_max: Plan::new(architecture).relu_max,
        }
    }

    fn activate(&self, x: &[f32], activation: Activation) -> Vec<f32> {
        match activation {
            Activation::ReLU => x.iter().map(|v| v.clamp(0.0, self.relu_max)).collect(),
            Activation::CReLU => x.iter().map(|v| v.clamp(0.0, 1.0)).collect(),
            Activation::SCReLU => x.iter().map(|v| v.clamp(0.0, 1.0).powi(2)).collect(),
            Activation::PairwiseCReLU => {
                let (first, second) = x.split_at(x.len() / 2);
                first
                    .iter()
                    .zip(second)
                    .map(|(a, b)| a.clamp(0.0, 1.0) * b.clamp(0.0, 1.0))
                    .collect()
            }
        }
    }
}

/// Little-endian reader of a byte slice.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(len)
            .context("float weights: overflow")?;
        let slice = self
            .bytes
            .get(self.at..end)
            .context("truncated float weights")?;
        self.at = end;
        Ok(slice)
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into()?))
    }

    fn f32s(&mut self, len: usize) -> Result<Vec<f32>> {
        let bytes = self.take(len.checked_mul(4).context("float weights: overflow")?)?;
        Ok(bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use chess_dataset::{Outcome, Record};
    use chess_nnue::spec::presets;

    use super::*;
    use crate::batch::{Batch, make_batch};

    fn cpu() -> Device {
        Device::flex()
    }

    /// Small variants of every kind of architecture: the presets' feature
    /// sets, activations, hidden layers and buckets, at sizes the CPU
    /// backend trains quickly.
    fn architectures() -> Vec<Architecture> {
        let mut archs: Vec<Architecture> = ["fast-768", "halfka-medium", "multilayer"]
            .iter()
            .map(|id| presets::get(id).unwrap().architecture.clone())
            .collect();
        for arch in &mut archs {
            arch.accumulator.size = 64;
        }
        let kp = chess_nnue::spec::parse(
            r#"{"schemaVersion":1,"metadata":{"name":"kp"},
                "features":{"set":"HalfKP","perspective":true},
                "accumulator":{"size":32,"activation":"ReLU"},
                "hiddenLayers":[{"size":8,"activation":"SCReLU"}],
                "outputBuckets":{"strategy":"pieceCount","count":4,"scope":"all"},
                "quantization":{"weights":"i16","qa":181,"qb":64,"evalScale":400}}"#,
            chess_nnue::spec::Format::Json,
        )
        .unwrap();
        let white_only = chess_nnue::spec::parse(
            r#"{"schemaVersion":1,"metadata":{"name":"white"},
                "features":{"set":"Chess768","perspective":false},
                "accumulator":{"size":32,"activation":"CReLU"},
                "quantization":{"weights":"i8","qa":255,"qb":64,"evalScale":400}}"#,
            chess_nnue::spec::Format::Json,
        )
        .unwrap();
        archs.push(kp);
        archs.push(white_only);
        archs.push(chess_nnue::legacy_architecture().unwrap());
        archs
    }

    const FENS: [&str; 5] = [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 b - - 0 10",
        "6k1/5ppp/8/8/8/8/1q3PPP/3R2K1 b - - 0 1",
    ];

    fn records() -> Vec<Record> {
        FENS.iter()
            .map(|fen| Record::new(&Position::from_fen(fen).unwrap(), 0, Outcome::Draw))
            .collect()
    }

    fn inputs(batch: &Batch, device: &Device) -> Inputs {
        crate::trainer::tensors(batch.clone(), device).0
    }

    fn outputs(model: &Model, batch: &Batch) -> Vec<f32> {
        model
            .forward(inputs(batch, &cpu()))
            .into_data()
            .try_to_vec::<f32>()
            .unwrap()
    }

    #[test]
    fn forward_matches_the_float_reference_for_every_architecture() {
        for arch in architectures() {
            let model = Model::new(&arch, 11, &cpu());
            let weights = model.to_float(&arch).unwrap();
            let encoder = Encoder::new(&arch);
            let batch = make_batch(
                &records(),
                &encoder,
                model.plan().layout.max_active,
                0.0,
                400.0,
            );
            let outputs = outputs(&model, &batch);
            let scale = arch.quantization.eval_scale as f32;
            for ((fen, y), sign) in FENS.iter().zip(&outputs).zip(&batch.sign) {
                let pos = Position::from_fen(fen).unwrap();
                let reference = weights.evaluate(&pos);
                let float = y * sign * scale;
                assert!(
                    (float - reference).abs() < 1e-3 * scale,
                    "{}: {fen}: {float} vs {reference}",
                    arch.metadata.name
                );
            }

            // Gathered rows and one-hot products agree.
            let mut other = Model::from_float(&weights, &cpu()).unwrap();
            other.plan.dense_inputs = !other.plan.dense_inputs;
            for (a, b) in outputs.iter().zip(super::tests::outputs(&other, &batch)) {
                assert!((a - b).abs() < 1e-5, "{}: {a} vs {b}", arch.metadata.name);
            }
        }
    }

    #[test]
    fn quantized_network_follows_the_float_one() {
        for arch in architectures() {
            let model = Model::new(&arch, 5, &cpu());
            let weights = model.to_float(&arch).unwrap();
            let network = weights.quantize().unwrap();
            assert_eq!(network.hash(), arch.hash());
            let network = Network::from_bytes(&network.to_bytes()).unwrap();
            let scale = arch.quantization.eval_scale as f32;
            for fen in FENS {
                let pos = Position::from_fen(fen).unwrap();
                let float = weights.evaluate(&pos);
                let quantized = network.evaluate(&pos) as f32;
                // Rounding errors of a few per cent of the output scale at
                // most (they mostly cancel out).
                assert!(
                    (float - quantized).abs() <= 0.03 * scale + 2.0,
                    "{}: {fen}: {float} vs {quantized}",
                    arch.metadata.name
                );
            }
        }
    }

    #[test]
    fn float_weights_round_trip_with_ids() {
        for arch in architectures() {
            let model = Model::new(&arch, 1, &cpu());
            let weights = model.to_float(&arch).unwrap();
            let decoded = FloatWeights::from_bytes(&weights.to_bytes().unwrap()).unwrap();
            assert_eq!(decoded, weights);
            let rebuilt = Model::from_float(&decoded, &cpu()).unwrap();
            assert_eq!(rebuilt.to_float(&arch).unwrap(), weights);
            assert_eq!(rebuilt.ft_weight.id, model.ft_weight.id);
        }
        assert!(FloatWeights::from_bytes(b"junk").is_err());
    }

    #[test]
    fn reads_version_1_checkpoints() {
        let arch = chess_nnue::legacy_architecture().unwrap();
        let weights = Model::new(&arch, 2, &cpu()).to_float(&arch).unwrap();
        let mut v1 = Vec::new();
        v1.extend_from_slice(b"CBFW");
        v1.extend_from_slice(&1_u32.to_le_bytes());
        for id in &weights.ids {
            v1.extend_from_slice(&id.to_le_bytes());
        }
        for value in weights.tensors.iter().flatten() {
            v1.extend_from_slice(&value.to_le_bytes());
        }
        assert_eq!(FloatWeights::from_bytes(&v1).unwrap(), weights);
        assert!(FloatWeights::from_bytes(&v1[..v1.len() - 4]).is_err());
    }

    #[test]
    fn initialization_is_reproducible_and_within_bounds() {
        let arch = presets::get("multilayer").unwrap().architecture.clone();
        let a = Model::new(&arch, 9, &cpu()).to_float(&arch).unwrap();
        let b = Model::new(&arch, 9, &cpu()).to_float(&arch).unwrap();
        let c = Model::new(&arch, 10, &cpu()).to_float(&arch).unwrap();
        assert_eq!(a, b);
        assert_ne!(a.tensors, c.tensors);
        // Quantizable as drawn.
        a.quantize().unwrap();
    }

    #[test]
    fn clipping_bounds_the_weights() {
        let arch = presets::get("fast-768").unwrap().architecture.clone();
        let mut weights = Model::new(&arch, 1, &cpu()).to_float(&arch).unwrap();
        for tensor in &mut weights.tensors {
            tensor.iter_mut().for_each(|w| *w *= 1000.0);
        }
        let clipped = Model::from_float(&weights, &cpu())
            .unwrap()
            .clip()
            .to_float(&arch)
            .unwrap();
        let plan = Plan::new(&arch);
        let max = |i: usize| {
            clipped.tensors[i]
                .iter()
                .fold(0.0_f32, |m, w| m.max(w.abs()))
        };
        assert!(max(0) <= plan.ft_clip + 1e-6);
        assert!(max(2) <= plan.weight_clip + 1e-6);
        assert_eq!(clipped.ids, weights.ids);
        clipped.quantize().unwrap();
    }
}
