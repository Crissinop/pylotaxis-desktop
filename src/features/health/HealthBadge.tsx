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
