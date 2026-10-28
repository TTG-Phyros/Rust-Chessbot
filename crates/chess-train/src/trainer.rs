//! NNUE training: the [`Trainer`] trait every backend implements, and the
//! burn backend — AdamW on a sigmoid loss, superbatch checkpoints that can
//! be resumed exactly (weights with their parameter ids, optimizer
//! moments, progress), quantized export in the `CBNN` version 2 format.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use burn::optim::{AdamWConfig, GradientsParams, ModuleOptimizer, OptimizerRecord};
use burn::prelude::*;
use burn::tensor::activation::sigmoid;
use chess_nnue::Encoder;
use chess_nnue::spec::{Architecture, Format, presets};
use chess_search::StopFlag;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::batch::{Batch, BatchLoader, FileSource, LoaderConfig};
use crate::device::{SelectedDevice, vram_used_mb};
use crate::metrics::Metrics;
use crate::model::{FloatWeights, Inputs, Model};

/// Training parameters (`[train]` section of the configuration).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct TrainConfig {
    /// Architecture: a preset id or a JSON/YAML file (default: the
    /// `legacy-768x256` preset).
    pub(crate) architecture: Option<String>,
    /// Positions per optimizer step. At 16 384, the one-hot inputs rebuilt
    /// on the device take about 100 MB: comfortable on an 8 GB GPU.
    pub(crate) batch_size: usize,
    /// Steps between two checkpoints/reports.
    pub(crate) batches_per_superbatch: usize,
    /// Superbatches to train; 0 = until stopped.
    pub(crate) superbatches: u64,
    pub(crate) learning_rate: f64,
    /// The learning rate is multiplied by `lr_drop_factor` every `lr_drop_every` superbatches.
    pub(crate) lr_drop_every: u64,
    pub(crate) lr_drop_factor: f64,
    /// Weight of the game result in the target (the rest is the search score).
    pub(crate) wdl: f32,
    /// Centipawns per unit of the sigmoid of the target and the loss.
    pub(crate) score_scale: f32,
    /// Records shuffled together by the loader (32 bytes each).
    pub(crate) shuffle_records: usize,
    /// Export quantized weights every this many superbatches (0: only at the end).
    pub(crate) export_every: u64,
    /// Seed of the initialization and of the data order.
    pub(crate) seed: u64,
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self {
            architecture: None,
            batch_size: 16_384,
            batches_per_superbatch: 200,
            superbatches: 0,
            learning_rate: 1e-3,
            lr_drop_every: 40,
            lr_drop_factor: 0.3,
            wdl: 0.25,
            score_scale: 400.0,
            shuffle_records: 2_000_000,
            export_every: 5,
            seed: 0x7A11,
        }
    }
}

impl TrainConfig {
    fn learning_rate_at(&self, superbatch: u64) -> f64 {
        let drops = superbatch / self.lr_drop_every.max(1);
        self.learning_rate * self.lr_drop_factor.powi(drops.min(64) as i32)
    }

    /// The architecture to train.
    pub(crate) fn architecture(&self) -> Result<Architecture> {
        let Some(name) = &self.architecture else {
            return Ok(chess_nnue::legacy_architecture()?);
        };
        if let Some(preset) = presets::get(name) {
            return Ok(preset.architecture.clone());
        }
        let text = std::fs::read_to_string(name)
            .with_context(|| format!("{name} is neither a preset nor a readable file"))?;
        let format = if name.ends_with(".yaml") || name.ends_with(".yml") {
            Format::Yaml
        } else {
            Format::Json
        };
        chess_nnue::spec::parse(&text, format).map_err(|errors| {
            let errors: Vec<String> = errors.iter().map(ToString::to_string).collect();
            anyhow::anyhow!("invalid architecture {name}: {}", errors.join("; "))
        })
    }
}

/// Progress saved with every checkpoint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct TrainState {
    pub(crate) superbatch: u64,
    pub(crate) step: u64,
    pub(crate) positions: u64,
}

/// What to train, on what, and where to keep checkpoints.
pub(crate) struct TrainingRun {
    pub(crate) architecture: Architecture,
    pub(crate) config: TrainConfig,
    /// Dataset files, asked again at every epoch.
    pub(crate) files: FileSource,
    /// Checkpoint directory of the run.
    pub(crate) checkpoint: PathBuf,
    /// Continue from the checkpoint, if there is one.
    pub(crate) resume: bool,
    pub(crate) stop: StopFlag,
}

/// How a training ended.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TrainingOutcome {
    pub(crate) state: TrainState,
    /// The network (`CBNN` version 2), if anything was trained.
    pub(crate) network: Option<Vec<u8>>,
    /// Loss of the last superbatch.
    pub(crate) loss: Option<f64>,
    /// Whether the planned superbatches were all trained (otherwise it was
    /// stopped, or ran out of data).
    pub(crate) finished: bool,
}

/// One superbatch, as reported.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SuperbatchReport {
    pub(crate) state: TrainState,
    /// Planned superbatches (0: until stopped).
    pub(crate) superbatches: u64,
    pub(crate) loss: f64,
    pub(crate) learning_rate: f64,
    pub(crate) positions_per_second: f64,
    pub(crate) vram_used_mb: Option<u64>,
}

/// Follows a training: where its reports go.
pub(crate) trait Observer {
    /// Training starts, from the checkpoint's state when resuming.
    fn started(&self, _resumed: Option<TrainState>) {}
    fn superbatch(&self, _report: &SuperbatchReport) {}
    fn checkpoint(&self, _state: TrainState) {}
    /// A quantized network: every `export_every` superbatches, and at the end.
    fn export(&self, _network: &[u8], _superbatch: u64) -> Result<()> {
        Ok(())
    }
}

/// A training backend.
pub(crate) trait Trainer {
    /// Name training jobs ask for.
    fn id(&self) -> &'static str;

    /// Trains `run` until its superbatches are done, its data ends or it is
    /// stopped (a checkpoint is then saved).
    fn train(&self, run: TrainingRun, observer: &dyn Observer) -> Result<TrainingOutcome>;
}

/// The built-in backend: burn, on the GPU (CUDA) or the CPU.
pub(crate) struct BurnTrainer<'a> {
    pub(crate) device: &'a SelectedDevice,
}

const WEIGHTS_FILE: &str = "weights.cbfw";
const OPTIMIZER_FILE: &str = "optimizer.bpk";
const STATE_FILE: &str = "state.json";

/// The tensors of a batch: network inputs, signs and targets (`[batch, 1]`).
pub(crate) fn tensors(batch: Batch, device: &Device) -> (Inputs, Tensor<2>, Tensor<2>) {
    let rows = batch.size;
    let shape = [rows, batch.max_active];
    let indices =
        |data: Vec<i32>| Tensor::<2, Int>::from_data(TensorData::new(data, shape), device);
    let mask = |data: Vec<f32>| Tensor::<2>::from_data(TensorData::new(data, shape), device);
    let column = |data: Vec<f32>| Tensor::<2>::from_data(TensorData::new(data, [rows, 1]), device);
    let nstm = (!batch.nstm.is_empty()).then(|| (indices(batch.nstm), mask(batch.nstm_mask)));
    let inputs = Inputs {
        stm: indices(batch.stm),
        stm_mask: mask(batch.stm_mask),
        nstm,
        bucket: Tensor::<2, Int>::from_data(TensorData::new(batch.bucket, [rows, 1]), device),
    };
    (inputs, column(batch.sign), column(batch.target))
}

impl Trainer for BurnTrainer<'_> {
    fn id(&self) -> &'static str {
        chess_jobs::BURN
    }

    fn train(&self, run: TrainingRun, observer: &dyn Observer) -> Result<TrainingOutcome> {
        let device = self.device.device.clone().autodiff();
        let architecture = run.architecture.normalized();
        let config = &run.config;
        let loaded = if run.resume {
            load_checkpoint(&run.checkpoint, &architecture, &device)
        } else {
            None
        };
        let resumed = loaded.as_ref().map(|(_, _, state)| *state);
        let (mut model, mut optimizer, mut state) = loaded.unwrap_or_else(|| {
            (
                Model::new(&architecture, config.seed, &device),
                AdamWConfig::new().init(),
                TrainState::default(),
            )
        });
        if let Some(state) = resumed {
            tracing::info!(superbatch = state.superbatch, "resuming from checkpoint");
        }
        observer.started(resumed);

        let max_active = model.plan().layout.max_active;
        let loader = BatchLoader::spawn(
            LoaderConfig {
                batch_size: config.batch_size,
                shuffle_records: config.shuffle_records,
                wdl: config.wdl,
                score_scale: config.score_scale,
                seed: config.seed ^ state.step,
                encoder: Arc::new(Encoder::new(&architecture)),
                max_active,
            },
            run.files,
            run.stop.clone(),
        );
        // The output times evalScale is in centipawns; the sigmoid wants them
        // in units of score_scale.
        let output_scale = architecture.quantization.eval_scale as f32 / config.score_scale;
        tracing::info!(
            device = %self.device.info,
            batch = config.batch_size,
            architecture = %architecture.metadata.name,
            "training started"
        );

        let mut last_loss = None;
        'training: while !run.stop.is_stopped()
            && (config.superbatches == 0 || state.superbatch < config.superbatches)
        {
            let lr = config.learning_rate_at(state.superbatch);
            let started = Instant::now();
            let (mut loss_sum, mut loss_samples, mut positions) = (0.0_f64, 0_u32, 0_u64);

            for index in 0..config.batches_per_superbatch.max(1) {
                let Some(batch) = loader.next() else {
                    break 'training;
                };
                if run.stop.is_stopped() {
                    break 'training;
                }
                let rows = batch.size;
                let (inputs, sign, target) = tensors(batch, &device);
                let output = model.forward(inputs) * sign;
                let prediction = sigmoid(output.mul_scalar(output_scale));
                let loss = (prediction - target).powi_scalar(2).mean();
                // Reading the loss synchronizes with the GPU: sample it only.
                if index % 16 == 0 {
                    let value = loss.clone().try_into_scalar::<f32>().map_err(|err| {
                        anyhow::anyhow!("training step failed on {}: {err:?}", self.device.info)
                    })?;
                    loss_sum += f64::from(value);
                    loss_samples += 1;
                }
                let grads = GradientsParams::from_grads(loss.backward(), &model);
                model = optimizer.step(lr, model, grads).clip();
                state.step += 1;
                positions += rows as u64;
            }
            if positions == 0 {
                break;
            }

            state.superbatch += 1;
            state.positions += positions;
            let loss = loss_sum / f64::from(loss_samples.max(1));
            last_loss = Some(loss);
            let report = SuperbatchReport {
                state,
                superbatches: config.superbatches,
                loss,
                learning_rate: lr,
                positions_per_second: positions as f64 / started.elapsed().as_secs_f64().max(1e-6),
                vram_used_mb: vram_used_mb(&self.device.info),
            };
            tracing::info!(
                superbatch = state.superbatch,
                loss = format!("{loss:.6}"),
                lr,
                positions_per_sec = report.positions_per_second as u64,
                "superbatch done"
            );
            observer.superbatch(&report);

            save_checkpoint(&run.checkpoint, &model, &optimizer, state, &architecture)?;
            observer.checkpoint(state);
            if config.export_every > 0
                && state.superbatch % config.export_every == 0
                && !is_last(config, state)
            {
                let network = model.to_float(&architecture)?.quantize()?.to_bytes();
                observer.export(&network, state.superbatch)?;
            }
        }

        let network = if state.superbatch > 0 {
            let network = model.to_float(&architecture)?.quantize()?.to_bytes();
            observer.export(&network, state.superbatch)?;
            Some(network)
        } else {
            None
        };
        Ok(TrainingOutcome {
            state,
            network,
            loss: last_loss,
            finished: is_last(config, state),
        })
    }
}

/// Whether `state` is the end of a run with a superbatch limit.
fn is_last(config: &TrainConfig, state: TrainState) -> bool {
    config.superbatches > 0 && state.superbatch >= config.superbatches
}

/// Saves a checkpoint in `dir`, through a temporary directory swapped in
/// at the end, so that a crash never leaves a half-written checkpoint.
fn save_checkpoint(
    dir: &Path,
    model: &Model,
    optimizer: &ModuleOptimizer,
    state: TrainState,
    architecture: &Architecture,
) -> Result<()> {
    let temp_dir = dir.with_extension("tmp");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
    }
    std::fs::create_dir_all(&temp_dir)
        .with_context(|| format!("cannot create {}", temp_dir.display()))?;
    std::fs::write(
        temp_dir.join(WEIGHTS_FILE),
        model.to_float(architecture)?.to_bytes()?,
    )?;
    optimizer
        .to_record()
        .save(temp_dir.join(OPTIMIZER_FILE))
        .map_err(|err| anyhow::anyhow!("cannot save the optimizer state: {err:?}"))?;
    std::fs::write(
        temp_dir.join(STATE_FILE),
        serde_json::to_vec_pretty(&state)?,
    )?;
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    std::fs::rename(&temp_dir, dir)?;
    Ok(())
}

/// Loads the checkpoint of `dir`, if any. A corrupt one, or one of another
/// architecture, is reported and ignored.
fn load_checkpoint(
    dir: &Path,
    architecture: &Architecture,
    device: &Device,
) -> Option<(Model, ModuleOptimizer, TrainState)> {
    if !dir.join(STATE_FILE).exists() {
        return None;
    }
    let load = || -> Result<(Model, ModuleOptimizer, TrainState)> {
        let weights = FloatWeights::from_bytes(&std::fs::read(dir.join(WEIGHTS_FILE))?)?;
        if weights.architecture.hash() != architecture.hash() {
            bail!(
                "the checkpoint is of architecture {} ({}), not {} ({})",
                weights.architecture.metadata.name,
                weights.architecture.hash(),
                architecture.metadata.name,
                architecture.hash()
            );
        }
        let record = OptimizerRecord::load(dir.join(OPTIMIZER_FILE))
            .map_err(|err| anyhow::anyhow!("cannot read the optimizer state: {err:?}"))?;
        let state: TrainState = serde_json::from_slice(&std::fs::read(dir.join(STATE_FILE))?)?;
        let optimizer = AdamWConfig::new().init().load_record(record);
        Ok((Model::from_float(&weights, device)?, optimizer, state))
    };
    match load() {
        Ok(loaded) => Some(loaded),
        Err(err) => {
            tracing::warn!(error = %format!("{err:#}"), dir = %dir.display(), "ignoring checkpoint");
            None
        }
    }
}

/// Exports the checkpoint in `dir` as a quantized network to `out`.
pub(crate) fn export_checkpoint(dir: &Path, out: &Path) -> Result<()> {
    let path = dir.join(WEIGHTS_FILE);
    let bytes =
        std::fs::read(&path).with_context(|| format!("no checkpoint at {}", path.display()))?;
    write_atomically(
        out,
        &FloatWeights::from_bytes(&bytes)?.quantize()?.to_bytes(),
    )
}

/// Writes `bytes` to `path` through a temporary file, so that readers never
/// see a half-written file.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // `x.nnue.tmp`, not `x.tmp`: `x.nnue` and `x.json` may be written together.
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);
    std::fs::write(&temp, bytes).with_context(|| format!("cannot write {}", temp.display()))?;
    std::fs::rename(&temp, path).with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

/// The observer of the `train` and `pipeline` commands: metrics files for
/// the training page, `<weights_dir>/<run>.nnue` and numbered snapshots.
pub(crate) struct RunObserver<'a> {
    pub(crate) run: String,
    pub(crate) weights_dir: PathBuf,
    pub(crate) metrics: &'a Metrics,
    /// Receives every export (snapshot file, superbatch), e.g. to measure it.
    pub(crate) exports: Option<std::sync::mpsc::Sender<(PathBuf, u64)>>,
}

impl RunObserver<'_> {
    /// Path of the latest export of the run.
    pub(crate) fn latest_weights_path(&self) -> PathBuf {
        self.weights_dir.join(format!("{}.nnue", self.run))
    }
}

impl Observer for RunObserver<'_> {
    fn started(&self, resumed: Option<TrainState>) {
        match resumed {
            Some(state) => self
                .metrics
                .update(|status| status.superbatch = state.superbatch),
            None => {
                // Superbatches restart from 1: the old snapshots would be overwritten.
                let snapshots = self.weights_dir.join(&self.run);
                if let Err(err) = crate::ranking::reset(&snapshots) {
                    tracing::warn!(error = %err, "cannot clear the previous snapshots");
                }
            }
        }
    }

    fn superbatch(&self, report: &SuperbatchReport) {
        let state = report.state;
        self.metrics.event(
            "superbatch",
            json!({
                "run": self.run, "superbatch": state.superbatch, "step": state.step,
                "loss": report.loss, "learning_rate": report.learning_rate,
                "positions": state.positions,
                "positions_per_second": report.positions_per_second,
                "vram_used_mb": report.vram_used_mb,
            }),
        );
        self.metrics.update(|status| {
            status.superbatch = state.superbatch;
            status.loss = Some(report.loss);
            status.learning_rate = Some(report.learning_rate);
            status.positions_per_second = Some(report.positions_per_second);
            status.vram_used_mb = report.vram_used_mb;
        });
    }

    fn checkpoint(&self, state: TrainState) {
        self.metrics.event(
            "checkpoint",
            json!({ "run": self.run, "superbatch": state.superbatch }),
        );
    }

    fn export(&self, network: &[u8], superbatch: u64) -> Result<()> {
        let snapshot = self
            .weights_dir
            .join(&self.run)
            .join(crate::ranking::snapshot_name(superbatch));
        write_atomically(&snapshot, network)?;
        let latest = self.latest_weights_path();
        write_atomically(&latest, network)?;
        tracing::info!(file = %latest.display(), superbatch, "weights exported");
        self.metrics.event(
            "export",
            json!({ "run": self.run, "superbatch": superbatch, "file": snapshot }),
        );
        self.metrics
            .update(|status| status.latest_weights = Some(latest.display().to_string()));
        if let Some(exports) = &self.exports {
            // A closed receiver only means nobody measures the exports.
            let _ = exports.send((snapshot, superbatch));
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use chess_core::Position;
    use chess_dataset::{DatasetWriter, Outcome, Record};
    use chess_nnue::Network;

    use super::*;
    use crate::device::{DevicePreference, select};
    use crate::metrics::Status;
    use crate::test_dir;

    /// A small dataset where the score is pure material, so there is
    /// something learnable.
    pub(crate) fn write_dataset(dir: &Path) -> PathBuf {
        let path = dir.join("train.bin");
        let mut writer = DatasetWriter::create(&path).unwrap();
        let fens = [
            ("4k3/8/8/8/8/8/8/3QK3 w - - 0 1", 900, Outcome::WhiteWins),
            ("3qk3/8/8/8/8/8/8/4K3 w - - 0 1", -900, Outcome::BlackWins),
            ("4k3/8/8/8/8/8/8/3RK3 b - - 0 1", 500, Outcome::WhiteWins),
            ("4k3/pppp4/8/8/8/8/PPPP4/4K3 w - - 0 1", 0, Outcome::Draw),
        ];
        for i in 0..256 {
            let (fen, score, outcome) = fens[i % fens.len()];
            writer
                .write(&Record::new(
                    &Position::from_fen(fen).unwrap(),
                    score,
                    outcome,
                ))
                .unwrap();
        }
        writer.finish().unwrap();
        path
    }

    #[test]
    fn learning_rate_schedule() {
        let config = TrainConfig {
            learning_rate: 1.0,
            lr_drop_every: 10,
            lr_drop_factor: 0.5,
            ..TrainConfig::default()
        };
        assert!((config.learning_rate_at(0) - 1.0).abs() < 1e-12);
        assert!((config.learning_rate_at(9) - 1.0).abs() < 1e-12);
        assert!((config.learning_rate_at(25) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn architectures_from_presets_and_files() {
        let dir = test_dir("trainer-arch");
        let config = |architecture: Option<&str>| TrainConfig {
            architecture: architecture.map(str::to_owned),
            ..TrainConfig::default()
        };
        assert_eq!(
            config(None).architecture().unwrap().hash(),
            chess_nnue::legacy_architecture().unwrap().hash()
        );
        let fast = config(Some("fast-768")).architecture().unwrap();
        let file = dir.join("arch.yaml");
        std::fs::write(&file, serde_norway::to_string(&fast).unwrap()).unwrap();
        let from_file = config(file.to_str()).architecture().unwrap();
        assert_eq!(from_file.hash(), fast.hash());
        assert!(config(Some("no-such-preset")).architecture().is_err());
    }

    fn files(dataset: &Path) -> FileSource {
        let dataset = dataset.to_owned();
        Box::new(move || vec![dataset.clone()])
    }

    #[test]
    fn trains_checkpoints_resumes_and_exports() {
        let dir = test_dir("trainer");
        let dataset = write_dataset(&dir);
        let device = select(DevicePreference::Cpu).unwrap();
        let metrics = Metrics::open(&dir, Status::default()).unwrap();
        let config = TrainConfig {
            batch_size: 64,
            batches_per_superbatch: 8,
            superbatches: 3,
            learning_rate: 5e-3,
            shuffle_records: 256,
            export_every: 2,
            ..TrainConfig::default()
        };
        let observer = RunObserver {
            run: "test".into(),
            weights_dir: dir.join("weights"),
            metrics: &metrics,
            exports: None,
        };
        let trainer = BurnTrainer { device: &device };
        let run = |superbatches: u64| TrainingRun {
            architecture: config.architecture().unwrap(),
            config: TrainConfig {
                superbatches,
                ..config.clone()
            },
            files: files(&dataset),
            checkpoint: dir.join("checkpoints/test"),
            resume: true,
            stop: StopFlag::new(),
        };

        let outcome = trainer.train(run(3), &observer).unwrap();
        assert_eq!(
            outcome.state,
            TrainState {
                superbatch: 3,
                step: 24,
                positions: 24 * 64
            }
        );
        assert!(outcome.finished);
        assert!(dir.join("checkpoints/test/state.json").exists());
        assert!(dir.join("weights/test/sb00002.nnue").exists());
        let file = std::fs::read(observer.latest_weights_path()).unwrap();
        assert_eq!(Some(&file), outcome.network.as_ref());
        let net = Network::from_bytes(&file).unwrap();
        assert_eq!(chess_nnue::inspect(&file).unwrap().version, 2);
        // The trained network knows a queen up is good for the side to move.
        let queen_up = Position::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1").unwrap();
        assert!(net.evaluate(&queen_up) > 100, "{}", net.evaluate(&queen_up));

        // Resuming continues from superbatch 3 with the saved optimizer.
        let resumed = trainer.train(run(4), &observer).unwrap();
        assert_eq!(resumed.state.superbatch, 4);
        assert_eq!(resumed.state.step, 32);

        let events = std::fs::read_to_string(dir.join("metrics/events.jsonl")).unwrap();
        assert_eq!(
            events
                .lines()
                .filter(|l| l.contains("\"kind\":\"superbatch\""))
                .count(),
            4
        );

        let out = dir.join("exported.nnue");
        export_checkpoint(&dir.join("checkpoints/test"), &out).unwrap();
        assert!(Network::from_bytes(&std::fs::read(out).unwrap()).is_ok());

        // A checkpoint of another architecture is not resumed.
        let mut other = run(1);
        other.architecture = presets::get("fast-768").unwrap().architecture.clone();
        assert_eq!(trainer.train(other, &observer).unwrap().state.superbatch, 1);
    }

    #[test]
    fn same_seed_same_network() {
        let dir = test_dir("trainer-seed");
        let dataset = write_dataset(&dir);
        let device = select(DevicePreference::Cpu).unwrap();
        let trainer = BurnTrainer { device: &device };
        let train = |seed: u64, name: &str| {
            let run = TrainingRun {
                architecture: presets::get("multilayer").unwrap().architecture.clone(),
                config: TrainConfig {
                    batch_size: 32,
                    batches_per_superbatch: 4,
                    superbatches: 2,
                    shuffle_records: 128,
                    export_every: 0,
                    seed,
                    ..TrainConfig::default()
                },
                files: files(&dataset),
                checkpoint: dir.join(name),
                resume: false,
                stop: StopFlag::new(),
            };
            struct Quiet;
            impl Observer for Quiet {}
            trainer.train(run, &Quiet).unwrap().network.unwrap()
        };
        let a = train(1, "a");
        assert_eq!(a, train(1, "b"));
        assert_ne!(a, train(2, "c"));
    }
}
