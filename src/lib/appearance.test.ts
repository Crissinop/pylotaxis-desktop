import { describe, expect, test } from 'vitest';

import {
  APPEARANCE_KEY,
  DEFAULT_APPEARANCE,
  applyAppearance,
  loadAppearance,
  parseAppearance,
  saveAppearance,
} from './appearance';

describe('aspetto (v0.8.0)', () => {
  test('legge le preferenze salvate', () => {
    expect(parseAppearance('{"theme":"dark","accent":"lapis"}')).toEqual({
      theme: 'dark',
      accent: 'lapis',
    });
  });

  test('torna al predefinito campo per campo davanti a valori non validi', () => {
    expect(parseAppearance(null)).toEqual(DEFAULT_APPEARANCE);
    expect(parseAppearance('non è json')).toEqual(DEFAULT_APPEARANCE);
    expect(parseAppearance('"dark"')).toEqual(DEFAULT_APPEARANCE);
    expect(parseAppearance('{"theme":"viola","accent":"slate"}')).toEqual({
      theme: 'system',
      accent: 'slate',
    });
    expect(parseAppearance('{"theme":"light","accent":42}')).toEqual({
      theme: 'light',
      accent: 'verdigris',
    });
  });

  test('salva e rilegge con la stessa chiave, e un archivio guasto non rompe nulla', () => {
    const memory = new Map<string, string>();
    const storage = {
      getItem: (key: string) => memory.get(key) ?? null,
      setItem: (key: string, value: string) => void memory.set(key, value),
    };
    saveAppearance({ theme: 'light', accent: 'porphyry' }, storage);
    expect([...memory.keys()]).toEqual([APPEARANCE_KEY]);
    expect(loadAppearance(storage)).toEqual({ theme: 'light', accent: 'porphyry' });
    const broken = {
      getItem: () => {
        throw new Error('negato');
      },
      setItem: () => {
        throw new Error('negato');
      },
    };
    expect(loadAppearance(broken)).toEqual(DEFAULT_APPEARANCE);
    expect(() => saveAppearance(DEFAULT_APPEARANCE, broken)).not.toThrow();
  });

  test('"Come Windows" e il verderame tolgono gli attributi: decide il CSS', () => {
    const root = { dataset: {} as DOMStringMap };
    applyAppearance({ theme: 'dark', accent: 'slate' }, root);
    expect(root.dataset).toEqual({ theme: 'dark', accent: 'slate' });
    applyAppearance(DEFAULT_APPEARANCE, root);
    expect(root.dataset).toEqual({});
  });
});
