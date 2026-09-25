import { describe, expect, test } from 'vitest';

import { errorCode, UNKNOWN_ERROR } from './errors';

describe('errorCode', () => {
  test('legge il codice degli errori dei comandi', () => {
    expect(errorCode({ code: 'HASH_MISMATCH' })).toBe('HASH_MISMATCH');
  });

  test('qualunque altra forma diventa UNKNOWN (casi negativi)', () => {
    for (const value of [
      null,
      undefined,
      'testo',
      new Error('x'),
      { code: 42 },
      { code: 'a b' },
      {},
    ]) {
      expect(errorCode(value)).toBe(UNKNOWN_ERROR);
    }
  });
});
