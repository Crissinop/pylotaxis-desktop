import { useTranslation } from 'react-i18next';

import { EnvironmentBadge } from '../../components/EnvironmentBadge';
import type { Category, Group, HealthStatus, RegisteredApp, Registry } from '../../lib/ipc';
import { HealthBadge } from '../health/HealthBadge';
import { countByApp } from '../secrets/grouping';
import { toSections } from './sections';

interface RegistryViewProps {
  registry: Registry;
  /** Stato delle app con il controllo acceso (v0.6.0). */
  health: ReadonlyMap<string, HealthStatus>;
  onLaunchGroup: (group: Group) => void;
  onCreateGroup: () => void;
  onEditGroup: (group: Group) => void;
  onDeleteGroup: (group: Group) => void;
  onLaunch: (app: RegisteredApp) => void;
  onSecrets: (app: RegisteredApp) => void;
  onEditApp: (app: RegisteredApp) => void;
  onDeleteApp: (app: RegisteredApp) => void;
  onRenameCategory: (category: Category) => void;
  onDeleteCategory: (category: Category) => void;
  onAddApp: () => void;
  onAddCategory: () => void;
}

interface AppRowProps {
  app: RegisteredApp;
  secretCount: number;
  health: HealthStatus | undefined;
  onLaunch: (app: RegisteredApp) => void;
  onSecrets: (app: RegisteredApp) => void;
  onEdit: (app: RegisteredApp) => void;
  onDelete: (app: RegisteredApp) => void;
}

/** Riga di un'app: il nome è il pulsante di avvio, così Invio e clic fanno la stessa cosa. */
function AppRow({ app, secretCount, health, onLaunch, onSecrets, onEdit, onDelete }: AppRowProps) {
  const { t } = useTranslation();
  // La produzione si riconosce a colpo d'occhio: filetto bronzo ed etichetta (v0.6.0).
  return (
    <li className={app.environment === 'production' ? 'app-row app-row--production' : 'app-row'}>
      <button
        type="button"
        className="app-row__launch"
        onClick={() => onLaunch(app)}
        aria-label={t('registry.launch', { name: app.name })}
      >
        <span className="app-row__name">{app.name}</span>
        <span className="app-row__meta">
          <span className="badge">{t(`registry.kind.${app.kind}`)}</span>
          <EnvironmentBadge environment={app.environment} />
          {app.healthCheck && <HealthBadge status={health} />}
          <span className="app-row__target">{app.target}</span>
        </span>
      </button>
      {app.tags.length > 0 && (
        <ul className="tag-list">
          {app.tags.map((tag) => (
            <li key={tag} className="tag">
              {tag}
            </li>
          ))}
        </ul>
      )}
      <div className="app-row__actions">
        <button
          type="button"
          className="button button--ghost"
          onClick={() => onSecrets(app)}
          aria-label={t('registry.secretsFor', { name: app.name })}
        >
          {secretCount > 0
            ? t('registry.secretCount', { count: secretCount })
            : t('registry.secrets')}
        </button>
        <button
          type="button"
          className="button button--ghost"
          onClick={() => onEdit(app)}
          aria-label={t('registry.editApp', { name: app.name })}
        >
          {t('actions.edit')}
        </button>
        <button
          type="button"
          className="button button--ghost"
          onClick={() => onDelete(app)}
          aria-label={t('registry.deleteApp', { name: app.name })}
        >
          {t('actions.delete')}
        </button>
      </div>
    </li>
  );
}

interface GroupRowProps {
  group: Group;
  apps: ReadonlyMap<string, RegisteredApp>;
  onLaunch: (group: Group) => void;
  onEdit: (group: Group) => void;
  onDelete: (group: Group) => void;
}

/** Riga di un gruppo: il nome apre la conferma con l'elenco di ciò che partirà (A.8). */
function GroupRow({ group, apps, onLaunch, onEdit, onDelete }: GroupRowProps) {
  const { t } = useTranslation();
  const production = group.appIds.some((id) => apps.get(id)?.environment === 'production');
  return (
    <li className={production ? 'app-row app-row--production' : 'app-row'}>
      <button
        type="button"
        className="app-row__launch"
        disabled={group.appIds.length === 0}
        onClick={() => onLaunch(group)}
        aria-label={t('groups.launch', { name: group.name })}
      >
        <span className="app-row__name">{group.name}</span>
        <span className="app-row__meta">
          <span className="badge">{t('groups.count', { count: group.appIds.length })}</span>
          {production && <EnvironmentBadge environment="production" />}
        </span>
      </button>
      <div className="app-row__actions">
        <button
          type="button"
          className="button button--ghost"
          onClick={() => onEdit(group)}
          aria-label={t('groups.editGroup', { name: group.name })}
        >
          {t('actions.edit')}
        </button>
        <button
          type="button"
          className="button button--ghost"
          onClick={() => onDelete(group)}
          aria-label={t('groups.deleteGroup', { name: group.name })}
        >
          {t('actions.delete')}
        </button>
      </div>
    </li>
  );
}

export function RegistryView({
  registry,
  health,
  onLaunchGroup,
  onCreateGroup,
  onEditGroup,
  onDeleteGroup,
  onLaunch,
  onSecrets,
  onEditApp,
  onDeleteApp,
  onRenameCategory,
  onDeleteCategory,
  onAddApp,
  onAddCategory,
}: RegistryViewProps) {
  const { t } = useTranslation();
  const secretCounts = countByApp(registry.secrets);
  const appsById = new Map(registry.apps.map((app) => [app.id, app]));

  if (registry.apps.length === 0 && registry.categories.length === 0) {
    return (
      <section className="empty-state" aria-labelledby="registry-empty-title">
        <h1 id="registry-empty-title" className="empty-state__title">
          {t('registry.emptyTitle')}
        </h1>
        <p className="empty-state__body">{t('registry.emptyBody')}</p>
        <div className="toolbar">
          <button type="button" className="button button--primary" onClick={onAddApp}>
            {t('actions.addFirstApp')}
          </button>
          <button type="button" className="button button--secondary" onClick={onAddCategory}>
            {t('actions.newCategory')}
          </button>
        </div>
      </section>
    );
  }

  return (
    <div className="registry">
      {registry.apps.length > 0 && (
        <section aria-labelledby="groups-title">
          <header className="section__header">
            <h2 id="groups-title" className="section__title">
              {t('groups.title')}
            </h2>
            <div className="section__actions">
              <button type="button" className="button button--ghost" onClick={onCreateGroup}>
                {t('groups.new')}
              </button>
            </div>
          </header>
          {registry.groups.length === 0 ? (
            <p className="groups__empty">{t('groups.empty')}</p>
          ) : (
            <ul className="app-list">
              {registry.groups.map((group) => (
                <GroupRow
                  key={group.id}
                  group={group}
                  apps={appsById}
                  onLaunch={onLaunchGroup}
                  onEdit={onEditGroup}
                  onDelete={onDeleteGroup}
                />
              ))}
            </ul>
          )}
        </section>
      )}
      {toSections(registry).map(({ category, apps }) => {
        const title = category?.name ?? t('registry.uncategorized');
        return (
          <section key={category?.id ?? 'uncategorized'} aria-label={title}>
            <header className="section__header">
              <h2 className="section__title">{title}</h2>
              {category && (
                <div className="section__actions">
                  <button
                    type="button"
                    className="button button--ghost"
                    onClick={() => onRenameCategory(category)}
                    aria-label={t('registry.renameCategory', { name: category.name })}
                  >
                    {t('actions.rename')}
                  </button>
                  <button
                    type="button"
                    className="button button--ghost"
                    onClick={() => onDeleteCategory(category)}
                    aria-label={t('registry.deleteCategory', { name: category.name })}
                  >
                    {t('actions.delete')}
                  </button>
                </div>
              )}
            </header>
            {apps.length > 0 && (
              <ul className="app-list">
                {apps.map((app) => (
                  <AppRow
                    key={app.id}
                    app={app}
                    secretCount={secretCounts.get(app.id) ?? 0}
                    health={health.get(app.id)}
                    onLaunch={onLaunch}
                    onSecrets={onSecrets}
                    onEdit={onEditApp}
                    onDelete={onDeleteApp}
                  />
                ))}
              </ul>
            )}
          </section>
        );
      })}
    </div>
  );
}
