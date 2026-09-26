import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Dialog } from '../../components/Dialog';
import { errorCode } from '../../lib/errors';
import { disableLock, setPin, type LockStatus } from '../../lib/ipc';
import { PIN_INPUT_MAX_LENGTH, digitsOnly } from '../lock/timing';

/** `set`: primo PIN; `change`: serve quello attuale; `disable`: solo quello attuale. */
export type PinDialogMode = 'set' | 'change' | 'disable';

interface PinDialogProps {
  mode: PinDialogMode;
  onDone: (status: LockStatus) => void;
  onCancel: () => void;
}

const TITLE: Record<PinDialogMode, string> = {
  set: 'pin.setTitle',
  change: 'pin.changeTitle',
  disable: 'pin.disableTitle',
};

interface PinFieldProps {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  invalid: boolean;
  describedBy?: string;
  autoFocus?: boolean;
}

/** Campo PIN: cifre soltanto, mascherate, senza completamento automatico. */
function PinField({ id, label, value, onChange, invalid, describedBy, autoFocus }: PinFieldProps) {
  return (
    <div className="field">
      <label htmlFor={id} className="field__label">
        {label}
      </label>
      <input
        id={id}
        className="field__input field__input--pin"
        type="password"
        inputMode="numeric"
        autoComplete="off"
        maxLength={PIN_INPUT_MAX_LENGTH}
        value={value}
        onChange={(event) => onChange(digitsOnly(event.target.value))}
        aria-invalid={invalid}
        aria-describedby={describedBy}
        autoFocus={autoFocus}
      />
    </div>
  );
}

/**
 * Impostazione, cambio e disattivazione del PIN. Qui si controlla solo che i due PIN nuovi
 * coincidano, perché Rust non vede la conferma; lunghezza, cifre banali e PIN attuale li
 * controlla Rust, e l'errore torna qui con il suo codice (A.7.3). (v0.3.0)
 */
export function PinDialog({ mode, onDone, onCancel }: PinDialogProps) {
  const { t } = useTranslation();
  const ids = { current: useId(), next: useId(), repeat: useId(), hint: useId(), error: useId() };
  const [current, setCurrent] = useState('');
  const [next, setNext] = useState('');
  const [repeat, setRepeat] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const needsCurrent = mode !== 'set';
  const needsNew = mode !== 'disable';
  const complete =
    (!needsCurrent || current.length > 0) && (!needsNew || (next.length > 0 && repeat.length > 0));

  const submit = () => {
    if (needsNew && next !== repeat) {
      setError('mismatch');
      return;
    }
    setSaving(true);
    setError(null);
    const request =
      mode === 'disable' ? disableLock(current) : setPin(needsCurrent ? current : null, next);
    request
      .then(onDone)
      .catch((failure: unknown) => {
        setCurrent('');
        setError(errorCode(failure));
      })
      .finally(() => setSaving(false));
  };

  const message =
    error === 'mismatch'
      ? t('pin.mismatch')
      : error
        ? t(`errors.${error}`, { defaultValue: t('errors.UNKNOWN') })
        : null;
  const describedBy = [needsNew ? ids.hint : null, message ? ids.error : null]
    .filter(Boolean)
    .join(' ');

  return (
    <Dialog title={t(TITLE[mode])} onClose={onCancel}>
      <form
        className="form"
        onSubmit={(event) => {
          event.preventDefault();
          if (complete && !saving) submit();
        }}
      >
        {mode === 'disable' && <p className="dialog__body">{t('pin.disableBody')}</p>}
        {needsCurrent && (
          <PinField
            id={ids.current}
            label={t('pin.currentLabel')}
            value={current}
            onChange={setCurrent}
            invalid={error !== null && error !== 'mismatch'}
            describedBy={message ? ids.error : undefined}
            autoFocus
          />
        )}
        {needsNew && (
          <>
            <PinField
              id={ids.next}
              label={t('pin.newLabel')}
              value={next}
              onChange={setNext}
              invalid={error !== null}
              describedBy={describedBy || undefined}
              autoFocus={!needsCurrent}
            />
            <PinField
              id={ids.repeat}
              label={t('pin.repeatLabel')}
              value={repeat}
              onChange={setRepeat}
              invalid={error === 'mismatch'}
              describedBy={message ? ids.error : undefined}
            />
            <p id={ids.hint} className="field__hint">
              {t('pin.hint')}
            </p>
          </>
        )}
        {message && (
          <p id={ids.error} className="form__error" role="alert">
            {message}
          </p>
        )}
        <div className="dialog__actions">
          <button type="button" className="button button--secondary" onClick={onCancel}>
            {t('actions.cancel')}
          </button>
          <button
            type="submit"
            className={mode === 'disable' ? 'button button--danger' : 'button button--primary'}
            disabled={!complete || saving}
          >
            {mode === 'disable' ? t('pin.disableConfirm') : t('pin.save')}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
