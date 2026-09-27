import { describe, expect, test } from 'vitest';

import { formatShortcut } from './shortcut';

describe('formatShortcut', () => {
  test('traduce solo lo spazio e allarga i separatori', () => {
    expect(formatShortcut('Ctrl+Alt+Space', 'Spazio')).toBe('Ctrl + Alt + Spazio');
    expect(formatShortcut('Ctrl+Alt+P', 'Spazio')).toBe('Ctrl + Alt + P');
  });
});
