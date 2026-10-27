import { describe, expect, it } from 'vitest';

import type { ReviewAnalysis } from '../../api/generated/ReviewAnalysis';
import type { ReviewClass } from '../../api/generated/ReviewClass';
import type { ReviewGameView } from '../../api/generated/ReviewGameView';
import type { ReviewPosition } from '../../api/generated/ReviewPosition';
import {
  BEST_ARROW,
  CLASSES,
  CLASS_ORDER,
  MISSED_ARROW,
  clockAt,
  formatClock,
  formatResult,
  outcomeFor,
  reviewArrows,
  sideOf,
} from './review';

const STATS = {
  moves: 1,
  counts: {
    brilliant: 0,
    great: 0,
    best: 0,
    excellent: 0,
    good: 0,
    book: 0,
    inaccuracy: 0,
    mistake: 0,
    miss: 0,
    blunder: 0,
  },
};

function position(best: string): ReviewPosition {
  return {
    fen: '',
    score: { kind: 'cp', value: 0 },
    win: 50,
    checkmated: false,
    lines: [{ score: { kind: 'cp', value: 0 }, depth: 10, uci: [best], san: [best] }],
  };
}

/** 1. f3 e5 2. g4, the second move judged `second`. */
function game(
  second: ReviewClass,
  startFen = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1',
) {
  const verdict = (kind: ReviewClass) => ({
    class: kind,
    winBefore: 50,
    winAfter: 50,
    accuracy: 100,
    cpLoss: 0,
  });
  const analysis: ReviewAnalysis = {
    engine: 'test',
    budget: 'depth 10',
    multipv: 1,
    positions: [position('e2e4'), position('e7e5'), position('d2d4'), position('d8h4')],
    moves: [verdict('book'), verdict(second), verdict('blunder')],
    white: STATS,
    black: STATS,
  };
  const view: ReviewGameView = {
    game: {
      id: 'g',
      url: '',
      endTime: 0,
      timeClass: 'blitz',
      timeControl: '180',
      rated: true,
      white: { username: 'Alice', rating: 900 },
      black: { username: 'bob', rating: 950 },
      result: '0-1',
      plies: 3,
      analysed: true,
    },
    startFen,
    moves: [
      { uci: 'f2f3', san: 'f3', clockMs: 179_000, book: true },
      { uci: 'e7e5', san: 'e5', clockMs: 178_000, book: false },
      { uci: 'g2g4', san: 'g4', book: false },
    ],
    analysis,
  };
  return view;
}

describe('classes', () => {
  it('have a style each, in order', () => {
    expect(CLASS_ORDER).toHaveLength(10);
    expect(new Set(CLASS_ORDER.map((kind) => CLASSES[kind].color)).size).toBe(10);
  });
});

describe('reviewArrows', () => {
  it('shows the best move of the position', () => {
    expect(reviewArrows(game('best'), 2)).toEqual([
      { startSquare: 'd2', endSquare: 'd4', color: BEST_ARROW },
    ]);
  });

  it('shows what should have been played after a mistake', () => {
    expect(reviewArrows(game('mistake'), 2)).toEqual([
      { startSquare: 'e7', endSquare: 'e5', color: MISSED_ARROW },
      { startSquare: 'd2', endSquare: 'd4', color: BEST_ARROW },
    ]);
  });

  it('shows nothing without an analysis', () => {
    expect(reviewArrows({ ...game('best'), analysis: null }, 1)).toEqual([]);
  });
});

describe('clockAt', () => {
  it('finds the latest clock of each side', () => {
    const view = game('best');
    expect(clockAt(view, 0, 'white')).toBeNull();
    expect(clockAt(view, 1, 'white')).toBe(179_000);
    expect(clockAt(view, 1, 'black')).toBeNull();
    expect(clockAt(view, 2, 'black')).toBe(178_000);
    // The 3rd move has no clock: White's stays the one of its 1st.
    expect(clockAt(view, 3, 'white')).toBe(179_000);
  });

  it('follows games starting with Black to move', () => {
    const view = game('best', '8/8/8/8/8/8/8/k6K b - - 0 1');
    expect(clockAt(view, 1, 'black')).toBe(179_000);
    expect(clockAt(view, 2, 'white')).toBe(178_000);
  });
});

describe('formatClock', () => {
  it('formats minutes, hours and the last seconds', () => {
    expect(formatClock(303_000)).toBe('5:03');
    expect(formatClock(3_720_000)).toBe('1:02:00');
    expect(formatClock(9_400)).toBe('0:09.4');
    expect(formatClock(-5)).toBe('0:00.0');
  });
});

describe('sideOf and formatResult', () => {
  it('match usernames without case', () => {
    const summary = game('best').game;
    expect(sideOf(summary, 'BOB')).toBe('black');
    expect(sideOf(summary, 'alice')).toBe('white');
    expect(sideOf(summary, 'carol')).toBeNull();
    expect(sideOf(summary, null)).toBeNull();
    expect(formatResult('1/2-1/2')).toBe('½-½');
  });
});

describe('outcomeFor', () => {
  it('reads the result from each side', () => {
    expect(outcomeFor('1-0', 'white')).toBe('win');
    expect(outcomeFor('1-0', 'black')).toBe('loss');
    expect(outcomeFor('0-1', 'black')).toBe('win');
    expect(outcomeFor('1/2-1/2', 'white')).toBe('draw');
  });
});
