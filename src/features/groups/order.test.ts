import { describe, expect, test } from 'vitest';

import type { RegisteredApp } from '../../lib/ipc';
import { GROUP_MAX_APPS, add, membersOf, move, remove } from './order';

describe('ordine del gruppo', () => {
  test('sposta di un posto e resta fermo ai bordi', () => {
    expect(move(['a', 'b', 'c'], 1, -1)).toEqual(['b', 'a', 'c']);
    expect(move(['a', 'b', 'c'], 1, 1)).toEqual(['a', 'c', 'b']);
    expect(move(['a', 'b'], 0, -1)).toEqual(['a', 'b']);
    expect(move(['a', 'b'], 1, 1)).toEqual(['a', 'b']);
  });

  test('aggiunge in fondo senza doppioni né oltre il limite', () => {
    expect(add(['a'], 'b')).toEqual(['a', 'b']);
    expect(add(['a'], 'a')).toEqual(['a']);
    const full = Array.from({ length: GROUP_MAX_APPS }, (_, i) => `${i}`);
    expect(add(full, 'x')).toHaveLength(GROUP_MAX_APPS);
  });

  test("le app seguono l'ordine del gruppo e quelle sparite si saltano", () => {
    const apps = ['a', 'b'].map((id) => ({ id, name: id }) as RegisteredApp);
    const members = membersOf({ id: 'g', name: 'G', appIds: ['b', 'x', 'a'] }, apps);
    expect(members.map((member) => member.id)).toEqual(['b', 'a']);
  });

  test('toglie solo la voce indicata', () => {
    expect(remove(['a', 'b', 'c'], 'b')).toEqual(['a', 'c']);
  });
});
