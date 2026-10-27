import type { Side } from '../../api/generated/Side';

/** `04:47` (minutes and seconds), `1:02:05` past an hour; with `hundredths`,
 * `00:00.04` under an hour (a bot thinks in milliseconds). */
export function formatClock(ms: number, hundredths = false): string {
  const clamped = Math.max(0, ms);
  const total = Math.floor(clamped / 1000);
  const seconds = String(total % 60).padStart(2, '0');
  const minutes = Math.floor(total / 60);
  if (minutes >= 60) {
    return `${Math.floor(minutes / 60)}:${String(minutes % 60).padStart(2, '0')}:${seconds}`;
  }
  const clock = `${String(minutes).padStart(2, '0')}:${seconds}`;
  return hundredths
    ? `${clock}.${String(Math.floor((clamped % 1000) / 10)).padStart(2, '0')}`
    : clock;
}

/**
 * Time spent by each side, and the increments earned. Without a time
 * control the clocks count up; with one, they count down from it.
 */
export type Clocks = {
  used: Record<Side, number>;
  earned: Record<Side, number>;
  /** Side whose clock runs, if any. */
  running: Side | null;
  /** When `running`'s clock started (ms since the epoch). */
  since: number;
};

const other = (side: Side): Side => (side === 'white' ? 'black' : 'white');

/** Fresh clocks, `running`'s started at `now`. */
export function startClocks(now: number, running: Side | null): Clocks {
  return { used: { white: 0, black: 0 }, earned: { white: 0, black: 0 }, running, since: now };
}

/** Time `side` has spent so far, the move in progress included. */
export function timeUsed(clocks: Clocks, side: Side, now: number): number {
  return clocks.used[side] + (clocks.running === side ? Math.max(0, now - clocks.since) : 0);
}

/** Time left to `side` on a clock of `baseMs`. */
export function timeLeft(clocks: Clocks, side: Side, now: number, baseMs: number): number {
  return baseMs + clocks.earned[side] - timeUsed(clocks, side, now);
}

/** `side` moved at `now`: its clock stops (plus `incrementMs`), the other one starts. */
export function afterMove(clocks: Clocks, side: Side, now: number, incrementMs: number): Clocks {
  return {
    used: { ...clocks.used, [side]: timeUsed(clocks, side, now) },
    earned: { ...clocks.earned, [side]: clocks.earned[side] + incrementMs },
    running: other(side),
    since: now,
  };
}

/** Stops the clocks at `now` (game over). */
export function stopClocks(clocks: Clocks, now: number): Clocks {
  if (clocks.running === null) return clocks;
  return {
    ...clocks,
    used: { ...clocks.used, [clocks.running]: timeUsed(clocks, clocks.running, now) },
    running: null,
    since: now,
  };
}

/** Starts the clock of `side` at `now` if none runs (a move taken back). */
export function resumeClocks(clocks: Clocks, side: Side, now: number): Clocks {
  if (clocks.running === side) return clocks;
  return { ...stopClocks(clocks, now), running: side, since: now };
}
