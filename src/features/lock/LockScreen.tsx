import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Mark } from '../../components/Mark';
import { APP_NAME } from '../../constants/app';
import { errorCode } from '../../lib/errors';
import { getLockStatus, unlockWithHello, unlockWithPin, type LockStatus } from '../../lib/ipc';
import { PIN_INPUT_MAX_LENGTH, digitsOnly, secondsLeft, shouldAutoSubmit } from './timing';

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
 *
 * Dalla v0.7.0 il PIN si verifica da sé quando le cifre raggiungono la sua lunghezza, come sui
 * telefoni; se la lunghezza non è ancora nota si conferma con Invio e Rust la impara. Il campo
 * resta attivo durante la verifica (solo in lettura), così il fuoco non si perde.
 */
export function LockScreen({ status, receivedAt, onUnlocked }: LockScreenProps) {
  const { t } = useTranslation();
  const ids = { title: useId(), pin: useId(), error: useId() };
  const input = useRef<HTMLInputElement>(null);
  const [pin, setPin] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [shaking, setShaking] = useState(false);
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

  const submitPin = (value: string) => {
    setChecking(true);
    setError(null);
    unlockWithPin(value)
      .then(onUnlocked)
      .catch((failure: unknown) => {
        setPin('');
        setShaking(true);
        fail(failure);
      })
      .finally(() => {
        setChecking(false);
        input.current?.focus();
      });
  };

  const startHello = () => {
    setHelloBusy(true);
    setError(null);
    unlockWithHello(t('lock.helloPrompt', { name: APP_NAME }))
      .then(onUnlocked)
      .catch(fail)
      .finally(() => setHelloBusy(false));
  };

  const waiting = wait !== null;
  const describedBy = error || wait ? ids.error : undefined;
  const maxLength = status.pinLength ?? PIN_INPUT_MAX_LENGTH;
  const message = wait
    ? t('lock.retryIn', { count: wait.seconds })
    : error
      ? t(`errors.${error}`, { defaultValue: t('errors.UNKNOWN') })
      : null;

  return (
    <section className="lock" aria-labelledby={ids.title}>
      <div className="lock__card">
        <Mark size={56} />
        <h1 id={ids.title} className="lock__title">
          {APP_NAME}
        </h1>
        <p className="lock__body">
          {status.helloEnabled ? t('lock.bodyHello') : t('lock.bodyPin')}
        </p>

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
            if (!checking && !waiting && pin.length > 0) submitPin(pin);
          }}
        >
          <label htmlFor={ids.pin} className="visually-hidden">
            {t('lock.pinLabel')}
          </label>
          <input
            ref={input}
            id={ids.pin}
            className={shaking ? 'lock__pin lock__pin--shake' : 'lock__pin'}
            type="password"
            inputMode="numeric"
            autoComplete="off"
            placeholder={t('lock.pinLabel')}
            maxLength={maxLength}
            value={pin}
            onChange={(event) => {
              const next = digitsOnly(event.target.value).slice(0, maxLength);
              setPin(next);
              if (!checking && shouldAutoSubmit(next, status.pinLength)) submitPin(next);
            }}
            onAnimationEnd={() => setShaking(false)}
            readOnly={checking}
            disabled={waiting}
            aria-invalid={error !== null}
            aria-describedby={describedBy}
            autoFocus={!status.helloEnabled}
          />
          {/* Altezza riservata: messaggi e conto alla rovescia non spostano il layout (A.8). */}
          <p
            id={ids.error}
            className={message ? 'lock__message lock__message--error' : 'lock__message'}
            role="alert"
          >
            {message ?? (status.pinLength === null ? t('lock.enterHint') : '')}
          </p>
        </form>
      </div>
    </section>
  );
}
