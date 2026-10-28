"""Trains a network through the NNUE lab API with the Python SDK: validates
and saves examples/architecture.json, uploads a dataset, trains, follows the
live events, then activates and downloads the model.

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
    stream = runs.stream_training_events_without_preload_content(str(run.id))
    pending = b""
    for chunk in stream.stream(decode_content=True):
        *lines, pending = (pending + chunk).split(b"\n")
        for line in lines:
            if line.startswith(b"data:"):
                show(lab.JobEvent.from_json(line[5:].decode("utf-8")))
    stream.release_conn()

    job = runs.get_training_run(str(run.id))
    if job.status != lab.JobStatus.SUCCEEDED:
        sys.exit(f"run ended {job.status.value}: {job.error or ''}")

    # 6. Make it the engine's network, and download the file.
    models.activate_model(str(job.model))
    path = Path(f"{job.model}.nnue")
    path.write_bytes(models.download_model(str(job.model)))
    print(f"model {job.model} active, saved to {path}")


def show(event: lab.JobEvent) -> None:
    if event.kind == lab.EventKind.PROGRESS:
        data = event.data
        print(f"  superbatch {data['superbatch']}/{data['superbatches']}: loss {data['loss']}")
    elif event.kind == lab.EventKind.STATUS:
        print(f"  status: {event.data['status']}")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: train.py DATASET.cbds")
    try:
        main(Path(sys.argv[1]))
    except ApiException as err:
        # Problem details (RFC 9457): type, title, status, detail, errors.
        sys.exit(f"HTTP {err.status}: {err.body}")
