import type { Arrow } from 'react-chessboard';

import type { ReviewClass } from '../../api/generated/ReviewClass';
import type { ReviewGameSummary } from '../../api/generated/ReviewGameSummary';
import type { ReviewGameView } from '../../api/generated/ReviewGameView';
import type { Side } from '../../api/generated/Side';

/** How a class of move is shown: its name, a glyph, a color. */
export type ClassStyle = { label: string; symbol: string; color: string };

export const CLASSES: Record<ReviewClass, ClassStyle> = {
  brilliant: { label: 'Brillant', symbol: '!!', color: '#2ad1b5' },
  great: { label: 'Très bon coup', symbol: '!', color: '#6aa8ff' },
  best: { label: 'Meilleur coup', symbol: '★', color: '#8fd14f' },
  excellent: { label: 'Excellent', symbol: '✓', color: '#7dbb63' },
  good: { label: 'Bon', symbol: '○', color: '#9aa597' },
  book: { label: 'Théorique', symbol: '≡', color: '#c19a6b' },
  inaccuracy: { label: 'Imprécision', symbol: '?!', color: '#f2c94c' },
  mistake: { label: 'Erreur', symbol: '?', color: '#ff9f43' },
  miss: { label: 'Occasion manquée', symbol: '✗', color: '#ff6f91' },
  blunder: { label: 'Gaffe', symbol: '??', color: '#ff4d4d' },
};

/** Every class, from the best to the worst. */
export const CLASS_ORDER: readonly ReviewClass[] = [
  'brilliant',
  'great',
  'best',
  'excellent',
  'good',
  'book',
  'inaccuracy',
  'mistake',
  'miss',
  'blunder',
];

/** Classes worth a badge in the move list and a mark on the graph. */
const NOTABLE: ReadonlySet<ReviewClass> = new Set([
  'brilliant',
  'great',
  'inaccuracy',
  'mistake',
  'miss',
  'blunder',
]);

/** Classes after which the board shows what should have been played. */
const WRONG: ReadonlySet<ReviewClass> = new Set(['inaccuracy', 'mistake', 'miss', 'blunder']);

export function isNotable(kind: ReviewClass): boolean {
  return NOTABLE.has(kind);
}

/** Arrow colors: the best move of the position, the move that should have been played. */
export const BEST_ARROW = 'rgba(200, 245, 96, 0.85)';
export const MISSED_ARROW = 'rgba(157, 140, 255, 0.85)';

function arrow(uci: string | undefined, color: string): Arrow[] {
  return uci ? [{ startSquare: uci.slice(0, 2), endSquare: uci.slice(2, 4), color }] : [];
}

/**
 * Arrows of the position after `ply` moves: the engine's best move there, and,
 * after an inaccuracy or worse, the best move instead of the one played.
 */
export function reviewArrows(view: ReviewGameView, ply: number): Arrow[] {
  const analysis = view.analysis;
  if (!analysis) return [];
  const missed =
    ply > 0 && WRONG.has(analysis.moves[ply - 1]?.class ?? 'best')
      ? arrow(analysis.positions[ply - 1]?.lines[0]?.uci[0], MISSED_ARROW)
      : [];
  return [...missed, ...arrow(analysis.positions[ply]?.lines[0]?.uci[0], BEST_ARROW)];
}

/** Clock of `side` after the first `ply` moves (its latest `[%clk]`), if known. */
export function clockAt(view: ReviewGameView, ply: number, side: Side): number | null {
  const first: Side = view.startFen.split(' ')[1] === 'b' ? 'black' : 'white';
  for (let index = Math.min(ply, view.moves.length) - 1; index >= 0; index -= 1) {
    const mover: Side = (index % 2 === 0) === (first === 'white') ? 'white' : 'black';
    const clock = view.moves[index]?.clockMs;
    if (mover === side && clock !== undefined) return clock;
  }
  return null;
}

/** `5:03`, `1:02:00`, and tenths under 20 seconds: `0:09.4`. */
export function formatClock(ms: number): string {
  const total = Math.max(0, ms) / 1000;
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  const secondsText =
    total < 20 ? seconds.toFixed(1).padStart(4, '0') : String(Math.floor(seconds)).padStart(2, '0');
  return hours > 0
    ? `${hours}:${String(minutes).padStart(2, '0')}:${secondsText}`
    : `${minutes}:${secondsText}`;
}

/** The side `username` played (case-insensitive), if they did. */
export function sideOf(game: ReviewGameSummary, username: string | null): Side | null {
  const name = username?.toLowerCase();
  if (!name) return null;
  if (game.white.username.toLowerCase() === name) return 'white';
  if (game.black.username.toLowerCase() === name) return 'black';
  return null;
}

/** `1-0`, `0-1`, `½-½`. */
export function formatResult(result: string): string {
  return result === '1/2-1/2' ? '½-½' : result;
}

/** End date of a game: `12 oct. 2026`. */
export function formatDate(endTime: number): string {
  return new Date(endTime * 1000).toLocaleDateString('fr-FR', {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  });
}

export const TIME_CLASS_NAME: Record<ReviewGameSummary['timeClass'], string> = {
  bullet: 'Bullet',
  blitz: 'Blitz',
  rapid: 'Rapide',
  daily: 'Daily',
};

export type Outcome = 'win' | 'draw' | 'loss';

/** How the game ended for `side`. */
export function outcomeFor(result: string, side: Side): Outcome {
  if (result === '1-0') return side === 'white' ? 'win' : 'loss';
  if (result === '0-1') return side === 'black' ? 'win' : 'loss';
  return 'draw';
}
