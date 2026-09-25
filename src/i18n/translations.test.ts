import { describe, expect, test } from 'vitest';

import { APP_NAME } from '../constants/app';
import { FALLBACK_LANGUAGE, SUPPORTED_LANGUAGES } from './language';
import en from './locales/en.json';
import itLocale from './locales/it.json';

// Parità delle traduzioni (A.6 n. 6), resa automatica. (v0.1.0)
const LOCALES: Record<string, unknown> = { it: itLocale, en };

/** Appiattisce un file di lingua in coppie [chiave.annidata, testo]. */
function entries(node: unknown, prefix = ''): [string, string][] {
  if (typeof node === 'string') return [[prefix, node]];
  if (node === null || typeof node !== 'object' || Array.isArray(node)) {
    throw new Error(`Valore non ammesso in "${prefix || '(radice)'}": solo testi e oggetti`);
  }
  return Object.entries(node).flatMap(([key, value]) =>
    entries(value, prefix ? `${prefix}.${key}` : key),
  );
}

const placeholders = (text: string) =>
  [...text.matchAll(/\{\{\s*(\w+)\s*\}\}/g)].map((m) => m[1]).sort();

const reference = new Map(entries(LOCALES[FALLBACK_LANGUAGE]));

describe('traduzioni', () => {
  test.each(SUPPORTED_LANGUAGES)(
    '%s esiste e ha le stesse chiavi della lingua di riferimento',
    (lang) => {
      const keys = entries(LOCALES[lang]).map(([key]) => key);
      expect(keys.sort()).toEqual([...reference.keys()].sort());
    },
  );

  test.each(SUPPORTED_LANGUAGES)(
    '%s usa gli stessi segnaposto della lingua di riferimento',
    (lang) => {
      for (const [key, text] of entries(LOCALES[lang])) {
        expect(placeholders(text), key).toEqual(placeholders(reference.get(key) ?? ''));
      }
    },
  );

  test.each(SUPPORTED_LANGUAGES)('%s non ha testi vuoti né il nome del prodotto', (lang) => {
    for (const [key, text] of entries(LOCALES[lang])) {
      expect(text.trim(), key).not.toBe('');
      expect(text.toLowerCase(), key).not.toContain(APP_NAME.toLowerCase());
    }
  });

  test('i plurali hanno sempre sia _one sia _other', () => {
    for (const key of reference.keys()) {
      const match = /^(.*)_(one|other)$/.exec(key);
      if (!match) continue;
      const twin = `${match[1]}_${match[2] === 'one' ? 'other' : 'one'}`;
      expect(reference.has(twin), `manca ${twin}`).toBe(true);
    }
  });
});
