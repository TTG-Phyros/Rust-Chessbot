import { describe, expect, it } from 'vitest';

import { deviceName, eloBand, formatCount, formatElapsed, formatElo } from './format';

// toLocaleString('fr-FR') groups digits with a narrow no-break space.
const nbsp = (text: string) => text.replace(/\s/g, ' ');

describe('training formatting', () => {
  it('formats counts', () => {
    expect(nbsp(formatCount(12_345))).toBe('12 345');
    expect(nbsp(formatCount(1_250_000))).toBe('1,3 M');
    expect(formatCount(7)).toBe('7');
  });

  it('formats elapsed time', () => {
    expect(formatElapsed(100, 142)).toBe('42 s');
    expect(formatElapsed(0, 300)).toBe('5 min');
    expect(formatElapsed(0, 7620)).toBe('2 h 07');
    expect(formatElapsed(50, 10)).toBe('0 s');
  });

  it('formats Elo with its interval', () => {
    expect(formatElo(12.4, 30.2)).toBe('+12 ± 30');
    expect(formatElo(-80, 50)).toBe('−80 ± 50');
    expect(formatElo(0, 5)).toBe('0 ± 5');
  });

  it('builds the confidence band', () => {
    expect(eloBand([{ superbatch: 5, elo: -40, error: 25, games: 64 }])).toEqual([
      { superbatch: 5, elo: -40, band: [-65, -15], error: 25, games: 64 },
    ]);
  });
});

describe('deviceName', () => {
  it('keeps the model of a GPU', () => {
    expect(
      deviceName('GPU NVIDIA GeForce RTX 4060 (compute 8.9, 8187 MiB VRAM, 5291 MiB free)'),
    ).toBe('NVIDIA GeForce RTX 4060');
  });

  it('leaves other devices alone', () => {
    expect(deviceName('CPU')).toBe('CPU');
  });
});
