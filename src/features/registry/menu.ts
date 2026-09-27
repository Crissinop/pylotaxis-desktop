/** Voce attiva dopo un tasto nel menu di una tessera; `null` se il tasto non sposta. (v0.7.0) */
export function nextIndex(current: number, key: string, count: number): number | null {
  if (count <= 0) return null;
  switch (key) {
    case 'ArrowDown':
      return (current + 1) % count;
    case 'ArrowUp':
      return (current - 1 + count) % count;
    case 'Home':
      return 0;
    case 'End':
      return count - 1;
    default:
      return null;
  }
}

/** Tasti che aprono il menu delle azioni come il tasto destro: Menu e Maiusc+F10. */
export function opensMenu(key: string, shiftKey: boolean): boolean {
  return key === 'ContextMenu' || (shiftKey && key === 'F10');
}
