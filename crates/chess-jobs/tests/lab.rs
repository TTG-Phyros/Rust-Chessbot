//! The lab from a client's and from workers' points of view.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use chess_core::Position;
use chess_dataset::{DatasetWriter, Outcome, Record};
use chess_jobs::{
    Broker, DatasetStatus, Device, Directive, EventKind, Hyperparameters, JobKind, JobStatus, Lab,
    LabError, LabPaths, Progress, SelfplayRequest, TrainingProgress, TrainingRequest,
    TrainingSummary, WorkerInfo,
};
use chess_nnue::Network;
use chess_nnue::spec::presets;
use serde_json::{Value, json};

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("chess-jobs-tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn lab(name: &str) -> (Lab, PathBuf) {
    let dir = test_dir(name);
    let lab = Lab::open(&dir.join("lab.sqlite"), LabPaths::under(&dir)).unwrap();
    (lab, dir)
}

/// A small valid dataset file.
fn dataset_bytes(dir: &std::path::Path, positions: usize) -> Vec<u8> {
    let path = dir.join("source.bin");
    let mut writer = DatasetWriter::create(&path).unwrap();
    let pos = Position::startpos();
    for i in 0..positions {
        writer
            .write(&Record::new(&pos, i as i16, Outcome::Draw))
            .unwrap();
    }
    writer.finish().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    bytes
}

fn architecture(id: &str) -> Value {
    serde_json::to_value(&presets::get(id).unwrap().architecture).unwrap()
}

fn training(lab: &Lab, dir: &std::path::Path, device: Device) -> chess_jobs::Job {
    let dataset = lab
        .import_dataset("data", dataset_bytes(dir, 100).as_slice(), 1 << 20)
        .unwrap();
    lab.create_training(&TrainingRequest {
        name: None,
        architecture: architecture("fast-768"),
        datasets: vec![dataset.id],
        hyperparameters: Hyperparameters {
            superbatches: 3,
            batch_size: 64,
            shuffle_records: 64,
            ..Hyperparameters::default()
        },
        seed: Some(7),
        device,
        trainer: "burn".into(),
    })
    .unwrap()
}

fn worker(id: &str, gpu: bool) -> WorkerInfo {
    WorkerInfo {
        id: id.into(),
        gpu,
        kinds: vec![JobKind::Training, JobKind::Selfplay],
        trainers: vec!["burn".into()],
    }
}

#[test]
fn uploads_are_checked_hashed_and_described() {
    let (lab, dir) = lab("uploads");
    let bytes = dataset_bytes(&dir, 50);
    let dataset = lab
        .import_dataset("  openings  ", bytes.as_slice(), 1 << 20)
        .unwrap();
    assert_eq!(dataset.name, "openings");
    assert_eq!(dataset.status, DatasetStatus::Ready);
    assert_eq!(dataset.records, 50);
    assert_eq!(dataset.stats.as_ref().unwrap().pieces[32], 50);
    let files = lab.paths().dataset_files(dataset.id);
    assert_eq!(
        chess_dataset::scan(&files).unwrap().sha256,
        dataset.sha256.clone().unwrap()
    );
    assert_eq!(lab.dataset(dataset.id).unwrap(), dataset);

    // A corrupt upload leaves nothing behind.
    let mut corrupt = bytes.clone();
    corrupt[16 + 26] = 9;
    let err = lab
        .import_dataset("bad", corrupt.as_slice(), 1 << 20)
        .unwrap_err();
    assert!(matches!(err, LabError::Dataset(_)), "{err}");
    let err = lab
        .import_dataset("big", bytes.as_slice(), 100)
        .unwrap_err();
    assert!(err.to_string().contains("larger than 100 bytes"), "{err}");
    assert!(matches!(
        lab.import_dataset("", bytes.as_slice(), 1 << 20),
        Err(LabError::Invalid(_))
    ));
    assert_eq!(lab.datasets(None, 10).unwrap().len(), 1);
    assert_eq!(
        std::fs::read_dir(lab.paths().datasets.clone())
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn training_requests_report_every_problem() {
    let (lab, dir) = lab("validation");
    let (selfplay, generating) = lab
        .create_selfplay(&SelfplayRequest {
            name: None,
            games: 10,
            nodes: 100,
            seed: None,
            network: None,
            threads: 1,
        })
        .unwrap();
    assert_eq!(selfplay.status, JobStatus::Queued);
    let ready = lab
        .import_dataset("ready", dataset_bytes(&dir, 5).as_slice(), 1 << 20)
        .unwrap();

    let mut arch = architecture("fast-768");
    arch["accumulator"]["size"] = json!(100);
    let request: TrainingRequest = serde_json::from_value(json!({
        "architecture": arch,
        "datasets": [ready.id, generating.id, ready.id, "0190a8f0-0000-7000-8000-000000000000"],
        "hyperparameters": { "batchSize": 1 },
        "trainer": "pytorch",
    }))
    .unwrap();
    let Err(LabError::Invalid(errors)) = lab.create_training(&request) else {
        panic!("accepted an invalid request");
    };
    let paths: Vec<&str> = errors.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "/architecture/accumulator/size",
            "/hyperparameters/batchSize",
            "/trainer",
            "/datasets/1",
            "/datasets/2",
            "/datasets/3",
        ],
        "{errors:?}"
    );
    assert!(errors[3].message.contains("generating"));

    // Unknown fields are rejected when the request is read.
    assert!(
        serde_json::from_value::<TrainingRequest>(json!({
            "architecture": {}, "datasets": [], "path": "/etc"
        }))
        .is_err()
    );
}

#[test]
fn batches_must_fit_in_the_datasets() {
    let (lab, dir) = lab("batch-size");
    let dataset = lab
        .import_dataset("tiny", dataset_bytes(&dir, 100).as_slice(), 1 << 20)
        .unwrap();
    let mut request = TrainingRequest {
        name: None,
        architecture: architecture("fast-768"),
        datasets: vec![dataset.id],
        hyperparameters: Hyperparameters::default(),
        seed: None,
        device: Device::Auto,
        trainer: "burn".into(),
    };
    let Err(LabError::Invalid(errors)) = lab.create_training(&request) else {
        panic!("accepted batches larger than the dataset");
    };
    assert_eq!(errors[0].path, "/hyperparameters/batchSize");
    assert!(errors[0].message.contains("100 positions"));
    request.hyperparameters.batch_size = 64;
    request.hyperparameters.shuffle_records = 64;
    let job = lab.create_training(&request).unwrap();
    // A seed was drawn and recorded.
    let chess_jobs::JobSpec::Training(spec) = job.spec else {
        panic!("not a training job");
    };
    assert_ne!(spec.seed, 0);
}

#[test]
fn jobs_record_everything_needed_to_reproduce_them() {
    let (lab, dir) = lab("reproducible");
    let job = training(&lab, &dir, Device::Auto);
    let chess_jobs::JobSpec::Training(spec) = &job.spec else {
        panic!("not a training job");
    };
    let preset = &presets::get("fast-768").unwrap().architecture;
    assert_eq!(spec.architecture, preset.normalized());
    assert_eq!(spec.architecture_hash, preset.hash().to_string());
    assert_eq!(spec.seed, 7);
    assert_eq!(spec.datasets.len(), 1);
    assert_eq!(spec.datasets[0].sha256.len(), 64);
    assert_eq!(job.name, preset.metadata.name);
}

#[test]
fn one_job_at_a_time_on_the_gpu() {
    let (lab, dir) = lab("gpu-slot");
    let first = training(&lab, &dir, Device::Auto);
    let second = training(&lab, &dir, Device::Gpu);
    let third = training(&lab, &dir, Device::Auto);

    let gpu = lab.claim(&worker("gpu-a", true)).unwrap().unwrap();
    assert_eq!((gpu.job.id, gpu.attempt, gpu.gpu), (first.id, 1, true));
    assert_eq!(gpu.job.status, JobStatus::Running);
    // The GPU is taken: another GPU worker gets nothing.
    assert!(lab.claim(&worker("gpu-b", true)).unwrap().is_none());
    // A CPU worker cannot take the GPU-only job, but takes the next one.
    let cpu = lab.claim(&worker("cpu", false)).unwrap().unwrap();
    assert_eq!((cpu.job.id, cpu.gpu), (third.id, false));

    // The GPU is free again once the first job ends.
    lab.fail(gpu.token(), "out of memory").unwrap();
    assert_eq!(
        lab.job(first.id).unwrap().error.as_deref(),
        Some("out of memory")
    );
    let next = lab.claim(&worker("gpu-b", true)).unwrap().unwrap();
    assert_eq!((next.job.id, next.gpu), (second.id, true));
    // A worker without the trainer gets nothing.
    let mut python = worker("py", false);
    python.trainers = vec!["pytorch".into()];
    assert!(lab.claim(&python).unwrap().is_none());
}

#[test]
fn stop_resume_and_complete() {
    let (lab, dir) = lab("lifecycle");
    let job = training(&lab, &dir, Device::Auto);
    let claim = lab.claim(&worker("w", false)).unwrap().unwrap();
    let token = claim.token();
    let progress = Progress::Training(TrainingProgress {
        superbatch: 1,
        superbatches: 3,
        loss: 0.05,
        ..TrainingProgress::default()
    });
    assert_eq!(lab.progress(token, &progress).unwrap(), Directive::Continue);
    assert_eq!(lab.job(job.id).unwrap().progress, Some(progress.clone()));

    // Stop: the worker is told at its next report, then acknowledges.
    assert_eq!(lab.stop(job.id).unwrap().status, JobStatus::Stopping);
    assert_eq!(lab.heartbeat(token).unwrap(), Directive::Stop);
    lab.event(token, EventKind::Checkpoint, json!({ "superbatch": 1 }))
        .unwrap();
    lab.stopped(token).unwrap();
    let stopped = lab.job(job.id).unwrap();
    assert_eq!(stopped.status, JobStatus::Stopped);
    assert!(stopped.finished_at.is_some());
    assert!(matches!(lab.stop(job.id), Err(LabError::Conflict(_))));
    assert!(matches!(lab.heartbeat(token), Err(LabError::Lost(_))));

    // Resume: queued again, the next claim is attempt 2.
    assert_eq!(lab.resume(job.id).unwrap().status, JobStatus::Queued);
    let again = lab.claim(&worker("w2", false)).unwrap().unwrap();
    assert_eq!(again.attempt, 2);
    assert_eq!(again.job.progress, Some(progress));
    assert!(matches!(
        lab.progress(token, &Progress::Training(TrainingProgress::default())),
        Err(LabError::Lost(_))
    ));

    // The network must be a version 2 file of the job's architecture.
    let summary = TrainingSummary {
        superbatches: 3,
        positions: 300,
        final_loss: Some(0.04),
        device: "CPU".into(),
    };
    let other = Network::pseudo_random(&presets::get("halfka-medium").unwrap().architecture, 1)
        .unwrap()
        .to_bytes();
    let err = lab
        .complete_training(again.token(), &other, &summary)
        .unwrap_err();
    assert!(err.to_string().contains("the job trains"), "{err}");
    assert!(matches!(
        lab.complete_training(again.token(), b"junk", &summary),
        Err(LabError::Invalid(_))
    ));

    let network = Network::pseudo_random(&presets::get("fast-768").unwrap().architecture, 1)
        .unwrap()
        .to_bytes();
    let model = lab
        .complete_training(again.token(), &network, &summary)
        .unwrap();
    assert_eq!(
        std::fs::read(lab.paths().model_file(model.id)).unwrap(),
        network
    );
    assert_eq!(model.provenance.seed, 7);
    assert_eq!(model.provenance.final_loss, Some(0.04));
    assert_eq!(model.bytes, network.len() as u64);
    let done = lab.job(job.id).unwrap();
    assert_eq!(
        (done.status, done.model),
        (JobStatus::Succeeded, Some(model.id))
    );
    assert_eq!(lab.models(None, 10).unwrap(), vec![model]);

    let kinds: Vec<(EventKind, Option<String>)> = lab
        .events(job.id, 0, 100)
        .unwrap()
        .into_iter()
        .map(|e| (e.kind, e.data["status"].as_str().map(str::to_owned)))
        .collect();
    let status = |s: &str| (EventKind::Status, Some(s.to_owned()));
    assert_eq!(
        kinds,
        [
            status("queued"),
            status("running"),
            (EventKind::Progress, None),
            status("stopping"),
            (EventKind::Checkpoint, None),
            status("stopped"),
            status("queued"),
            status("running"),
            (EventKind::Model, None),
            status("succeeded"),
        ]
    );
    // Events page by sequence number.
    let events = lab.events(job.id, 0, 3).unwrap();
    let rest = lab.events(job.id, events[2].seq, 100).unwrap();
    assert_eq!(rest.len(), 7);
}

#[test]
fn silent_workers_lose_their_jobs() {
    let (lab, dir) = lab("reap");
    let job = training(&lab, &dir, Device::Auto);
    let claim = lab.claim(&worker("w", true)).unwrap().unwrap();
    // Not stale yet.
    assert!(lab.reap(job.created_at - 1).unwrap().is_empty());
    assert_eq!(lab.reap(i64::MAX).unwrap(), vec![job.id]);
    assert_eq!(lab.job(job.id).unwrap().status, JobStatus::Interrupted);
    assert!(matches!(
        lab.heartbeat(claim.token()),
        Err(LabError::Lost(_))
    ));
    // The GPU slot was released.
    lab.resume(job.id).unwrap();
    assert!(lab.claim(&worker("w2", true)).unwrap().unwrap().gpu);
}

#[test]
fn selfplay_fills_its_dataset() {
    let (lab, dir) = lab("selfplay");
    let request = SelfplayRequest {
        name: Some("sp".into()),
        games: 2,
        nodes: 100,
        seed: Some(3),
        network: None,
        threads: 1,
    };
    let (job, dataset) = lab.create_selfplay(&request).unwrap();
    assert_eq!(dataset.status, DatasetStatus::Generating);
    assert_eq!(dataset.job, Some(job.id));
    let claim = lab.claim(&worker("w", true)).unwrap().unwrap();
    assert!(!claim.gpu);

    // The worker writes its files into the dataset's directory.
    std::fs::create_dir_all(lab.paths().dataset_dir(dataset.id)).unwrap();
    std::fs::write(
        lab.paths().dataset_dir(dataset.id).join("sp-0000.bin"),
        dataset_bytes(&dir, 20),
    )
    .unwrap();
    let ready = lab.complete_selfplay(claim.token()).unwrap();
    assert_eq!((ready.status, ready.records), (DatasetStatus::Ready, 20));
    assert_eq!(lab.job(job.id).unwrap().status, JobStatus::Succeeded);
    assert!(matches!(lab.resume(job.id), Err(LabError::Conflict(_))));

    // Stopping a queued self-play job cancels it and fails its dataset.
    let (queued, dataset) = lab.create_selfplay(&request).unwrap();
    assert_eq!(lab.stop(queued.id).unwrap().status, JobStatus::Canceled);
    let dataset = lab.dataset(dataset.id).unwrap();
    assert_eq!(dataset.status, DatasetStatus::Failed);
    assert_eq!(dataset.error.as_deref(), Some("canceled"));

    // A missing model is reported.
    let mut bad = request;
    bad.network = Some(chess_jobs::ModelId::generate());
    let Err(LabError::Invalid(errors)) = lab.create_selfplay(&bad) else {
        panic!("accepted a missing model");
    };
    assert_eq!(errors[0].path, "/network");
}

#[test]
fn lists_page_newest_first() {
    let (lab, dir) = lab("pages");
    let ids: Vec<_> = (0..5)
        .map(|_| training(&lab, &dir, Device::Cpu).id)
        .collect();
    let first = lab.jobs(None, 2).unwrap();
    assert_eq!(
        first.iter().map(|j| j.id).collect::<Vec<_>>(),
        [ids[4], ids[3]]
    );
    let second = lab.jobs(Some(first[1].id), 10).unwrap();
    assert_eq!(
        second.iter().map(|j| j.id).collect::<Vec<_>>(),
        [ids[2], ids[1], ids[0]]
    );
    assert!(matches!(
        lab.job(chess_jobs::JobId::generate()),
        Err(LabError::NotFound { what: "job", .. })
    ));
}
