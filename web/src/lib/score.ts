import type { ScoreView } from '../api/generated/ScoreView';

/** `+0.35`, `-1.20`, `#3` (White mates), `-#2` (Black mates). */
export function formatScore(score: ScoreView): string {
  if (score.kind === 'mate') {
    if (score.moves === 0) return '#';
    return score.moves > 0 ? `#${score.moves}` : `-#${-score.moves}`;
  }
  const pawns = score.value / 100;
  return `${pawns > 0 ? '+' : ''}${pawns.toFixed(2)}`;
}

/**
 * Share of the evaluation bar owned by White, in `[0, 1]`: a logistic curve
 * of the centipawn score (about 75 % at +2.00), all or nothing for mates.
 */
export function whiteShare(score: ScoreView): number {
  if (score.kind === 'mate') {
    if (score.moves === 0) return 0.5;
    return score.moves > 0 ? 1 : 0;
  }
  return 1 / (1 + Math.exp(-score.value / 180));
}

/** The score in pawns for White, clamped to `[-cap, cap]`; mates sit at the bounds. */
export function clampedPawns(score: ScoreView, cap: number): number {
  if (score.kind === 'mate') {
    if (score.moves === 0) return 0;
    return score.moves > 0 ? cap : -cap;
  }
  return Math.max(-cap, Math.min(cap, score.value / 100));
}
