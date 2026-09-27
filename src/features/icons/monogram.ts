/**
 * Iniziali delle app senza icona (v0.7.0). I toni sono pietre scure dell'identità: mai il
 * verderame, riservato alla chiave di volta, né il bronzo, riservato alla produzione (A.2).
 */
export const MONOGRAM_TONES = 6;

const WORD = /[\p{L}\p{N}]+/gu;

function firstChar(word: string): string {
  return Array.from(word)[0] ?? '';
}

/**
 * Le prime lettere delle prime due parole ("Portale Clienti" → "PC"), oppure le prime due di
 * una parola sola ("Grafana" → "Gr"). La punteggiatura non conta.
 */
export function initials(name: string): string {
  const words = name.match(WORD) ?? [];
  const [first, second] = words;
  if (first === undefined) return '?';
  if (second !== undefined) return (firstChar(first) + firstChar(second)).toLocaleUpperCase();
  const [head = '', next = ''] = Array.from(first);
  return head.toLocaleUpperCase() + next.toLocaleLowerCase();
}

/** Tono stabile per un nome (FNV-1a a 32 bit): la stessa app ha sempre lo stesso colore. */
export function monogramTone(name: string): number {
  let hash = 0x811c9dc5;
  for (const char of name.trim().toLocaleLowerCase()) {
    hash ^= char.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash % MONOGRAM_TONES;
}
