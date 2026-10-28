#!/usr/bin/env bash
# Trains a network through the NNUE lab API with curl and jq: validates and
# saves examples/architecture.json, uploads a dataset, trains, follows the
# live events, downloads the model, evaluates it against the classical
# evaluation, then activates it.
#
#   CHESS_API_URL=http://localhost:8080 CHESS_API_KEY=... ./train.sh positions.cbds
set -euo pipefail

dataset=${1:?usage: train.sh DATASET.cbds}
api=${CHESS_API_URL:-http://localhost:8080}/api/v1
key=${CHESS_API_KEY:?set CHESS_API_KEY to one of the keys in CHESS_API_KEYS}
here=$(dirname "$0")

# Fails on HTTP errors and prints the problem details (RFC 9457) the API sends.
call() {
  local out status
  out=$(mktemp)
  status=$(curl -sS -o "$out" -w '%{http_code}' -H "X-API-Key: $key" "$@")
  if [ "$status" -ge 400 ]; then
    echo "HTTP $status: $(cat "$out")" >&2
    rm -f "$out"
    exit 1
  fi
  cat "$out"
  rm -f "$out"
}

# Prints the live events (Server-Sent Events) of a job until it ends.
follow() {
  curl -sS -N -H "X-API-Key: $key" "$api/$1/events" |
    while IFS= read -r line; do
      case $line in
        data:*) jq -r 'if .kind == "status" then "  status: \(.data.status)"
            elif .kind == "progress" and .data.kind == "training" then
              "  superbatch \(.data.superbatch)/\(.data.superbatches): loss \(.data.loss)"
            elif .kind == "progress" and .data.kind == "evaluation" and .data.games > 0 then
              "  games \(.data.games)/\(.data.gamesMax): +\(.data.wins) =\(.data.draws) -\(.data.losses)"
            else empty end' <<<"${line#data:}" ;;
      esac
    done
}

# 1. Validate the architecture: errors, size and cost before training anything.
report=$(call -X POST "$api/architectures:validate" \
  -H 'Content-Type: application/json' --data-binary "@$here/../architecture.json")
if [ "$(jq -r .valid <<<"$report")" != true ]; then
  jq -r '.errors[] | "\(.path): \(.message)"' <<<"$report" >&2
  exit 1
fi
jq -r '"architecture \(.hash[:12]): \(.estimate.parameters) parameters, \(.estimate.weightsBytes) bytes"' <<<"$report"

# 2. Save it. An Idempotency-Key makes the request safe to retry.
architecture=$(call -X POST "$api/architectures" \
  -H 'Content-Type: application/json' -H "Idempotency-Key: $(uuidgen 2>/dev/null || date +%s%N)" \
  --data-binary "@$here/../architecture.json" | jq -r .id)

# 3. Upload the dataset, streamed (the server checks every record).
upload=$(call -X POST "$api/datasets?name=$(basename "$dataset")" \
  -H 'Content-Type: application/octet-stream' --data-binary "@$dataset")
dataset_id=$(jq -r .id <<<"$upload")
jq -r '"dataset \(.name): \(.records) positions, sha256 \(.sha256[:12])"' <<<"$upload"

# 4. Queue a short training run on the saved architecture.
run=$(call -X POST "$api/training-runs" -H 'Content-Type: application/json' -d "$(jq -n \
  --arg architecture "$architecture" --arg dataset "$dataset_id" '{
    name: "curl example",
    architectureId: $architecture,
    datasets: [$dataset],
    seed: 1,
    hyperparameters: { superbatches: 3, batchesPerSuperbatch: 50, batchSize: 1024 }
  }')" | jq -r .id)
echo "training run $run queued"

# 5. Follow its live events until the run is over.
follow "training-runs/$run"
job=$(call "$api/training-runs/$run")
if [ "$(jq -r .status <<<"$job")" != succeeded ]; then
  echo "run ended $(jq -r '"\(.status): \(.error // "")"' <<<"$job")" >&2
  exit 1
fi
model=$(jq -r .model <<<"$job")

# 6. Download the network file.
curl -sS --fail-with-body -H "X-API-Key: $key" -o "$model.nnue" "$api/models/$model/file"
echo "model $model saved to $model.nnue"

# 7. Evaluate it: a match against the classical evaluation, at a fixed number
#    of nodes per move, stopped as soon as the SPRT concludes.
evaluation=$(call -X POST "$api/evaluations" -H 'Content-Type: application/json' -d "$(jq -n \
  --arg model "$model" '{
    candidate: $model, games: 20, nodes: 1000, sprt: { elo0: 0, elo1: 50 }, seed: 1, threads: 2
  }')" | jq -r .id)
echo "evaluation $evaluation queued"
follow "evaluations/$evaluation"
finished=$(call "$api/evaluations/$evaluation")
if [ "$(jq -r .status <<<"$finished")" != succeeded ]; then
  echo "evaluation ended $(jq -r '"\(.status): \(.error // "")"' <<<"$finished")" >&2
  exit 1
fi
jq -r '.progress as $r | .spec.candidate as $c |
  "against the classical evaluation: +\($r.wins) =\($r.draws) -\($r.losses), " +
  "Elo \($r.elo * 10 | round / 10) ± \($r.eloError * 10 | round / 10), " +
  "LOS \($r.los * 1000 | round / 10)%, SPRT \($r.sprt.verdict)",
  "speed \($r.candidateNps) nodes/s (classical: \($r.baselineNps)), " +
  "network \($c.parameters) parameters, \($c.bytes) bytes"' <<<"$finished"

# 8. Make it the engine's network.
call -X POST "$api/models/$model:activate" >/dev/null
echo "model $model active"
