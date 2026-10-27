import { describe, expect, it } from 'vitest';

import { clampedPawns, formatScore, whiteShare } from './score';

describe('formatScore', () => {
  it('formats centipawns as pawns with a sign', () => {
    expect(formatScore({ kind: 'cp', value: 35 })).toBe('+0.35');
    expect(formatScore({ kind: 'cp', value: -120 })).toBe('-1.20');
    expect(formatScore({ kind: 'cp', value: 0 })).toBe('0.00');
  });

  it('formats mates for either side', () => {
    expect(formatScore({ kind: 'mate', moves: 3 })).toBe('#3');
    expect(formatScore({ kind: 'mate', moves: -2 })).toBe('-#2');
    expect(formatScore({ kind: 'mate', moves: 0 })).toBe('#');
  });
});

describe('whiteShare', () => {
  it('is centred and monotonic', () => {
    expect(whiteShare({ kind: 'cp', value: 0 })).toBeCloseTo(0.5);
    expect(whiteShare({ kind: 'cp', value: 200 })).toBeGreaterThan(0.7);
    expect(whiteShare({ kind: 'cp', value: -200 })).toBeLessThan(0.3);
    expect(whiteShare({ kind: 'cp', value: 5000 })).toBeLessThanOrEqual(1);
  });

  it('saturates on mates', () => {
    expect(whiteShare({ kind: 'mate', moves: 4 })).toBe(1);
    expect(whiteShare({ kind: 'mate', moves: -1 })).toBe(0);
  });
});

describe('clampedPawns', () => {
  it('converts centipawns and clamps them', () => {
    expect(clampedPawns({ kind: 'cp', value: 42 }, 3)).toBeCloseTo(0.42);
    expect(clampedPawns({ kind: 'cp', value: -900 }, 3)).toBe(-3);
  });

  it('puts mates at the bounds', () => {
    expect(clampedPawns({ kind: 'mate', moves: 2 }, 3)).toBe(3);
    expect(clampedPawns({ kind: 'mate', moves: -1 }, 3)).toBe(-3);
  });
});
