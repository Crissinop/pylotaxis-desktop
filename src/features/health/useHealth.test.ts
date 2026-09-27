import { describe, expect, test } from 'vitest';

import { fromEntries, withEntry } from './useHealth';

describe('stato delle app', () => {
  test("l'elenco completo sostituisce tutto e ignora le app ancora senza risultato", () => {
    const map = fromEntries([
      { id: 'a', status: 'active' },
      { id: 'b', status: null },
    ]);
    expect([...map]).toEqual([['a', 'active']]);
  });

  test('un evento aggiorna solo la sua app', () => {
    const map = withEntry(fromEntries([{ id: 'a', status: 'active' }]), {
      id: 'b',
      status: 'unreachable',
    });
    expect(map.get('a')).toBe('active');
    expect(map.get('b')).toBe('unreachable');
  });
});
