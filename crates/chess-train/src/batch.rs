//! Turns dataset files into training batches on a background thread:
//! files are streamed in chunks, each chunk is shuffled, and batches are
//! handed over through a bounded channel (so memory use stays constant
//! whatever the dataset size).

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

use chess_eval::nnue::{INPUTS, SCALE};
use chess_search::StopFlag;

use crate::dataset::{DatasetReader, Record};
use crate::rng::Rng;

/// One batch, as dense inputs ready to become tensors.
pub(crate) struct Batch {
    pub(crate) size: usize,
    /// `[size, INPUTS]` features from the side to move's perspective.
    pub(crate) stm: Vec<f32>,
    /// `[size, INPUTS]` features from the opponent's perspective.
    pub(crate) nstm: Vec<f32>,
    /// `[size]` targets in `[0, 1]`.
    pub(crate) target: Vec<f32>,
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Builds a batch. The target blends the search score (as a win
/// probability) with the game result: `(1 - wdl) · σ(score / SCALE) + wdl · result`.
pub(crate) fn make_batch(records: &[Record], wdl: f32) -> Batch {
    let size = records.len();
    let mut stm = vec![0.0; size * INPUTS];
    let mut nstm = vec![0.0; size * INPUTS];
    let mut target = Vec::with_capacity(size);
    for (row, record) in records.iter().enumerate() {
        let us = record.side_to_move;
        for feature in record.features(us) {
            stm[row * INPUTS + feature] = 1.0;
        }
        for feature in record.features(!us) {
            nstm[row * INPUTS + feature] = 1.0;
        }
        let score = sigmoid(record.score_for_side_to_move() / SCALE as f32);
        target.push((1.0 - wdl) * score + wdl * record.outcome.score_for(us));
    }
    Batch {
        size,
        stm,
        nstm,
        target,
    }
}

/// Where the loader gets its files from at the start of every epoch (the
/// pipeline adds files while training runs).
pub(crate) type FileSource = Box<dyn FnMut() -> Vec<PathBuf> + Send>;

pub(crate) struct LoaderConfig {
    pub(crate) batch_size: usize,
    /// Records shuffled together (memory: 32 bytes each).
    pub(crate) shuffle_records: usize,
    pub(crate) wdl: f32,
    pub(crate) seed: u64,
}

/// Background batch producer.
pub(crate) struct BatchLoader {
    receiver: Receiver<Batch>,
    handle: Option<JoinHandle<()>>,
}

impl BatchLoader {
    /// Starts producing batches, epoch after epoch, until `stop` is raised
    /// or the loader is dropped. With no file available it waits and retries.
    pub(crate) fn spawn(config: LoaderConfig, mut files: FileSource, stop: StopFlag) -> Self {
        // A few batches in flight hide the CPU work behind the GPU's.
        let (sender, receiver) = sync_channel(4);
        let handle = std::thread::Builder::new()
            .name("batch-loader".into())
            .spawn(move || {
                let mut rng = Rng::new(config.seed);
                while !stop.is_stopped() {
                    let mut epoch = files();
                    if epoch.is_empty() {
                        std::thread::sleep(Duration::from_secs(1));
                        continue;
                    }
                    rng.shuffle(&mut epoch);
                    if !produce_epoch(&epoch, &config, &mut rng, &sender, &stop) {
                        return;
                    }
                }
            })
            .ok();
        Self { receiver, handle }
    }

    /// Next batch; `None` once the loader has stopped.
    pub(crate) fn next(&self) -> Option<Batch> {
        self.receiver.recv().ok()
    }
}

impl Drop for BatchLoader {
    fn drop(&mut self) {
        // Dropping the receiver makes the producer's next send fail and exit.
        let (_, empty) = sync_channel(0);
        drop(std::mem::replace(&mut self.receiver, empty));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Streams one epoch. Returns `false` when the consumer is gone.
fn produce_epoch(
    files: &[PathBuf],
    config: &LoaderConfig,
    rng: &mut Rng,
    sender: &SyncSender<Batch>,
    stop: &StopFlag,
) -> bool {
    let batch_size = config.batch_size.max(1);
    for path in files {
        let mut reader = match DatasetReader::open(path) {
            Ok(reader) => reader,
            Err(err) => {
                tracing::warn!(error = %err, "skipping unreadable dataset file");
                continue;
            }
        };
        loop {
            if stop.is_stopped() {
                return false;
            }
            let mut chunk = match reader.read_chunk(config.shuffle_records.max(batch_size)) {
                Ok(chunk) if chunk.is_empty() => break,
                Ok(chunk) => chunk,
                Err(err) => {
                    tracing::warn!(file = %path.display(), error = %err, "corrupt dataset file");
                    break;
                }
            };
            rng.shuffle(&mut chunk);
            // Incomplete trailing batches are dropped: shapes stay constant.
            for records in chunk.chunks_exact(batch_size) {
                if sender.send(make_batch(records, config.wdl)).is_err() {
                    return false;
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use chess_core::Position;

    use super::*;
    use crate::dataset::{DatasetWriter, Outcome};
    use crate::test_dir;

    #[test]
    fn batch_layout_and_targets() {
        let white = Record::new(&Position::startpos(), 0, Outcome::WhiteWins);
        let black_pos =
            Position::from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1")
                .unwrap();
        let black = Record::new(&black_pos, 400, Outcome::WhiteWins);
        let batch = make_batch(&[white, black], 0.5);
        assert_eq!(batch.size, 2);
        let row_sum =
            |data: &[f32], row: usize| data[row * INPUTS..(row + 1) * INPUTS].iter().sum::<f32>();
        assert!((row_sum(&batch.stm, 0) - 32.0).abs() < f32::EPSILON);
        assert!((row_sum(&batch.nstm, 1) - 32.0).abs() < f32::EPSILON);
        // Start position, white wins: 0.5 · σ(0) + 0.5 · 1.
        assert!((batch.target[0] - 0.75).abs() < 1e-6);
        // Black to move, White +400 and White wins: Black's target is low.
        let expected = 0.5 * sigmoid(-1.0);
        assert!((batch.target[1] - expected).abs() < 1e-6);
        // The start position looks the same from both sides.
        assert_eq!(batch.stm[..INPUTS], batch.nstm[..INPUTS]);
    }

    #[test]
    fn loader_streams_full_batches_until_dropped() {
        let dir = test_dir("loader");
        let path = dir.join("data.bin");
        let mut writer = DatasetWriter::create(&path).unwrap();
        let record = Record::new(&Position::startpos(), 10, Outcome::Draw);
        for _ in 0..25 {
            writer.write(&record).unwrap();
        }
        writer.finish().unwrap();
        let config = LoaderConfig {
            batch_size: 8,
            shuffle_records: 10,
            wdl: 0.0,
            seed: 1,
        };
        let loader = BatchLoader::spawn(
            config,
            Box::new(move || vec![path.clone()]),
            StopFlag::new(),
        );
        // Chunks of 10 records give one batch of 8 each; epochs repeat.
        for _ in 0..7 {
            let batch = loader.next().unwrap();
            assert_eq!(batch.size, 8);
            assert_eq!(batch.target.len(), 8);
        }
        drop(loader);
    }
}
