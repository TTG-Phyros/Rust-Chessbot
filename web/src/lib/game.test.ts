import { describe, expect, it } from 'vitest';

import type { MoveView } from '../api/generated/MoveView';
import {
  STARTPOS_FEN,
  destinations,
  fenMoveNumber,
  formatLine,
  moveLabel,
  moveRows,
  movesBetween,
  newGame,
  pieceSide,
  withMove,
  withoutLast,
} from './game';

const move = (uci: string, promotion: string | null = null): MoveView => ({
  uci,
  san: uci,
  from: uci.slice(0, 2),
  to: uci.slice(2, 4),
  promotion,
});

describe('game bookkeeping', () => {
  it('appends and takes back moves without mutating', () => {
    const game = newGame();
    const played = withMove(withMove(game, 'e2e4'), 'e7e5');
    expect(game.moves).toEqual([]);
    expect(played.moves).toEqual(['e2e4', 'e7e5']);
    expect(withoutLast(played, 1).moves).toEqual(['e2e4']);
    expect(withoutLast(played, 5).moves).toEqual([]);
    expect(newGame('8/8/8/8/8/8/8/K6k w - - 0 1').fen).toBe('8/8/8/8/8/8/8/K6k w - - 0 1');
  });

  it('finds the server moves matching a drag', () => {
    const legal = [move('e7e8q', 'q'), move('e7e8n', 'n'), move('e7d8q', 'q'), move('g1f3')];
    expect(movesBetween(legal, 'e7', 'e8').map((m) => m.uci)).toEqual(['e7e8q', 'e7e8n']);
    expect(movesBetween(legal, 'g1', 'g3')).toEqual([]);
    expect(destinations(legal, 'e7')).toEqual(['e8', 'd8']);
  });

  it('reads the move number of a FEN', () => {
    expect(fenMoveNumber('4k3/8/8/8/8/8/8/4K3 b - - 0 40')).toEqual({ side: 'black', number: 40 });
    expect(fenMoveNumber('4k3/8/8/8/8/8/8/4K3 w - -')).toEqual({ side: 'white', number: 1 });
  });

  it('numbers move rows', () => {
    expect(moveRows(['e4', 'e5', 'Nf3'])).toEqual([
      { number: 1, white: { san: 'e4', ply: 0 }, black: { san: 'e5', ply: 1 } },
      { number: 2, white: { san: 'Nf3', ply: 2 }, black: null },
    ]);
    expect(moveRows(['Kd7', 'e4'], '4k3/8/8/8/8/8/4P3/4K3 b - - 0 40')).toEqual([
      { number: 40, white: null, black: { san: 'Kd7', ply: 0 } },
      { number: 41, white: { san: 'e4', ply: 1 }, black: null },
    ]);
    expect(moveRows([])).toEqual([]);
  });

  it('formats lines with move numbers', () => {
    expect(formatLine(['e4', 'e5', 'Nf3'], STARTPOS_FEN)).toBe('1. e4 e5 2. Nf3');
    expect(formatLine(['Kd7', 'e4', 'Ke6'], '4k3/8/8/8/8/8/4P3/4K3 b - - 0 40')).toBe(
      '40... Kd7 41. e4 Ke6',
    );
    expect(formatLine([], STARTPOS_FEN)).toBe('');
  });

  it('reads piece colors', () => {
    expect(pieceSide('wQ')).toBe('white');
    expect(pieceSide('bN')).toBe('black');
  });
});

describe('moveLabel', () => {
  it('numbers white and black moves', () => {
    const history = ['e4', 'e5', 'Nf3'];
    expect(moveLabel(history, 0)).toBe('1. e4');
    expect(moveLabel(history, 1)).toBe('1… e5');
    expect(moveLabel(history, 2)).toBe('2. Nf3');
    expect(moveLabel(history, 3)).toBeNull();
  });

  it('follows a start position with Black to move', () => {
    const fen = 'r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 2 7';
    expect(moveLabel(['Nf6'], 0, fen)).toBe('7… Nf6');
  });
});
