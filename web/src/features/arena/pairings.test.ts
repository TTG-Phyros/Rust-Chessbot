import { describe, expect, it } from 'vitest';

import { allPairs, enabledPairs, groups, idleBots, pairKey, toggleRow } from './pairings';

const bots = ['net', 'classic', 'stockfish-3', 'stockfish-7'];

describe('tournament pairings', () => {
  it('lists every pair once, in playing order', () => {
    expect(allPairs(['a', 'b', 'c'])).toEqual([
      ['a', 'b'],
      ['a', 'c'],
      ['b', 'c'],
    ]);
    expect(pairKey('a', 'b')).toBe(pairKey('b', 'a'));
  });

  it('switches pairings off and finds idle bots', () => {
    const disabled = new Set([pairKey('stockfish-7', 'stockfish-3'), pairKey('net', 'classic')]);
    const pairs = enabledPairs(bots, disabled);
    expect(pairs).toHaveLength(4);
    expect(idleBots(bots, pairs)).toEqual([]);
    expect(idleBots(bots, [['net', 'classic']])).toEqual(['stockfish-3', 'stockfish-7']);
  });

  it('toggles a whole row', () => {
    let disabled = toggleRow('stockfish-7', bots, new Set());
    expect(enabledPairs(bots, disabled)).toHaveLength(3);
    expect(idleBots(bots, enabledPairs(bots, disabled))).toEqual(['stockfish-7']);
    // All off now: the same click switches them back on.
    disabled = toggleRow('stockfish-7', bots, disabled);
    expect(disabled.size).toBe(0);
  });

  it('finds groups that never meet', () => {
    expect(groups(bots, allPairs(bots))).toHaveLength(1);
    const split = groups(bots, [
      ['net', 'classic'],
      ['stockfish-3', 'stockfish-7'],
    ]);
    expect(split).toEqual([
      ['net', 'classic'],
      ['stockfish-3', 'stockfish-7'],
    ]);
    // A chain links everyone.
    expect(
      groups(bots, [
        ['net', 'classic'],
        ['classic', 'stockfish-3'],
        ['stockfish-3', 'stockfish-7'],
      ]),
    ).toHaveLength(1);
  });
});
