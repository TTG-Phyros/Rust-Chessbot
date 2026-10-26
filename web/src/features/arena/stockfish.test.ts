import { describe, expect, it } from 'vitest';

import {
  STOCKFISH_LEVELS,
  botLabel,
  describeLevel,
  stockfishBot,
  stockfishLevel,
} from './stockfish';

describe('Stockfish bots', () => {
  it('have 10 increasingly strong levels', () => {
    expect(STOCKFISH_LEVELS).toHaveLength(10);
    for (let i = 1; i < STOCKFISH_LEVELS.length; i++) {
      const [weaker, stronger] = [STOCKFISH_LEVELS[i - 1], STOCKFISH_LEVELS[i]];
      expect(stronger?.skill).toBeGreaterThanOrEqual(weaker?.skill ?? 0);
      expect(stronger?.movetimeMs).toBeGreaterThan(weaker?.movetimeMs ?? 0);
    }
    expect(describeLevel(STOCKFISH_LEVELS[4] as never)).toContain('300 ms');
  });

  it('are named stockfish-1 to stockfish-10', () => {
    expect(stockfishBot(3)).toBe('stockfish-3');
    expect(stockfishLevel('stockfish-10')).toBe(10);
    for (const name of ['stockfish-0', 'stockfish-11', 'stockfish-01', 'main']) {
      expect(stockfishLevel(name)).toBeNull();
    }
    expect(botLabel('stockfish-7')).toBe('Stockfish 7');
    expect(botLabel('classic')).toBe('PeSTO');
    expect(botLabel('main-sb305')).toBe('main-sb305');
  });
});
