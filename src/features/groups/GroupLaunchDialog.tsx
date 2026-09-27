import { useTranslation } from 'react-i18next';

import { Dialog } from '../../components/Dialog';
import { EnvironmentBadge } from '../../components/EnvironmentBadge';
import type { Group, RegisteredApp } from '../../lib/ipc';
import { membersOf } from './order';

interface GroupLaunchDialogProps {
  group: Group;
  apps: RegisteredApp[];
  onConfirm: () => void;
  onCancel: () => void;
}

/** Mostra che cosa aprirà il gruppo prima di aprirlo (A.8). (v0.6.0) */
export function GroupLaunchDialog({ group, apps, onConfirm, onCancel }: GroupLaunchDialogProps) {
  const { t } = useTranslation();
  const members = membersOf(group, apps);
  const production = members.some((app) => app.environment === 'production');
  return (
    <Dialog title={t('groups.launchTitle', { name: group.name })} onClose={onCancel}>
      <ol className="group-order">
        {members.map((app) => (
          <li key={app.id} className="group-order__item">
            <span className="group-order__name">{app.name}</span>
            <EnvironmentBadge environment={app.environment} />
          </li>
        ))}
      </ol>
      {production && <p className="group-launch__warning">{t('groups.containsProduction')}</p>}
      <div className="dialog__actions">
        <button type="button" className="button button--secondary" onClick={onCancel}>
          {t('actions.cancel')}
        </button>
        <button
          type="button"
          className="button button--primary"
          disabled={members.length === 0}
          onClick={onConfirm}
        >
          {t('groups.launchConfirm', { count: members.length })}
        </button>
      </div>
    </Dialog>
  );
}
