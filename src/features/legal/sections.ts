/**
 * Sezioni dei testi legali, nell'ordine in cui si leggono (v0.8.0). Il testo è nei file di
 * lingua (`legal.terms.<id>` e `legal.privacy.<id>`), con titolo e corpo per ogni sezione.
 */
export const TERMS_SECTIONS = ['use', 'warranty', 'data', 'apps', 'security', 'changes'] as const;
export const PRIVACY_SECTIONS = [
  'collected',
  'where',
  'outgoing',
  'hello',
  'clipboard',
  'erase',
  'developer',
] as const;

export type LegalDocument = 'terms' | 'privacy';

export const SECTIONS: Record<LegalDocument, readonly string[]> = {
  terms: TERMS_SECTIONS,
  privacy: PRIVACY_SECTIONS,
};

/** Data dell'ultima revisione dei testi, uguale in tutte le lingue. */
export const LEGAL_UPDATED = new Date(2026, 8, 27);
