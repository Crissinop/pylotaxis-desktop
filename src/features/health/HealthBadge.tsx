import { useTranslation } from 'react-i18next';

import type { HealthStatus } from '../../lib/ipc';

/** Stato di un'app web: un punto colorato e sempre il testo, per chi non distingue i colori. */
export function HealthBadge({ status }: { status: HealthStatus | undefined }) {
  const { t } = useTranslation();
  const key = status ?? 'pending';
  return (
    <span className={`health health--${key}`}>
      <span className="health__dot" aria-hidden="true" />
      {t(`health.${key}`)}
    </span>
  );
}

/**
 * Stato su una tessera (v0.7.0). La forma distingue quanto il colore, pieno se l'app risponde
 * e anello se c'è un problema; il testo resta nel suggerimento e per il lettore di schermo.
 */
export function HealthDot({ id, status }: { id: string; status: HealthStatus | undefined }) {
  const { t } = useTranslation();
  const key = status ?? 'pending';
  const label = t(`health.${key}`);
  return (
    <span
      id={id}
      className={`health-dot health-dot--${key}`}
      role="img"
      aria-label={label}
      title={label}
    />
  );
}
