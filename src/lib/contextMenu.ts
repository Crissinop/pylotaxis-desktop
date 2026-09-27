/**
 * Il menu del webview (Indietro, Aggiorna, Salva con nome, Stampa) non appartiene a un'app
 * desktop, e "Aggiorna" ricaricherebbe l'interfaccia. Lo script di trascinamento di Tauri
 * gestisce solo il tasto sinistro: sulla barra del titolo il destro arrivava al webview. Il
 * menu resta solo dove si scrive, per copiare e incollare. (v0.7.0)
 */
export const EDITABLE_SELECTOR = 'input, textarea, [contenteditable="true"]';

interface MaybeElement {
  closest?: (selector: string) => unknown;
}

/** Vero se il tasto destro cade in un campo di testo, dove il menu di sistema serve. */
export function keepsNativeMenu(target: EventTarget | null): boolean {
  const element = target as MaybeElement | null;
  return typeof element?.closest === 'function' && element.closest(EDITABLE_SELECTOR) !== null;
}

export function disableDefaultContextMenu(root: Document = document): void {
  root.addEventListener('contextmenu', (event) => {
    if (!keepsNativeMenu(event.target)) event.preventDefault();
  });
}
