import { describe, expect, test } from 'vitest';

import type { RegisteredApp, Registry } from '../../lib/ipc';
import { PALETTE_MAX_RESULTS, buildEntries, normalize, rank } from './ranking';

const app = (id: string, name: string, tags: string[] = [], categoryId: string | null = null) =>
  ({ id, name, kind: 'web', target: 'https://example.com/', categoryId, tags }) as RegisteredApp;

const registry: Registry = {
  categories: [{ id: 'c1', name: 'Clienti' }],
  apps: [
    app('1', 'Portale'),
    app('2', 'Report mensile', ['contabilità']),
    app('3', 'Caffè', [], 'c1'),
  ],
  secrets: [
    { id: 's1', appId: '1', label: 'Password', username: 'mario', updatedMs: 0 },
    { id: 's2', appId: 'x', label: 'Orfano', username: null, updatedMs: 0 },
  ],
};

const entries = buildEntries(registry, { lock: 'Blocca', open: 'Apri' });
const titles = (query: string) => rank(entries, query).map((entry) => entry.title);

describe('normalize', () => {
  test('ignora maiuscole e accenti', () => {
    expect(normalize('Caffè ÀÉ')).toBe('caffe ae');
  });
});

describe('buildEntries', () => {
  test("i segreti portano il nome dell'app; quelli senza app si scartano", () => {
    expect(entries.filter((e) => e.kind === 'secret').map((e) => e.title)).toEqual([
      'Portale · Password',
    ]);
  });
});

describe('rank', () => {
  test('senza ricerca: app per nome, poi le azioni, nessun segreto', () => {
    expect(titles('')).toEqual(['Caffè', 'Portale', 'Report mensile', 'Apri', 'Blocca']);
  });

  test('il prefisso vince sulla parola interna e sulla sottostringa', () => {
    expect(titles('port')).toEqual(['Portale', 'Portale · Password', 'Report mensile']);
    expect(titles('mens')).toEqual(['Report mensile']);
  });

  test('accenti, tag, categorie e nome utente', () => {
    expect(titles('caffe')).toEqual(['Caffè']);
    expect(titles('contabilita')).toEqual(['Report mensile']);
    expect(titles('clienti')).toEqual(['Caffè']);
    expect(titles('mario')).toEqual(['Portale · Password']);
  });

  test('tutte le parole devono trovare posto; nessuna voce inventata', () => {
    expect(titles('portale password')).toEqual(['Portale · Password']);
    expect(titles('xyz')).toEqual([]);
    expect(titles('portale xyz')).toEqual([]);
  });

  test('al massimo il numero stabilito di risultati', () => {
    const many: Registry = {
      categories: [],
      apps: Array.from({ length: 80 }, (_, i) => app(`a${i}`, `App ${i}`)),
      secrets: [],
    };
    expect(rank(buildEntries(many, { lock: 'Blocca', open: 'Apri' }), 'app')).toHaveLength(
      PALETTE_MAX_RESULTS,
    );
  });
});
