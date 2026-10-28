// Trains a network through the NNUE lab API with the TypeScript SDK:
// validates and saves examples/architecture.json, uploads a dataset, trains,
// follows the live events, downloads the model, evaluates it against the
// classical evaluation, then activates it.
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
  EvaluationsApi,
  EventKind,
  JobEventFromJSON,
  JobStatus,
  ModelsApi,
  ResponseError,
  TrainingRunsApi,
  type ApiResponse,
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
const evaluations = new EvaluationsApi(config);

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
  await follow(await runs.streamTrainingEventsRaw({ id: run.id }));
  const job = await runs.getTrainingRun({ id: run.id });
  if (job.status !== JobStatus.Succeeded || !job.model) {
    console.error(`run ended ${job.status}: ${job.error ?? ''}`);
    process.exit(1);
  }
  const model = job.model;

  // 6. Download the network file.
  const file = await models.downloadModel({ id: model });
  await writeFile(`${model}.nnue`, new Uint8Array(await file.arrayBuffer()));
  console.log(`model ${model} saved to ${model}.nnue`);

  // 7. Evaluate it: a match against the classical evaluation, at a fixed
  //    number of nodes per move, stopped as soon as the SPRT concludes.
  const evaluation = await evaluations.createEvaluation({
    evaluationRequest: {
      candidate: model,
      games: 20,
      nodes: 1000,
      sprt: { elo0: 0, elo1: 50 },
      seed: 1,
      threads: 2,
    },
  });
  console.log(`evaluation ${evaluation.id} queued`);
  await follow(await evaluations.streamEvaluationEventsRaw({ id: evaluation.id }));
  const done = await evaluations.getEvaluation({ id: evaluation.id });
  const result = done.progress;
  if (done.status !== JobStatus.Succeeded || result?.kind !== 'evaluation') {
    console.error(`evaluation ended ${done.status}: ${done.error ?? ''}`);
    process.exit(1);
  }
  console.log(
    `against the classical evaluation: +${result.wins} =${result.draws} -${result.losses}, ` +
      `Elo ${signed(result.elo)} ± ${result.eloError.toFixed(1)}, ` +
      `LOS ${(100 * result.los).toFixed(1)}%, SPRT ${result.sprt?.verdict}`,
  );
  if (done.spec.kind === 'evaluation') {
    const candidate = done.spec.candidate;
    console.log(
      `speed ${result.candidateNps} nodes/s (classical: ${result.baselineNps}), ` +
        `network ${candidate.parameters} parameters, ${candidate.bytes} bytes`,
    );
  }

  // 8. Make it the engine's network.
  await models.activateModel({ id: model });
  console.log(`model ${model} active`);
} catch (error) {
  // Problem details (RFC 9457): type, title, status, detail, errors.
  if (error instanceof ResponseError) {
    console.error(`HTTP ${error.response.status}: ${await error.response.text()}`);
    process.exit(1);
  }
  throw error;
}

/** Prints the events of a job's stream until it ends. */
async function follow(stream: ApiResponse<string>): Promise<void> {
  let pending = '';
  for await (const chunk of stream.raw.body!.pipeThrough(new TextDecoderStream())) {
    const lines = (pending + chunk).split('\n');
    pending = lines.pop()!;
    for (const line of lines) {
      if (line.startsWith('data:')) show(JobEventFromJSON(JSON.parse(line.slice(5))));
    }
  }
}

function show(event: JobEvent): void {
  const data = event.data as Record<string, unknown>;
  if (event.kind === EventKind.Status) {
    console.log(`  status: ${data.status}`);
  } else if (event.kind === EventKind.Progress && data.kind === 'training') {
    console.log(`  superbatch ${data.superbatch}/${data.superbatches}: loss ${data.loss}`);
  } else if (event.kind === EventKind.Progress && data.kind === 'evaluation' && data.games) {
    console.log(
      `  games ${data.games}/${data.gamesMax}: +${data.wins} =${data.draws} -${data.losses}`,
    );
  }
}

function signed(value: number): string {
  return (value >= 0 ? '+' : '') + value.toFixed(1);
}
