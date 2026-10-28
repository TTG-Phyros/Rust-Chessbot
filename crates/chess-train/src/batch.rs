//! Turns dataset files into training batches on a background thread:
//! files are streamed in chunks, each chunk is shuffled, and batches are
//! handed over through a bounded channel (so memory use stays constant
//! whatever the dataset size).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

use chess_core::{Color, Piece, Square};
use chess_dataset::{DatasetReader, Record};
use chess_nnue::Encoder;
use chess_search::StopFlag;

use crate::rng::Rng;

/// One batch, as sparse inputs: the indices of the active features of each
/// position, padded to `max_active`. The dense inputs are rebuilt on the
/// device (see `model::Model::forward`), so the CPU fills and the bus
/// carries a few indices per position instead of every input.
#[derive(Clone)]
pub(crate) struct Batch {
    pub(crate) size: usize,
    /// Features per position, padding included.
    pub(crate) max_active: usize,
    /// `[size, max_active]` features of the side to move (of White for
    /// networks without perspective).
    pub(crate) stm: Vec<i32>,
    /// `[size, max_active]`: 1 for a feature, 0 for padding.
    pub(crate) stm_mask: Vec<f32>,
    /// The same for the opponent; empty without perspective.
    pub(crate) nstm: Vec<i32>,
    pub(crate) nstm_mask: Vec<f32>,
    /// `[size]` output bucket.
    pub(crate) bucket: Vec<i32>,
    /// `[size]`: 1, or −1 when the network's output is White's and Black
    /// is to move.
    pub(crate) sign: Vec<f32>,
    /// `[size]` targets in `[0, 1]`, for the side to move.
    pub(crate) target: Vec<f32>,
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Builds a batch. The target blends the search score (as a win
/// probability) with the game result:
/// `(1 - wdl) · σ(score / score_scale) + wdl · result`.
pub(crate) fn make_batch(
    records: &[Record],
    encoder: &Encoder,
    max_active: usize,
    wdl: f32,
    score_scale: f32,
) -> Batch {
    let size = records.len();
    let perspective = encoder.perspectives().len() == 2;
    let width = size * max_active;
    // Padding points at feature 0 with a mask of 0: it adds nothing.
    let mut batch = Batch {
        size,
        max_active,
        stm: vec![0; width],
        stm_mask: vec![0.0; width],
        nstm: if perspective {
            vec![0; width]
        } else {
            Vec::new()
        },
        nstm_mask: if perspective {
            vec![0.0; width]
        } else {
            Vec::new()
        },
        bucket: Vec::with_capacity(size),
        sign: Vec::with_capacity(size),
        target: Vec::with_capacity(size),
    };
    let mut pieces: Vec<(Piece, Square)> = Vec::with_capacity(32);
    let mut features = Vec::with_capacity(max_active);
    let fill = |indices: &mut [i32], mask: &mut [f32], features: &[usize]| {
        for ((slot, weight), &feature) in indices.iter_mut().zip(mask).zip(features) {
            *slot = feature as i32;
            *weight = 1.0;
        }
    };
    for (row, record) in records.iter().enumerate() {
        pieces.clear();
        pieces.extend(record.pieces());
        let us = record.side_to_move;
        let slots = row * max_active..(row + 1) * max_active;

        features.clear();
        encoder.active(
            if perspective { us } else { Color::White },
            &pieces,
            &mut features,
        );
        fill(
            &mut batch.stm[slots.clone()],
            &mut batch.stm_mask[slots.clone()],
            &features,
        );
        if perspective {
            features.clear();
            encoder.active(!us, &pieces, &mut features);
            fill(
                &mut batch.nstm[slots.clone()],
                &mut batch.nstm_mask[slots],
                &features,
            );
        }
        batch.bucket.push(encoder.output_bucket(&pieces) as i32);
        batch.sign.push(if perspective || us == Color::White {
            1.0
        } else {
            -1.0
        });
        let score = sigmoid(record.score_for_side_to_move() / score_scale);
        batch
            .target
            .push((1.0 - wdl) * score + wdl * record.outcome.score_for(us));
    }
    batch
}

/// Where the loader gets its files from at the start of every epoch (the
/// pipeline adds files while training runs).
pub(crate) type FileSource = Box<dyn FnMut() -> Vec<PathBuf> + Send>;

pub(crate) struct LoaderConfig {
    pub(crate) batch_size: usize,
    /// Records shuffled together (memory: 32 bytes each).
    pub(crate) shuffle_records: usize,
    pub(crate) wdl: f32,
    pub(crate) score_scale: f32,
    pub(crate) seed: u64,
    pub(crate) encoder: Arc<Encoder>,
    pub(crate) max_active: usize,
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
                // Records not yet in a batch, carried over to the next file
                // and epoch: small files still make full batches.
                let mut pool = Vec::new();
                while !stop.is_stopped() {
                    let mut epoch = files();
                    if epoch.is_empty() {
                        std::thread::sleep(Duration::from_secs(1));
                        continue;
                    }
                    rng.shuffle(&mut epoch);
                    if !produce_epoch(&epoch, &config, &mut rng, &mut pool, &sender, &stop) {
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
    pool: &mut Vec<Record>,
    sender: &SyncSender<Batch>,
    stop: &StopFlag,
) -> bool {
    let batch_size = config.batch_size.max(1);
    let shuffle = config.shuffle_records.max(batch_size);
    for path in files {
        let mut reader = match DatasetReader::open(path) {
            Ok(reader) => reader,
            Err(err) => {
                tracing::warn!(file = %path.display(), error = %err, "skipping unreadable dataset file");
                continue;
            }
        };
        loop {
            if stop.is_stopped() {
                return false;
            }
            match reader.read_chunk(shuffle.saturating_sub(pool.len()).max(batch_size)) {
                Ok(chunk) if chunk.is_empty() => break,
                Ok(chunk) => pool.extend(chunk),
                Err(err) => {
                    tracing::warn!(file = %path.display(), error = %err, "corrupt dataset file");
                    break;
                }
            }
            if pool.len() >= shuffle && !send_batches(pool, config, rng, sender) {
                return false;
            }
        }
    }
    // The end of the epoch: what is left makes as many batches as it can.
    send_batches(pool, config, rng, sender)
}

/// Shuffles `pool` and sends its full batches; the rest stays in `pool`.
fn send_batches(
    pool: &mut Vec<Record>,
    config: &LoaderConfig,
    rng: &mut Rng,
    sender: &SyncSender<Batch>,
) -> bool {
    rng.shuffle(pool);
    let batch_size = config.batch_size.max(1);
    let full = pool.len() / batch_size * batch_size;
    for records in pool[..full].chunks_exact(batch_size) {
        let batch = make_batch(
            records,
            &config.encoder,
            config.max_active,
            config.wdl,
            config.score_scale,
        );
        if sender.send(batch).is_err() {
            return false;
        }
    }
    pool.drain(..full);
    true
}

#[cfg(test)]
mod tests {
    use chess_core::Position;
    use chess_dataset::{DatasetWriter, Outcome};
    use chess_nnue::spec::presets;

    use super::*;
    use crate::test_dir;

    fn encoder(id: &str) -> Encoder {
        Encoder::new(&presets::get(id).unwrap().architecture)
    }

    #[test]
    fn batch_layout_and_targets() {
        let encoder = encoder(presets::LEGACY);
        let white = Record::new(&Position::startpos(), 0, Outcome::WhiteWins);
        let black_pos =
            Position::from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1")
                .unwrap();
        let black = Record::new(&black_pos, 400, Outcome::WhiteWins);
        let batch = make_batch(&[white, black], &encoder, 32, 0.5, 400.0);
        assert_eq!(batch.size, 2);
        assert_eq!(batch.stm.len(), 2 * 32);
        let row = |data: &[i32], row: usize| {
            let mut row = data[row * 32..(row + 1) * 32].to_vec();
            row.sort_unstable();
            row
        };
        let expected = |perspective| {
            let mut features: Vec<i32> =
                chess_nnue::legacy::active_features(&black_pos, perspective)
                    .map(|f| f as i32)
                    .collect();
            features.sort_unstable();
            features
        };
        assert_eq!(row(&batch.stm, 1), expected(Color::Black));
        assert_eq!(row(&batch.nstm, 1), expected(Color::White));
        assert!(batch.stm_mask.iter().all(|&m| m == 1.0));
        assert_eq!(batch.sign, [1.0, 1.0]);
        assert_eq!(batch.bucket, [0, 0]);
        // Start position, white wins: 0.5 · σ(0) + 0.5 · 1.
        assert!((batch.target[0] - 0.75).abs() < 1e-6);
        // Black to move, White +400 and White wins: Black's target is low.
        let expected = 0.5 * sigmoid(-1.0);
        assert!((batch.target[1] - expected).abs() < 1e-6);
        // The start position looks the same from both sides.
        assert_eq!(row(&batch.stm, 0), row(&batch.nstm, 0));

        // Fewer pieces: the rest of the row is masked padding.
        let endgame = Position::from_fen("8/8/4k3/8/8/3K4/4P3/8 w - - 0 1").unwrap();
        let batch = make_batch(
            &[Record::new(&endgame, 0, Outcome::Draw)],
            &encoder,
            32,
            0.0,
            400.0,
        );
        assert_eq!(batch.stm_mask.iter().sum::<f32>(), 3.0);
        assert!(batch.stm_mask[..3].iter().all(|&m| m == 1.0));
        assert!(batch.stm[3..].iter().all(|&f| f == 0));
    }

    #[test]
    fn buckets_signs_and_king_features() {
        // HalfKP has no king features: 30 per side in the start position.
        let kp = chess_nnue::spec::parse(
            r#"{"schemaVersion":1,"metadata":{"name":"kp"},"features":{"set":"HalfKP"},
                "accumulator":{"size":16,"activation":"CReLU"},
                "outputBuckets":{"strategy":"pieceCount","count":8},
                "quantization":{"weights":"i8","qa":255,"qb":64,"evalScale":400}}"#,
            chess_nnue::spec::Format::Json,
        )
        .unwrap();
        let encoder = Encoder::new(&kp);
        let start = Record::new(&Position::startpos(), 0, Outcome::Draw);
        let batch = make_batch(&[start], &encoder, 30, 0.0, 400.0);
        assert_eq!(batch.stm_mask.iter().sum::<f32>(), 30.0);
        assert_eq!(batch.bucket, [7]);

        // Without perspective: White's features, and a sign for Black to move.
        let white_only = chess_nnue::spec::parse(
            r#"{"schemaVersion":1,"metadata":{"name":"w"},
                "features":{"set":"Chess768","perspective":false},
                "accumulator":{"size":16,"activation":"CReLU"},
                "quantization":{"weights":"i8","qa":255,"qb":64,"evalScale":400}}"#,
            chess_nnue::spec::Format::Json,
        )
        .unwrap();
        let encoder = Encoder::new(&white_only);
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/3QK3 b - - 0 1").unwrap();
        let batch = make_batch(
            &[Record::new(&pos, 900, Outcome::WhiteWins)],
            &encoder,
            32,
            0.0,
            400.0,
        );
        assert!(batch.nstm.is_empty());
        assert_eq!(batch.sign, [-1.0]);
        let mut white: Vec<i32> = chess_nnue::legacy::active_features(&pos, Color::White)
            .map(|f| f as i32)
            .collect();
        white.sort_unstable();
        let mut stm = batch.stm[..3].to_vec();
        stm.sort_unstable();
        assert_eq!(stm, white);
    }

    #[test]
    fn loader_streams_full_batches_until_dropped() {
        let dir = test_dir("loader");
        // Files smaller than a batch: leftovers carry over.
        let files: Vec<PathBuf> = (0..3)
            .map(|i| {
                let path = dir.join(format!("data-{i}.bin"));
                let mut writer = DatasetWriter::create(&path).unwrap();
                let record = Record::new(&Position::startpos(), 10, Outcome::Draw);
                for _ in 0..5 {
                    writer.write(&record).unwrap();
                }
                writer.finish().unwrap();
                path
            })
            .collect();
        let config = LoaderConfig {
            batch_size: 8,
            shuffle_records: 10,
            wdl: 0.0,
            score_scale: 400.0,
            seed: 1,
            encoder: Arc::new(encoder("fast-768")),
            max_active: 32,
        };
        let loader = BatchLoader::spawn(config, Box::new(move || files.clone()), StopFlag::new());
        for _ in 0..7 {
            let batch = loader.next().unwrap();
            assert_eq!(batch.size, 8);
            assert_eq!(batch.target.len(), 8);
        }
        drop(loader);
    }
}
