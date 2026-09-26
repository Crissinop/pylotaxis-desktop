import { useEffect, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { APP_NAME } from '../../constants/app';
import { errorCode } from '../../lib/errors';
import {
  checkHelloAvailability,
  setHelloEnabled,
  setIdleMinutes,
  type LockStatus,
} from '../../lib/ipc';
import { idleChoices } from '../lock/timing';
import { PinDialog, type PinDialogMode } from './PinDialog';

interface SecurityViewProps {
  status: LockStatus;
  onStatus: (status: LockStatus, message: string) => void;
}

type HelloProbe =
  { state: 'checking' } | { state: 'available' } | { state: 'unavailable'; code: string };

/** Chiede a Rust se Windows Hello è pronto. Funzione asincrona pura: l'effetto ne applica il risultato (lezione 33). */
async function probeHello(): Promise<HelloProbe> {
  try {
    await checkHelloAvailability();
    return { state: 'available' };
  } catch (failure: unknown) {
    return { state: 'unavailable', code: errorCode(failure) };
  }
}

const IDLE_NEVER = 'never';

/**
 * Impostazioni di sicurezza. Rivelazione progressiva (A.8): senza PIN c'è una sola azione,
 * impostarlo; con il PIN compaiono Windows Hello, l'inattività, il cambio e la
 * disattivazione. Ogni modifica passa da Rust, che risponde con lo stato aggiornato. (v0.3.0)
 */
export function SecurityView({ status, onStatus }: SecurityViewProps) {
  const { t } = useTranslation();
  const ids = { title: useId(), hello: useId(), helloHint: useId(), idle: useId(), error: useId() };
  const [dialog, setDialog] = useState<PinDialogMode | null>(null);
  const [hello, setHello] = useState<HelloProbe>({ state: 'checking' });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void probeHello().then((result) => {
      if (active) setHello(result);
    });
    return () => {
      active = false;
    };
  }, []);

  const apply = (request: Promise<LockStatus>, message: string) => {
    setBusy(true);
    setError(null);
    request
      .then((next) => onStatus(next, message))
      .catch((failure: unknown) => {
        const code = errorCode(failure);
        if (code !== 'HELLO_CANCELED') setError(code);
      })
      .finally(() => setBusy(false));
  };

  const toggleHello = () => {
    const enable = !status.helloEnabled;
    apply(
      setHelloEnabled(enable, t('security.helloPrompt', { name: APP_NAME })),
      enable ? t('security.helloOn') : t('security.helloOff'),
    );
  };

  // Hello si può sempre spegnere; accenderlo ha senso solo se Windows lo offre.
  const helloSwitchDisabled = busy || (!status.helloEnabled && hello.state !== 'available');
  const helloHint =
    hello.state === 'checking'
      ? t('security.helloChecking')
      : hello.state === 'unavailable'
        ? t(`errors.${hello.code}`, { defaultValue: t('errors.UNKNOWN') })
        : t('security.helloHint');

  return (
    <section className="settings" aria-labelledby={ids.title}>
      <h1 id={ids.title} className="settings__title">
        {t('security.title')}
      </h1>

      <div className="settings__group">
        <h2 className="settings__heading">
          {status.pinSet ? t('security.onTitle') : t('security.offTitle')}
        </h2>
        <p className="settings__text">
          {status.pinSet ? t('security.onBody') : t('security.offBody')}
        </p>
        {!status.pinSet && (
          <button type="button" className="button button--primary" onClick={() => setDialog('set')}>
            {t('security.setPin')}
          </button>
        )}
      </div>

      {status.pinSet && (
        <>
          <div className="settings__group">
            <div className="settings__row">
              <label htmlFor={ids.hello} className="settings__label">
                {t('security.helloLabel')}
              </label>
              <button
                id={ids.hello}
                type="button"
                role="switch"
                className="switch"
                aria-checked={status.helloEnabled}
                aria-describedby={ids.helloHint}
                disabled={helloSwitchDisabled}
                onClick={toggleHello}
              >
                <span className="switch__thumb" aria-hidden="true" />
              </button>
            </div>
            <p id={ids.helloHint} className="field__hint">
              {helloHint}
            </p>
          </div>

          <div className="settings__group">
            <div className="settings__row">
              <label htmlFor={ids.idle} className="settings__label">
                {t('security.idleLabel')}
              </label>
              <select
                id={ids.idle}
                className="field__input settings__select"
                value={status.idleMinutes === null ? IDLE_NEVER : String(status.idleMinutes)}
                disabled={busy}
                onChange={(event) => {
                  const value = event.target.value;
                  apply(
                    setIdleMinutes(value === IDLE_NEVER ? null : Number(value)),
                    t('security.idleSaved'),
                  );
                }}
              >
                {idleChoices(status.idleMinutes).map((minutes) =>
                  minutes === null ? (
                    <option key={IDLE_NEVER} value={IDLE_NEVER}>
                      {t('security.idleNever')}
                    </option>
                  ) : (
                    <option key={minutes} value={String(minutes)}>
                      {t('security.idleMinutes', { count: minutes })}
                    </option>
                  ),
                )}
              </select>
            </div>
          </div>

          <div className="settings__actions">
            <button
              type="button"
              className="button button--secondary"
              onClick={() => setDialog('change')}
            >
              {t('security.changePin')}
            </button>
            <button
              type="button"
              className="button button--danger"
              onClick={() => setDialog('disable')}
            >
              {t('security.disable')}
            </button>
          </div>
        </>
      )}

      {/* Altezza riservata: l'errore compare senza spostare il resto (A.8). */}
      <p id={ids.error} className="form__error settings__error" role="alert">
        {error ? t(`errors.${error}`, { defaultValue: t('errors.UNKNOWN') }) : ''}
      </p>

      {dialog && (
        <PinDialog
          mode={dialog}
          onCancel={() => setDialog(null)}
          onDone={(next) => {
            setDialog(null);
            onStatus(next, dialog === 'disable' ? t('security.disabled') : t('security.pinSaved'));
          }}
        />
      )}
    </section>
  );
}
