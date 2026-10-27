import { describe, expect, it } from 'vitest';

import { frenchSan } from './san';

describe('frenchSan', () => {
  it('translates the piece letters', () => {
    expect(frenchSan('Nf3')).toBe('Cf3');
    expect(frenchSan('Bxc6')).toBe('Fxc6');
    expect(frenchSan('Qh5+')).toBe('Dh5+');
    expect(frenchSan('Rad1')).toBe('Tad1');
    expect(frenchSan('Kxe2#')).toBe('Rxe2#');
  });

  it('translates promotions, not files', () => {
    expect(frenchSan('exd8=Q+')).toBe('exd8=D+');
    expect(frenchSan('b8=N')).toBe('b8=C');
    expect(frenchSan('bxc6')).toBe('bxc6');
  });

  it('leaves castling and pawn moves alone', () => {
    expect(frenchSan('O-O-O')).toBe('O-O-O');
    expect(frenchSan('e4')).toBe('e4');
  });
});
