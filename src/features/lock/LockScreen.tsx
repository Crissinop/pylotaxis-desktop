import { useEffect, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { APP_NAME } from '../../constants/app';
import { errorCode } from '../../lib/errors';
import { getLockStatus, unlockWithHello, unlockWithPin, type LockStatus } from '../../lib/ipc';
import { PIN_INPUT_MAX_LENGTH, digitsOnly, secondsLeft } from './timing';

interface LockScreenProps {
  status: LockStatus;
  /** Istante (ms) in cui è arrivato `status`: da qui si conta l'attesa del PIN. */
  receivedAt: number;
  onUnlocked: (status: LockStatus) => void;
}

/** Attesa in corso dopo troppi tentativi: quando finisce e quanti secondi mancano. */
interface Wait {
  until: number;
  seconds: number;
}

function waitFrom(retryAfterMs: number, since: number): Wait | null {
  if (retryAfterMs <= 0) return null;
  const until = since + retryAfterMs;
  return { until, seconds: secondsLeft(until, since) };
}

/**
 * Schermata di blocco. È solo presentazione: il blocco vero lo applica Rust, che rifiuta
 * ogni comando tranne lo sblocco (src-tauri/src/lock.rs). Windows Hello non parte da solo:
 * il pulsante ha il fuoco e basta Invio, così il prompt compare solo dopo un gesto e con
 * l'app in primo piano (A.7.5). (v0.3.0)
 */
export function LockScreen({ status, receivedAt, onUnlocked }: LockScreenProps) {
  const { t } = useTranslation();
  const ids = { title: useId(), pin: useId(), error: useId() };
  const [pin, setPin] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [helloBusy, setHelloBusy] = useState(false);
  const [wait, setWait] = useState<Wait | null>(() => waitFrom(status.retryAfterMs, receivedAt));

  // Il conto alla rovescia si aggiorna solo nel callback del timer, mai nel corpo
  // dell'effetto (lezione 33); il timer dipende dalla scadenza, non da ogni secondo.
  // Alla fine l'attesa sparisce e il campo si riabilita.
  const until = wait?.until ?? null;
  useEffect(() => {
    if (until === null) return;
    const timer = window.setInterval(() => {
      const seconds = secondsLeft(until, Date.now());
      setWait((current) => {
        if (seconds === 0) return null;
        return current?.seconds === seconds ? current : { until, seconds };
      });
    }, 250);
    return () => window.clearInterval(timer);
  }, [until]);

  const fail = (failure: unknown) => {
    const code = errorCode(failure);
    // Un prompt annullato (dall'utente o da uno sblocco con il PIN) non è un errore.
    if (code !== 'HELLO_CANCELED') setError(code);
    // Dopo un PIN sbagliato l'attesa la decide Rust: la si rilegge invece di calcolarla qui.
    if (code === 'PIN_WRONG' || code === 'PIN_THROTTLED') {
      getLockStatus()
        .then((next) => setWait(waitFrom(next.retryAfterMs, Date.now())))
        .catch(() => undefined);
    }
  };

  const submitPin = () => {
    setChecking(true);
    setError(null);
    unlockWithPin(pin)
      .then(onUnlocked)
      .catch((failure: unknown) => {
        setPin('');
        fail(failure);
      })
      .finally(() => setChecking(false));
  };

  const startHello = () => {
    setHelloBusy(true);
    setError(null);
    unlockWithHello(t('lock.helloPrompt', { name: APP_NAME }))
      .then(onUnlocked)
      .catch(fail)
      .finally(() => setHelloBusy(false));
  };

  const pinDisabled = checking || wait !== null;
  const describedBy = error || wait ? ids.error : undefined;

  return (
    <section className="lock" aria-labelledby={ids.title}>
      <h1 id={ids.title} className="lock__title">
        {t('lock.title')}
      </h1>
      <p className="lock__body">{status.helloEnabled ? t('lock.bodyHello') : t('lock.bodyPin')}</p>

      {status.helloEnabled && (
        <button
          type="button"
          className="button button--primary lock__hello"
          onClick={startHello}
          disabled={helloBusy}
          autoFocus
        >
          {helloBusy ? t('lock.helloWaiting') : t('lock.helloButton')}
        </button>
      )}

      <form
        className="lock__form"
        onSubmit={(event) => {
          event.preventDefault();
          if (!pinDisabled && pin.length > 0) submitPin();
        }}
      >
        <label htmlFor={ids.pin} className="field__label">
          {t('lock.pinLabel')}
        </label>
        <div className="lock__pin-row">
          <input
            id={ids.pin}
            className="field__input lock__pin"
            type="password"
            inputMode="numeric"
            autoComplete="off"
            maxLength={PIN_INPUT_MAX_LENGTH}
            value={pin}
            onChange={(event) => setPin(digitsOnly(event.target.value))}
            disabled={pinDisabled}
            aria-invalid={error !== null}
            aria-describedby={describedBy}
            autoFocus={!status.helloEnabled}
          />
          <button
            type="submit"
            className={status.helloEnabled ? 'button button--secondary' : 'button button--primary'}
            disabled={pinDisabled || pin.length === 0}
          >
            {t('lock.pinSubmit')}
          </button>
        </div>
        {/* Altezza riservata: messaggi e conto alla rovescia non spostano il layout (A.8). */}
        <p id={ids.error} className="lock__message" role="alert">
          {wait
            ? t('lock.retryIn', { count: wait.seconds })
            : error
              ? t(`errors.${error}`, { defaultValue: t('errors.UNKNOWN') })
              : ''}
        </p>
      </form>
    </section>
  );
}
