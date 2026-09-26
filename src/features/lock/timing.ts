/**
 * Calcoli di presentazione della schermata di blocco e delle impostazioni. Le regole vere
 * (lunghezza del PIN, attese, limiti dell'inattività) stanno in Rust: qui si decide solo cosa
 * mostrare (A.7.3). (v0.3.0)
 */

/** Lunghezza massima del campo PIN: un aiuto alla digitazione, il controllo è in Rust. */
export const PIN_INPUT_MAX_LENGTH = 16;

/** Minuti proposti per il blocco da inattività; `null` = mai. */
export const IDLE_OPTIONS: readonly (number | null)[] = [1, 5, 15, 30, 60, null];

/** Secondi interi ancora da attendere, per eccesso: "1 secondo" finché non è davvero zero. */
export function secondsLeft(until: number, now: number): number {
  return Math.max(0, Math.ceil((until - now) / 1000));
}

/** Tiene solo le cifre ASCII, fino alla lunghezza massima del campo. */
export function digitsOnly(text: string): string {
  return text.replace(/[^0-9]/g, '').slice(0, PIN_INPUT_MAX_LENGTH);
}

/**
 * Opzioni del menu dell'inattività. Un valore salvato che non è tra quelle proposte (scritto
 * da una versione diversa) compare comunque, al suo posto in ordine: il menu non deve mai
 * mostrare una scelta diversa da quella in vigore.
 */
export function idleChoices(current: number | null): (number | null)[] {
  if (IDLE_OPTIONS.includes(current)) return [...IDLE_OPTIONS];
  const minutes = IDLE_OPTIONS.filter((option): option is number => option !== null);
  return [...[...minutes, current as number].sort((a, b) => a - b), null];
}
