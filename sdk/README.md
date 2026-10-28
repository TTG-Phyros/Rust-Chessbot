# SDKs of the NNUE lab API

Clients of `/api/v1` (see the [README](../README.md#api-du-laboratoire-apiv1)
and the interactive documentation on `/docs`), **generated** from the
OpenAPI document [`schemas/nnue-api.v1.openapi.json`](../schemas/nnue-api.v1.openapi.json):

| Directory | Package | Generator |
| --- | --- | --- |
| [`python/`](python) | `chess_nnue_lab` (Python ≥ 3.10, pydantic 2, urllib3) | `python` |
| [`typescript/`](typescript) | `chess-nnue-lab` (fetch, ES2015+, Node ≥ 18 or browsers) | `typescript-fetch` |

Do not edit them by hand: change the API, then run

```sh
just gen-sdk   # = just gen-types (OpenAPI document) + openapi-generator in Docker
```

and commit `schemas/` and `sdk/`. The generator is a Docker image pinned by
digest (`openapi_generator` in the [justfile](../justfile), `OPENAPI_GENERATOR`
in the CI); the options are in [`python.yaml`](python.yaml) and
[`typescript.yaml`](typescript.yaml). The CI regenerates both SDKs and fails if
anything differs, then runs the [examples](../examples) with them against a
real server.

## Install

```sh
pip install ./sdk/python
npm install ./sdk/typescript   # builds dist/ (npm run build)
```

## Use

The key goes in `X-API-Key` (security scheme `api_key`). Errors are problem
details (RFC 9457): `ApiException.body` in Python, `ResponseError.response` in
TypeScript. POST methods take an optional `idempotency_key` / `idempotencyKey`.

```python
import chess_nnue_lab as lab

config = lab.Configuration(host="http://localhost:8080")
config.api_key["api_key"] = "…"
client = lab.ApiClient(config)
page = lab.ModelsApi(client).list_models(limit=10)
```

```ts
import { Configuration, ModelsApi } from 'chess-nnue-lab';

const config = new Configuration({ basePath: 'http://localhost:8080', apiKey: '…' });
const page = await new ModelsApi(config).listModels({ limit: 10 });
```

Live events (`…/events`, Server-Sent Events) are read from the raw response:
`stream_training_events_without_preload_content` in Python,
`streamTrainingEventsRaw(…).raw.body` in TypeScript; each `data:` line is a
`JobEvent` (see the examples).
