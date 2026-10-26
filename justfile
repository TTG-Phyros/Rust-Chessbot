# Project shortcuts. `just` lists them. Works on Linux, macOS and Windows
# (PowerShell); every recipe is a plain command line, no shell-specific syntax.

set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

cuda_image := "nvidia/cuda:12.9.1-base-ubuntu24.04"

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

# Run the full perft suite (deep reference counts, ~760M nodes) in release mode
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
    docker compose up --build

# Stop the Docker stack
down:
    docker compose down

# UI + API + training on the NVIDIA GPU (start/stop from the Training page)
train:
    docker compose --profile train up --build

# UI + API + training on the CPU (image without CUDA)
train-cpu:
    docker compose --profile train-cpu up --build

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

# Regenerate the TypeScript API types from the Rust structs (ts-rs)
gen-types:
    cargo test -p chess-server --lib

# lint + test
ci: lint test

# Engine signature and speed: fixed-depth search over the bench positions
engine-bench depth="10":
    cargo run --release -p chess-uci -- bench {{ depth }}
