import { describe, expect, it } from 'vitest';

import { formatClock } from './clock';

describe('formatClock', () => {
  it('shows minutes and seconds', () => {
    expect(formatClock(0)).toBe('00:00');
    expect(formatClock(287_900)).toBe('04:47');
  });

  it('adds hours past an hour', () => {
    expect(formatClock(3_725_000)).toBe('1:02:05');
  });

  it('shows hundredths on demand', () => {
    expect(formatClock(43, true)).toBe('00:00.04');
    expect(formatClock(61_250, true)).toBe('01:01.25');
  });

  it('never goes negative', () => {
    expect(formatClock(-5)).toBe('00:00');
  });
});
