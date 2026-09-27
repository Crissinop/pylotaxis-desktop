import { describe, expect, test } from 'vitest';

import type { RegisteredApp } from '../../lib/ipc';
import { toSections } from './sections';

const app = (id: string, categoryId: string | null): RegisteredApp => ({
  id,
  name: id,
  kind: 'web',
  target: 'https://example.com/',
  categoryId,
  tags: [],
  environment: null,
  healthCheck: false,
  iconRev: null,
});

describe('toSections', () => {
  test('rispetta l’ordine delle categorie e mette in fondo le app senza categoria', () => {
    const sections = toSections({
      secrets: [],
      groups: [],
      categories: [
        { id: 'b', name: 'Lavoro' },
        { id: 'a', name: 'Casa' },
      ],
      apps: [app('1', 'a'), app('2', null), app('3', 'b')],
    });
    expect(sections.map((s) => s.category?.id ?? null)).toEqual(['b', 'a', null]);
    expect(sections.map((s) => s.apps.map((a) => a.id))).toEqual([['3'], ['1'], ['2']]);
  });

  test('tiene le categorie vuote e omette la sezione senza categoria se non serve', () => {
    const sections = toSections({
      secrets: [],
      groups: [],
      categories: [{ id: 'a', name: 'Casa' }],
      apps: [],
    });
    expect(sections).toEqual([{ category: { id: 'a', name: 'Casa' }, apps: [] }]);
  });

  test('un’app con categoria sconosciuta non sparisce (caso negativo)', () => {
    const sections = toSections({
      secrets: [],
      groups: [],
      categories: [],
      apps: [app('1', 'sconosciuta')],
    });
    expect(sections).toEqual([{ category: null, apps: [app('1', 'sconosciuta')] }]);
  });
});
