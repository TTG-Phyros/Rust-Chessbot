import { describe, expect, it } from 'vitest';

import { levelLabel, levelLimits } from './level';

describe('levels', () => {
  it('maps to search limits, clamped', () => {
    expect(levelLimits({ mode: 'depth', depth: 6, timeMs: 1000 })).toEqual({ depth: 6 });
    expect(levelLimits({ mode: 'depth', depth: 99, timeMs: 1000 })).toEqual({ depth: 20 });
    expect(levelLimits({ mode: 'time', depth: 6, timeMs: 1500 })).toEqual({ movetimeMs: 1500 });
    expect(levelLimits({ mode: 'time', depth: 6, timeMs: 5 })).toEqual({ movetimeMs: 100 });
  });

  it('has readable labels', () => {
    expect(levelLabel({ mode: 'depth', depth: 4, timeMs: 0 })).toBe('profondeur 4');
    expect(levelLabel({ mode: 'time', depth: 0, timeMs: 1500 })).toBe('1,5 s / coup');
  });
});
