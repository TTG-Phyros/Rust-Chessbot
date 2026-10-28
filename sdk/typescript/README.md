# chess-nnue-lab@1.0.0

A TypeScript SDK client for the localhost API.

## Usage

First, install the SDK from npm.

```bash
npm install chess-nnue-lab --save
```

Next, try it out.


```ts
import {
  Configuration,
  ArchitecturesApi,
} from 'chess-nnue-lab';
import type { CreateArchitectureRequest } from 'chess-nnue-lab';

async function example() {
  console.log("🚀 Testing chess-nnue-lab SDK...");
  const config = new Configuration({ 
    // To configure API key authorization: api_key
    apiKey: "YOUR API KEY",
  });
  const api = new ArchitecturesApi(config);

  const body = {
    // Architecture | Architecture document
    architecture: ...,
    // string | Makes retries safe (optional)
    idempotencyKey: idempotencyKey_example,
  } satisfies CreateArchitectureRequest;

  try {
    const data = await api.createArchitecture(body);
    console.log(data);
  } catch (error) {
    console.error(error);
  }
}

// Run the test
example().catch(console.error);
```


## Documentation

### API Endpoints

All URIs are relative to *http://localhost*

| Class | Method | HTTP request | Description
| ----- | ------ | ------------ | -------------
*ArchitecturesApi* | [**createArchitecture**](docs/ArchitecturesApi.md#createarchitecture) | **POST** /api/v1/architectures | Saves an architecture for training runs to use (&#x60;architectureId&#x60;).
*ArchitecturesApi* | [**deleteArchitecture**](docs/ArchitecturesApi.md#deletearchitecture) | **DELETE** /api/v1/architectures/{id} | Deletes a saved architecture (runs and models keep their copy).
*ArchitecturesApi* | [**getArchitecture**](docs/ArchitecturesApi.md#getarchitecture) | **GET** /api/v1/architectures/{id} | A saved architecture.
*ArchitecturesApi* | [**getArchitectureSchema**](docs/ArchitecturesApi.md#getarchitectureschema) | **GET** /api/v1/architectures/schema | The JSON Schema of architecture documents (for editors: &#x60;$schema&#x60;).
*ArchitecturesApi* | [**listArchitectures**](docs/ArchitecturesApi.md#listarchitectures) | **GET** /api/v1/architectures | Saved architectures.
*ArchitecturesApi* | [**listPresets**](docs/ArchitecturesApi.md#listpresets) | **GET** /api/v1/architectures/presets | The ready-made architectures.
*ArchitecturesApi* | [**validateArchitecture**](docs/ArchitecturesApi.md#validatearchitecture) | **POST** /api/v1/architectures:validate | Checks an architecture document without saving it.
*DatasetsApi* | [**deleteDataset**](docs/DatasetsApi.md#deletedataset) | **DELETE** /api/v1/datasets/{id} | Deletes a dataset and its files.
*DatasetsApi* | [**getDataset**](docs/DatasetsApi.md#getdataset) | **GET** /api/v1/datasets/{id} | A dataset.
*DatasetsApi* | [**getDatasetStats**](docs/DatasetsApi.md#getdatasetstats) | **GET** /api/v1/datasets/{id}/stats | Statistics of a ready dataset.
*DatasetsApi* | [**listDatasets**](docs/DatasetsApi.md#listdatasets) | **GET** /api/v1/datasets | Datasets.
*DatasetsApi* | [**startSelfplay**](docs/DatasetsApi.md#startselfplay) | **POST** /api/v1/datasets:selfplay | Starts a self-play run that generates a new dataset.
*DatasetsApi* | [**uploadDataset**](docs/DatasetsApi.md#uploaddataset) | **POST** /api/v1/datasets | Uploads a dataset file (&#x60;CBDS&#x60;, see the documentation). It is checked record by record while it streams to disk; nothing is kept if it is invalid.
*EvaluationsApi* | [**compareModels**](docs/EvaluationsApi.md#comparemodels) | **GET** /api/v1/evaluations:compare | Compares models (and the classical evaluation) over every evaluation played between two of them, whatever its settings: network size, speed, results of each pair, and Elo ratings fitted over all the games (Bradley–Terry maximum likelihood, relative to the first player).
*EvaluationsApi* | [**createEvaluation**](docs/EvaluationsApi.md#createevaluation) | **POST** /api/v1/evaluations | Queues an evaluation match: the candidate model plays pairs of games against the baseline (a model, or the classical evaluation) from the same random opening with colors swapped, at a fixed number of nodes (reproducible) or time per move. With &#x60;sprt&#x60;, the match stops as soon as the test concludes. The worker first measures the speed (nodes per second) of both engines.
*EvaluationsApi* | [**getEvaluation**](docs/EvaluationsApi.md#getevaluation) | **GET** /api/v1/evaluations/{id} | An evaluation, with its latest result: W/D/L, Elo difference ± its 95 % error, likelihood of superiority, SPRT status, and the speed of both engines. Its &#x60;spec&#x60; describes both contestants (name, architecture, size), even once their models are deleted.
*EvaluationsApi* | [**listEvaluations**](docs/EvaluationsApi.md#listevaluations) | **GET** /api/v1/evaluations | Evaluations, newest first.
*EvaluationsApi* | [**stopEvaluation**](docs/EvaluationsApi.md#stopevaluation) | **POST** /api/v1/evaluations/{id}:stop | Stops an evaluation: a queued one is canceled, a running one stops after its current games and keeps their result.
*EvaluationsApi* | [**streamEvaluationEvents**](docs/EvaluationsApi.md#streamevaluationevents) | **GET** /api/v1/evaluations/{id}/events | Live events of an evaluation (Server-Sent Events): &#x60;status&#x60;, and a &#x60;progress&#x60; after each batch of games. See the training runs\&#39; events.
*MetaApi* | [**getOpenApiDocument**](docs/MetaApi.md#getopenapidocument) | **GET** /api/v1/openapi.json | This document
*ModelsApi* | [**activateModel**](docs/ModelsApi.md#activatemodel) | **POST** /api/v1/models/{id}:activate | Makes the engine play (and analyse) with this model, from the next search on.
*ModelsApi* | [**deactivateModel**](docs/ModelsApi.md#deactivatemodel) | **POST** /api/v1/models:deactivate | Goes back to the server\&#39;s configured network.
*ModelsApi* | [**deleteModel**](docs/ModelsApi.md#deletemodel) | **DELETE** /api/v1/models/{id} | Deletes a model and its file.
*ModelsApi* | [**downloadModel**](docs/ModelsApi.md#downloadmodel) | **GET** /api/v1/models/{id}/file | The network file (&#x60;CBNN&#x60; version 2), loadable by the engine (&#x60;setoption name EvalFile&#x60;).
*ModelsApi* | [**getModel**](docs/ModelsApi.md#getmodel) | **GET** /api/v1/models/{id} | A model, with how it was made.
*ModelsApi* | [**listModels**](docs/ModelsApi.md#listmodels) | **GET** /api/v1/models | Models, newest first.
*SelfplayRunsApi* | [**getSelfplayRun**](docs/SelfplayRunsApi.md#getselfplayrun) | **GET** /api/v1/selfplay-runs/{id} | A self-play run (start one with &#x60;POST /api/v1/datasets:selfplay&#x60;).
*SelfplayRunsApi* | [**listSelfplayRuns**](docs/SelfplayRunsApi.md#listselfplayruns) | **GET** /api/v1/selfplay-runs | Self-play runs.
*SelfplayRunsApi* | [**stopSelfplayRun**](docs/SelfplayRunsApi.md#stopselfplayrun) | **POST** /api/v1/selfplay-runs/{id}:stop | Stops a self-play run: its dataset keeps the games already played.
*SelfplayRunsApi* | [**streamSelfplayEvents**](docs/SelfplayRunsApi.md#streamselfplayevents) | **GET** /api/v1/selfplay-runs/{id}/events | Live events of a self-play run (see the training runs\&#39; events).
*TrainingRunsApi* | [**createTrainingRun**](docs/TrainingRunsApi.md#createtrainingrun) | **POST** /api/v1/training-runs | Queues a training run. Everything needed to reproduce it is recorded: the normalized architecture, the content hash of each dataset, the hyperparameters and the seed (drawn when not given).
*TrainingRunsApi* | [**getTrainingMetrics**](docs/TrainingRunsApi.md#gettrainingmetrics) | **GET** /api/v1/training-runs/{id}/metrics | The training curve: loss, learning rate and speed per superbatch.
*TrainingRunsApi* | [**getTrainingRun**](docs/TrainingRunsApi.md#gettrainingrun) | **GET** /api/v1/training-runs/{id} | A training run, with its latest progress.
*TrainingRunsApi* | [**listTrainingRuns**](docs/TrainingRunsApi.md#listtrainingruns) | **GET** /api/v1/training-runs | Training runs.
*TrainingRunsApi* | [**resumeTrainingRun**](docs/TrainingRunsApi.md#resumetrainingrun) | **POST** /api/v1/training-runs/{id}:resume | Queues a stopped, interrupted or failed training run again: it resumes from its last checkpoint.
*TrainingRunsApi* | [**stopTrainingRun**](docs/TrainingRunsApi.md#stoptrainingrun) | **POST** /api/v1/training-runs/{id}:stop | Stops a training run: a queued run is canceled, a running one saves a checkpoint at the end of its superbatch and stops.
*TrainingRunsApi* | [**streamTrainingEvents**](docs/TrainingRunsApi.md#streamtrainingevents) | **GET** /api/v1/training-runs/{id}/events | Live events of a training run (Server-Sent Events): &#x60;status&#x60;, &#x60;progress&#x60;, &#x60;checkpoint&#x60;, &#x60;model&#x60;, &#x60;log&#x60;. Each event\&#39;s &#x60;id&#x60; is its sequence number; reconnecting with &#x60;Last-Event-ID&#x60; (or &#x60;after&#x60;) resumes after it. The stream replays the run\&#39;s history first, and ends once the run is over.


### Models

- [Accumulator](docs/Accumulator.md)
- [AccumulatorType](docs/AccumulatorType.md)
- [Activation](docs/Activation.md)
- [Architecture](docs/Architecture.md)
- [ArchitecturePage](docs/ArchitecturePage.md)
- [BucketScope](docs/BucketScope.md)
- [BucketStrategy](docs/BucketStrategy.md)
- [ComparedPlayer](docs/ComparedPlayer.md)
- [Comparison](docs/Comparison.md)
- [Contestant](docs/Contestant.md)
- [Dataset](docs/Dataset.md)
- [DatasetPage](docs/DatasetPage.md)
- [DatasetRef](docs/DatasetRef.md)
- [DatasetSource](docs/DatasetSource.md)
- [DatasetStats](docs/DatasetStats.md)
- [DatasetStatus](docs/DatasetStatus.md)
- [DenseLayer](docs/DenseLayer.md)
- [DenseShape](docs/DenseShape.md)
- [Device](docs/Device.md)
- [Estimate](docs/Estimate.md)
- [EvaluationJob](docs/EvaluationJob.md)
- [EvaluationProgress](docs/EvaluationProgress.md)
- [EvaluationRequest](docs/EvaluationRequest.md)
- [EventKind](docs/EventKind.md)
- [FeatureSet](docs/FeatureSet.md)
- [Features](docs/Features.md)
- [FieldError](docs/FieldError.md)
- [Histogram](docs/Histogram.md)
- [Hyperparameters](docs/Hyperparameters.md)
- [Job](docs/Job.md)
- [JobEvent](docs/JobEvent.md)
- [JobPage](docs/JobPage.md)
- [JobSpec](docs/JobSpec.md)
- [JobSpecOneOf](docs/JobSpecOneOf.md)
- [JobSpecOneOf1](docs/JobSpecOneOf1.md)
- [JobSpecOneOf2](docs/JobSpecOneOf2.md)
- [JobStatus](docs/JobStatus.md)
- [Layout](docs/Layout.md)
- [Metadata](docs/Metadata.md)
- [MetricPoint](docs/MetricPoint.md)
- [Metrics](docs/Metrics.md)
- [Mirror](docs/Mirror.md)
- [Model](docs/Model.md)
- [ModelPage](docs/ModelPage.md)
- [ModelView](docs/ModelView.md)
- [Outcomes](docs/Outcomes.md)
- [OutputBuckets](docs/OutputBuckets.md)
- [PairResult](docs/PairResult.md)
- [Preset](docs/Preset.md)
- [Problem](docs/Problem.md)
- [Progress](docs/Progress.md)
- [ProgressOneOf](docs/ProgressOneOf.md)
- [ProgressOneOf1](docs/ProgressOneOf1.md)
- [ProgressOneOf2](docs/ProgressOneOf2.md)
- [Provenance](docs/Provenance.md)
- [Quantization](docs/Quantization.md)
- [SelfplayJob](docs/SelfplayJob.md)
- [SelfplayProgress](docs/SelfplayProgress.md)
- [SelfplayRequest](docs/SelfplayRequest.md)
- [SelfplayStarted](docs/SelfplayStarted.md)
- [Sprt](docs/Sprt.md)
- [SprtStatus](docs/SprtStatus.md)
- [StoredArchitecture](docs/StoredArchitecture.md)
- [SumWidth](docs/SumWidth.md)
- [TrainingJob](docs/TrainingJob.md)
- [TrainingProgress](docs/TrainingProgress.md)
- [TrainingRequest](docs/TrainingRequest.md)
- [ValidationReport](docs/ValidationReport.md)
- [Verdict](docs/Verdict.md)
- [WeightType](docs/WeightType.md)

### Authorization


Authentication schemes defined for the API:
<a id="api_key"></a>
#### api_key


- **Type**: API key
- **API key parameter name**: `X-API-Key`
- **Location**: HTTP header

## About

This TypeScript SDK client supports the [Fetch API](https://fetch.spec.whatwg.org/)
and is automatically generated by the
[OpenAPI Generator](https://openapi-generator.tech) project:

- API version: `1.0.0`
- Package version: `1.0.0`
- Generator version: `7.26.0`
- Build package: `org.openapitools.codegen.languages.TypeScriptFetchClientCodegen`

The generated npm module supports the following:

- Environments
  * Node.js
  * Webpack
  * Browserify
- Language levels
  * ES5 - you must have a Promises/A+ library installed
  * ES6
- Module systems
  * CommonJS
  * ES6 module system


## Development

### Building

To build the TypeScript source code, you need to have Node.js and npm installed.
After cloning the repository, navigate to the project directory and run:

```bash
npm install
npm run build
```

### Publishing

Once you've built the package, you can publish it to npm:

```bash
npm publish
```

## License

[MIT OR Apache-2.0]()
