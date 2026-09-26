import { describe, expect, test } from 'vitest';

import { IDLE_OPTIONS, digitsOnly, idleChoices, secondsLeft } from './timing';

describe('secondsLeft', () => {
  test('arrotonda per eccesso e non scende sotto zero', () => {
    expect(secondsLeft(30_000, 0)).toBe(30);
    expect(secondsLeft(30_000, 29_001)).toBe(1);
    expect(secondsLeft(30_000, 30_000)).toBe(0);
    expect(secondsLeft(30_000, 45_000)).toBe(0);
  });
});

describe('digitsOnly', () => {
  test('tiene solo le cifre ASCII, fino a 16', () => {
    expect(digitsOnly('48 29-15')).toBe('482915');
    expect(digitsOnly('４８２９１５')).toBe('');
    expect(digitsOnly('12345678901234567890')).toBe('1234567890123456');
    expect(digitsOnly('')).toBe('');
  });
});

describe('idleChoices', () => {
  test('propone le opzioni standard quando il valore è tra queste', () => {
    expect(idleChoices(15)).toEqual(IDLE_OPTIONS);
    expect(idleChoices(null)).toEqual(IDLE_OPTIONS);
  });

  test('un valore diverso compare al suo posto, prima di "mai"', () => {
    expect(idleChoices(240)).toEqual([1, 5, 15, 30, 60, 240, null]);
    expect(idleChoices(10)).toEqual([1, 5, 10, 15, 30, 60, null]);
  });
});
