import { useState } from 'react';
import { Link, useSearchParams } from 'react-router';

import { api } from '../../api/client';
import type { ControllerView } from '../../api/generated/ControllerView';
import type { NetworkSummary } from '../../api/generated/NetworkSummary';
import type { TrainingView } from '../../api/generated/TrainingView';
import { useTraining } from '../../api/hooks';
import { Button, ErrorText, Panel, Stat } from '../../components/ui';
import { DataTable, EloChart, LossChart } from './charts';
import {
  CONTROLLER_LABEL,
  PHASE_LABEL,
  formatCount,
  formatDate,
  formatElapsed,
  formatElo,
} from './format';
import { NetworkForm } from './NetworkForm';
import { PARAM_FIELDS, formatParam } from './params';
import { DurationPicker, TrainingTimer } from './TrainingTimer';

const STATE_DOT: Record<string, string> = {
  running: 'bg-emerald-400',
  stopping: 'bg-amber-400',
  idle: 'bg-slate-400',
  unavailable: 'bg-rose-500',
};

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

function NetworkList({
  view,
  selected,
  onSelect,
}: {
  view: TrainingView;
  selected: string | null;
  onSelect: (name: string) => void;
}) {
  if (view.networks.length === 0) {
    return (
      <p className="text-sm text-slate-400">
        Aucun réseau pour l’instant : crée le premier avec « Nouveau réseau ».
      </p>
    );
  }
  return (
    <div className="-mx-4 overflow-x-auto px-4">
      <table className="w-full min-w-[36rem] text-sm">
        <thead>
          <tr className="text-left text-xs text-slate-500">
            <th className="py-1.5 font-normal">Réseau</th>
            <th className="py-1.5 text-right font-normal">Superbatchs</th>
            <th className="py-1.5 text-right font-normal">Meilleur Elo vs PeSTO</th>
            <th className="py-1.5 text-right font-normal">Nœuds self-play</th>
            <th className="py-1.5 text-right font-normal">Taux d’app.</th>
            <th className="py-1.5 text-right font-normal">Batch</th>
          </tr>
        </thead>
        <tbody>
          {view.networks.map((network) => {
            const best = network.best[0];
            const training = isTraining(view.controller, network.name);
            return (
              <tr
                key={network.name}
                onClick={() => {
                  onSelect(network.name);
                }}
                className={`cursor-pointer border-t border-slate-800 ${
                  network.name === selected ? 'bg-slate-800/70' : 'hover:bg-slate-800/40'
                }`}
              >
                <td className="py-2 pr-3">
                  {/* Focusable; its click (mouse or keyboard) bubbles to the row. */}
                  <button
                    type="button"
                    className="flex items-center gap-2 font-medium text-slate-100"
                  >
                    <span
                      aria-hidden
                      className={`inline-block size-2 rounded-full ${training ? 'bg-emerald-400' : 'bg-slate-600'}`}
                    />
                    {network.name}
                    {training && (
                      <span className="text-xs font-normal text-emerald-400">en cours</span>
                    )}
                  </button>
                </td>
                <td className="py-2 text-right font-mono">{network.superbatch ?? '—'}</td>
                <td className="py-2 text-right font-mono">
                  {best ? `${formatElo(best.elo, best.eloError)} (sb ${best.superbatch})` : '—'}
                </td>
                <td className="py-2 text-right font-mono">
                  {network.params
                    ? formatParam('selfplayNodes', network.params.selfplayNodes)
                    : '—'}
                </td>
                <td className="py-2 text-right font-mono">
                  {network.params ? formatParam('learningRate', network.params.learningRate) : '—'}
                </td>
                <td className="py-2 text-right font-mono">
                  {network.params ? formatParam('batchSize', network.params.batchSize) : '—'}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/** Inline "save as bot" form for one snapshot (or the latest export). */
function SaveBot({
  network,
  superbatch,
  label,
}: {
  network: string;
  superbatch: number | null;
  label: string;
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
      <span className="text-xs text-emerald-400">
        Bot « {state.name} » sauvegardé —{' '}
        <Link to="/arena" className="underline hover:text-emerald-300">
          voir l’arène
        </Link>
      </span>
    );
  }
  if (name === null) {
    return (
      <Button
        variant="ghost"
        className="px-2 py-1 text-xs"
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
        className="w-36 rounded-md border border-slate-700 bg-slate-950 px-2 py-1 font-mono text-xs"
      />
      <Button
        type="submit"
        variant="primary"
        className="px-2 py-1 text-xs"
        disabled={!name || state.kind === 'saving'}
      >
        Sauvegarder
      </Button>
      <Button
        variant="ghost"
        className="px-2 py-1 text-xs"
        onClick={() => {
          setName(null);
          setState({ kind: 'idle' });
        }}
      >
        Annuler
      </Button>
      {state.kind === 'error' && (
        <span className="w-full text-right text-xs text-rose-400">{state.message}</span>
      )}
    </form>
  );
}

function BestSnapshots({ network }: { network: NetworkSummary }) {
  return (
    <>
      {network.best.length === 0 ? (
        <p className="text-sm text-slate-400">
          Chaque export joue un match contre PeSTO ; les 5 meilleurs sont gardés ici, les autres
          snapshots sont supprimés.
        </p>
      ) : (
        <div className="-mx-4 overflow-x-auto px-4">
          <table className="w-full min-w-[32rem] text-sm">
            <thead>
              <tr className="text-left text-xs text-slate-500">
                <th className="py-1.5 pr-3 font-normal">#</th>
                <th className="py-1.5 font-normal">Superbatch</th>
                <th className="py-1.5 text-right font-normal">Elo vs PeSTO</th>
                <th className="py-1.5 text-right font-normal">Parties</th>
                <th className="py-1.5 text-right font-normal">Mesuré le</th>
                <th className="py-1.5" />
              </tr>
            </thead>
            <tbody>
              {network.best.map((snapshot, rank) => (
                <tr key={snapshot.superbatch} className="border-t border-slate-800">
                  <td className="py-2 pr-3 font-mono text-slate-400">{rank + 1}</td>
                  <td className="py-2 font-mono">{snapshot.superbatch}</td>
                  <td className="py-2 text-right font-mono">
                    {formatElo(snapshot.elo, snapshot.eloError)}
                  </td>
                  <td className="py-2 text-right font-mono">{snapshot.games}</td>
                  <td className="py-2 text-right text-slate-400">
                    {formatDate(snapshot.measuredAt)}
                  </td>
                  <td className="py-2 text-right">
                    <SaveBot
                      network={network.name}
                      superbatch={snapshot.superbatch}
                      label="Sauvegarder comme bot"
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {network.hasWeights && (
        <div className="mt-3 flex items-center justify-between gap-2 border-t border-slate-800 pt-3 text-sm">
          <span className="text-slate-400">Dernier export (pas forcément mesuré)</span>
          <SaveBot
            network={network.name}
            superbatch={null}
            label="Sauvegarder la dernière version"
          />
        </div>
      )}
    </>
  );
}

function NetworkParamsView({ network }: { network: NetworkSummary }) {
  const params = network.params;
  if (!params) {
    return (
      <p className="text-sm text-slate-400">
        Paramètres enregistrés au premier démarrage depuis l’interface.
      </p>
    );
  }
  return (
    <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-sm sm:grid-cols-3">
      {PARAM_FIELDS.map((field) => (
        <div key={field.key} title={field.help}>
          <dt className="text-xs text-slate-500">{field.label}</dt>
          <dd className="font-mono text-slate-200">{formatParam(field.key, params[field.key])}</dd>
        </div>
      ))}
      <div>
        <dt className="text-xs text-slate-500">Réutilise les parties de</dt>
        <dd className="font-mono text-slate-200">
          {params.datasetsFrom.length > 0 ? params.datasetsFrom.join(', ') : '—'}
        </dd>
      </div>
    </dl>
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

  return (
    <div className="space-y-4">
      <Panel title="Réseaux de neurones">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
          <p className="flex items-center gap-2 text-sm">
            <span
              aria-hidden
              className={`inline-block size-2.5 rounded-full ${STATE_DOT[state] ?? ''}`}
            />
            {CONTROLLER_LABEL[state]}
            {(state === 'running' || state === 'stopping') && controller?.run && (
              <span className="text-slate-400">— « {controller.run} »</span>
            )}
          </p>
          <Button
            variant="primary"
            disabled={creating}
            onClick={() => {
              setCreating(true);
            }}
          >
            Nouveau réseau
          </Button>
        </div>
        {controller?.error && <p className="mb-3 text-sm text-slate-400">{controller.error}</p>}
        {error && <ErrorText>{error}</ErrorText>}
        {view ? (
          <NetworkList view={view} selected={selected} onSelect={select} />
        ) : (
          <p className="text-sm text-slate-400">Chargement…</p>
        )}
      </Panel>

      {creating && view && (
        <Panel title="Nouveau réseau">
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
        </Panel>
      )}

      {selected && (
        <Panel title={`Réseau « ${selected} »`}>
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
              {network?.superbatch ? 'Reprendre l’entraînement' : 'Démarrer l’entraînement'}
            </Button>
            {state === 'idle' && (
              <span className="inline-flex items-center gap-2 text-sm text-slate-400">
                pendant
                <DurationPicker value={duration} onChange={setDuration} disabled={busy} />
              </span>
            )}
            <Button
              disabled={!trainingThis || state !== 'running' || busy}
              onClick={() => void act(() => api.stopTraining())}
            >
              Arrêter
            </Button>
            <span className="flex-1" />
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
              variant="ghost"
              className="text-rose-400 hover:text-rose-300"
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
          {trainingOther && controller?.run && (
            <p className="mt-2 text-sm text-slate-400">
              « {controller.run} » s’entraîne : un seul réseau à la fois (le GPU et le CPU sont
              partagés). Arrête-le pour lancer celui-ci.
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

          <div className="mt-4 grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-5">
            <Stat
              label="Phase"
              value={status ? (PHASE_LABEL[status.phase] ?? status.phase) : '—'}
              hint={
                status ? `depuis ${formatElapsed(status.startedAt, status.updatedAt)}` : undefined
              }
            />
            <Stat label="Superbatchs" value={status?.superbatch ?? network?.superbatch ?? '—'} />
            <Stat label="Parties de self-play" value={status ? formatCount(status.games) : '—'} />
            <Stat label="Positions générées" value={status ? formatCount(status.positions) : '—'} />
            <Stat label="Perte" value={status?.loss != null ? status.loss.toFixed(5) : '—'} />
            <Stat
              label="Vitesse"
              value={
                status?.positionsPerSecond != null
                  ? `${formatCount(status.positionsPerSecond)}/s`
                  : '—'
              }
            />
            <Stat label="Device" value={status?.device ?? '—'} hint={status?.device} />
            <Stat
              label="VRAM"
              value={status?.vramUsedMb != null ? `${formatCount(status.vramUsedMb)} Mo` : '—'}
            />
            <Stat
              label="Dernier Elo vs PeSTO"
              value={status?.elo != null ? formatElo(status.elo, status.eloError ?? 0) : '—'}
            />
            <Stat
              label="Évaluation du moteur (Jouer)"
              value={view?.engine.evaluator === 'nnue' ? 'NNUE' : 'PeSTO'}
              hint={view?.engine.weightsFile ?? 'pas de réseau chargé'}
            />
          </div>
        </Panel>
      )}

      {network && (
        <div className="grid gap-4 lg:grid-cols-2">
          <Panel title="Top 5 des snapshots (Elo contre PeSTO)">
            <BestSnapshots network={network} />
          </Panel>
          <Panel title="Hyperparamètres">
            <NetworkParamsView network={network} />
          </Panel>
        </div>
      )}

      {selected && (
        <div className="grid gap-4 lg:grid-cols-2">
          <Panel title="Perte d’entraînement par superbatch">
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
              <p className="text-sm text-slate-400">Pas encore de superbatch terminé.</p>
            )}
          </Panel>
          <Panel title="Elo contre PeSTO (intervalle à 95 %)">
            {current && current.elo.length > 0 ? (
              <>
                <EloChart points={current.elo} />
                <DataTable
                  headers={['Superbatch', 'Elo', 'Parties']}
                  rows={current.elo.map((p) => [p.superbatch, formatElo(p.elo, p.error), p.games])}
                />
              </>
            ) : (
              <p className="text-sm text-slate-400">
                Chaque export de poids est mesuré contre l’évaluation classique : la courbe apparaît
                après le premier.
              </p>
            )}
          </Panel>
        </div>
      )}
    </div>
  );
}
