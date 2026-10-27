import type { EngineLimits } from '../../api/generated/EngineLimits';

/** Fixed depth, fixed time per move, or a real game with clocks. */
export type LevelMode = 'depth' | 'time' | 'clock';

/** A time control: minutes for the game, seconds added after each move. */
export type Cadence = { minutes: number; incrementS: number };

export const CADENCES: readonly Cadence[] = [
  { minutes: 1, incrementS: 0 },
  { minutes: 3, incrementS: 0 },
  { minutes: 3, incrementS: 2 },
  { minutes: 5, incrementS: 0 },
  { minutes: 10, incrementS: 0 },
  { minutes: 15, incrementS: 10 },
];

export type Level = {
  mode: LevelMode;
  /** Plies, when `mode` is `depth`. */
  depth: number;
  /** Milliseconds per move, when `mode` is `time`. */
  timeMs: number;
  /** Time control, when `mode` is `clock`. */
  cadence: Cadence;
};

export const DEFAULT_LEVEL: Level = {
  mode: 'depth',
  depth: 4,
  timeMs: 1000,
  cadence: { minutes: 3, incrementS: 2 },
};

export const DEPTH_RANGE = { min: 1, max: 20 } as const;
export const TIME_RANGE = { min: 100, max: 10_000, step: 100 } as const;

/**
 * Search limits sent to the server for this level. With clocks, none: the
 * caller sends the clocks themselves.
 */
export function levelLimits(level: Level): EngineLimits {
  switch (level.mode) {
    case 'depth':
      return { depth: clamp(level.depth, DEPTH_RANGE.min, DEPTH_RANGE.max) };
    case 'time':
      return { movetimeMs: clamp(level.timeMs, TIME_RANGE.min, TIME_RANGE.max) };
    case 'clock':
      return {};
  }
}

/** `3 + 2`: minutes and increment of a cadence. */
export function cadenceLabel(cadence: Cadence): string {
  return `${cadence.minutes} + ${cadence.incrementS}`;
}

/** Short French label: `profondeur 4`, `1,5 s / coup`, `pendule 3 + 2`. */
export function levelLabel(level: Level): string {
  switch (level.mode) {
    case 'depth':
      return `profondeur ${level.depth}`;
    case 'time': {
      const seconds = (level.timeMs / 1000).toLocaleString('fr-FR', { maximumFractionDigits: 1 });
      return `${seconds} s / coup`;
    }
    case 'clock':
      return `pendule ${cadenceLabel(level.cadence)}`;
  }
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, Math.round(value)));
}
