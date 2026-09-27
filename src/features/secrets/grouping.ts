import type { SecretInfo } from '../../lib/ipc';

/**
 * Segreti di un'app, per etichetta. Rust li manda tutti insieme con il loro `appId`: qui si
 * decide solo come mostrarli (A.7.3). (v0.4.0)
 */
export function secretsOf(secrets: readonly SecretInfo[], appId: string): SecretInfo[] {
  return secrets
    .filter((secret) => secret.appId === appId)
    .sort((a, b) => a.label.localeCompare(b.label, undefined, { sensitivity: 'base' }));
}

/** Quanti segreti ha ogni app, per il pulsante nella riga. */
export function countByApp(secrets: readonly SecretInfo[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const secret of secrets) counts.set(secret.appId, (counts.get(secret.appId) ?? 0) + 1);
  return counts;
}
