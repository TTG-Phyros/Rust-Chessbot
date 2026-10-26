/**
 * How long a tournament will take, from measurements on the reference
 * machine (i7-9700KF, one thread per game). An order of magnitude, not a
 * promise: game lengths vary, and so does the time a depth takes.
 */
import type { MoveLimit } from '../../api/generated/MoveLimit';
import { type Pair, allPairs } from './pairings';
import { stockfishLevel } from './stockfish';

/** Plies of an average game, adjudication included (measured: 90 to 100). */
export const PLIES_PER_GAME = 100;
/** Search speed of our engine with the NNUE, per thread (PeSTO is a bit faster). */
const NODES_PER_SECOND = 3_500_000;
/** Fixed cost of a move (search setup), in ms. */
const MOVE_OVERHEAD_MS = 0.2;
/** A time limit is overrun by about this much: the running iteration ends first. */
const TIME_OVERRUN = 1.1;
/** Mean time of a move at depth 4, in ms, and its growth per extra ply (measured 4 to 10). */
const DEPTH_4_MS = 0.9;
const DEPTH_GROWTH = 1.9;
/**
 * Mean time of a Stockfish move per level, in ms. Levels 1 to 6 stop at their
 * depth long before their time is up; from level 7 they use all of it.
 */
const STOCKFISH_MOVE_MS = [5, 5, 5, 12, 13, 27, 500, 1000, 1500, 2000];
/** Starting a Stockfish process, once per thread and batch of games. */
const STOCKFISH_START_MS = 300;

/** Mean time of one of our bots' moves under `limit`, in ms. */
export function ourMoveMs(limit: MoveLimit): number {
  switch (limit.kind) {
    case 'nodes':
      return (limit.value / NODES_PER_SECOND) * 1000 + MOVE_OVERHEAD_MS;
    case 'depth':
      return DEPTH_4_MS * DEPTH_GROWTH ** (limit.value - 4);
    case 'time':
      return limit.value * TIME_OVERRUN + MOVE_OVERHEAD_MS;
  }
}

/** Mean time of a move of `bot`, in ms: Stockfish plays at its level. */
export function botMoveMs(bot: string, limit: MoveLimit): number {
  const level = stockfishLevel(bot);
  return level === null ? ourMoveMs(limit) : (STOCKFISH_MOVE_MS[level - 1] ?? 2000);
}

export type PairingEstimate = { a: string; b: string; seconds: number };

export type TournamentEstimate = {
  /** Wall-clock time of the whole tournament. */
  seconds: number;
  games: number;
  /** Pairings are played one after the other: the longest one dominates. */
  slowest: PairingEstimate | null;
};

/** Wall-clock seconds of one pairing of `gamesPerPair` games. */
export function pairingSeconds(
  a: string,
  b: string,
  { gamesPerPair, limit, threads }: { gamesPerPair: number; limit: MoveLimit; threads: number },
): number {
  // Pairs of games (same opening, colors swapped, one after the other on a
  // thread) run `threads` at a time.
  const pairsOfGames = Math.max(1, Math.ceil(gamesPerPair / 2));
  const batches = Math.ceil(pairsOfGames / Math.max(1, threads));
  // Each side plays half the plies.
  const gameMs = (PLIES_PER_GAME / 2) * (botMoveMs(a, limit) + botMoveMs(b, limit));
  const startMs = stockfishLevel(a) !== null || stockfishLevel(b) !== null ? STOCKFISH_START_MS : 0;
  return (batches * (2 * gameMs + startMs)) / 1000;
}

/**
 * Pairings are played one after the other: `pairings` (default: every pair
 * of `bots`) add up.
 */
export function estimateTournament({
  bots,
  pairings,
  gamesPerPair,
  limit,
  threads,
}: {
  bots: readonly string[];
  pairings?: readonly Pair[];
  gamesPerPair: number;
  limit: MoveLimit;
  threads: number;
}): TournamentEstimate {
  const settings = { gamesPerPair, limit, threads };
  let seconds = 0;
  let slowest: PairingEstimate | null = null;
  const played = pairings ?? allPairs(bots);
  for (const [a, b] of played) {
    const pairing = pairingSeconds(a, b, settings);
    seconds += pairing;
    if (!slowest || pairing > slowest.seconds) slowest = { a, b, seconds: pairing };
  }
  const games = played.length * 2 * Math.max(1, Math.ceil(gamesPerPair / 2));
  return { seconds, games, slowest };
}

/** `≈ 40 s`, `≈ 12 min`, `≈ 2 h 05`, `≈ 3 j 4 h`. */
export function formatDuration(seconds: number): string {
  if (seconds < 1) return '< 1 s';
  if (seconds < 60) return `≈ ${String(Math.round(seconds))} s`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `≈ ${String(minutes)} min`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `≈ ${String(hours)} h ${String(minutes % 60).padStart(2, '0')}`;
  return `≈ ${String(Math.floor(hours / 24))} j ${String(hours % 24)} h`;
}
