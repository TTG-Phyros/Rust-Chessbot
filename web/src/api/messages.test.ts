import { describe, expect, it } from 'vitest';

import { parseServerMessage } from './messages';

describe('parseServerMessage', () => {
  it('accepts known messages', () => {
    expect(parseServerMessage('{"type":"pong","id":3}')).toEqual({ type: 'pong', id: 3 });
    expect(parseServerMessage('{"type":"hello","version":"0.1.0"}')).toEqual({
      type: 'hello',
      version: '0.1.0',
    });
  });

  it('rejects malformed or unknown frames', () => {
    expect(parseServerMessage('not json')).toBeNull();
    expect(parseServerMessage('42')).toBeNull();
    expect(parseServerMessage('{"id":1}')).toBeNull();
    expect(parseServerMessage('{"type":"future"}')).toBeNull();
  });
});
