import { describe, expect, it } from 'vitest';

import { nextIndex, opensMenu } from './menu';

describe('menu delle tessere (v0.7.0)', () => {
  it('le frecce girano in tondo, Home e Fine vanno agli estremi', () => {
    expect(nextIndex(0, 'ArrowDown', 3)).toBe(1);
    expect(nextIndex(2, 'ArrowDown', 3)).toBe(0);
    expect(nextIndex(0, 'ArrowUp', 3)).toBe(2);
    expect(nextIndex(1, 'Home', 3)).toBe(0);
    expect(nextIndex(0, 'End', 3)).toBe(2);
  });

  it('gli altri tasti, e un menu vuoto, non spostano nulla', () => {
    expect(nextIndex(0, 'a', 3)).toBeNull();
    expect(nextIndex(0, 'ArrowDown', 0)).toBeNull();
  });

  it('si apre con il tasto Menu e con Maiusc+F10, non con F10 da solo', () => {
    expect(opensMenu('ContextMenu', false)).toBe(true);
    expect(opensMenu('F10', true)).toBe(true);
    expect(opensMenu('F10', false)).toBe(false);
    expect(opensMenu('Enter', true)).toBe(false);
  });
});
