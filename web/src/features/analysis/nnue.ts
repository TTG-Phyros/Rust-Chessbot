/** The NNUE piece map: what each piece brings, as square colors. */
import type { PieceContribution } from '../../api/generated/PieceContribution';

/** White's gains in the signal color, Black's in the engine color. */
const WHITE_RGB = '200, 245, 96';
const BLACK_RGB = '157, 140, 255';

/**
 * Color of a contribution of `cp` (White's point of view) on a scale where
 * `max` is the largest one: the stronger, the more opaque.
 */
export function contributionColor(cp: number, max: number): string {
  const strength = max > 0 ? Math.min(1, Math.abs(cp) / max) : 0;
  const alpha = (0.08 + 0.62 * Math.sqrt(strength)).toFixed(2);
  return `rgba(${cp >= 0 ? WHITE_RGB : BLACK_RGB}, ${alpha})`;
}

/**
 * Color of each occupied square. Kings are left out of the scale (the
 * network values them far more than any piece, which would wash out the
 * others) but still colored, at full strength.
 */
export function contributionColors(pieces: readonly PieceContribution[]): Record<string, string> {
  const max = Math.max(
    1,
    ...pieces.filter((p) => p.piece.toLowerCase() !== 'k').map((p) => Math.abs(p.cp)),
  );
  return Object.fromEntries(pieces.map((p) => [p.square, contributionColor(p.cp, max)]));
}
