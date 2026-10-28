# Network file format (`CBNN`)

Every network the engine loads is a `.nnue` file in the `CBNN` format.
Version 2 is self-describing: the header carries the architecture (see the
[architecture schema](../schemas/nnue-architecture.v1.json)) and its hash, so
the engine needs nothing else to load and run the network. Version 1 is the
format of the networks trained before architectures became configurable;
it is still read.

All integers are little-endian.

## Version 2

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 4 | magic `CBNN` |
| 4 | 4 | format version, `u32` = 2 |
| 8 | 4 | header length `H`, `u32`: offset of the first tensor, a multiple of 64 |
| 12 | 4 | architecture length `S`, `u32` (at most 1 MiB) |
| 16 | 8 | weights length `P`, `u64` |
| 24 | 32 | architecture hash (SHA-256, see below) |
| 56 | 32 | weights hash: SHA-256 of the `P` bytes of tensors |
| 88 | `S` | architecture, JSON (UTF-8), every default written out |
| 88 + `S` | … | zero padding up to `H` |
| `H` | `P` | tensors |

The file is exactly `H + P` bytes.

### Architecture hash

The SHA-256 of the architecture's canonical JSON ([RFC 8785][jcs]), computed
on the normalized document (every default written out) **without** its
`metadata` (and `$schema`). Two documents that describe the same network
have the same hash whatever their names, key order or omitted defaults.
Anything that changes the meaning of the weights changes it.

`chess-nnue info <file>` prints it; the API returns it with every
architecture and model.

[jcs]: https://www.rfc-editor.org/rfc/rfc8785

### Tensors

In this order, with no padding between them. Shapes come from the
architecture (the [layout](../crates/chess-nnue-spec/src/layout.rs)):

1. Feature transformer weights: `inputs × accumulator` `i16`, feature-major
   (the `accumulator` weights of feature 0, then of feature 1, …).
2. Feature transformer biases: `accumulator` `i16`.
3. For each dense layer, hidden layers first and the output layer last:
   - weights: `buckets × outputs × inputs`, `i8` or `i16` as
     `quantization.weights` says, one row of `inputs` per neuron;
   - biases: `buckets × outputs` `i32`.

   `buckets` is the number of output buckets for the output layer (and for
   every dense layer with `outputBuckets.scope = "all"`), 1 otherwise.

The inputs of the first dense layer are the activated accumulator of the
side to move, then the opponent's (only White's without perspective).

### Loading

The engine refuses a file, with a precise error, when:

- the magic or the version is unknown;
- the header lengths are inconsistent, or the file is not `H + P` bytes;
- the architecture is not valid (the same validation as the API);
- **the architecture hash does not match the architecture** — the header
  was altered, or the weights were trained for another architecture;
- the weights hash does not match the tensors (corruption);
- the tensors do not have the sizes the architecture implies;
- a value exceeds the bounds of the quantization (`ftWeightClip` and
  `weightClip` times the scales, biases beyond ±2³⁰), which would overflow
  the integer sums.

A caller can also require a given architecture hash (`Network::expect_hash`).

## Evaluation

Integer-only; divisions truncate toward zero.

- Accumulators (`i16`): bias + the rows of the active features.
- Accumulator activation, on the `qa` scale: ReLU `max(x, 0)`, CReLU
  `clamp(x, 0, qa)`, SCReLU `clamp(x, 0, qa)² / qa`, pairwise CReLU
  `clamp(a, 0, qa) · clamp(b, 0, qa) / qa` (first half of a perspective ×
  second half).
- Dense layer: `sum = bias + Σ input · weight` (scale `qa · qb`); a hidden
  layer then computes `h = sum / qb` and applies its activation to `h`
  (ReLU saturates at 32767).
- Output: `sum · evalScale / (qa · qb)` centipawns for the side to move
  (negated when Black is to move for networks without perspective).

### Features

From a perspective, squares are flipped vertically for Black (`square ^ 56`)
and pieces are "own" or "opponent". With `mirror = "kingFile"`, when the
perspective's own king (after the flip) is on files e–h, squares are also
mirrored (`^ 7`). The king bucket is `kingBuckets[own king square]` (after
both transformations).

Feature index: `(bucket × planes + plane) × 64 + square`, with planes:

| Set | Planes |
| --- | --- |
| `Chess768` | own pawn … own king (0–5), opponent pawn … king (6–11) |
| `HalfKP` | own pawn … queen (0–4), opponent pawn … queen (5–9); kings have none |
| `HalfKAv2_hm` | as `HalfKP`, both kings in plane 10 |

### Output buckets

- `pieceCount`: `min(count − 1, (pieces − 2) / (32 / count))`, kings counted.
- `material`: `min(count − 1, material · count / 79)`, with P = 1, N = B = 3,
  R = 5, Q = 9 over both sides.

## Version 1 (legacy)

The fixed `768 → 256 ×2 → 1` network, the `legacy-768x256` preset:

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 4 | magic `CBNN` |
| 4 | 4 | version, `u32` = 1 |
| 8 | 20 | `u32` inputs (768), hidden (256), `QA` (255), `QB` (64), scale (400) |
| 28 | 768 × 256 × 2 | feature transformer weights, `i16`, feature-major |
| … | 256 × 2 | feature transformer biases, `i16` |
| … | 512 × 2 | output weights, `i16` (side to move half first) |
| … | 4 | output bias, `i32` |

Version 1 files load as that preset and evaluate exactly as before.
`chess-nnue convert old.nnue new.nnue` rewrites one in version 2 (same
weights, same evaluations; the result is checked to load back identically).

## Tools

```sh
just nnue-info data/weights/main.nnue           # header, architecture, hash
just nnue-convert old.nnue new.nnue             # any version → version 2
just nnue-random fast-768 /tmp/fast.nnue        # untrained network, for tests
```
