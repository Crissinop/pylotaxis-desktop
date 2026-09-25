import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { errorCode } from '../lib/errors';
import { Dialog } from './Dialog';

interface NameDialogProps {
  title: string;
  label: string;
  initialName?: string;
  /** Salva il nome; se Rust lo rifiuta, l'errore resta nella finestra. */
  onSubmit: (name: string) => Promise<void>;
  onCancel: () => void;
}

export function NameDialog({
  title,
  label,
  initialName = '',
  onSubmit,
  onCancel,
}: NameDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(initialName);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const inputId = useId();
  const errorId = useId();

  return (
    <Dialog title={title} onClose={onCancel}>
      <form
        className="form"
        onSubmit={(event) => {
          event.preventDefault();
          setSaving(true);
          onSubmit(name)
            .catch((failure: unknown) => setError(errorCode(failure)))
            .finally(() => setSaving(false));
        }}
      >
        <div className="field">
          <label htmlFor={inputId} className="field__label">
            {label}
          </label>
          <input
            id={inputId}
            className="field__input"
            value={name}
            onChange={(event) => setName(event.target.value)}
            aria-invalid={error !== null}
            aria-describedby={error ? errorId : undefined}
            autoFocus
          />
        </div>
        {error && (
          <p id={errorId} className="form__error" role="alert">
            {t(`errors.${error}`, { defaultValue: t('errors.UNKNOWN') })}
          </p>
        )}
        <div className="dialog__actions">
          <button type="button" className="button button--secondary" onClick={onCancel}>
            {t('actions.cancel')}
          </button>
          <button type="submit" className="button button--primary" disabled={saving}>
            {t('actions.save')}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
