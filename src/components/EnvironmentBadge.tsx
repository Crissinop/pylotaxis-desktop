import { useTranslation } from 'react-i18next';

import type { Environment } from '../lib/ipc';

/**
 * Ambiente di un'app (v0.6.0). La produzione è l'unico ambiente a colori (bronzo), e porta
 * sempre la sua etichetta: il segnale non si affida al solo colore (A.7.8, A.8).
 */
export function EnvironmentBadge({ environment }: { environment: Environment | null }) {
  const { t } = useTranslation();
  if (!environment) return null;
  return (
    <span className={`env-badge env-badge--${environment}`}>{t(`environment.${environment}`)}</span>
  );
}
