import { describe, expect, test } from 'vitest';

import { formatTags, parseTags } from './tags';

describe('tag', () => {
  test('divide per virgole e toglie spazi e voci vuote', () => {
    expect(parseTags(' lavoro, rust ,, ')).toEqual(['lavoro', 'rust']);
    expect(parseTags('')).toEqual([]);
  });

  test('andata e ritorno conservano i tag', () => {
    const tags = ['lavoro', 'rust'];
    expect(parseTags(formatTags(tags))).toEqual(tags);
  });
});
