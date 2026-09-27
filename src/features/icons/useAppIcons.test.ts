import { describe, expect, it } from 'vitest';

import { iconKey } from './useAppIcons';

describe('firma delle icone (v0.7.0)', () => {
  it("non dipende dall'ordine delle app", () => {
    const a = { id: 'a', iconRev: '01' };
    const b = { id: 'b', iconRev: null };
    expect(iconKey([a, b])).toBe(iconKey([b, a]));
  });

  it("cambia con un'icona nuova, un'app nuova o un'app tolta", () => {
    const base = iconKey([{ id: 'a', iconRev: '01' }]);
    expect(iconKey([{ id: 'a', iconRev: '02' }])).not.toBe(base);
    expect(iconKey([{ id: 'a', iconRev: null }])).not.toBe(base);
    expect(
      iconKey([
        { id: 'a', iconRev: '01' },
        { id: 'b', iconRev: null },
      ]),
    ).not.toBe(base);
    expect(iconKey([])).not.toBe(base);
  });
});
