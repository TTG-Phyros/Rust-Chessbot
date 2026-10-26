import { describe, expect, it } from 'vitest';

import { PARAM_FIELDS, formatParam, nameError, paramError } from './params';

const field = (key: string) => {
  const found = PARAM_FIELDS.find((f) => f.key === key);
  if (!found) throw new Error(key);
  return found;
};

describe('network names', () => {
  it('follows the server rule', () => {
    for (const name of ['main', 'lr-3e4', 'exp_2', 'A1']) expect(nameError(name)).toBeNull();
    for (const name of ['', '-x', 'a b', 'a/b', '..', 'x'.repeat(33), 'Classic']) {
      expect(nameError(name)).not.toBeNull();
    }
  });
});

describe('parameters', () => {
  it('are checked against the service bounds', () => {
    expect(paramError(field('selfplayNodes'), 5000)).toBeNull();
    expect(paramError(field('selfplayNodes'), 50)).toMatch(/entre/);
    expect(paramError(field('learningRate'), 0)).not.toBeNull();
    expect(paramError(field('wdl'), Number.NaN)).toBe('nombre attendu');
    expect(paramError(field('superbatches'), 0)).toBeNull();
  });

  it('are shown compactly', () => {
    expect(formatParam('learningRate', 0.001)).toBe('1.0e-3');
    expect(formatParam('batchSize', 16384).replace(/\s/g, ' ')).toBe('16 384');
  });

  it('cover every key once', () => {
    const keys = PARAM_FIELDS.map((f) => f.key);
    expect(new Set(keys).size).toBe(keys.length);
    expect(keys).toHaveLength(12);
  });
});
