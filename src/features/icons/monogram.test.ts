import { describe, expect, it } from 'vitest';

import { MONOGRAM_TONES, initials, monogramTone } from './monogram';

describe('iniziali (v0.7.0)', () => {
  it('prendono le prime due parole, o due lettere di una parola sola', () => {
    expect(initials('Portale Clienti')).toBe('PC');
    expect(initials('grafana')).toBe('Gr');
    expect(initials('  pgAdmin 4')).toBe('P4');
    expect(initials('über tool')).toBe('ÜT');
    expect(initials('Ω')).toBe('Ω');
  });

  it('ignorano la punteggiatura e non restano mai vuote', () => {
    expect(initials('[dev] API')).toBe('DA');
    expect(initials('---')).toBe('?');
    expect(initials('')).toBe('?');
  });
});

describe('tono delle iniziali', () => {
  it('è stabile per lo stesso nome, a meno di spazi e maiuscole', () => {
    expect(monogramTone('Grafana')).toBe(monogramTone('  grafana '));
  });

  it('resta tra i toni previsti e distingue nomi diversi', () => {
    const names = ['Grafana', 'Jira', 'Portale', 'Posta', 'Vault', 'Build', 'Wiki', 'CRM'];
    const tones = names.map(monogramTone);
    for (const tone of tones) {
      expect(tone).toBeGreaterThanOrEqual(0);
      expect(tone).toBeLessThan(MONOGRAM_TONES);
    }
    expect(new Set(tones).size).toBeGreaterThanOrEqual(4);
  });
});
