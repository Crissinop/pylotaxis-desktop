import { describe, expect, test } from 'vitest';

import type { SecretInfo } from '../../lib/ipc';
import { countByApp, secretsOf } from './grouping';

const secret = (appId: string, label: string): SecretInfo => ({
  id: `${appId}-${label}`,
  appId,
  label,
  username: null,
  updatedMs: 0,
});

const all = [secret('a', 'token'), secret('b', 'Password'), secret('a', 'Password')];

describe('secretsOf', () => {
  test("tiene solo i segreti dell'app, per etichetta senza badare alle maiuscole", () => {
    expect(secretsOf(all, 'a').map((s) => s.label)).toEqual(['Password', 'token']);
    expect(secretsOf(all, 'c')).toEqual([]);
  });

  test("non riordina l'elenco ricevuto", () => {
    secretsOf(all, 'a');
    expect(all.map((s) => s.label)).toEqual(['token', 'Password', 'Password']);
  });
});

describe('countByApp', () => {
  test('conta per app; le app senza segreti non compaiono', () => {
    const counts = countByApp(all);
    expect([counts.get('a'), counts.get('b'), counts.get('c')]).toEqual([2, 1, undefined]);
  });
});
