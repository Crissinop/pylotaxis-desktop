import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Dialog } from '../../components/Dialog';
import { errorCode } from '../../lib/errors';
import {
  copySecret,
  createSecret,
  deleteSecret,
  replaceSecret,
  updateSecret,
  type RegisteredApp,
  type SecretInfo,
} from '../../lib/ipc';

interface SecretsDialogProps {
  app: RegisteredApp;
  secrets: SecretInfo[];
  /** Dopo ogni modifica: il registro si rilegge da Rust e il messaggio va nella barra. */
  onChanged: (message: string) => void;
  onMessage: (message: string) => void;
  /** "Apri e copia": il valore è già negli appunti, resta da avviare l'app. */
  onLaunch: (app: RegisteredApp) => void;
  onClose: () => void;
}

/** Una sola vista alla volta dentro la finestra: rivelazione progressiva (A.8). */
type View =
  | { mode: 'list' }
  | { mode: 'create' }
  | { mode: 'edit'; secret: SecretInfo }
  | { mode: 'replace'; secret: SecretInfo }
  | { mode: 'delete'; secret: SecretInfo };

const LIST: View = { mode: 'list' };

interface SecretFormProps {
  view: Extract<View, { mode: 'create' | 'edit' | 'replace' }>;
  appId: string;
  onDone: (message: string) => void;
  onCancel: () => void;
}

/**
 * Modulo di un segreto. Il campo del valore è in sola scrittura: parte vuoto anche quando si
 * sostituisce, perché il valore attuale non arriva mai al webview. Le regole (lunghezze,
 * etichette doppie, limite del Credential Manager) le applica Rust. (v0.4.0)
 */
function SecretForm({ view, appId, onDone, onCancel }: SecretFormProps) {
  const { t } = useTranslation();
  const ids = { label: useId(), username: useId(), value: useId(), hint: useId(), error: useId() };
  const current = view.mode === 'create' ? null : view.secret;
  const [label, setLabel] = useState(current?.label ?? '');
  const [username, setUsername] = useState(current?.username ?? '');
  const [value, setValue] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const editsFields = view.mode !== 'replace';
  const editsValue = view.mode !== 'edit';
  const complete = (!editsFields || label.trim().length > 0) && (!editsValue || value.length > 0);
  const title =
    view.mode === 'create'
      ? t('secrets.createTitle')
      : view.mode === 'edit'
        ? t('secrets.editTitle', { label: view.secret.label })
        : t('secrets.replaceTitle', { label: view.secret.label });

  const submit = () => {
    setSaving(true);
    setError(null);
    const user = username.trim() === '' ? null : username;
    const request =
      view.mode === 'create'
        ? createSecret(appId, label, user, value)
        : view.mode === 'edit'
          ? updateSecret(view.secret.id, label, user)
          : replaceSecret(view.secret.id, value);
    request
      .then(() => {
        setValue('');
        onDone(t('secrets.saved'));
      })
      .catch((failure: unknown) => setError(errorCode(failure)))
      .finally(() => setSaving(false));
  };

  return (
    <form
      className="form"
      aria-label={title}
      onSubmit={(event) => {
        event.preventDefault();
        if (complete && !saving) submit();
      }}
    >
      <h3 className="secrets__heading">{title}</h3>
      {editsFields && (
        <>
          <div className="field">
            <label htmlFor={ids.label} className="field__label">
              {t('secrets.labelLabel')}
            </label>
            <input
              id={ids.label}
              className="field__input"
              value={label}
              onChange={(event) => setLabel(event.target.value)}
              placeholder={t('secrets.labelPlaceholder')}
              maxLength={60}
              autoFocus
            />
          </div>
          <div className="field">
            <label htmlFor={ids.username} className="field__label">
              {t('secrets.usernameLabel')}
            </label>
            <input
              id={ids.username}
              className="field__input"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              autoComplete="off"
              maxLength={200}
            />
          </div>
        </>
      )}
      {editsValue && (
        <div className="field">
          <label htmlFor={ids.value} className="field__label">
            {view.mode === 'replace' ? t('secrets.newValueLabel') : t('secrets.valueLabel')}
          </label>
          <input
            id={ids.value}
            className="field__input"
            type="password"
            value={value}
            onChange={(event) => setValue(event.target.value)}
            autoComplete="off"
            spellCheck={false}
            aria-describedby={ids.hint}
            autoFocus={!editsFields}
          />
          <p id={ids.hint} className="field__hint">
            {t('secrets.valueHint')}
          </p>
        </div>
      )}
      {error && (
        <p id={ids.error} className="form__error" role="alert">
          {t(`errors.${error}`, { defaultValue: t('errors.UNKNOWN') })}
        </p>
      )}
      <div className="dialog__actions">
        <button type="button" className="button button--secondary" onClick={onCancel}>
          {t('actions.cancel')}
        </button>
        <button type="submit" className="button button--primary" disabled={!complete || saving}>
          {t('secrets.save')}
        </button>
      </div>
    </form>
  );
}

/**
 * Segreti di un'app: elenco, aggiunta, modifica, sostituzione, eliminazione e copia. Del
 * valore non si mostra nulla, nemmeno la lunghezza o le ultime cifre (v0.4.0).
 */
export function SecretsDialog({
  app,
  secrets,
  onChanged,
  onMessage,
  onLaunch,
  onClose,
}: SecretsDialogProps) {
  const { t, i18n } = useTranslation();
  const [view, setView] = useState<View>(LIST);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium' });

  const copy = (secret: SecretInfo, thenLaunch: boolean) => {
    setError(null);
    copySecret(secret.id)
      .then(({ clearAfterMs }) => {
        onMessage(
          t('secrets.copied', { label: secret.label, count: Math.round(clearAfterMs / 1000) }),
        );
        if (thenLaunch) onLaunch(app);
      })
      .catch((failure: unknown) => setError(errorCode(failure)));
  };

  const remove = (secret: SecretInfo) => {
    setBusy(true);
    setError(null);
    deleteSecret(secret.id)
      .then(() => {
        setView(LIST);
        onChanged(t('secrets.deleted'));
      })
      .catch((failure: unknown) => setError(errorCode(failure)))
      .finally(() => setBusy(false));
  };

  const renderList = () => (
    <>
      <p className="dialog__body">{t('secrets.intro')}</p>
      {secrets.length === 0 ? (
        <p className="secrets__empty">{t('secrets.empty')}</p>
      ) : (
        <ul className="secrets__list">
          {secrets.map((secret) => (
            <li key={secret.id} className="secrets__item">
              <div className="secrets__text">
                <span className="secrets__label">{secret.label}</span>
                {secret.username && <span className="secrets__meta">{secret.username}</span>}
                <span className="secrets__meta">
                  {t('secrets.updated', { date: date.format(new Date(secret.updatedMs)) })}
                </span>
              </div>
              <div className="secrets__actions">
                <button
                  type="button"
                  className="button button--secondary"
                  onClick={() => copy(secret, false)}
                  aria-label={t('secrets.copyFor', { label: secret.label })}
                >
                  {t('secrets.copy')}
                </button>
                {app.kind === 'web' && (
                  <button
                    type="button"
                    className="button button--ghost"
                    onClick={() => copy(secret, true)}
                  >
                    {t('secrets.openAndCopy')}
                  </button>
                )}
                <button
                  type="button"
                  className="button button--ghost"
                  onClick={() => setView({ mode: 'edit', secret })}
                >
                  {t('actions.edit')}
                </button>
                <button
                  type="button"
                  className="button button--ghost"
                  onClick={() => setView({ mode: 'replace', secret })}
                >
                  {t('secrets.replace')}
                </button>
                <button
                  type="button"
                  className="button button--ghost"
                  onClick={() => setView({ mode: 'delete', secret })}
                >
                  {t('actions.delete')}
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
      {error && (
        <p className="form__error" role="alert">
          {t(`errors.${error}`, { defaultValue: t('errors.UNKNOWN') })}
        </p>
      )}
      <div className="dialog__actions">
        <button type="button" className="button button--secondary" onClick={onClose}>
          {t('secrets.close')}
        </button>
        <button
          type="button"
          className="button button--primary"
          onClick={() => setView({ mode: 'create' })}
        >
          {t('secrets.add')}
        </button>
      </div>
    </>
  );

  const renderDelete = (secret: SecretInfo) => (
    <>
      <h3 className="secrets__heading">{t('secrets.deleteTitle', { label: secret.label })}</h3>
      <p className="dialog__body">{t('secrets.deleteBody')}</p>
      {error && (
        <p className="form__error" role="alert">
          {t(`errors.${error}`, { defaultValue: t('errors.UNKNOWN') })}
        </p>
      )}
      <div className="dialog__actions">
        <button type="button" className="button button--secondary" onClick={() => setView(LIST)}>
          {t('actions.cancel')}
        </button>
        <button
          type="button"
          className="button button--danger"
          onClick={() => remove(secret)}
          disabled={busy}
          autoFocus
        >
          {t('actions.delete')}
        </button>
      </div>
    </>
  );

  return (
    <Dialog title={t('secrets.title', { name: app.name })} onClose={onClose}>
      {view.mode === 'list' && renderList()}
      {view.mode === 'delete' && renderDelete(view.secret)}
      {(view.mode === 'create' || view.mode === 'edit' || view.mode === 'replace') && (
        <SecretForm
          key={view.mode === 'create' ? 'create' : `${view.mode}-${view.secret.id}`}
          view={view}
          appId={app.id}
          onCancel={() => setView(LIST)}
          onDone={(message) => {
            setView(LIST);
            onChanged(message);
          }}
        />
      )}
    </Dialog>
  );
}
