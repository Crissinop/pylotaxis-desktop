import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Dialog } from '../../components/Dialog';
import { EnvironmentBadge } from '../../components/EnvironmentBadge';
import { errorCode } from '../../lib/errors';
import type { Group, GroupLaunchResult, RegisteredApp } from '../../lib/ipc';

interface GroupResultDialogProps {
  group: Group;
  result: GroupLaunchResult;
  apps: RegisteredApp[];
  /** Avvia un eseguibile cambiato dopo la conferma esplicita, uno per uno. */
  onConfirmChanged: (app: RegisteredApp) => Promise<void>;
  onClose: () => void;
}

/**
 * Esito di un avvio di gruppo quando qualcosa non è partito (v0.6.0). Un eseguibile cambiato
 * non parte in blocco: qui lo si conferma, uno per uno. Le altre app mostrano il loro errore.
 */
export function GroupResultDialog({
  group,
  result,
  apps,
  onConfirmChanged,
  onClose,
}: GroupResultDialogProps) {
  const { t } = useTranslation();
  const byId = new Map(apps.map((app) => [app.id, app]));
  const [pending, setPending] = useState<string[]>(result.changed);
  const [errors, setErrors] = useState<Record<string, string>>({});

  const confirm = (app: RegisteredApp) => {
    onConfirmChanged(app)
      .then(() => setPending((current) => current.filter((id) => id !== app.id)))
      .catch((failure: unknown) =>
        setErrors((current) => ({ ...current, [app.id]: errorCode(failure) })),
      );
  };
  const errorText = (code: string) => t(`errors.${code}`, { defaultValue: t('errors.UNKNOWN') });

  return (
    <Dialog title={t('groups.resultTitle', { name: group.name })} onClose={onClose}>
      <p className="settings__text">
        {t('groups.resultLaunched', { count: result.launched.length })}
      </p>
      {pending.length > 0 && (
        <>
          <p className="settings__text">{t('groups.resultChanged')}</p>
          <ul className="group-order">
            {pending.map((id) => {
              const app = byId.get(id);
              if (!app) return null;
              return (
                <li key={id} className="group-order__item">
                  <span className="group-order__name">{app.name}</span>
                  <EnvironmentBadge environment={app.environment} />
                  <span className="group-order__actions">
                    <button
                      type="button"
                      className="button button--secondary"
                      onClick={() => confirm(app)}
                    >
                      {t('groups.confirmLaunch')}
                    </button>
                  </span>
                  {errors[id] && <span className="form__error">{errorText(errors[id])}</span>}
                </li>
              );
            })}
          </ul>
        </>
      )}
      {result.failed.length > 0 && (
        <ul className="group-order">
          {result.failed.map(({ id, code }) => (
            <li key={id} className="group-order__item">
              <span className="group-order__name">{byId.get(id)?.name ?? id}</span>
              <span className="form__error">{errorText(code)}</span>
            </li>
          ))}
        </ul>
      )}
      <div className="dialog__actions">
        <button type="button" className="button button--primary" onClick={onClose}>
          {t('actions.close')}
        </button>
      </div>
    </Dialog>
  );
}
