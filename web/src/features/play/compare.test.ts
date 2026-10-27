import { describe, expect, it } from 'vitest';

import type { StockfishResult } from '../../api/generated/StockfishResult';
import { compare, formatDelta } from './compare';

const result = (level: number, uci: string, cp: number): StockfishResult => ({
  level,
  bestMove: { uci, san: uci, from: uci.slice(0, 2), to: uci.slice(2, 4) },
  score: { kind: 'cp', value: cp },
  depth: 10,
  timeMs: 100,
});

describe('compare', () => {
  it('measures each level against the bot', () => {
    const comparison = compare('c4d5', { kind: 'cp', value: 42 }, [
      result(7, 'c4d5', 38),
      result(10, 'd4c5', 49),
    ]);
    expect(comparison.agreeing).toBe(1);
    expect(comparison.rows.map((row) => row.agrees)).toEqual([true, false]);
    expect(comparison.rows[0]?.delta).toBeCloseTo(-0.04);
    expect(comparison.rows[1]?.delta).toBeCloseTo(0.07);
    expect(comparison.meanDelta).toBeCloseTo(0.015);
  });

  it('copes with missing moves and scores', () => {
    const comparison = compare(null, null, [
      { level: 3, depth: 0, timeMs: 5, bestMove: null, score: null },
    ]);
    expect(comparison.agreeing).toBe(0);
    expect(comparison.meanDelta).toBeNull();
  });
});

describe('formatDelta', () => {
  it('signs the difference', () => {
    expect(formatDelta(0.07)).toBe('+0.07');
    expect(formatDelta(-0.04)).toBe('−0.04');
    expect(formatDelta(0)).toBe('0.00');
  });
});
