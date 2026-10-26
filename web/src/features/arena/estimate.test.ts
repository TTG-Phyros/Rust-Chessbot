import { describe, expect, it } from 'vitest';

import {
  PLIES_PER_GAME,
  botMoveMs,
  estimateTournament,
  formatDuration,
  ourMoveMs,
} from './estimate';

describe('tournament estimate', () => {
  it('matches the measured game lengths of our engine', () => {
    // Measured on the reference machine, seconds per game (both sides ours).
    const perGame = (limit: Parameters<typeof ourMoveMs>[0]) =>
      (PLIES_PER_GAME * ourMoveMs(limit)) / 1000;
    const close = (estimate: number, measured: number) => {
      expect(estimate / measured).toBeGreaterThan(0.5);
      expect(estimate / measured).toBeLessThan(2);
    };
    close(perGame({ kind: 'nodes', value: 5000 }), 0.17);
    close(perGame({ kind: 'nodes', value: 100_000 }), 2.7);
    close(perGame({ kind: 'depth', value: 8 }), 1.2);
    close(perGame({ kind: 'depth', value: 10 }), 3.96);
    close(perGame({ kind: 'time', value: 20 }), 1.9);
  });

  it('matches measured tournaments', () => {
    // Real durations on the reference machine, 8 threads.
    const cases = [
      {
        bots: ['net', 'classic', 'stockfish-3', 'stockfish-6'],
        games: 8,
        limit: { kind: 'depth', value: 6 },
        real: 13.2,
      },
      {
        bots: ['net', 'classic', 'net2'],
        games: 20,
        limit: { kind: 'nodes', value: 50_000 },
        real: 20.6,
      },
      {
        bots: ['net', 'classic', 'stockfish-5'],
        games: 8,
        limit: { kind: 'time', value: 30 },
        real: 20.1,
      },
    ] as const;
    for (const { bots, games, limit, real } of cases) {
      const { seconds } = estimateTournament({ bots, gamesPerPair: games, limit, threads: 8 });
      expect(seconds / real, limit.kind).toBeGreaterThan(0.7);
      expect(seconds / real, limit.kind).toBeLessThan(1.4);
    }
  });

  it('lets Stockfish play at its level', () => {
    const limit = { kind: 'nodes', value: 5000 } as const;
    expect(botMoveMs('stockfish-1', limit)).toBeLessThan(10);
    expect(botMoveMs('stockfish-8', limit)).toBe(1000);
    expect(botMoveMs('main', limit)).toBe(ourMoveMs(limit));
  });

  it('adds up pairings and spreads pairs of games over threads', () => {
    const limit = { kind: 'time', value: 100 } as const;
    const base = { bots: ['a', 'b'], gamesPerPair: 8, limit };
    const one = estimateTournament({ ...base, threads: 1 });
    const four = estimateTournament({ ...base, threads: 4 });
    // 4 pairs of games: 4 rounds on one thread, 1 on four.
    expect(one.seconds / four.seconds).toBeCloseTo(4);
    expect(one.games).toBe(8);
    // 8 games in a row of PLIES_PER_GAME moves of ~100 ms each.
    expect(one.seconds).toBeCloseTo((8 * PLIES_PER_GAME * ourMoveMs(limit)) / 1000, 6);

    const three = estimateTournament({ ...base, bots: ['a', 'b', 'stockfish-9'], threads: 4 });
    expect(three.games).toBe(24);
    expect(three.slowest?.b).toBe('stockfish-9');

    // Only the chosen pairings count.
    const chosen = estimateTournament({
      ...base,
      bots: ['a', 'b', 'stockfish-9'],
      pairings: [['a', 'b']],
      threads: 4,
    });
    expect(chosen.games).toBe(8);
    expect(chosen.seconds).toBeCloseTo(four.seconds);
  });

  it('formats durations', () => {
    expect(formatDuration(0.4)).toBe('< 1 s');
    expect(formatDuration(42)).toBe('≈ 42 s');
    expect(formatDuration(750)).toBe('≈ 13 min');
    expect(formatDuration(2 * 3600 + 5 * 60)).toBe('≈ 2 h 05');
    expect(formatDuration(3 * 86_400 + 4 * 3600)).toBe('≈ 3 j 4 h');
  });
});
