"""Trains a network through the NNUE lab API with the Python SDK: validates
and saves examples/architecture.json, uploads a dataset, trains, follows the
live events, downloads the model, evaluates it against the classical
evaluation, then activates it.

    pip install ../../sdk/python
    CHESS_API_URL=http://localhost:8080 CHESS_API_KEY=... python train.py positions.cbds
"""

import json
import os
import sys
import uuid
from pathlib import Path

import chess_nnue_lab as lab
from chess_nnue_lab.exceptions import ApiException


def main(dataset: Path) -> None:
    config = lab.Configuration(host=os.environ.get("CHESS_API_URL", "http://localhost:8080"))
    config.api_key["api_key"] = os.environ["CHESS_API_KEY"]
    client = lab.ApiClient(config)
    architectures = lab.ArchitecturesApi(client)
    datasets = lab.DatasetsApi(client)
    runs = lab.TrainingRunsApi(client)
    models = lab.ModelsApi(client)
    evaluations = lab.EvaluationsApi(client)

    # 1. Validate the architecture: errors, size and cost before training anything.
    document = json.loads((Path(__file__).parent.parent / "architecture.json").read_text("utf-8"))
    architecture = lab.Architecture.from_dict(document)
    report = architectures.validate_architecture(architecture)
    if not report.valid:
        for error in report.errors:
            print(f"{error.path}: {error.message}", file=sys.stderr)
        sys.exit(1)
    print(
        f"architecture {report.hash[:12]}: {report.estimate.parameters} parameters, "
        f"{report.estimate.weights_bytes} bytes"
    )

    # 2. Save it. An Idempotency-Key makes the request safe to retry.
    saved = architectures.create_architecture(architecture, idempotency_key=str(uuid.uuid4()))

    # 3. Upload the dataset (the server checks every record).
    uploaded = datasets.upload_dataset(name=dataset.name, body=str(dataset))
    print(f"dataset {uploaded.name}: {uploaded.records} positions, sha256 {uploaded.sha256[:12]}")

    # 4. Queue a short training run on the saved architecture.
    run = runs.create_training_run(
        lab.TrainingRequest(
            name="python example",
            architecture_id=saved.id,
            datasets=[uploaded.id],
            seed=1,
            hyperparameters=lab.Hyperparameters(
                superbatches=3, batches_per_superbatch=50, batch_size=1024
            ),
        )
    )
    print(f"training run {run.id} queued")

    # 5. Follow its live events (Server-Sent Events) until the run is over.
    follow(runs.stream_training_events_without_preload_content(str(run.id)))
    job = runs.get_training_run(str(run.id))
    if job.status != lab.JobStatus.SUCCEEDED:
        sys.exit(f"run ended {job.status.value}: {job.error or ''}")
    model = str(job.model)

    # 6. Download the network file.
    path = Path(f"{model}.nnue")
    path.write_bytes(models.download_model(model))
    print(f"model {model} saved to {path}")

    # 7. Evaluate it: a match against the classical evaluation, at a fixed
    #    number of nodes per move, stopped as soon as the SPRT concludes.
    evaluation = evaluations.create_evaluation(
        lab.EvaluationRequest(
            candidate=model,
            games=20,
            nodes=1000,
            sprt=lab.Sprt(elo0=0, elo1=50),
            seed=1,
            threads=2,
        )
    )
    print(f"evaluation {evaluation.id} queued")
    follow(evaluations.stream_evaluation_events_without_preload_content(str(evaluation.id)))
    done = evaluations.get_evaluation(str(evaluation.id))
    if done.status != lab.JobStatus.SUCCEEDED:
        sys.exit(f"evaluation ended {done.status.value}: {done.error or ''}")
    result = done.progress.actual_instance
    candidate = done.spec.actual_instance.candidate
    print(
        f"against the classical evaluation: +{result.wins} ={result.draws} -{result.losses}, "
        f"Elo {result.elo:+.1f} ± {result.elo_error:.1f}, LOS {result.los:.1%}, "
        f"SPRT {result.sprt.verdict.value}"
    )
    print(
        f"speed {result.candidate_nps} nodes/s (classical: {result.baseline_nps}), "
        f"network {candidate.parameters} parameters, {candidate.bytes} bytes"
    )

    # 8. Make it the engine's network.
    models.activate_model(model)
    print(f"model {model} active")


def follow(stream) -> None:
    """Prints the events of a job's stream until it ends."""
    pending = b""
    for chunk in stream.stream(decode_content=True):
        *lines, pending = (pending + chunk).split(b"\n")
        for line in lines:
            if line.startswith(b"data:"):
                show(lab.JobEvent.from_json(line[5:].decode("utf-8")))
    stream.release_conn()


def show(event: lab.JobEvent) -> None:
    data = event.data
    if event.kind == lab.EventKind.STATUS:
        print(f"  status: {data['status']}")
    elif event.kind == lab.EventKind.PROGRESS and data["kind"] == "training":
        print(f"  superbatch {data['superbatch']}/{data['superbatches']}: loss {data['loss']}")
    elif event.kind == lab.EventKind.PROGRESS and data["kind"] == "evaluation" and data["games"]:
        print(
            f"  games {data['games']}/{data['gamesMax']}: "
            f"+{data['wins']} ={data['draws']} -{data['losses']}"
        )


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: train.py DATASET.cbds")
    try:
        main(Path(sys.argv[1]))
    except ApiException as err:
        # Problem details (RFC 9457): type, title, status, detail, errors.
        sys.exit(f"HTTP {err.status}: {err.body}")
