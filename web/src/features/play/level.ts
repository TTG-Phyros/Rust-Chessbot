import type { EngineLimits } from '../../api/generated/EngineLimits';

export type LevelMode = 'depth' | 'time';

export type Level = {
  mode: LevelMode;
  /** Plies, when `mode` is `depth`. */
  depth: number;
  /** Milliseconds per move, when `mode` is `time`. */
  timeMs: number;
};

export const DEFAULT_LEVEL: Level = { mode: 'depth', depth: 4, timeMs: 1000 };

export const DEPTH_RANGE = { min: 1, max: 20 } as const;
export const TIME_RANGE = { min: 100, max: 10_000, step: 100 } as const;

/** Search limits sent to the server for this level. */
export function levelLimits(level: Level): EngineLimits {
  return level.mode === 'depth'
    ? { depth: clamp(level.depth, DEPTH_RANGE.min, DEPTH_RANGE.max) }
    : { movetimeMs: clamp(level.timeMs, TIME_RANGE.min, TIME_RANGE.max) };
}

/** Short French label: `profondeur 4`, `1,5 s / coup`. */
export function levelLabel(level: Level): string {
  if (level.mode === 'depth') return `profondeur ${level.depth}`;
  const seconds = (level.timeMs / 1000).toLocaleString('fr-FR', { maximumFractionDigits: 1 });
  return `${seconds} s / coup`;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, Math.round(value)));
}
