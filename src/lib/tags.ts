/**
 * Conversione tra il campo di testo dei tag e l'elenco inviato a Rust. Solo formato: pulizia,
 * limiti e duplicati li decide il dominio, che resta l'unica fonte delle regole (A.7.3). (v0.2.0)
 */
export function parseTags(text: string): string[] {
  return text
    .split(',')
    .map((tag) => tag.trim())
    .filter((tag) => tag.length > 0);
}

export function formatTags(tags: readonly string[]): string {
  return tags.join(', ');
}
