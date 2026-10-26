//! NNUE training loop: AdamW on a sigmoid loss, superbatch checkpoints that
//! can be resumed exactly (weights with their parameter ids, optimizer
//! moments, progress), and periodic export of quantized weights.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use burn::optim::{AdamWConfig, GradientsParams, ModuleOptimizer, OptimizerRecord};
use burn::prelude::*;
use burn::tensor::activation::sigmoid;
use chess_eval::nnue::INPUTS;
use chess_search::StopFlag;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::batch::{BatchLoader, FileSource, LoaderConfig};
use crate::device::{SelectedDevice, vram_used_mb};
use crate::metrics::Metrics;
use crate::model::{FloatWeights, Network};

/// Training parameters (`[train]` section of the configuration).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct TrainConfig {
    /// Positions per optimizer step. 16 384 dense positions take about
    /// 100 MB of input per step: comfortable on an 8 GB GPU.
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
    /// Records shuffled together by the loader (32 bytes each).
    pub(crate) shuffle_records: usize,
    /// Export quantized weights every this many superbatches.
    pub(crate) export_every: u64,
    pub(crate) seed: u64,
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self {
            batch_size: 16_384,
            batches_per_superbatch: 200,
            superbatches: 0,
            learning_rate: 1e-3,
            lr_drop_every: 40,
            lr_drop_factor: 0.3,
            wdl: 0.25,
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
}

/// Progress saved with every checkpoint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct TrainState {
    pub(crate) superbatch: u64,
    pub(crate) step: u64,
    pub(crate) positions: u64,
}

/// Everything a training run needs besides its data.
pub(crate) struct Trainer<'a> {
    pub(crate) config: TrainConfig,
    pub(crate) run: String,
    pub(crate) checkpoint_dir: PathBuf,
    pub(crate) weights_dir: PathBuf,
    pub(crate) device: &'a SelectedDevice,
    pub(crate) metrics: &'a Metrics,
    pub(crate) stop: StopFlag,
    /// Receives every quantized export (file, superbatch), e.g. to measure it.
    pub(crate) exports: Option<std::sync::mpsc::Sender<(PathBuf, u64)>>,
}

const WEIGHTS_FILE: &str = "weights.cbfw";
const OPTIMIZER_FILE: &str = "optimizer.bpk";
const STATE_FILE: &str = "state.json";

impl Trainer<'_> {
    fn checkpoint_path(&self) -> PathBuf {
        self.checkpoint_dir.join(&self.run)
    }

    /// Path of the latest quantized export of this run.
    pub(crate) fn latest_weights_path(&self) -> PathBuf {
        self.weights_dir.join(format!("{}.nnue", self.run))
    }

    /// Trains until `config.superbatches`, the end of the data or a stop.
    pub(crate) fn train(&self, files: FileSource, resume: bool) -> Result<TrainState> {
        let device = self.device.device.clone().autodiff();
        let (mut model, mut optimizer, mut state) =
            match resume.then(|| self.load_checkpoint(&device)) {
                Some(Some(loaded)) => {
                    let (_, _, state) = &loaded;
                    tracing::info!(superbatch = state.superbatch, "resuming from checkpoint");
                    let superbatch = state.superbatch;
                    self.metrics.update(|status| status.superbatch = superbatch);
                    loaded
                }
                _ => (
                    Network::new(&device),
                    AdamWConfig::new().init(),
                    TrainState::default(),
                ),
            };

        let config = &self.config;
        let loader = BatchLoader::spawn(
            LoaderConfig {
                batch_size: config.batch_size,
                shuffle_records: config.shuffle_records,
                wdl: config.wdl,
                seed: config.seed ^ state.step,
            },
            files,
            self.stop.clone(),
        );
        tracing::info!(device = %self.device.info, batch = config.batch_size, "training started");

        'training: while !self.stop.is_stopped()
            && (config.superbatches == 0 || state.superbatch < config.superbatches)
        {
            let lr = config.learning_rate_at(state.superbatch);
            let started = Instant::now();
            let (mut loss_sum, mut loss_samples, mut positions) = (0.0_f64, 0_u32, 0_u64);

            for index in 0..config.batches_per_superbatch.max(1) {
                let Some(batch) = loader.next() else {
                    break 'training;
                };
                if self.stop.is_stopped() {
                    break 'training;
                }
                let rows = batch.size;
                let stm =
                    Tensor::<2>::from_data(TensorData::new(batch.stm, [rows, INPUTS]), &device);
                let nstm =
                    Tensor::<2>::from_data(TensorData::new(batch.nstm, [rows, INPUTS]), &device);
                let target =
                    Tensor::<2>::from_data(TensorData::new(batch.target, [rows, 1]), &device);

                let prediction = sigmoid(model.forward(stm, nstm));
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
            let speed = positions as f64 / started.elapsed().as_secs_f64().max(1e-6);
            let vram = vram_used_mb(&self.device.info);
            tracing::info!(
                superbatch = state.superbatch,
                loss = format!("{loss:.6}"),
                lr,
                positions_per_sec = speed as u64,
                "superbatch done"
            );
            self.metrics.event(
                "superbatch",
                json!({
                    "run": self.run, "superbatch": state.superbatch, "step": state.step,
                    "loss": loss, "learning_rate": lr, "positions": state.positions,
                    "positions_per_second": speed, "vram_used_mb": vram,
                }),
            );
            self.metrics.update(|status| {
                status.superbatch = state.superbatch;
                status.loss = Some(loss);
                status.learning_rate = Some(lr);
                status.positions_per_second = Some(speed);
                status.vram_used_mb = vram;
            });

            self.save_checkpoint(&model, &optimizer, state)?;
            if state.superbatch % self.config.export_every.max(1) == 0 {
                self.export(&model, state.superbatch)?;
            }
        }

        if state.superbatch > 0 {
            self.export(&model, state.superbatch)?;
        }
        Ok(state)
    }

    fn save_checkpoint(
        &self,
        model: &Network,
        optimizer: &ModuleOptimizer,
        state: TrainState,
    ) -> Result<()> {
        let final_dir = self.checkpoint_path();
        let temp_dir = final_dir.with_extension("tmp");
        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir)?;
        }
        std::fs::create_dir_all(&temp_dir)
            .with_context(|| format!("cannot create {}", temp_dir.display()))?;
        std::fs::write(temp_dir.join(WEIGHTS_FILE), model.to_float()?.to_bytes())?;
        optimizer
            .to_record()
            .save(temp_dir.join(OPTIMIZER_FILE))
            .map_err(|err| anyhow::anyhow!("cannot save the optimizer state: {err:?}"))?;
        std::fs::write(
            temp_dir.join(STATE_FILE),
            serde_json::to_vec_pretty(&state)?,
        )?;
        // Swap directories so that a crash never leaves a half-written checkpoint.
        if final_dir.exists() {
            std::fs::remove_dir_all(&final_dir)?;
        }
        std::fs::rename(&temp_dir, &final_dir)?;
        self.metrics.event(
            "checkpoint",
            json!({ "run": self.run, "superbatch": state.superbatch }),
        );
        Ok(())
    }

    /// Loads the run's checkpoint, if any (a corrupt one is reported and ignored).
    fn load_checkpoint(&self, device: &Device) -> Option<(Network, ModuleOptimizer, TrainState)> {
        let dir = self.checkpoint_path();
        if !dir.join(STATE_FILE).exists() {
            return None;
        }
        let load = || -> Result<(Network, ModuleOptimizer, TrainState)> {
            let weights = FloatWeights::from_bytes(&std::fs::read(dir.join(WEIGHTS_FILE))?)?;
            let record = OptimizerRecord::load(dir.join(OPTIMIZER_FILE))
                .map_err(|err| anyhow::anyhow!("cannot read the optimizer state: {err:?}"))?;
            let state: TrainState = serde_json::from_slice(&std::fs::read(dir.join(STATE_FILE))?)?;
            let optimizer = AdamWConfig::new().init().load_record(record);
            Ok((Network::from_float(&weights, device), optimizer, state))
        };
        match load() {
            Ok(loaded) => Some(loaded),
            Err(err) => {
                tracing::warn!(error = %err, dir = %dir.display(), "ignoring unreadable checkpoint");
                None
            }
        }
    }

    /// Writes `<weights_dir>/<run>.nnue` and a numbered snapshot.
    fn export(&self, model: &Network, superbatch: u64) -> Result<()> {
        let bytes = model.to_float()?.quantize().to_bytes();
        let snapshot = self
            .weights_dir
            .join(&self.run)
            .join(format!("sb{superbatch:05}.nnue"));
        write_atomically(&snapshot, &bytes)?;
        let latest = self.latest_weights_path();
        write_atomically(&latest, &bytes)?;
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

/// Exports the checkpoint of `run` as quantized weights to `out`.
pub(crate) fn export_checkpoint(checkpoint_dir: &Path, run: &str, out: &Path) -> Result<()> {
    let path = checkpoint_dir.join(run).join(WEIGHTS_FILE);
    let bytes =
        std::fs::read(&path).with_context(|| format!("no checkpoint at {}", path.display()))?;
    write_atomically(
        out,
        &FloatWeights::from_bytes(&bytes)?.quantize().to_bytes(),
    )
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, bytes).with_context(|| format!("cannot write {}", temp.display()))?;
    std::fs::rename(&temp, path).with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use chess_core::Position;
    use chess_eval::nnue::NnueWeights;

    use super::*;
    use crate::dataset::{DatasetWriter, Outcome, Record};
    use crate::device::{DevicePreference, select};
    use crate::metrics::Status;
    use crate::test_dir;

    /// A small dataset where the score is pure material, so there is
    /// something learnable.
    fn write_dataset(dir: &Path) -> PathBuf {
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
        let trainer = Trainer {
            config: config.clone(),
            run: "test".into(),
            checkpoint_dir: dir.join("checkpoints"),
            weights_dir: dir.join("weights"),
            device: &device,
            metrics: &metrics,
            stop: StopFlag::new(),
            exports: None,
        };
        let files = || -> FileSource {
            let dataset = dataset.clone();
            Box::new(move || vec![dataset.clone()])
        };

        let state = trainer.train(files(), true).unwrap();
        assert_eq!(
            state,
            TrainState {
                superbatch: 3,
                step: 24,
                positions: 24 * 64
            }
        );
        assert!(dir.join("checkpoints/test/state.json").exists());
        assert!(dir.join("weights/test/sb00002.nnue").exists());
        let net = NnueWeights::from_bytes(&std::fs::read(trainer.latest_weights_path()).unwrap())
            .unwrap();
        // The trained network knows a queen up is good for the side to move.
        let queen_up = Position::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1").unwrap();
        assert!(net.evaluate(&queen_up) > 100, "{}", net.evaluate(&queen_up));

        // Resuming continues from superbatch 3 with the saved optimizer.
        let more = Trainer {
            config: TrainConfig {
                superbatches: 4,
                ..config
            },
            ..trainer
        };
        let resumed = more.train(files(), true).unwrap();
        assert_eq!(resumed.superbatch, 4);
        assert_eq!(resumed.step, 32);

        let events = std::fs::read_to_string(dir.join("metrics/events.jsonl")).unwrap();
        assert_eq!(
            events
                .lines()
                .filter(|l| l.contains("\"kind\":\"superbatch\""))
                .count(),
            4
        );

        let out = dir.join("exported.nnue");
        export_checkpoint(&dir.join("checkpoints"), "test", &out).unwrap();
        assert!(NnueWeights::from_bytes(&std::fs::read(out).unwrap()).is_ok());
    }
}
