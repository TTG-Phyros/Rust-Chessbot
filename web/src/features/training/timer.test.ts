import { describe, expect, it } from 'vitest';

import { DURATION_PRESETS, formatClock, formatRemaining, minutesError } from './timer';

describe('training timer', () => {
  it('accepts a minute to a week', () => {
    expect(minutesError(1)).toBeNull();
    expect(minutesError(10_080)).toBeNull();
    expect(minutesError(0)).not.toBeNull();
    expect(minutesError(10_081)).not.toBeNull();
    expect(minutesError(1.5)).toBe('nombre entier de minutes');
    for (const preset of DURATION_PRESETS) {
      if (preset.minutes !== null) expect(minutesError(preset.minutes)).toBeNull();
    }
  });

  it('shows the time left', () => {
    expect(formatRemaining(42)).toBe('42 s');
    expect(formatRemaining(8 * 60 + 5)).toBe('8 min');
    expect(formatRemaining(3600 + 5 * 60)).toBe('1 h 05');
    expect(formatRemaining(2 * 86_400 + 3 * 3600)).toBe('2 j 3 h');
    expect(formatRemaining(-5)).toBe('0 s');
  });

  it('shows the day only when it is not today', () => {
    const now = new Date(2026, 9, 26, 12, 0);
    const today = new Date(2026, 9, 26, 18, 30).getTime() / 1000;
    const tomorrow = new Date(2026, 9, 27, 9, 5).getTime() / 1000;
    expect(formatClock(today, now)).toBe('18:30');
    expect(formatClock(tomorrow, now)).toMatch(/27\/10 09:05$/);
  });
});
