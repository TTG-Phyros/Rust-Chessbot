# Project shortcuts. `just` lists them. Works on Linux, macOS and Windows
# (PowerShell); every recipe is a plain command line, no shell-specific syntax.

set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

cuda_image := "nvidia/cuda:12.9.1-base-ubuntu24.04"
# Same image as OPENAPI_GENERATOR in .github/workflows/ci.yml
openapi_generator := "openapitools/openapi-generator-cli:v7.26.0@sha256:a304ddf1e2e5f24f68fa3153568d6174cea4959d09aa8e3db6d526fd0782326d"

# List recipes
default:
    @just --list

# Build every crate in release mode
build:
    cargo build --workspace --release

# Run the Rust and front-end test suites
test:
    cargo test --workspace
    npm --prefix web test

# Run the full perft suite (deep reference counts, ~790M nodes) in release mode
perft:
    cargo test --release -p chess-core --test perft -- --include-ignored

# Start the UCI engine on stdin/stdout
play:
    cargo run --release -p chess-uci

# Start chess-server natively on http://localhost:3000
serve:
    cargo run -p chess-server

# Start UI + API in Docker on http://localhost:8080
up:
    docker compose up -d --build

# Stop the Docker stack, training included (it checkpoints first)
down:
    docker compose --profile train --profile train-cpu down

# UI + API + training daemon on the NVIDIA GPU, in the background; idle until started from the Training page
train:
    docker compose --profile train up -d --build

# UI + API + training daemon on the CPU (image without CUDA), in the background; idle until started
train-cpu:
    docker compose --profile train-cpu up -d --build

# Follow the logs of a service: train (default), train-cpu, server or web
logs service="train":
    docker compose --profile train --profile train-cpu logs -f {{ service }}

# One-off chess-train command in the CUDA image, e.g. `just train-run match a.nnue classic`
train-run *args:
    docker compose --profile train run --rm --build train {{ args }}

# Check that Docker sees the GPU (nvidia-smi) and that burn computes on it
gpu-check:
    docker run --rm --gpus all {{ cuda_image }} nvidia-smi
    docker compose --profile train run --rm --build train gpu-check

# Same check without Docker, on this machine (CPU unless built with --features cuda)
gpu-check-local *args:
    cargo run --release -p chess-train -- gpu-check {{ args }}

# Vite dev server (hot reload) + native chess-server, proxied on one origin
[parallel]
web-dev: serve web-vite

[private]
web-vite:
    npm --prefix web install --no-audit --no-fund
    npm --prefix web run dev

# Run the criterion benchmarks
bench:
    cargo bench --workspace

# Format Rust and front-end code
fmt:
    cargo fmt --all
    npm --prefix web run format

# Every check the CI runs, without the Docker builds
lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo check -p chess-search
    npm --prefix web run lint
    npm --prefix web run typecheck
    npm --prefix web run format:check

# Regenerate the TypeScript API types (ts-rs), the JSON Schemas and the lab's OpenAPI document (schemas/) from the Rust code
gen-types:
    cargo test -p chess-server --lib
    cargo test -p chess-nnue-spec --test schema
    cargo test -p chess-nnue-api --test openapi

# Regenerate the Python and TypeScript SDKs (sdk/) from the OpenAPI document, with the generator pinned in the CI
gen-sdk: gen-types
    docker run --rm -v "{{ justfile_directory() }}:/local" -w /local {{ openapi_generator }} batch --clean sdk/python.yaml sdk/typescript.yaml

# Validate NNUE architecture documents (JSON or YAML) and print their hash and size, e.g. `just nnue-validate my-arch.yaml`
nnue-validate +files:
    cargo run -q -p chess-nnue-spec --example validate -- {{ files }}

# Print the header, architecture and hash of a network file
nnue-info file:
    cargo run -q --release -p chess-nnue -- info {{ file }}

# Rewrite a network (e.g. a legacy version 1 file) in the current format
nnue-convert input output:
    cargo run -q --release -p chess-nnue -- convert {{ input }} {{ output }}

# Write an untrained network of an architecture (preset id or file), for tests and benchmarks
nnue-random architecture output *args:
    cargo run -q --release -p chess-nnue -- random {{ architecture }} {{ output }} {{ args }}

# NNUE lab from the command line (datasets, training jobs, models), e.g. `just lab jobs`
lab *args:
    cargo run -q --release -p chess-train --features cuda -- lab {{ args }}

# NNUE lab worker on this machine: runs the queued jobs, on the GPU when there is one
lab-worker *args:
    cargo run --release -p chess-train --features cuda -- worker {{ args }}

# lint + test
ci: lint test

# Engine signature and speed: fixed-depth search over the bench positions
engine-bench depth="10":
    cargo run --release -p chess-uci -- bench {{ depth }}

# Sync the Chess.com games of a player into the review cache and list them, e.g. `just chesscom hikaru -n 10 --time-class blitz`
chesscom username *args:
    cargo run --release -p chess-reviewer -- fetch {{ username }} {{ args }}

# Collect the Elo calibration sample (Chess.com games, analysed), e.g. `just elo-collect --seed hikaru --country IS --until 2026-09-30 --preset fast`
elo-collect *args:
    cargo run --release -p chess-reviewer -- calibrate collect {{ args }}

# Recompute the sample's features from the cached analyses (after a classification change)
elo-refresh:
    cargo run --release -p chess-reviewer -- calibrate refresh

# Fit the Elo model on the sample: config/elo-samples.json -> config/elo-model.json
elo-fit:
    cargo run --release -p chess-reviewer -- calibrate fit
