import { describe, expect, test } from 'vitest';

import { FALLBACK_LANGUAGE, pickLanguage } from './language';

describe('pickLanguage', () => {
  test('usa la lingua di base del primo tag supportato', () => {
    expect(pickLanguage(['en-GB', 'it-IT'])).toBe('en');
    expect(pickLanguage(['fr-FR', 'it-IT'])).toBe('it');
    expect(pickLanguage(['EN-us'])).toBe('en');
  });

  test('ricade sulla lingua di riferimento', () => {
    expect(pickLanguage([])).toBe(FALLBACK_LANGUAGE);
    expect(pickLanguage(['de-DE', 'ja'])).toBe(FALLBACK_LANGUAGE);
  });

  test('non scambia proprietà di oggetto per lingue (casi negativi, A.6 n. 9)', () => {
    expect(pickLanguage(['constructor', 'toString', '__proto__'])).toBe(FALLBACK_LANGUAGE);
    expect(pickLanguage(['', ' ', '-'])).toBe(FALLBACK_LANGUAGE);
  });
});
