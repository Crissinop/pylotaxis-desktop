import { describe, expect, test } from 'vitest';

import type { RegisteredApp, Registry } from '../../lib/ipc';
import { PALETTE_MAX_RESULTS, buildEntries, normalize, rank } from './ranking';

const app = (id: string, name: string, tags: string[] = [], categoryId: string | null = null) =>
  ({
    id,
    name,
    kind: 'web',
    target: 'https://example.com/',
    categoryId,
    tags,
    environment: null,
    healthCheck: false,
  }) as RegisteredApp;

const registry: Registry = {
  categories: [{ id: 'c1', name: 'Clienti' }],
  apps: [
    app('1', 'Portale'),
    app('2', 'Report mensile', ['contabilità']),
    app('3', 'Caffè', [], 'c1'),
  ],
  groups: [{ id: 'g1', name: 'Mattino', appIds: ['3', 'x', '1'] }],
  secrets: [
    { id: 's1', appId: '1', label: 'Password', username: 'mario', updatedMs: 0 },
    { id: 's2', appId: 'x', label: 'Orfano', username: null, updatedMs: 0 },
  ],
};

const labels = {
  lock: 'Blocca',
  open: 'Apri',
  group: 'Gruppo',
  environments: { development: 'Sviluppo', test: 'Collaudo', production: 'Produzione' },
};
const entries = buildEntries(registry, labels);
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
  test('senza ricerca: app per nome, poi i gruppi e le azioni, nessun segreto', () => {
    expect(titles('')).toEqual([
      'Caffè',
      'Portale',
      'Report mensile',
      'Gruppo · Mattino',
      'Apri',
      'Blocca',
    ]);
  });

  test("un gruppo tiene l'ordine delle sue app e scarta quelle che non esistono", () => {
    const group = entries.find((entry) => entry.kind === 'group');
    expect(group?.kind === 'group' && group.apps.map((member) => member.id)).toEqual(['3', '1']);
    expect(titles('grup')).toEqual(['Gruppo · Mattino']);
  });

  test("l'ambiente si cerca per nome e la produzione marca il gruppo", () => {
    const withProduction = buildEntries(
      {
        ...registry,
        apps: [{ ...app('p', 'Gestionale'), environment: 'production' }],
        groups: [{ id: 'g2', name: 'Rilascio', appIds: ['p'] }],
      },
      labels,
    );
    expect(rank(withProduction, 'produz').map((entry) => entry.title)).toEqual(['Gestionale']);
    const group = withProduction.find((entry) => entry.kind === 'group');
    expect(group?.kind === 'group' && group.production).toBe(true);
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
      groups: [],
      secrets: [],
    };
    expect(rank(buildEntries(many, labels), 'app')).toHaveLength(PALETTE_MAX_RESULTS);
  });
});
