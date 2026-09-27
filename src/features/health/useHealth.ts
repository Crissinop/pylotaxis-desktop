import { useEffect, useState } from 'react';

import { getHealth, onHealthChanged, type HealthEntry, type HealthStatus } from '../../lib/ipc';

/** Ogni quanto si richiede lo stato con la finestra in vista: Rust ricontrolla solo lo scaduto. */
export const HEALTH_REFRESH_MS = 60_000;

/** Elenco completo da Rust: sostituisce tutto, così un controllo spento sparisce. */
export function fromEntries(entries: readonly HealthEntry[]): Map<string, HealthStatus> {
  const next = new Map<string, HealthStatus>();
  for (const entry of entries) if (entry.status) next.set(entry.id, entry.status);
  return next;
}

/** Un risultato singolo (evento): aggiorna solo la sua app. */
export function withEntry(
  current: ReadonlyMap<string, HealthStatus>,
  entry: HealthEntry,
): Map<string, HealthStatus> {
  const next = new Map(current);
  if (entry.status) next.set(entry.id, entry.status);
  return next;
}

/**
 * Stato delle app web con il controllo acceso (v0.6.0). Con `active` falso (bloccata, altra
 * vista) non chiede nulla; con la pagina nascosta nemmeno. Rust verifica comunque che la
 * finestra sia in vista prima di partire con una richiesta.
 */
export function useHealth(active: boolean): Map<string, HealthStatus> {
  const [statuses, setStatuses] = useState<Map<string, HealthStatus>>(() => new Map());

  useEffect(() => {
    if (!active) return;
    let mounted = true;
    let stop: (() => void) | null = null;
    const refresh = () => {
      if (document.visibilityState !== 'visible') return;
      void getHealth()
        .then((entries) => mounted && setStatuses(fromEntries(entries)))
        .catch(() => undefined);
    };
    onHealthChanged((entry) => mounted && setStatuses((current) => withEntry(current, entry)))
      .then((unlisten) => {
        if (mounted) stop = unlisten;
        else unlisten();
      })
      .catch(() => undefined);
    refresh();
    const timer = window.setInterval(refresh, HEALTH_REFRESH_MS);
    document.addEventListener('visibilitychange', refresh);
    return () => {
      mounted = false;
      stop?.();
      window.clearInterval(timer);
      document.removeEventListener('visibilitychange', refresh);
    };
  }, [active]);

  return active ? statuses : new Map();
}
