import { describe, expect, it } from 'vitest';

import {
  afterMove,
  formatClock,
  resumeClocks,
  startClocks,
  stopClocks,
  timeLeft,
  timeUsed,
} from './clock';

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

describe('clocks', () => {
  it('run for the side to move only', () => {
    const clocks = startClocks(1_000, 'white');
    expect(timeUsed(clocks, 'white', 4_000)).toBe(3_000);
    expect(timeUsed(clocks, 'black', 4_000)).toBe(0);
  });

  it('switch after a move and add the increment', () => {
    const base = 60_000;
    let clocks = startClocks(0, 'white');
    clocks = afterMove(clocks, 'white', 5_000, 2_000);
    expect(clocks.running).toBe('black');
    expect(timeLeft(clocks, 'white', 9_000, base)).toBe(57_000);
    expect(timeLeft(clocks, 'black', 9_000, base)).toBe(56_000);
    clocks = afterMove(clocks, 'black', 9_000, 2_000);
    expect(timeLeft(clocks, 'black', 20_000, base)).toBe(58_000);
    expect(timeLeft(clocks, 'white', 20_000, base)).toBe(46_000);
  });

  it('stop and resume', () => {
    let clocks = stopClocks(startClocks(0, 'white'), 3_000);
    expect(clocks.running).toBeNull();
    expect(timeUsed(clocks, 'white', 10_000)).toBe(3_000);
    clocks = resumeClocks(clocks, 'white', 10_000);
    expect(timeUsed(clocks, 'white', 11_000)).toBe(4_000);
    expect(resumeClocks(clocks, 'white', 12_000)).toBe(clocks);
  });
});
