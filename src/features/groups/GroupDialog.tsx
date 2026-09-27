import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Dialog } from '../../components/Dialog';
import { EnvironmentBadge } from '../../components/EnvironmentBadge';
import { errorCode } from '../../lib/errors';
import type { Group, GroupInput, RegisteredApp } from '../../lib/ipc';
import { GROUP_MAX_APPS, add, move, remove } from './order';

interface GroupDialogProps {
  group?: Group;
  apps: RegisteredApp[];
  onSubmit: (input: GroupInput) => Promise<void>;
  onCancel: () => void;
}

/**
 * Crea o modifica un gruppo: nome e app in ordine di avvio. Tastiera prima di tutto: si
 * aggiunge da un elenco e si riordina con ↑ ↓, senza trascinamenti. (v0.6.0)
 */
export function GroupDialog({ group, apps, onSubmit, onCancel }: GroupDialogProps) {
  const { t } = useTranslation();
  const ids = { name: useId(), list: useId(), add: useId(), hint: useId(), error: useId() };
  const [name, setName] = useState(group?.name ?? '');
  const [appIds, setAppIds] = useState<string[]>(group?.appIds ?? []);
  const [candidate, setCandidate] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const byId = new Map(apps.map((app) => [app.id, app]));
  const available = apps.filter((app) => !appIds.includes(app.id));
  const full = appIds.length >= GROUP_MAX_APPS;
  const optionLabel = (app: RegisteredApp) =>
    app.environment ? `${app.name} · ${t(`environment.${app.environment}`)}` : app.name;

  const submit = () => {
    setBusy(true);
    setError(null);
    onSubmit({ name, appIds })
      .catch((failure: unknown) => setError(errorCode(failure)))
      .finally(() => setBusy(false));
  };

  return (
    <Dialog
      title={group ? t('groups.editTitle', { name: group.name }) : t('groups.createTitle')}
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
        <div className="field">
          <label htmlFor={ids.name} className="field__label">
            {t('groups.nameLabel')}
          </label>
          <input
            id={ids.name}
            className="field__input"
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </div>
        <div className="field">
          <span id={ids.list} className="field__label">
            {t('groups.appsLabel')}
          </span>
          {appIds.length === 0 ? (
            <p className="field__hint">{t('groups.noApps')}</p>
          ) : (
            <ol className="group-order" aria-labelledby={ids.list}>
              {appIds.map((id, index) => {
                const app = byId.get(id);
                if (!app) return null;
                return (
                  <li key={id} className="group-order__item">
                    <span className="group-order__name">{app.name}</span>
                    <EnvironmentBadge environment={app.environment} />
                    <span className="group-order__actions">
                      <button
                        type="button"
                        className="button button--ghost"
                        disabled={index === 0}
                        aria-label={t('groups.moveUp', { name: app.name })}
                        onClick={() => setAppIds(move(appIds, index, -1))}
                      >
                        ↑
                      </button>
                      <button
                        type="button"
                        className="button button--ghost"
                        disabled={index === appIds.length - 1}
                        aria-label={t('groups.moveDown', { name: app.name })}
                        onClick={() => setAppIds(move(appIds, index, 1))}
                      >
                        ↓
                      </button>
                      <button
                        type="button"
                        className="button button--ghost"
                        aria-label={t('groups.remove', { name: app.name })}
                        onClick={() => setAppIds(remove(appIds, id))}
                      >
                        ✕
                      </button>
                    </span>
                  </li>
                );
              })}
            </ol>
          )}
        </div>
        <div className="field">
          <label htmlFor={ids.add} className="field__label">
            {t('groups.addLabel')}
          </label>
          <div className="group-add">
            <select
              id={ids.add}
              className="field__input"
              value={candidate}
              disabled={full || available.length === 0}
              aria-describedby={ids.hint}
              onChange={(event) => setCandidate(event.target.value)}
            >
              <option value="">{t('groups.choose')}</option>
              {available.map((app) => (
                <option key={app.id} value={app.id}>
                  {optionLabel(app)}
                </option>
              ))}
            </select>
            <button
              type="button"
              className="button button--secondary"
              disabled={candidate === '' || full}
              onClick={() => {
                setAppIds(add(appIds, candidate));
                setCandidate('');
              }}
            >
              {t('groups.add')}
            </button>
          </div>
          <span id={ids.hint} className="field__hint">
            {t('groups.limit', { max: GROUP_MAX_APPS })}
          </span>
        </div>

        {error && (
          <p id={ids.error} className="form__error" role="alert">
            {t(`errors.${error}`, { defaultValue: t('errors.UNKNOWN') })}
          </p>
        )}

        <div className="dialog__actions">
          <button type="button" className="button button--secondary" onClick={onCancel}>
            {t('actions.cancel')}
          </button>
          <button type="submit" className="button button--primary" disabled={busy}>
            {t('actions.save')}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
