/** Durate e curve delle animazioni fatte da codice, le stesse del foglio di stile (A.7.7). */
export const DIALOG_EXIT_MS = 150;
export const PALETTE_ENTER_MS = 120;
/** Uscita dei menu delle tessere: la stessa durata dell'entrata (v0.8.0). */
export const MENU_EXIT_MS = 120;
export const EASE_OUT = 'cubic-bezier(0.2, 0.7, 0.2, 1)';
/**
 * Sotto questa età una finestra non si anima in uscita: in sviluppo StrictMode la smonta e la
 * rimonta subito dopo l'apertura, e nessuno chiude una finestra in 50 ms.
 */
export const GHOST_MIN_AGE_MS = 50;

export function prefersReducedMotion(): boolean {
  return (
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  );
}

type FormField = HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement;

/**
 * Uscita di una finestra modale che React sta smontando: al suo posto resta per un istante una
 * copia inerte che svanisce. Ogni chiusura si anima, da qualunque parte arrivi, senza tenere le
 * finestre montate dopo la chiusura. La copia si fa dal DOM già reso, senza HTML ricostruito
 * da testo, e perde gli id per non duplicarli. (v0.7.0)
 */
export function leaveGhost(dialog: HTMLElement): void {
  if (prefersReducedMotion() || !dialog.isConnected) return;
  const rect = dialog.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) return;

  const ghost = document.createElement('div');
  ghost.className = `${dialog.className} ghost`;
  for (const child of Array.from(dialog.childNodes)) ghost.append(child.cloneNode(true));
  // I valori digitati non sono attributi: senza copiarli i campi sparirebbero vuoti.
  const sources = dialog.querySelectorAll<FormField>('input, textarea, select');
  const copies = ghost.querySelectorAll<FormField>('input, textarea, select');
  sources.forEach((source, index) => {
    const copy = copies[index];
    if (!copy) return;
    copy.value = source.value;
    if (source instanceof HTMLInputElement && copy instanceof HTMLInputElement) {
      copy.checked = source.checked;
    }
  });
  for (const withId of Array.from(ghost.querySelectorAll('[id]'))) withId.removeAttribute('id');
  ghost.setAttribute('aria-hidden', 'true');
  ghost.inert = true;
  Object.assign(ghost.style, {
    top: `${rect.top}px`,
    left: `${rect.left}px`,
    width: `${rect.width}px`,
    height: `${rect.height}px`,
  });

  const backdrop = document.createElement('div');
  backdrop.className = 'ghost-backdrop';
  backdrop.setAttribute('aria-hidden', 'true');
  document.body.append(backdrop, ghost);
  window.setTimeout(() => {
    ghost.remove();
    backdrop.remove();
  }, DIALOG_EXIT_MS);
}

/**
 * Entrata della palette a ogni apertura. La finestra resta montata e nascosta tra un'apertura
 * e l'altra, quindi un'animazione CSS non ripartirebbe da sola. (v0.7.0)
 */
export function playEntrance(element: HTMLElement): void {
  if (prefersReducedMotion() || typeof element.animate !== 'function') return;
  element.animate(
    [
      { opacity: 0, transform: 'scale(0.985)' },
      { opacity: 1, transform: 'none' },
    ],
    { duration: PALETTE_ENTER_MS, easing: EASE_OUT },
  );
}
