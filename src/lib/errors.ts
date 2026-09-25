/** Codice usato quando l'errore non arriva da un comando Rust. */
export const UNKNOWN_ERROR = 'UNKNOWN';

/**
 * Estrae il codice stabile da un errore dei comandi (`{ code }`, src-tauri/src/errors.rs).
 * Qualunque altra forma diventa UNKNOWN: l'interfaccia mostra sempre un messaggio tradotto,
 * mai un testo tecnico (A.7.9). (v0.2.0)
 */
export function errorCode(error: unknown): string {
  if (typeof error === 'object' && error !== null && 'code' in error) {
    const { code } = error as { code: unknown };
    if (typeof code === 'string' && /^[A-Z_]+$/.test(code)) return code;
  }
  return UNKNOWN_ERROR;
}
