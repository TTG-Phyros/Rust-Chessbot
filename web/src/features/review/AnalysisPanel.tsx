import { useEffect, useMemo, useState } from 'react';

import { api } from '../../api/client';
import type { ReviewEstimate } from '../../api/generated/ReviewEstimate';
import type { ReviewJobState } from '../../api/generated/ReviewJobState';
import type { ReviewJobView } from '../../api/generated/ReviewJobView';
import type { ReviewOptions } from '../../api/generated/ReviewOptions';
import { useReviewEstimate, useReviewJob } from '../../api/hooks';
import { Button, Card, ErrorText } from '../../components/ui';
import { type Filters, formatDuration, reviewRequest } from './profile';

const STATE_NAME: Record<ReviewJobState, string> = {
  fetching: 'Téléchargement des parties',
  analysing: 'Analyse',
  done: 'Terminé',
  failed: 'Échec',
  cancelled: 'Annulé',
};

const FINAL: ReadonlySet<ReviewJobState> = new Set(['done', 'failed', 'cancelled']);

function settingsLabel(estimate: ReviewEstimate): string {
  const budget =
    estimate.depth != null
      ? `profondeur ${String(estimate.depth)}`
      : `${String(estimate.movetimeMs ?? 0)} ms par coup`;
  return `${budget} · ${String(estimate.multipv)} lignes`;
}

/** Progress of a job: a bar, what it does, the time left. */
function Progress({ job, onCancel }: { job: ReviewJobView; onCancel: () => void }) {
  const fetching = job.state === 'fetching';
  const [done, total] = fetching
    ? [job.monthsDone, job.monthsTotal]
    : [job.positionsDone, job.positionsTotal];
  const share = total === 0 ? (FINAL.has(job.state) ? 1 : 0) : done / total;
  const toAnalyse = job.games.length - job.gamesCached;
  return (
    <div className="space-y-2">
      <div className="flex items-baseline justify-between gap-3 text-sm">
        <span>
          {STATE_NAME[job.state]}
          {fetching && total > 0 && ` · ${String(done)} / ${String(total)} mois`}
          {!fetching &&
            ` · ${String(job.gamesDone)} / ${String(toAnalyse)} parties (${String(job.gamesCached)} déjà analysées)`}
        </span>
        <span className="font-mono text-xs text-dim">
          {job.etaSeconds != null && !FINAL.has(job.state)
            ? `${formatDuration(job.etaSeconds)} restantes`
            : (job.engine ?? '')}
        </span>
      </div>
      <div
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(share * 100)}
        className="h-2 overflow-hidden rounded-full bg-ink-800"
      >
        <div
          className={`h-full rounded-full transition-[width] duration-300 ${
            job.state === 'failed' ? 'bg-alert' : 'bg-signal'
          }`}
          style={{ width: `${share * 100}%` }}
        />
      </div>
      {job.gamesFailed > 0 && (
        <p className="text-xs text-warn">{job.gamesFailed} parties n'ont pas pu être analysées.</p>
      )}
      {job.error && <ErrorText>{job.error}</ErrorText>}
      {!FINAL.has(job.state) && (
        <Button size="sm" variant="danger" onClick={onCancel}>
          Annuler
        </Button>
      )}
    </div>
  );
}

/**
 * Downloading and analysing a player's games: the presets with the time each
 * would take, then the progress of the job.
 */
export function AnalysisPanel({
  player,
  filters,
  options,
  version,
  onDone,
}: {
  player: string;
  filters: Filters;
  options: ReviewOptions;
  version: number;
  /** Called once a job ends, to load the new analyses. */
  onDone: () => void;
}) {
  const [preset, setPreset] = useState(options.defaultPreset);
  const [jobId, setJobId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const job = useReviewJob(jobId);
  const running = job !== null && !FINAL.has(job.state);
  const request = useMemo(
    () => (options.chesscomEnabled ? reviewRequest(player, filters) : null),
    [options.chesscomEnabled, player, filters],
  );
  // Not while a job runs: its downloads and analyses change the answer.
  const estimate = useReviewEstimate(running ? null : request, version);

  // A job that ends brings new analyses.
  const ended = job !== null && FINAL.has(job.state);
  useEffect(() => {
    if (ended) onDone();
  }, [ended, onDone]);

  if (!options.chesscomEnabled) {
    return (
      <Card title="Analyse">
        <p className="text-sm text-warn">
          Chess.com n'est pas joignable : renseignez <code>CHESS_REVIEW_USER_AGENT</code> (voir{' '}
          <code>.env.example</code>). Les parties déjà en cache restent consultables.
        </p>
      </Card>
    );
  }

  const start = () => {
    setError(null);
    api
      .startReview(reviewRequest(player, filters, preset))
      .then(({ jobId: id }) => {
        setJobId(id);
      })
      .catch((reason: unknown) => {
        setError(reason instanceof Error ? reason.message : String(reason));
      });
  };

  const choices = (estimate.view?.options ?? []).filter((option) => option.preset != null);
  const chosen = choices.find((option) => option.preset === preset);
  return (
    <Card
      title="Télécharger et analyser"
      aside={`${String(options.workers)} parties analysées en parallèle`}
    >
      {estimate.loading && !running && (
        <p className="text-sm text-dim">Estimation (liste des parties, vitesse du moteur)…</p>
      )}
      {estimate.error && <ErrorText>{estimate.error}</ErrorText>}
      {estimate.view && (
        <p className="mb-3 text-sm text-mute">
          {estimate.view.games} parties correspondent aux filtres.
        </p>
      )}
      {choices.length > 0 && (
        <div role="radiogroup" aria-label="Réglages" className="grid gap-2 sm:grid-cols-3">
          {choices.map((option) => {
            const active = option.preset === preset;
            return (
              <button
                key={option.preset}
                type="button"
                role="radio"
                aria-checked={active}
                disabled={running}
                onClick={() => {
                  if (option.preset) setPreset(option.preset);
                }}
                className={`rounded-xl border p-3 text-left transition-colors disabled:opacity-50 ${
                  active ? 'border-signal/60 bg-signal/[0.06]' : 'border-line hover:border-ink-600'
                }`}
              >
                <p className="font-medium">{option.label}</p>
                <p className="text-xs text-dim">{settingsLabel(option)}</p>
                <p className="mt-2 font-mono text-lg">
                  {option.pendingGames === 0 ? 'déjà fait' : formatDuration(option.seconds)}
                </p>
                <p className="text-xs text-dim">
                  {option.pendingGames} à analyser · {option.cachedGames} déjà faites
                  {option.measured ? '' : ' · vitesse estimée'}
                </p>
              </button>
            );
          })}
        </div>
      )}
      <div className="mt-4 space-y-3">
        {job ? (
          <Progress
            job={job}
            onCancel={() => {
              if (jobId !== null) void api.cancelReview(jobId);
            }}
          />
        ) : null}
        {!running && (
          <Button variant="primary" onClick={start} disabled={estimate.loading}>
            {chosen && chosen.pendingGames === 0
              ? 'Mettre à jour les parties'
              : `Analyser${chosen ? ` (${formatDuration(chosen.seconds)})` : ''}`}
          </Button>
        )}
        {error && <ErrorText>{error}</ErrorText>}
      </div>
    </Card>
  );
}
