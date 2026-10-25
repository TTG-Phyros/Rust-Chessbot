import { describe, expect, it } from 'vitest';

import { describeStatus, resultOf } from './status';

describe('describeStatus', () => {
  it('describes every state', () => {
    expect(describeStatus({ kind: 'ongoing' }, 'white', false)).toBe('Trait aux Blancs');
    expect(describeStatus({ kind: 'ongoing' }, 'black', true)).toBe('Trait aux Noirs — échec !');
    expect(describeStatus({ kind: 'checkmate', winner: 'black' }, 'white', true)).toBe(
      'Échec et mat — les Noirs gagnent',
    );
    expect(describeStatus({ kind: 'stalemate' }, 'white', false)).toBe('Pat — partie nulle');
    expect(describeStatus({ kind: 'draw', reason: 'threefoldRepetition' }, 'white', false)).toBe(
      'Partie nulle (triple répétition)',
    );
  });

  it('maps results', () => {
    expect(resultOf({ kind: 'checkmate', winner: 'white' })).toBe('1-0');
    expect(resultOf({ kind: 'draw', reason: 'fiftyMoveRule' })).toBe('1/2-1/2');
    expect(resultOf({ kind: 'ongoing' })).toBe('*');
  });
});
