/** Comparison of our bot with Stockfish levels on the same position. */
import type { ScoreView } from '../../api/generated/ScoreView';
import type { StockfishResult } from '../../api/generated/StockfishResult';
import { clampedPawns } from '../../lib/score';

/** Colors of the compared levels, in the order chosen (none is the signal: that is our bot). */
export const LEVEL_COLORS = ['#5ed6f5', '#ffb84d', '#f58bd6', '#8bf5b0'] as const;

/** Most levels compared at once (the server's limit). */
export const MAX_COMPARED = 4;

/** Bound of the differences, in pawns: a mate counts as this much. */
const CAP = 20;

export type ComparisonRow = {
  level: number;
  score: ScoreView | null;
  /** Its evaluation minus the bot's, in pawns for White. */
  delta: number | null;
  /** Its move, in UCI and SAN. */
  uci: string | null;
  san: string | null;
  /** Whether it plays the bot's move. */
  agrees: boolean;
};

export type Comparison = {
  rows: ComparisonRow[];
  /** Levels that play the bot's move. */
  agreeing: number;
  /** Mean of the differences, in pawns. */
  meanDelta: number | null;
};

/** Each level against the bot's move `botUci` and evaluation `botScore`. */
export function compare(
  botUci: string | null,
  botScore: ScoreView | null,
  results: readonly StockfishResult[],
): Comparison {
  const rows = results.map((result) => ({
    level: result.level,
    score: result.score ?? null,
    delta:
      result.score && botScore
        ? clampedPawns(result.score, CAP) - clampedPawns(botScore, CAP)
        : null,
    uci: result.bestMove?.uci ?? null,
    san: result.bestMove?.san ?? null,
    agrees: !!botUci && result.bestMove?.uci === botUci,
  }));
  const deltas = rows.flatMap((row) => (row.delta === null ? [] : [row.delta]));
  return {
    rows,
    agreeing: rows.filter((row) => row.agrees).length,
    meanDelta: deltas.length ? deltas.reduce((sum, d) => sum + d, 0) / deltas.length : null,
  };
}

/** `+0.07`, `−0.04`: a difference in pawns. */
export function formatDelta(delta: number): string {
  const sign = delta > 0.005 ? '+' : delta < -0.005 ? '−' : '';
  return `${sign}${Math.abs(delta).toFixed(2)}`;
}
