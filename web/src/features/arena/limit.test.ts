import { describe, expect, it } from 'vitest';

import { LIMIT_OPTIONS, describeLimit, limitError } from './limit';

describe('move limits', () => {
  it('start from valid values', () => {
    for (const option of LIMIT_OPTIONS) {
      expect(limitError(option.kind, option.initial)).toBeNull();
    }
  });

  it('follow the service bounds', () => {
    expect(limitError('nodes', 50)).not.toBeNull();
    expect(limitError('depth', 31)).not.toBeNull();
    expect(limitError('depth', 2.5)).toBe('nombre entier attendu');
    expect(limitError('time', 9)).not.toBeNull();
    expect(limitError('time', 60_000)).toBeNull();
  });

  it('are described in words', () => {
    expect(describeLimit({ kind: 'nodes', value: 5000 }).replace(/\s/g, ' ')).toBe(
      '5 000 nœuds par coup',
    );
    expect(describeLimit({ kind: 'depth', value: 8 })).toBe('profondeur 8');
    expect(describeLimit({ kind: 'time', value: 100 })).toBe('100 ms par coup');
  });
});
