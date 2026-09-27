/**
 * Aspetto dell'app (v0.8.0): tema e accento. Sono preferenze dell'interfaccia, non dati del
 * registro: stanno nel localStorage del webview, che le due finestre condividono, si leggono
 * prima del primo disegno e valgono anche da bloccata. Nulla di sensibile.
 */
export type ThemeChoice = 'system' | 'light' | 'dark';
export type AccentChoice = 'verdigris' | 'lapis' | 'porphyry' | 'slate';

export interface Appearance {
  theme: ThemeChoice;
  accent: AccentChoice;
}

export const THEMES: readonly ThemeChoice[] = ['system', 'light', 'dark'];
export const ACCENTS: readonly AccentChoice[] = ['verdigris', 'lapis', 'porphyry', 'slate'];
export const DEFAULT_APPEARANCE: Appearance = { theme: 'system', accent: 'verdigris' };

/** Chiave neutra: il nome dell'app compare solo nei punti elencati in A.2. */
export const APPEARANCE_KEY = 'appearance';

const isTheme = (value: unknown): value is ThemeChoice => THEMES.includes(value as ThemeChoice);
const isAccent = (value: unknown): value is AccentChoice => ACCENTS.includes(value as AccentChoice);

/**
 * Legge le preferenze salvate. Un valore illeggibile, o scritto a mano, torna al predefinito
 * campo per campo: una preferenza sbagliata non rompe l'interfaccia.
 */
export function parseAppearance(raw: string | null): Appearance {
  if (raw === null) return DEFAULT_APPEARANCE;
  let data: unknown;
  try {
    data = JSON.parse(raw);
  } catch {
    return DEFAULT_APPEARANCE;
  }
  const record = typeof data === 'object' && data !== null ? (data as Record<string, unknown>) : {};
  return {
    theme: isTheme(record.theme) ? record.theme : DEFAULT_APPEARANCE.theme,
    accent: isAccent(record.accent) ? record.accent : DEFAULT_APPEARANCE.accent,
  };
}

export function loadAppearance(
  storage: Pick<Storage, 'getItem'> = window.localStorage,
): Appearance {
  try {
    return parseAppearance(storage.getItem(APPEARANCE_KEY));
  } catch {
    // localStorage non disponibile: vale il predefinito, e l'app funziona lo stesso.
    return DEFAULT_APPEARANCE;
  }
}

export function saveAppearance(
  appearance: Appearance,
  storage: Pick<Storage, 'setItem'> = window.localStorage,
): void {
  try {
    storage.setItem(APPEARANCE_KEY, JSON.stringify(appearance));
  } catch {
    // Non salvata: vale fino alla chiusura. Non è un errore da mostrare.
  }
}

/**
 * Applica l'aspetto alla radice del documento. "Come Windows" toglie l'attributo, e il CSS
 * segue il sistema da solo, anche quando cambia mentre l'app è aperta.
 */
export function applyAppearance(
  appearance: Appearance,
  root: Pick<HTMLElement, 'dataset'> = document.documentElement,
): void {
  if (appearance.theme === 'system') delete root.dataset.theme;
  else root.dataset.theme = appearance.theme;
  if (appearance.accent === DEFAULT_APPEARANCE.accent) delete root.dataset.accent;
  else root.dataset.accent = appearance.accent;
}
