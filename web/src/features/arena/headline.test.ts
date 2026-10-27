import { describe, expect, it } from 'vitest';

import type { Pairing } from '../../api/generated/Pairing';
import type { Tournament } from '../../api/generated/Tournament';
import { headline, shareAgainst } from './headline';

const pairing = (a: string, b: string, wins: number, draws: number, losses: number): Pairing => ({
  a,
  b,
  wins,
  draws,
  losses,
  elo: 0,
  eloError: 0,
});

const tournament = (bots: string[], pairings: Pairing[], ranked: string[]): Tournament => ({
  id: 't1',
  state: 'finished',
  createdAt: 0,
  updatedAt: 0,
  bots,
  gamesPerPair: 100,
  limit: { kind: 'depth', value: 12 },
  pairings,
  standings: ranked.map((bot) => ({ bot, games: 100, points: 50, elo: 0 })),
});

describe('shareAgainst', () => {
  it('reads a pairing from either side', () => {
    const t = tournament(['x', 'y'], [pairing('x', 'y', 6, 2, 2)], ['x', 'y']);
    expect(shareAgainst(t, 'x', 'y')).toBeCloseTo(0.7);
    expect(shareAgainst(t, 'y', 'x')).toBeCloseTo(0.3);
    expect(shareAgainst(t, 'x', 'z')).toBeNull();
  });
});

describe('headline', () => {
  it('places our bot between the Stockfish levels it beats and loses to', () => {
    const t = tournament(
      ['main-sb305', 'stockfish-5', 'stockfish-6', 'stockfish-7'],
      [
        pairing('main-sb305', 'stockfish-5', 97, 2, 1),
        pairing('main-sb305', 'stockfish-6', 65, 14, 21),
        pairing('main-sb305', 'stockfish-7', 14, 16, 70),
      ],
      ['stockfish-7', 'main-sb305', 'stockfish-6', 'stockfish-5'],
    );
    expect(headline(t)).toEqual({
      bot: 'main-sb305',
      before: 'main-sb305 bat ',
      accent: 'Stockfish 6',
      after: ' et ne cède qu’au niveau 7.',
    });
  });

  it('prefers a saved bot to PeSTO', () => {
    const t = tournament(
      ['classic', 'net', 'stockfish-3'],
      [pairing('classic', 'stockfish-3', 10, 0, 0), pairing('net', 'stockfish-3', 0, 0, 10)],
      ['classic', 'stockfish-3', 'net'],
    );
    expect(headline(t)?.bot).toBe('net');
    expect(headline(t)?.accent).toBe('Stockfish 3');
  });

  it('names the winner when no Stockfish played', () => {
    const t = tournament(
      ['classic', 'net'],
      [pairing('classic', 'net', 3, 0, 7)],
      ['net', 'classic'],
    );
    expect(headline(t)).toEqual({
      bot: 'net',
      before: '',
      accent: 'net',
      after: ' remporte le tournoi.',
    });
  });

  it('has nothing to say about an empty tournament', () => {
    expect(headline(tournament([], [], []))).toBeNull();
  });
});
