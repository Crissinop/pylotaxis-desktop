import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Dialog } from '../../components/Dialog';
import { errorCode } from '../../lib/errors';
import {
  type AppInput,
  type AppKind,
  type Category,
  type Environment,
  ENVIRONMENTS,
  type ExecutablePick,
  pickExecutable,
  type RegisteredApp,
} from '../../lib/ipc';
import { formatTags, parseTags } from '../../lib/tags';

interface AppDialogProps {
  /** Assente in inserimento, presente in modifica. */
  app?: RegisteredApp;
  categories: Category[];
  onSubmit: (input: AppInput) => Promise<void>;
  onCancel: () => void;
}

const KINDS: readonly AppKind[] = ['executable', 'web', 'protocol'];

const KIND_LABEL: Record<AppKind, string> = {
  executable: 'form.kindExecutable',
  web: 'form.kindWeb',
  protocol: 'form.kindProtocol',
};

/**
 * Inserimento e modifica di un'app. Rivelazione progressiva (A.8): prima il tipo, poi la
 * destinazione, poi i dettagli. Un eseguibile si sceglie solo con la finestra di sistema:
 * il modulo conserva un gettone, non un percorso (A.7.10). (v0.2.0)
 */
export function AppDialog({ app, categories, onSubmit, onCancel }: AppDialogProps) {
  const { t } = useTranslation();
  const ids = {
    name: useId(),
    url: useId(),
    uri: useId(),
    category: useId(),
    tags: useId(),
    environment: useId(),
    health: useId(),
    error: useId(),
  };

  const [kind, setKind] = useState<AppKind>(app?.kind ?? 'executable');
  const [pick, setPick] = useState<ExecutablePick | null>(null);
  const [url, setUrl] = useState(app?.kind === 'web' ? app.target : '');
  const [uri, setUri] = useState(app?.kind === 'protocol' ? app.target : '');
  const [name, setName] = useState(app?.name ?? '');
  const [categoryId, setCategoryId] = useState(app?.categoryId ?? '');
  const [tagsText, setTagsText] = useState(formatTags(app?.tags ?? []));
  const [environment, setEnvironment] = useState<Environment | ''>(app?.environment ?? '');
  const [healthCheck, setHealthCheck] = useState(app?.healthCheck ?? false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const keepsExecutable = app?.kind === 'executable' && kind === 'executable' && pick === null;
  const executablePath = pick?.path ?? (keepsExecutable ? app.target : null);
  const hasTarget =
    kind === 'executable' ? executablePath !== null : (kind === 'web' ? url : uri).trim() !== '';

  const choose = () => {
    setBusy(true);
    setError(null);
    pickExecutable(t('form.pickDialogTitle'), t('form.pickFilter'))
      .then((picked) => {
        if (!picked) return;
        setPick(picked);
        if (name.trim() === '') setName(picked.suggestedName);
      })
      .catch((failure: unknown) => setError(errorCode(failure)))
      .finally(() => setBusy(false));
  };

  const submit = () => {
    const target: AppInput['target'] =
      kind === 'executable'
        ? { kind, pickToken: pick?.token ?? null }
        : kind === 'web'
          ? { kind, url }
          : { kind, uri };
    setBusy(true);
    setError(null);
    onSubmit({
      name,
      target,
      categoryId: categoryId || null,
      tags: parseTags(tagsText),
      environment: environment || null,
      // Il controllo dello stato vale solo per le app web (v0.6.0).
      healthCheck: kind === 'web' && healthCheck,
    })
      .catch((failure: unknown) => setError(errorCode(failure)))
      .finally(() => setBusy(false));
  };

  return (
    <Dialog
      title={app ? t('form.editTitle', { name: app.name }) : t('form.createTitle')}
      onClose={onCancel}
    >
      <form
        className="form"
        aria-describedby={error ? ids.error : undefined}
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <fieldset className="field">
          <legend className="field__label">{t('form.kindLabel')}</legend>
          <div className="choice-group">
            {KINDS.map((option) => (
              <label key={option} className="choice">
                <input
                  type="radio"
                  name="kind"
                  value={option}
                  checked={kind === option}
                  onChange={() => setKind(option)}
                />
                {t(KIND_LABEL[option])}
              </label>
            ))}
          </div>
        </fieldset>

        {kind === 'executable' && (
          <div className="field">
            <span className="field__label">{t('form.pickedFile')}</span>
            <p className="field__value">{executablePath ?? t('form.noFilePicked')}</p>
            <button
              type="button"
              className="button button--secondary"
              onClick={choose}
              disabled={busy}
            >
              {executablePath ? t('form.pickAgain') : t('form.pickExecutable')}
            </button>
          </div>
        )}
        {kind === 'web' && (
          <div className="field">
            <label htmlFor={ids.url} className="field__label">
              {t('form.urlLabel')}
            </label>
            <input
              id={ids.url}
              className="field__input"
              type="url"
              inputMode="url"
              value={url}
              onChange={(event) => setUrl(event.target.value)}
              placeholder="https://"
            />
            <span className="field__hint">{t('form.urlHint')}</span>
          </div>
        )}
        {kind === 'protocol' && (
          <div className="field">
            <label htmlFor={ids.uri} className="field__label">
              {t('form.uriLabel')}
            </label>
            <input
              id={ids.uri}
              className="field__input"
              value={uri}
              onChange={(event) => setUri(event.target.value)}
            />
            <span className="field__hint">{t('form.uriHint')}</span>
          </div>
        )}

        {hasTarget && (
          <>
            <div className="field">
              <label htmlFor={ids.name} className="field__label">
                {t('form.nameLabel')}
              </label>
              <input
                id={ids.name}
                className="field__input"
                value={name}
                onChange={(event) => setName(event.target.value)}
              />
            </div>
            <div className="field">
              <label htmlFor={ids.category} className="field__label">
                {t('form.categoryLabel')}
              </label>
              <select
                id={ids.category}
                className="field__input"
                value={categoryId}
                onChange={(event) => setCategoryId(event.target.value)}
              >
                <option value="">{t('registry.uncategorized')}</option>
                {categories.map((category) => (
                  <option key={category.id} value={category.id}>
                    {category.name}
                  </option>
                ))}
              </select>
            </div>
            <div className="field">
              <label htmlFor={ids.environment} className="field__label">
                {t('form.environmentLabel')}
              </label>
              <select
                id={ids.environment}
                className="field__input"
                value={environment}
                onChange={(event) =>
                  setEnvironment(ENVIRONMENTS.find((code) => code === event.target.value) ?? '')
                }
              >
                <option value="">{t('environment.none')}</option>
                {ENVIRONMENTS.map((code) => (
                  <option key={code} value={code}>
                    {t(`environment.${code}`)}
                  </option>
                ))}
              </select>
            </div>
            <div className="field">
              <label htmlFor={ids.tags} className="field__label">
                {t('form.tagsLabel')}
              </label>
              <input
                id={ids.tags}
                className="field__input"
                value={tagsText}
                onChange={(event) => setTagsText(event.target.value)}
              />
              <span className="field__hint">{t('form.tagsHint')}</span>
            </div>
            {kind === 'web' && (
              <div className="field">
                <label className="field--check">
                  <input
                    type="checkbox"
                    checked={healthCheck}
                    aria-describedby={ids.health}
                    onChange={(event) => setHealthCheck(event.target.checked)}
                  />
                  {t('form.healthLabel')}
                </label>
                <span id={ids.health} className="field__hint">
                  {t('form.healthHint')}
                </span>
              </div>
            )}
          </>
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
          <button type="submit" className="button button--primary" disabled={busy || !hasTarget}>
            {t('actions.save')}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
