import { useEffect, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { errorCode } from '../../lib/errors';
import { getShortcut, setShortcut, type ShortcutStatus } from '../../lib/ipc';
import { formatShortcut } from './shortcut';

interface ShortcutSettingsProps {
  onMessage: (message: string) => void;
}

/** Legge lo stato della scorciatoia; funzione asincrona pura, l'effetto applica il risultato. */
async function loadShortcut(): Promise<ShortcutStatus | null> {
  try {
    return await getShortcut();
  } catch {
    return null;
  }
}

/**
 * Scorciatoia della palette: una scelta tra le opzioni che Rust propone. Se Windows non la
 * concede (la usa un'altra app), Rust tiene la precedente e qui lo si dice. (v0.5.0)
 */
export function ShortcutSettings({ onMessage }: ShortcutSettingsProps) {
  const { t } = useTranslation();
  const ids = { title: useId(), select: useId(), hint: useId() };
  const [status, setStatus] = useState<ShortcutStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let mounted = true;
    void loadShortcut().then((next) => mounted && setStatus(next));
    return () => {
      mounted = false;
    };
  }, []);

  if (!status) return null;
  const space = t('keys.space');

  const choose = (shortcut: string) => {
    setBusy(true);
    setError(null);
    setShortcut(shortcut)
      .then((next) => {
        setStatus(next);
        onMessage(t('settings.shortcutSaved'));
      })
      .catch((failure: unknown) => {
        setError(errorCode(failure));
        void loadShortcut().then((next) => next && setStatus(next));
      })
      .finally(() => setBusy(false));
  };

  return (
    <section className="settings" aria-labelledby={ids.title}>
      <h2 id={ids.title} className="settings__title">
        {t('settings.shortcutTitle')}
      </h2>
      <div className="settings__group">
        <p className="settings__text">{t('settings.shortcutBody')}</p>
        <div className="settings__row">
          <label htmlFor={ids.select} className="settings__label">
            {t('settings.shortcutLabel')}
          </label>
          <select
            id={ids.select}
            className="field__input settings__select"
            value={status.shortcut}
            disabled={busy}
            aria-describedby={ids.hint}
            onChange={(event) => choose(event.target.value)}
          >
            {status.options.map((option) => (
              <option key={option} value={option}>
                {formatShortcut(option, space)}
              </option>
            ))}
          </select>
        </div>
        <p id={ids.hint} className={status.active ? 'field__hint' : 'form__error'}>
          {status.active ? t('settings.shortcutActive') : t('settings.shortcutInactive')}
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
