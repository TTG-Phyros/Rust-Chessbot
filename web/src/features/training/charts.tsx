/** Training charts: loss and Elo per superbatch, each with a table view. */
import type { ReactNode } from 'react';
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

import type { EloPoint } from '../../api/generated/EloPoint';
import type { LossPoint } from '../../api/generated/LossPoint';
import { eloBand, formatCount, formatElo } from './format';

/** Color of the data series (lines and Elo band), readable on the dark panels. */
const SERIES = '#3987e5';
const GRID = '#334155';
const AXIS = '#94a3b8';

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

export function LossChart({ points }: { points: readonly LossPoint[] }) {
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

export function EloChart({ points }: { points: readonly EloPoint[] }) {
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
          label={{ value: 'PeSTO', position: 'insideTopLeft', fill: AXIS, fontSize: 11 }}
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
export function DataTable({ headers, rows }: { headers: string[]; rows: (string | number)[][] }) {
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
