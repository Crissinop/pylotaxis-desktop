/**
 * Scorciatoia in forma leggibile: i tasti restano quelli del plugin ("Ctrl+Alt+Space"), si
 * traduce solo il nome dello spazio e si allargano i separatori. (v0.5.0)
 */
export function formatShortcut(shortcut: string, spaceLabel: string): string {
  return shortcut
    .split('+')
    .map((key) => (key === 'Space' ? spaceLabel : key))
    .join(' + ');
}
