import { describe, expect, it } from 'vitest';

import {
  PARAM_FIELDS,
  formatParam,
  nameError,
  paramError,
  randomName,
  randomParams,
} from './params';

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

describe('random parameters', () => {
  const base = {
    selfplayNodes: 5000,
    learningRate: 0.001,
    lrDropEvery: 40,
    lrDropFactor: 0.3,
    wdl: 0.25,
    batchSize: 16384,
    batchesPerSuperbatch: 200,
    superbatches: 0,
    exportEvery: 5,
    minPositions: 2_000_000,
    maxReuse: 30,
    evalGames: 64,
    datasetsFrom: ['main'],
  };

  it('stay valid at the edges of their ranges', () => {
    for (const draw of [0, 0.5, 0.999_999]) {
      const params = randomParams(base, () => draw);
      for (const param of PARAM_FIELDS) {
        expect(paramError(param, params[param.key]), param.key).toBeNull();
      }
      expect(params.learningRate).toBeGreaterThanOrEqual(1e-4);
      expect(params.learningRate).toBeLessThanOrEqual(3e-3);
      expect(params.selfplayNodes % 500).toBe(0);
    }
  });

  it('keep duration, data and measurement settings', () => {
    const params = randomParams(base, () => 0.3);
    expect(params.superbatches).toBe(0);
    expect(params.minPositions).toBe(2_000_000);
    expect(params.evalGames).toBe(64);
    expect(params.datasetsFrom).toEqual(['main']);
  });

  it('come with a valid name', () => {
    const name = randomName(() => 0.7);
    expect(name).toMatch(/^rand-[a-z0-9]{4}$/);
    expect(nameError(name)).toBeNull();
  });
});
