import { useState } from 'react';
import { Link, useSearchParams } from 'react-router';

import { api } from '../../api/client';
import type { ControllerView } from '../../api/generated/ControllerView';
import type { NetworkSummary } from '../../api/generated/NetworkSummary';
import type { RankedSnapshot } from '../../api/generated/RankedSnapshot';
import type { TrainingView } from '../../api/generated/TrainingView';
import { useTraining } from '../../api/hooks';
import { Button, Card, Dot, ErrorText, Eyebrow, INPUT, Stat } from '../../components/ui';
import { DataTable, EloChart, LossChart } from './charts';
import {
  CONTROLLER_LABEL,
  PHASE_LABEL,
  deviceName,
  formatCount,
  formatDate,
  formatElapsed,
  formatElo,
} from './format';
import { NetworkForm } from './NetworkForm';
import { PARAM_GROUPS, formatParam } from './params';
import { DurationPicker, TrainingTimer } from './TrainingTimer';

const START_COMMAND = 'docker compose --profile train up';

/** Whether `network` is the one the daemon is training. */
function isTraining(controller: ControllerView | undefined, network: string): boolean {
  return (
    (controller?.state === 'running' || controller?.state === 'stopping') &&
    controller.run === network
  );
}

/** Runs a command, then reports its error and refreshes the view. */
function useAction(refresh: () => void) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const act = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'erreur');
    } finally {
      setBusy(false);
      refresh();
    }
  };
  return { busy, error, act };
}

/** The service is down: what is shown is its last known state, and how to start it. */
function OfflineBanner({ controller }: { controller: ControllerView }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="flex flex-wrap items-center gap-3 rounded-card border border-alert/30 bg-alert/[0.06] px-5 py-3.5">
      <span aria-hidden className="text-alert">
        ⚠
      </span>
      <p className="flex-1 text-sm" title={controller.error ?? undefined}>
        Service d’entraînement injoignable — les données ci-dessous sont le dernier état connu.
      </p>
      <code className="rounded-lg border border-line bg-ink-950 px-3 py-1.5 font-mono text-xs">
        {START_COMMAND}
      </code>
      <Button
        size="sm"
        onClick={() => {
          void navigator.clipboard.writeText(START_COMMAND).then(() => {
            setCopied(true);
          });
        }}
      >
        {copied ? 'Copié' : 'Copier'}
      </Button>
    </div>
  );
}

function NetworkCards({
  view,
  selected,
  creating,
  onSelect,
  onCreate,
}: {
  view: TrainingView;
  selected: string | null;
  creating: boolean;
  onSelect: (name: string) => void;
  onCreate: () => void;
}) {
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(13rem,1fr))] gap-3">
      {view.networks.map((network) => {
        const best = network.best[0];
        const training = isTraining(view.controller, network.name);
        const active = network.name === selected;
        return (
          <button
            key={network.name}
            type="button"
            aria-pressed={active}
            onClick={() => {
              onSelect(network.name);
            }}
            className={`rounded-2xl border p-4 text-left transition-colors ${
              active
                ? 'border-signal/50 bg-ink-800 shadow-[0_0_30px_-12px] shadow-signal/40'
                : 'border-line bg-ink-900 hover:border-ink-600'
            }`}
          >
            <div className="flex items-center justify-between gap-2">
              <span className="flex min-w-0 items-center gap-2 font-semibold">
                {training && <Dot tone="think" />}
                <span className="truncate">{network.name}</span>
              </span>
              <span className="font-mono text-xs text-dim">
                {network.superbatch != null ? `${formatCount(network.superbatch)} sb` : '—'}
              </span>
            </div>
            <p className="mt-2 font-mono text-xl">
              {best ? formatElo(best.elo, best.eloError) : '—'}
            </p>
            <p className="mt-1 truncate text-xs text-dim">
              {best
                ? `meilleur à sb ${best.superbatch}${
                    network.params
                      ? ` · ${formatParam('selfplayNodes', network.params.selfplayNodes)} nœuds`
                      : ''
                  }`
                : training
                  ? 'en cours, pas encore mesuré'
                  : 'jamais mesuré'}
            </p>
          </button>
        );
      })}
      <button
        type="button"
        disabled={creating}
        onClick={onCreate}
        className="flex min-h-28 flex-col items-center justify-center gap-1 rounded-2xl border border-dashed border-ink-600 text-sm text-mute transition-colors hover:border-signal/50 hover:text-bone disabled:opacity-40"
      >
        <span aria-hidden className="text-xl">
          ＋
        </span>
        Nouveau réseau
      </button>
    </div>
  );
}

/** Inline "save as bot" form for one snapshot (or the latest export). */
function SaveBot({
  network,
  superbatch,
  label,
  link = false,
}: {
  network: string;
  superbatch: number | null;
  label: string;
  /** Shows the trigger as a link rather than a button. */
  link?: boolean;
}) {
  const suggested = superbatch === null ? `${network}-latest` : `${network}-sb${superbatch}`;
  const [name, setName] = useState<string | null>(null);
  const [state, setState] = useState<
    | { kind: 'idle' }
    | { kind: 'saving' }
    | { kind: 'saved'; name: string }
    | { kind: 'error'; message: string }
  >({ kind: 'idle' });

  if (state.kind === 'saved') {
    return (
      <span className="text-xs text-signal">
        « {state.name} » sauvegardé —{' '}
        <Link to="/arena" className="underline hover:text-bone">
          voir l’arène
        </Link>
      </span>
    );
  }
  if (name === null) {
    return link ? (
      <button
        type="button"
        className="text-sm font-medium text-signal underline underline-offset-4 hover:text-bone"
        onClick={() => {
          setName(suggested);
        }}
      >
        {label}
      </button>
    ) : (
      <Button
        size="sm"
        onClick={() => {
          setName(suggested);
        }}
      >
        {label}
      </Button>
    );
  }
  const save = async () => {
    setState({ kind: 'saving' });
    try {
      const bot = await api.saveBot({ name, network, superbatch });
      setState({ kind: 'saved', name: bot.name });
    } catch (err: unknown) {
      setState({ kind: 'error', message: err instanceof Error ? err.message : 'erreur' });
    }
  };
  return (
    <form
      className="flex flex-wrap items-center justify-end gap-1"
      onSubmit={(event) => {
        event.preventDefault();
        void save();
      }}
    >
      <input
        value={name}
        onChange={(event) => {
          setName(event.target.value.trim());
        }}
        aria-label="Nom du bot"
        autoFocus
        className={`${INPUT} w-36 px-2 py-1 text-xs`}
      />
      <Button type="submit" variant="primary" size="sm" disabled={!name || state.kind === 'saving'}>
        Sauvegarder
      </Button>
      <Button
        variant="ghost"
        size="sm"
        onClick={() => {
          setName(null);
          setState({ kind: 'idle' });
        }}
      >
        Annuler
      </Button>
      {state.kind === 'error' && (
        <span className="w-full text-right text-xs text-alert">{state.message}</span>
      )}
    </form>
  );
}

/** A snapshot's Elo interval on a shared scale, PeSTO (0) marked. */
function EloRange({
  snapshot,
  domain,
  best,
}: {
  snapshot: RankedSnapshot;
  domain: readonly [number, number];
  best: boolean;
}) {
  const [low, high] = domain;
  const at = (elo: number) => `${((elo - low) / (high - low)) * 100}%`;
  return (
    <div className="relative h-5 min-w-24 flex-1" aria-hidden>
      <span className="absolute inset-x-0 top-1/2 h-px bg-line" />
      <span className="absolute top-0.5 bottom-0.5 w-px bg-dim" style={{ left: at(0) }} />
      <span
        className="absolute top-1/2 h-1.5 -translate-y-1/2 rounded-full bg-signal/30"
        style={{
          left: at(snapshot.elo - snapshot.eloError),
          width: `${((2 * snapshot.eloError) / (high - low)) * 100}%`,
        }}
      />
      <span
        className={`absolute top-1/2 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-ink-900 ${
          best ? 'bg-signal' : 'bg-bone'
        }`}
        style={{ left: at(snapshot.elo) }}
      />
    </div>
  );
}

function BestSnapshots({ network }: { network: NetworkSummary }) {
  const best = network.best;
  const low = Math.min(0, ...best.map((s) => s.elo - s.eloError));
  const high = Math.max(0, ...best.map((s) => s.elo + s.eloError));
  const pad = Math.max(20, (high - low) * 0.05);
  const domain = [low - pad, high + pad] as const;
  return (
    <Card
      title="Top 5 des snapshots"
      aside={best.length > 0 ? `${Math.round(low)} · 0 · +${Math.round(high)}` : undefined}
    >
      {best.length === 0 ? (
        <p className="text-sm text-dim">
          Chaque export joue un match contre PeSTO ; les 5 meilleurs sont gardés ici, les autres
          snapshots sont supprimés.
        </p>
      ) : (
        <ol>
          {best.map((snapshot, rank) => (
            <li
              key={snapshot.superbatch}
              className="flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-line py-3 last:border-b-0"
              title={`${snapshot.games} parties, mesuré le ${formatDate(snapshot.measuredAt)}`}
            >
              <span className="w-4 font-mono text-sm text-dim">{rank + 1}</span>
              <span className="w-14 font-mono text-sm">sb {snapshot.superbatch}</span>
              <EloRange snapshot={snapshot} domain={domain} best={rank === 0} />
              <span className="w-24 text-right font-mono text-sm">
                {formatElo(snapshot.elo, snapshot.eloError)}
              </span>
              <SaveBot
                network={network.name}
                superbatch={snapshot.superbatch}
                label="Sauver comme bot"
              />
            </li>
          ))}
        </ol>
      )}
      {network.hasWeights && (
        <div className="mt-3 flex flex-wrap items-center justify-between gap-2 border-t border-line pt-4 text-sm">
          <span className="text-dim">Dernier export (pas forcément mesuré)</span>
          <SaveBot
            network={network.name}
            superbatch={null}
            label="Sauvegarder la dernière version"
            link
          />
        </div>
      )}
    </Card>
  );
}

function ParamSheet({ network }: { network: NetworkSummary }) {
  const params = network.params;
  return (
    <Card title="Fiche technique · hyperparamètres">
      {!params ? (
        <p className="text-sm text-dim">
          Paramètres enregistrés au premier démarrage depuis l’interface.
        </p>
      ) : (
        <div className="space-y-5">
          {PARAM_GROUPS.map((group) => (
            <dl key={group.title}>
              <dt className="mb-1 text-xs font-semibold text-signal">{group.title}</dt>
              {group.fields.map((field) => (
                <div
                  key={field.key}
                  title={field.help}
                  className="flex justify-between gap-4 border-b border-line py-2 text-sm last:border-b-0"
                >
                  <dt className="text-mute">{field.label}</dt>
                  <dd className="font-mono">{formatParam(field.key, params[field.key])}</dd>
                </div>
              ))}
            </dl>
          ))}
          <div className="flex justify-between gap-4 text-sm">
            <span className="text-mute">Réutilise les parties de</span>
            <span className="font-mono">
              {params.datasetsFrom.length > 0 ? params.datasetsFrom.join(', ') : '—'}
            </span>
          </div>
        </div>
      )}
    </Card>
  );
}

/** Stacked layers of the network: input, accumulator, output. */
function LayerStack() {
  const plane = 'absolute left-1/2 rounded-md border';
  return (
    <div className="relative mx-auto h-40 w-full max-w-64 [perspective:900px]" aria-hidden>
      <div className="absolute inset-0 [transform:rotateX(58deg)_rotateZ(-38deg)] [transform-style:preserve-3d]">
        <div
          className={`${plane} top-[52%] h-28 w-28 -translate-x-1/2 border-ink-600 bg-[repeating-linear-gradient(0deg,#232731_0_1px,transparent_1px_10px),repeating-linear-gradient(90deg,#232731_0_1px,transparent_1px_10px)]`}
        />
        <div
          className={`${plane} top-[22%] h-24 w-24 -translate-x-1/2 border-signal/70 bg-signal/10 bg-[repeating-linear-gradient(90deg,rgba(200,245,96,0.35)_0_1px,transparent_1px_8px)] shadow-[0_0_30px] shadow-signal/20`}
        />
        <div className={`${plane} -top-[6%] h-7 w-7 -translate-x-1/2 border-bone bg-bone`} />
      </div>
    </div>
  );
}

function ArchitectureCard({ view }: { view: TrainingView | null }) {
  const status = view?.status;
  const rows = [
    ['Sortie', 'éval (cp)', false],
    ['Accumulateur', '2 × 256', true],
    ['Entrée', 'pièce × case (768)', false],
  ] as const;
  return (
    <Card title="Architecture · évaluation NNUE" className="flex flex-col">
      <LayerStack />
      <dl className="mt-4 space-y-2 text-sm">
        {rows.map(([label, value, lit]) => (
          <div key={label} className="flex justify-between">
            <dt className={lit ? 'text-signal' : 'text-mute'}>{label}</dt>
            <dd className="font-mono">{value}</dd>
          </div>
        ))}
      </dl>
      <div className="mt-4 grid grid-cols-2 gap-2">
        <div className="col-span-2 rounded-xl border border-line bg-ink-950 p-3">
          <Stat
            label="Device"
            value={status ? deviceName(status.device) : '—'}
            hint={status?.device}
          />
        </div>
        <div className="rounded-xl border border-line bg-ink-950 p-3">
          <Stat
            label="VRAM"
            value={status?.vramUsedMb != null ? formatCount(status.vramUsedMb) : '—'}
            unit={status?.vramUsedMb != null ? 'Mo' : undefined}
          />
        </div>
        <div className="rounded-xl border border-line bg-ink-950 p-3">
          <Stat
            label="Débit"
            value={
              status?.positionsPerSecond != null ? formatCount(status.positionsPerSecond) : '—'
            }
            unit={status?.positionsPerSecond != null ? 'pos/s' : undefined}
            hint="Positions d’entraînement par seconde"
          />
        </div>
      </div>
      <p className="mt-3 text-xs text-dim" title={view?.engine.weightsFile ?? undefined}>
        Le moteur de « Jouer » utilise {view?.engine.evaluator === 'nnue' ? 'le NNUE' : 'PeSTO'}
        {view?.engine.weightsFile ? ` (${view.engine.weightsFile.split('/').pop() ?? ''})` : ''}.
      </p>
    </Card>
  );
}

export function TrainingPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const requested = searchParams.get('network');
  const { view, error, refresh } = useTraining(requested);
  const { busy, error: actionError, act } = useAction(refresh);
  const [creating, setCreating] = useState(false);
  // Time limit of the next start: `null` none, `undefined` invalid custom value.
  const [duration, setDuration] = useState<number | null | undefined>(null);

  const select = (name: string) => {
    setSearchParams({ network: name });
  };

  const controller = view?.controller;
  const state = controller?.state ?? 'unavailable';
  const selected = requested ?? view?.network ?? null;
  const network = view?.networks.find((n) => n.name === selected) ?? null;
  // The curves and status of the view belong to the network it was asked for.
  const current = view && view.network === selected ? view : null;
  const status = current?.status;
  const trainingThis = selected !== null && isTraining(controller, selected);
  const trainingOther = state !== 'idle' && state !== 'unavailable' && !trainingThis;
  const best = network?.best[0];
  const phase = trainingThis
    ? CONTROLLER_LABEL[state]
    : status
      ? (PHASE_LABEL[status.phase] ?? status.phase)
      : 'jamais lancé';

  return (
    <div className="space-y-5">
      {controller && state === 'unavailable' && <OfflineBanner controller={controller} />}
      {error && <ErrorText>{error}</ErrorText>}

      {view ? (
        view.networks.length === 0 && !creating ? (
          <Card>
            <p className="text-sm text-dim">Aucun réseau pour l’instant.</p>
            <Button
              variant="primary"
              className="mt-3"
              onClick={() => {
                setCreating(true);
              }}
            >
              Créer le premier réseau
            </Button>
          </Card>
        ) : (
          <NetworkCards
            view={view}
            selected={selected}
            creating={creating}
            onSelect={select}
            onCreate={() => {
              setCreating(true);
            }}
          />
        )
      ) : (
        !error && <p className="text-sm text-dim">Chargement…</p>
      )}

      {creating && view && (
        <Card title="Nouveau réseau">
          <NetworkForm
            networks={view.networks}
            onCreated={(name) => {
              setCreating(false);
              select(name);
              refresh();
            }}
            onCancel={() => {
              setCreating(false);
            }}
          />
        </Card>
      )}

      {selected && (
        <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_22rem]">
          <section className="relative flex min-w-0 flex-col overflow-hidden rounded-card border border-line bg-ink-900 p-6">
            <div
              aria-hidden
              className="pointer-events-none absolute -top-24 right-0 h-72 w-96 rounded-full bg-signal/[0.04] blur-3xl"
            />
            <div className="relative flex flex-wrap items-start justify-between gap-4">
              <div>
                <Eyebrow>
                  Réseau « {selected} » · {phase}
                </Eyebrow>
                <p className="mt-1.5 text-sm text-mute">
                  {best
                    ? `Meilleur snapshot contre PeSTO (sb ${best.superbatch})`
                    : 'Pas encore de snapshot mesuré contre PeSTO'}
                </p>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <Button
                  variant="primary"
                  disabled={state !== 'idle' || busy || duration === undefined}
                  onClick={() =>
                    void act(() =>
                      api.startTraining({
                        network: selected,
                        fresh: false,
                        maxMinutes: duration ?? null,
                      }),
                    )
                  }
                >
                  ▶ {network?.superbatch ? 'Reprendre' : 'Démarrer'}
                </Button>
                <Button
                  disabled={!trainingThis || state !== 'running' || busy}
                  onClick={() => void act(() => api.stopTraining())}
                >
                  Arrêter
                </Button>
                <Button
                  variant="ghost"
                  disabled={state !== 'idle' || busy || !network?.superbatch}
                  onClick={() => {
                    if (
                      window.confirm(
                        `Repartir de zéro ? Le checkpoint, le top 5 et les snapshots de « ${selected} » seront effacés (ses parties et les bots sauvegardés restent).`,
                      )
                    ) {
                      void act(() =>
                        api.startTraining({
                          network: selected,
                          fresh: true,
                          maxMinutes: duration ?? null,
                        }),
                      );
                    }
                  }}
                >
                  Repartir de zéro
                </Button>
                <Button
                  variant="danger"
                  disabled={trainingThis || busy || state === 'unavailable'}
                  onClick={() => {
                    if (
                      window.confirm(
                        `Supprimer « ${selected} » ? Ses paramètres, son checkpoint et ses poids seront effacés (ses parties et les bots sauvegardés restent).`,
                      )
                    ) {
                      void act(async () => {
                        await api.deleteNetwork(selected);
                        setSearchParams({});
                      });
                    }
                  }}
                >
                  Supprimer
                </Button>
              </div>
            </div>
            {state === 'idle' && (
              <p className="relative mt-3 flex flex-wrap items-center justify-end gap-2 text-sm text-dim">
                pendant
                <DurationPicker value={duration} onChange={setDuration} disabled={busy} />
              </p>
            )}
            {trainingOther && controller?.run && (
              <p className="relative mt-3 text-sm text-dim">
                « {controller.run} » s’entraîne : un seul réseau à la fois (le GPU et le CPU sont
                partagés). Arrêtez-le pour lancer celui-ci.
              </p>
            )}
            {trainingThis && state === 'running' && (
              <TrainingTimer
                deadline={controller?.deadline ?? null}
                busy={busy}
                onSet={(minutes) => void act(() => api.setTrainingTimer(minutes))}
              />
            )}
            {actionError && <ErrorText>{actionError}</ErrorText>}

            <div className="relative my-auto flex flex-wrap items-end gap-x-6 gap-y-3 py-8">
              <span className="font-display text-[clamp(5rem,12vw,9rem)]">
                {best ? `${best.elo >= 0 ? '+' : '−'}${Math.abs(Math.round(best.elo))}` : '—'}
              </span>
              <div className="pb-3">
                {best && (
                  <p className="font-mono text-2xl text-signal">± {Math.round(best.eloError)}</p>
                )}
                <p className="mt-1 max-w-64 text-sm text-mute">
                  {best
                    ? `Elo contre PeSTO, intervalle à 95 %, mesuré sur ${best.games} parties le ${formatDate(best.measuredAt).replace(' ', ' à ')}.`
                    : 'Chaque export de poids est mesuré contre l’évaluation classique.'}
                </p>
              </div>
            </div>

            <div className="relative grid grid-cols-2 gap-y-4 sm:grid-cols-4">
              {[
                ['Superbatchs', status?.superbatch ?? network?.superbatch ?? '—'],
                ['Parties de self-play', status ? formatCount(status.games) : '—'],
                ['Positions générées', status ? formatCount(status.positions) : '—'],
                ['Perte', status?.loss != null ? status.loss.toFixed(5) : '—'],
              ].map(([label, value], i) => (
                <div
                  key={label}
                  className={`border-line px-4 ${i % 2 === 1 ? 'border-l' : ''} ${i === 2 ? 'sm:border-l' : ''}`}
                >
                  <Stat
                    label={String(label)}
                    value={value}
                    large
                    hint={
                      status && i === 0
                        ? `phase depuis ${formatElapsed(status.startedAt, status.updatedAt)}`
                        : undefined
                    }
                  />
                </div>
              ))}
            </div>
          </section>

          <ArchitectureCard view={current} />
        </div>
      )}

      {selected && (
        <div className="grid gap-5 lg:grid-cols-2">
          <Card title="Elo contre PeSTO · IC 95 %" aside="par snapshot">
            {current && current.elo.length > 0 ? (
              <>
                <EloChart points={current.elo} />
                <DataTable
                  headers={['Superbatch', 'Elo', 'Parties']}
                  rows={current.elo.map((p) => [p.superbatch, formatElo(p.elo, p.error), p.games])}
                />
              </>
            ) : (
              <p className="text-sm text-dim">
                Chaque export de poids est mesuré contre l’évaluation classique : la courbe apparaît
                après le premier.
              </p>
            )}
          </Card>
          <Card
            title="Perte par superbatch"
            aside={
              current && current.loss.length > 0
                ? `dernière ${current.loss[current.loss.length - 1]?.loss.toFixed(5) ?? ''}`
                : undefined
            }
          >
            {current && current.loss.length > 0 ? (
              <>
                <LossChart points={current.loss} />
                <DataTable
                  headers={['Superbatch', 'Perte', 'Positions/s']}
                  rows={current.loss.map((p) => [
                    p.superbatch,
                    p.loss.toFixed(5),
                    formatCount(p.positionsPerSecond),
                  ])}
                />
              </>
            ) : (
              <p className="text-sm text-dim">Pas encore de superbatch terminé.</p>
            )}
          </Card>
        </div>
      )}

      {network && (
        <div className="grid gap-5 lg:grid-cols-[minmax(0,1.15fr)_minmax(0,1fr)]">
          <BestSnapshots network={network} />
          <ParamSheet network={network} />
        </div>
      )}
    </div>
  );
}
