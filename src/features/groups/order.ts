/**
 * Elenco ordinato delle app di un gruppo: funzioni pure, così il modulo mostra e basta.
 * Il limite è lo stesso del dominio (GROUP_MAX_APPS). (v0.6.0)
 */
import type { Group, RegisteredApp } from '../../lib/ipc';

export const GROUP_MAX_APPS = 20;

/** App del gruppo nell'ordine di avvio; un id che il registro non ha più si salta. */
export function membersOf(group: Group, apps: readonly RegisteredApp[]): RegisteredApp[] {
  const byId = new Map(apps.map((app) => [app.id, app]));
  return group.appIds.flatMap((id) => {
    const app = byId.get(id);
    return app ? [app] : [];
  });
}

/** Sposta la voce di un posto (-1 su, +1 giù); ai bordi l'elenco resta com'è. */
export function move(ids: readonly string[], index: number, step: -1 | 1): string[] {
  const target = index + step;
  if (index < 0 || index >= ids.length || target < 0 || target >= ids.length) return [...ids];
  const next = [...ids];
  const moving = next[index];
  const other = next[target];
  if (moving === undefined || other === undefined) return next;
  next[index] = other;
  next[target] = moving;
  return next;
}

/** Aggiunge in fondo, senza doppioni e senza superare il limite. */
export function add(ids: readonly string[], id: string): string[] {
  if (ids.includes(id) || ids.length >= GROUP_MAX_APPS) return [...ids];
  return [...ids, id];
}

export function remove(ids: readonly string[], id: string): string[] {
  return ids.filter((current) => current !== id);
}
