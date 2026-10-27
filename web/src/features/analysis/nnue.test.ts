import { describe, expect, it } from 'vitest';

import { contributionColor, contributionColors } from './nnue';

describe('contributionColor', () => {
  it('colors by side and strength', () => {
    expect(contributionColor(300, 300)).toBe('rgba(200, 245, 96, 0.70)');
    expect(contributionColor(-300, 300)).toBe('rgba(157, 140, 255, 0.70)');
    expect(contributionColor(0, 300)).toBe('rgba(200, 245, 96, 0.08)');
    expect(contributionColor(900, 300)).toBe('rgba(200, 245, 96, 0.70)');
  });
});

describe('contributionColors', () => {
  it('scales on the pieces, not the kings', () => {
    const colors = contributionColors([
      { square: 'e1', piece: 'K', cp: 5000 },
      { square: 'd1', piece: 'Q', cp: 900 },
      { square: 'd8', piece: 'q', cp: -450 },
    ]);
    expect(colors.d1).toBe('rgba(200, 245, 96, 0.70)');
    expect(colors.e1).toBe('rgba(200, 245, 96, 0.70)');
    expect(colors.d8).toBe(contributionColor(-450, 900));
  });
});
