import { describe, expect, it } from 'vitest';

import { EDITABLE_SELECTOR, keepsNativeMenu } from './contextMenu';

/** Elemento finto: `closest` trova un antenato solo se `editable`. */
const element = (editable: boolean) => ({
  closest: (selector: string) => (editable && selector === EDITABLE_SELECTOR ? {} : null),
});

describe('menu del tasto destro (v0.7.0)', () => {
  it('resta nei campi di testo, dove si copia e si incolla', () => {
    expect(keepsNativeMenu(element(true) as unknown as EventTarget)).toBe(true);
  });

  it('sparisce altrove, barra del titolo compresa', () => {
    expect(keepsNativeMenu(element(false) as unknown as EventTarget)).toBe(false);
    expect(keepsNativeMenu(null)).toBe(false);
    expect(keepsNativeMenu({} as EventTarget)).toBe(false);
  });
});
