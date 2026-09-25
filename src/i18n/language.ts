/** Lingue supportate; la prima è quella di riferimento (A.2). (v0.1.0) */
export const SUPPORTED_LANGUAGES = ['it', 'en'] as const;
export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];

/** Lingua di riferimento e ripiego quando quella di sistema non è supportata. */
export const FALLBACK_LANGUAGE: SupportedLanguage = 'it';

function isSupported(value: string): value is SupportedLanguage {
  return (SUPPORTED_LANGUAGES as readonly string[]).includes(value);
}

/**
 * Sceglie la prima lingua supportata tra le preferenze di sistema, confrontando solo la lingua
 * di base ("en-GB" → "en"). Un elenco esplicito, e non una ricerca per chiave in un oggetto,
 * evita che tag come "constructor" risultino "supportati". (v0.1.0)
 */
export function pickLanguage(preferred: readonly string[]): SupportedLanguage {
  for (const tag of preferred) {
    const base = tag.trim().toLowerCase().split('-')[0] ?? '';
    if (isSupported(base)) return base;
  }
  return FALLBACK_LANGUAGE;
}
