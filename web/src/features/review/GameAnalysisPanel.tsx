import { useEffect, useState } from 'react';

import { api } from '../../api/client';
import type { ReviewGameView } from '../../api/generated/ReviewGameView';
import type { ReviewJobState } from '../../api/generated/ReviewJobState';
import type { ReviewOptions } from '../../api/generated/ReviewOptions';
import type { ReviewPreset } from '../../api/generated/ReviewPreset';
import { useReviewJob } from '../../api/hooks';
import { Button, Card, ErrorText } from '../../components/ui';
import { formatDuration } from './profile';
import { deeperThan } from './review';

const FINAL: ReadonlySet<ReviewJobState> = new Set(['done', 'failed', 'cancelled']);

function presetLabel(preset: ReviewPreset): string {
  const budget =
    preset.depth != null
      ? `profondeur ${String(preset.depth)}`
      : `${String(preset.movetimeMs ?? 0)} ms par coup`;
  return `${budget} · ${String(preset.multipv)} lignes`;
}

/**
 * The review of this game alone, as on Chess.com: theory, best moves, good
 * ones, mistakes, brilliant moves. Deeper than the review of many games by
 * default, its positions shared out between the workers. Hidden once the
 * game has an analysis at least as deep as the game preset.
 */
export function GameAnalysisPanel({
  view,
  options,
  onDone,
}: {
  view: ReviewGameView;
  options: ReviewOptions;
  /** Called once the review ends, to load it. */
  onDone: () => void;
}) {
  const choices = [options.gamePreset, ...options.presets].reverse();
  const [preset, setPreset] = useState(options.gamePreset.name);
  const [jobId, setJobId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const job = useReviewJob(jobId);
  const running = job !== null && !FINAL.has(job.state);

  const ended = job !== null && FINAL.has(job.state);
  useEffect(() => {
    if (ended) onDone();
  }, [ended, onDone]);

  const analysis = view.analysis;
  const deepen =
    analysis != null && deeperThan(options.gamePreset, analysis.budget, analysis.multipv);
  if (analysis && !deepen && !running && job?.state !== 'failed') return null;

  const start = (name: string) => {
    setError(null);
    api
      .analyseReviewGame(view.game.id, { preset: name })
      .then(({ jobId: id }) => {
        setJobId(id);
      })
      .catch((reason: unknown) => {
        setError(reason instanceof Error ? reason.message : String(reason));
      });
  };

  const share =
    job && job.positionsTotal > 0 ? job.positionsDone / job.positionsTotal : ended ? 1 : 0;
  return (
    <Card title="Bilan de la partie" highlight={!analysis}>
      {running ? (
        <div className="space-y-2">
          <div className="flex items-baseline justify-between gap-3 text-sm">
            <span>
              Analyse · {job.positionsDone} / {job.positionsTotal} positions
            </span>
            <span className="font-mono text-xs text-dim">
              {job.etaSeconds != null ? `${formatDuration(job.etaSeconds)} restantes` : ''}
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
              className="h-full rounded-full bg-signal transition-[width] duration-300"
              style={{ width: `${share * 100}%` }}
            />
          </div>
          <p className="text-xs text-dim">
            {options.workers} moteurs se partagent les positions de la partie.
          </p>
          <Button
            size="sm"
            variant="danger"
            onClick={() => {
              if (jobId !== null) void api.cancelReview(jobId);
            }}
          >
            Annuler
          </Button>
        </div>
      ) : deepen ? (
        <div className="space-y-3 text-sm">
          <p className="text-mute">
            Analyse actuelle : {analysis.budget.replace(/^depth /, 'profondeur ')} ·{' '}
            {analysis.multipv} lignes. Un bilan plus poussé affine les classifications (coups
            brillants, très bons coups, occasions manquées).
          </p>
          <Button
            onClick={() => {
              start(options.gamePreset.name);
            }}
          >
            Approfondir · {presetLabel(options.gamePreset)}
          </Button>
        </div>
      ) : (
        <div className="space-y-3">
          <p className="text-sm text-mute">
            Coups théoriques, meilleurs coups, bons coups, imprécisions, gaffes, coups brillants :
            chaque coup des deux joueurs est jugé par le moteur.
          </p>
          <div role="radiogroup" aria-label="Réglages" className="grid gap-2">
            {choices.map((choice) => {
              const active = choice.name === preset;
              return (
                <button
                  key={choice.name}
                  type="button"
                  role="radio"
                  aria-checked={active}
                  onClick={() => {
                    setPreset(choice.name);
                  }}
                  className={`flex items-baseline justify-between gap-3 rounded-xl border px-3 py-2 text-left transition-colors ${
                    active
                      ? 'border-signal/60 bg-signal/[0.06]'
                      : 'border-line hover:border-ink-600'
                  }`}
                >
                  <span className="font-medium">{choice.label}</span>
                  <span className="text-xs text-dim">{presetLabel(choice)}</span>
                </button>
              );
            })}
          </div>
          <Button
            variant="primary"
            onClick={() => {
              start(preset);
            }}
          >
            Lancer le bilan
          </Button>
        </div>
      )}
      {job?.state === 'failed' && <ErrorText>{job.error ?? "L'analyse a échoué."}</ErrorText>}
      {error && <ErrorText>{error}</ErrorText>}
    </Card>
  );
}
