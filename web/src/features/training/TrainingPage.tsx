import { type ReactNode, useState } from 'react';
import {
  Area,
  CartesianGrid,
  ComposedChart,
  Line,
  LineChart,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';

import { api } from '../../api/client';
import type { EloPoint } from '../../api/generated/EloPoint';
import type { LossPoint } from '../../api/generated/LossPoint';
import { useTraining } from '../../api/hooks';
import { Button, ErrorText, Panel } from '../../components/ui';
import {
  CONTROLLER_LABEL,
  PHASE_LABEL,
  eloBand,
  formatCount,
  formatElapsed,
  formatElo,
} from './format';

/** Categorical slot 1 (dark step), validated on the panel surface. */
const SERIES = '#3987e5';
const GRID = '#334155';
const AXIS = '#94a3b8';

const STATE_DOT: Record<string, string> = {
  running: 'bg-emerald-400',
  stopping: 'bg-amber-400',
  idle: 'bg-slate-400',
  unavailable: 'bg-rose-500',
};

function Stat({ label, value, hint }: { label: string; value: ReactNode; hint?: string }) {
  return (
    <div className="rounded-lg bg-slate-950/60 px-3 py-2">
      <p className="text-xs text-slate-400">{label}</p>
      <p className="mt-0.5 truncate font-mono text-lg text-slate-100" title={hint}>
        {value}
      </p>
    </div>
  );
}

function ChartTooltip({
  active,
  payload,
  render,
}: {
  active?: boolean;
  payload?: readonly { payload?: unknown }[];
  render: (point: never) => ReactNode;
}) {
  const point = payload?.[0]?.payload;
  if (!active || !point) return null;
  return (
    <div className="rounded-md border border-slate-700 bg-slate-900 px-3 py-2 text-xs text-slate-200 shadow-lg">
      {render(point as never)}
    </div>
  );
}

function LossChart({ points }: { points: readonly LossPoint[] }) {
  return (
    <ResponsiveContainer width="100%" height={240}>
      <LineChart data={[...points]} margin={{ top: 8, right: 16, bottom: 4, left: 0 }}>
        <CartesianGrid stroke={GRID} strokeDasharray="2 4" vertical={false} />
        <XAxis dataKey="superbatch" stroke={AXIS} tickLine={false} fontSize={12} />
        <YAxis stroke={AXIS} tickLine={false} fontSize={12} width={56} domain={['auto', 'auto']} />
        <Tooltip
          cursor={{ stroke: AXIS, strokeDasharray: '3 3' }}
          content={(props) => (
            <ChartTooltip
              {...props}
              render={(point: LossPoint) => (
                <>
                  <p className="font-semibold">Superbatch {point.superbatch}</p>
                  <p>Perte : {point.loss.toFixed(5)}</p>
                  <p>Taux d’apprentissage : {point.learningRate.toExponential(1)}</p>
                  <p>{formatCount(point.positionsPerSecond)} positions/s</p>
                </>
              )}
            />
          )}
        />
        <Line
          type="monotone"
          dataKey="loss"
          stroke={SERIES}
          strokeWidth={2}
          dot={points.length < 40 ? { r: 3, fill: SERIES } : false}
          activeDot={{ r: 5 }}
          isAnimationActive={false}
        />
      </LineChart>
    </ResponsiveContainer>
  );
}

function EloChart({ points }: { points: readonly EloPoint[] }) {
  return (
    <ResponsiveContainer width="100%" height={240}>
      <ComposedChart data={eloBand(points)} margin={{ top: 8, right: 16, bottom: 4, left: 0 }}>
        <CartesianGrid stroke={GRID} strokeDasharray="2 4" vertical={false} />
        <XAxis dataKey="superbatch" stroke={AXIS} tickLine={false} fontSize={12} />
        <YAxis stroke={AXIS} tickLine={false} fontSize={12} width={56} domain={['auto', 'auto']} />
        <ReferenceLine
          y={0}
          stroke={AXIS}
          strokeDasharray="4 4"
          label={{ value: 'PeSTO', position: 'insideTopRight', fill: AXIS, fontSize: 11 }}
        />
        <Tooltip
          cursor={{ stroke: AXIS, strokeDasharray: '3 3' }}
          content={(props) => (
            <ChartTooltip
              {...props}
              render={(point: ReturnType<typeof eloBand>[number]) => (
                <>
                  <p className="font-semibold">Superbatch {point.superbatch}</p>
                  <p>Elo vs PeSTO : {formatElo(point.elo, point.error)}</p>
                  <p>{point.games} parties</p>
                </>
              )}
            />
          )}
        />
        <Area
          type="monotone"
          dataKey="band"
          stroke="none"
          fill={SERIES}
          fillOpacity={0.15}
          isAnimationActive={false}
        />
        <Line
          type="monotone"
          dataKey="elo"
          stroke={SERIES}
          strokeWidth={2}
          dot={{ r: 4, fill: SERIES }}
          activeDot={{ r: 6 }}
          isAnimationActive={false}
        />
      </ComposedChart>
    </ResponsiveContainer>
  );
}

/** Accessible table view of a chart's data. */
function DataTable({ headers, rows }: { headers: string[]; rows: (string | number)[][] }) {
  if (rows.length === 0) return null;
  return (
    <details className="mt-2 text-xs text-slate-400">
      <summary className="cursor-pointer hover:text-slate-200">Voir les données</summary>
      <div className="mt-2 max-h-48 overflow-y-auto">
        <table className="w-full font-mono">
          <thead>
            <tr>
              {headers.map((header) => (
                <th key={header} className="py-1 text-left font-normal text-slate-500">
                  {header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={String(row[0])} className="text-slate-300">
                {row.map((cell, i) => (
                  <td key={i} className="py-0.5">
                    {cell}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </details>
  );
}

export function TrainingPage() {
  const { view, error, refresh } = useTraining();
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const act = async (action: () => Promise<void>) => {
    setBusy(true);
    setActionError(null);
    try {
      await action();
    } catch (err: unknown) {
      setActionError(err instanceof Error ? err.message : 'erreur');
    } finally {
      setBusy(false);
      refresh();
    }
  };

  const controller = view?.controller;
  const status = view?.status;
  const state = controller?.state ?? 'unavailable';
  const canStart = state === 'idle';

  return (
    <div className="space-y-4">
      <Panel title="Entraînement NNUE">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="flex items-center gap-2 text-base font-medium">
              <span
                aria-hidden
                className={`inline-block size-2.5 rounded-full ${STATE_DOT[state] ?? ''}`}
              />
              {CONTROLLER_LABEL[state]}
              {controller?.run && (
                <span className="text-sm text-slate-400">— run « {controller.run} »</span>
              )}
            </p>
            {controller?.error && <p className="mt-1 text-sm text-slate-400">{controller.error}</p>}
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              variant="primary"
              disabled={!canStart || busy}
              onClick={() => void act(() => api.startTraining({ fresh: false }))}
            >
              Démarrer / reprendre
            </Button>
            <Button
              disabled={!canStart || busy}
              onClick={() => {
                if (
                  window.confirm(
                    'Repartir d’un réseau vierge ? Le checkpoint actuel sera remplacé.',
                  )
                ) {
                  void act(() => api.startTraining({ fresh: true }));
                }
              }}
            >
              Nouveau réseau
            </Button>
            <Button
              disabled={state !== 'running' || busy}
              onClick={() => void act(() => api.stopTraining())}
            >
              Arrêter
            </Button>
          </div>
        </div>
        {(error ?? actionError) && <ErrorText>{actionError ?? error}</ErrorText>}

        <div className="mt-4 grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-5">
          <Stat label="Device" value={status?.device ?? '—'} hint={status?.device} />
          <Stat
            label="Phase"
            value={status ? (PHASE_LABEL[status.phase] ?? status.phase) : '—'}
            hint={
              status ? `depuis ${formatElapsed(status.startedAt, status.updatedAt)}` : undefined
            }
          />
          <Stat label="Parties de self-play" value={status ? formatCount(status.games) : '—'} />
          <Stat label="Positions générées" value={status ? formatCount(status.positions) : '—'} />
          <Stat label="Superbatchs" value={status ? status.superbatch : '—'} />
          <Stat label="Perte" value={status?.loss != null ? status.loss.toFixed(5) : '—'} />
          <Stat
            label="Vitesse"
            value={
              status?.positionsPerSecond != null
                ? `${formatCount(status.positionsPerSecond)}/s`
                : '—'
            }
          />
          <Stat
            label="VRAM"
            value={status?.vramUsedMb != null ? `${formatCount(status.vramUsedMb)} Mo` : '—'}
          />
          <Stat
            label="Elo vs PeSTO"
            value={status?.elo != null ? formatElo(status.elo, status.eloError ?? 0) : '—'}
          />
          <Stat
            label="Évaluation du moteur"
            value={view?.engine.evaluator === 'nnue' ? 'NNUE' : 'PeSTO'}
            hint={view?.engine.weightsFile ?? 'pas de réseau chargé'}
          />
        </div>
      </Panel>

      <div className="grid gap-4 lg:grid-cols-2">
        <Panel title="Perte d’entraînement par superbatch">
          {view && view.loss.length > 0 ? (
            <>
              <LossChart points={view.loss} />
              <DataTable
                headers={['Superbatch', 'Perte', 'Positions/s']}
                rows={view.loss.map((p) => [
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
          {view && view.elo.length > 0 ? (
            <>
              <EloChart points={view.elo} />
              <DataTable
                headers={['Superbatch', 'Elo', 'Parties']}
                rows={view.elo.map((p) => [p.superbatch, formatElo(p.elo, p.error), p.games])}
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
    </div>
  );
}
