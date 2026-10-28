# NNUE lab API examples

The same scenario in four languages, against `/api/v1`:

1. validate [`architecture.json`](architecture.json) (errors, or its hash, size and cost);
2. save it, with an `Idempotency-Key`;
3. upload a dataset file (`CBDS`);
4. queue a short training run (3 superbatches, seed 1) on it;
5. follow the run's live events (Server-Sent Events) until it ends;
6. activate the model (the engine plays with it) and download its file.

| Example | Needs | Run (from the example's directory) |
| --- | --- | --- |
| [`curl/train.sh`](curl/train.sh) | bash, curl, jq | `./train.sh positions.cbds` |
| [`python/train.py`](python/train.py) | Python ≥ 3.10, `pip install ../../sdk/python` | `python train.py positions.cbds` |
| [`typescript/train.ts`](typescript/train.ts) | Node ≥ 22.18, `npm install` (the SDK must be built: `npm --prefix ../../sdk/typescript install`) | `node train.ts positions.cbds` |
| [`go/main.go`](go/main.go) | Go ≥ 1.24, standard library only (`net/http`) | `go run . positions.cbds` |

Each reads `CHESS_API_URL` (default `http://localhost:8080`, the Docker
stack; `http://localhost:3000` for a native `chess-server`) and
`CHESS_API_KEY` (one of the server's `CHESS_API_KEYS`). A worker must be
running to train (`docker compose --profile train up -d`, or
`chess-train worker`).

A dataset file comes from self-play, for instance
`CHESS_TRAIN_DATASET_DIR=out chess-train selfplay --games 200 --nodes 300`
(a few thousand positions in `out/main-0000.bin`), or from
`POST /api/v1/datasets:selfplay` on the server itself.

With the same dataset and seed, the four examples train the same network,
bit for bit: the CI runs them all against a server and a CPU worker and
compares the files.
