import { useEffect, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { APP_NAME } from '../../constants/app';
import { errorCode } from '../../lib/errors';
import { getAutostart, setAutostart } from '../../lib/ipc';

interface AutostartSettingsProps {
  onMessage: (message: string) => void;
}

/** Legge lo stato da Windows; funzione asincrona pura, l'effetto applica il risultato. */
async function loadAutostart(): Promise<boolean | null> {
  try {
    return await getAutostart();
  } catch {
    return null;
  }
}

/**
 * Avvio con Windows, spento per default (v0.6.0). All'accesso l'app parte nella tray, senza
 * finestra: la scorciatoia della palette è subito disponibile.
 */
export function AutostartSettings({ onMessage }: AutostartSettingsProps) {
  const { t } = useTranslation();
  const ids = { title: useId(), hint: useId() };
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let mounted = true;
    void loadAutostart().then((next) => mounted && setEnabled(next));
    return () => {
      mounted = false;
    };
  }, []);

  if (enabled === null) return null;

  const choose = (next: boolean) => {
    setBusy(true);
    setError(null);
    setAutostart(next)
      .then((now) => {
        setEnabled(now);
        onMessage(now ? t('settings.autostartOn') : t('settings.autostartOff'));
      })
      .catch((failure: unknown) => setError(errorCode(failure)))
      .finally(() => setBusy(false));
  };

  return (
    <section className="settings" aria-labelledby={ids.title}>
      <h2 id={ids.title} className="settings__title">
        {t('settings.autostartTitle')}
      </h2>
      <div className="settings__group">
        <label className="field--check">
          <input
            type="checkbox"
            checked={enabled}
            disabled={busy}
            aria-describedby={ids.hint}
            onChange={(event) => choose(event.target.checked)}
          />
          {t('settings.autostartLabel', { name: APP_NAME })}
        </label>
        <p id={ids.hint} className="field__hint">
          {t('settings.autostartHint')}
        </p>
        {error && (
          <p className="form__error" role="alert">
            {t(`errors.${error}`, { defaultValue: t('errors.UNKNOWN') })}
          </p>
        )}
      </div>
    </section>
  );
}
