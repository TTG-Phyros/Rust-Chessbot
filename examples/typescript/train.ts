// Trains a network through the NNUE lab API with the TypeScript SDK:
// validates and saves examples/architecture.json, uploads a dataset, trains,
// follows the live events, then activates and downloads the model.
//
//   npm install
//   CHESS_API_URL=http://localhost:8080 CHESS_API_KEY=... node train.ts positions.cbds

import { openAsBlob } from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import { basename } from 'node:path';
import {
  ArchitecturesApi,
  Configuration,
  DatasetsApi,
  EventKind,
  JobEventFromJSON,
  JobStatus,
  ModelsApi,
  ResponseError,
  TrainingRunsApi,
  type Architecture,
  type JobEvent,
} from 'chess-nnue-lab';

const [datasetPath] = process.argv.slice(2);
if (!datasetPath) {
  console.error('usage: node train.ts DATASET.cbds');
  process.exit(1);
}
const config = new Configuration({
  basePath: process.env.CHESS_API_URL ?? 'http://localhost:8080',
  apiKey: process.env.CHESS_API_KEY,
});
const architectures = new ArchitecturesApi(config);
const datasets = new DatasetsApi(config);
const runs = new TrainingRunsApi(config);
const models = new ModelsApi(config);

try {
  // 1. Validate the architecture: errors, size and cost before training anything.
  const architecture = JSON.parse(
    await readFile(new URL('../architecture.json', import.meta.url), 'utf8'),
  ) as Architecture;
  const report = await architectures.validateArchitecture({ architecture });
  if (!report.valid) {
    for (const error of report.errors) console.error(`${error.path}: ${error.message}`);
    process.exit(1);
  }
  console.log(
    `architecture ${report.hash?.slice(0, 12)}: ${report.estimate?.parameters} parameters, ` +
      `${report.estimate?.weightsBytes} bytes`,
  );

  // 2. Save it. An Idempotency-Key makes the request safe to retry.
  const saved = await architectures.createArchitecture({
    architecture,
    idempotencyKey: crypto.randomUUID(),
  });

  // 3. Upload the dataset, read from disk as it is sent (the server checks every record).
  const uploaded = await datasets.uploadDataset({
    name: basename(datasetPath),
    body: await openAsBlob(datasetPath),
  });
  console.log(
    `dataset ${uploaded.name}: ${uploaded.records} positions, sha256 ${uploaded.sha256?.slice(0, 12)}`,
  );

  // 4. Queue a short training run on the saved architecture.
  const run = await runs.createTrainingRun({
    trainingRequest: {
      name: 'typescript example',
      architectureId: saved.id,
      datasets: [uploaded.id],
      seed: 1,
      hyperparameters: { superbatches: 3, batchesPerSuperbatch: 50, batchSize: 1024 },
    },
  });
  console.log(`training run ${run.id} queued`);

  // 5. Follow its live events (Server-Sent Events) until the run is over.
  const stream = await runs.streamTrainingEventsRaw({ id: run.id });
  let pending = '';
  for await (const chunk of stream.raw.body!.pipeThrough(new TextDecoderStream())) {
    const lines = (pending + chunk).split('\n');
    pending = lines.pop()!;
    for (const line of lines) {
      if (line.startsWith('data:')) show(JobEventFromJSON(JSON.parse(line.slice(5))));
    }
  }

  const job = await runs.getTrainingRun({ id: run.id });
  if (job.status !== JobStatus.Succeeded || !job.model) {
    console.error(`run ended ${job.status}: ${job.error ?? ''}`);
    process.exit(1);
  }

  // 6. Make it the engine's network, and download the file.
  await models.activateModel({ id: job.model });
  const file = await models.downloadModel({ id: job.model });
  await writeFile(`${job.model}.nnue`, new Uint8Array(await file.arrayBuffer()));
  console.log(`model ${job.model} active, saved to ${job.model}.nnue`);
} catch (error) {
  // Problem details (RFC 9457): type, title, status, detail, errors.
  if (error instanceof ResponseError) {
    console.error(`HTTP ${error.response.status}: ${await error.response.text()}`);
    process.exit(1);
  }
  throw error;
}

function show(event: JobEvent): void {
  const data = event.data as Record<string, unknown>;
  if (event.kind === EventKind.Progress) {
    console.log(`  superbatch ${data.superbatch}/${data.superbatches}: loss ${data.loss}`);
  } else if (event.kind === EventKind.Status) {
    console.log(`  status: ${data.status}`);
  }
}
