/** Training charts: loss and Elo per superbatch, each with a table view. */
import type { ReactNode } from 'react';
import {
  Area,
  CartesianGrid,
  ComposedChart,
  Line,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';

import type { EloPoint } from '../../api/generated/EloPoint';
import type { LossPoint } from '../../api/generated/LossPoint';
import { eloBand, formatCount, formatElo } from './format';

/** Elo is our bots' strength (signal); the loss is the engine learning (think). */
const ELO = '#c8f560';
const LOSS = '#9d8cff';
const GRID = '#1c2029';
const AXIS = '#8a909c';
const TICK = { fill: AXIS, fontSize: 11, fontFamily: 'JetBrains Mono, monospace' };

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
    <div className="rounded-xl border border-line bg-ink-800 px-3 py-2 font-mono text-xs text-bone shadow-lg">
      {render(point as never)}
    </div>
  );
}

export function LossChart({ points }: { points: readonly LossPoint[] }) {
  return (
    <ResponsiveContainer width="100%" height={240}>
      <ComposedChart data={[...points]} margin={{ top: 8, right: 8, bottom: 4, left: 0 }}>
        <defs>
          <linearGradient id="loss-fill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={LOSS} stopOpacity={0.25} />
            <stop offset="100%" stopColor={LOSS} stopOpacity={0} />
          </linearGradient>
        </defs>
        <CartesianGrid stroke={GRID} vertical={false} />
        <XAxis dataKey="superbatch" stroke={GRID} tick={TICK} tickLine={false} />
        <YAxis stroke={GRID} tick={TICK} tickLine={false} width={64} domain={['auto', 'auto']} />
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
        <Area
          type="monotone"
          dataKey="loss"
          stroke={LOSS}
          strokeWidth={2}
          fill="url(#loss-fill)"
          dot={false}
          activeDot={{ r: 5, fill: LOSS, stroke: '#0d0f14' }}
          isAnimationActive={false}
        />
      </ComposedChart>
    </ResponsiveContainer>
  );
}

export function EloChart({ points }: { points: readonly EloPoint[] }) {
  return (
    <ResponsiveContainer width="100%" height={240}>
      <ComposedChart data={eloBand(points)} margin={{ top: 8, right: 8, bottom: 4, left: 0 }}>
        <CartesianGrid stroke={GRID} vertical={false} />
        <XAxis dataKey="superbatch" stroke={GRID} tick={TICK} tickLine={false} />
        <YAxis stroke={GRID} tick={TICK} tickLine={false} width={48} domain={['auto', 'auto']} />
        <ReferenceLine
          y={0}
          stroke={AXIS}
          strokeDasharray="4 4"
          label={{ value: 'PeSTO = 0', position: 'insideBottomLeft', ...TICK }}
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
          type="linear"
          dataKey="band"
          stroke={ELO}
          strokeOpacity={0.2}
          fill={ELO}
          fillOpacity={0.1}
          isAnimationActive={false}
        />
        <Line
          type="linear"
          dataKey="elo"
          stroke={ELO}
          strokeWidth={2}
          dot={points.length < 40 ? { r: 4, fill: '#0d0f14', stroke: ELO, strokeWidth: 2 } : false}
          activeDot={{ r: 6, fill: ELO }}
          isAnimationActive={false}
        />
      </ComposedChart>
    </ResponsiveContainer>
  );
}

/** Accessible table view of a chart's data. */
export function DataTable({ headers, rows }: { headers: string[]; rows: (string | number)[][] }) {
  if (rows.length === 0) return null;
  return (
    <details className="mt-2 text-xs text-dim">
      <summary className="cursor-pointer hover:text-bone">Voir les données</summary>
      <div className="mt-2 max-h-48 overflow-y-auto">
        <table className="w-full font-mono">
          <thead>
            <tr>
              {headers.map((header) => (
                <th key={header} className="py-1 text-left font-normal text-dim">
                  {header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={String(row[0])} className="text-mute">
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
