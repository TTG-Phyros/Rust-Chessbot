import { describe, expect, it } from 'vitest';

import { DEFAULT_LEVEL, levelLabel, levelLimits } from './level';

describe('levels', () => {
  it('maps to search limits, clamped', () => {
    expect(levelLimits({ ...DEFAULT_LEVEL, mode: 'depth', depth: 6 })).toEqual({ depth: 6 });
    expect(levelLimits({ ...DEFAULT_LEVEL, mode: 'depth', depth: 99 })).toEqual({ depth: 20 });
    expect(levelLimits({ ...DEFAULT_LEVEL, mode: 'time', timeMs: 1500 })).toEqual({
      movetimeMs: 1500,
    });
    expect(levelLimits({ ...DEFAULT_LEVEL, mode: 'time', timeMs: 5 })).toEqual({
      movetimeMs: 100,
    });
    expect(levelLimits({ ...DEFAULT_LEVEL, mode: 'clock' })).toEqual({});
  });

  it('has readable labels', () => {
    expect(levelLabel({ ...DEFAULT_LEVEL, mode: 'depth', depth: 4 })).toBe('profondeur 4');
    expect(levelLabel({ ...DEFAULT_LEVEL, mode: 'time', timeMs: 1500 })).toBe('1,5 s / coup');
    expect(
      levelLabel({ ...DEFAULT_LEVEL, mode: 'clock', cadence: { minutes: 3, incrementS: 2 } }),
    ).toBe('pendule 3 + 2');
  });
});
