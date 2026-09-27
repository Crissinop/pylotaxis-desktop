import { useTranslation } from 'react-i18next';

import type { Category, RegisteredApp, Registry } from '../../lib/ipc';
import { toSections } from './sections';
import { countByApp } from '../secrets/grouping';

interface RegistryViewProps {
  registry: Registry;
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
  onLaunch: (app: RegisteredApp) => void;
  onSecrets: (app: RegisteredApp) => void;
  onEdit: (app: RegisteredApp) => void;
  onDelete: (app: RegisteredApp) => void;
}

/** Riga di un'app: il nome è il pulsante di avvio, così Invio e clic fanno la stessa cosa. */
function AppRow({ app, secretCount, onLaunch, onSecrets, onEdit, onDelete }: AppRowProps) {
  const { t } = useTranslation();
  return (
    <li className="app-row">
      <button
        type="button"
        className="app-row__launch"
        onClick={() => onLaunch(app)}
        aria-label={t('registry.launch', { name: app.name })}
      >
        <span className="app-row__name">{app.name}</span>
        <span className="app-row__meta">
          <span className="badge">{t(`registry.kind.${app.kind}`)}</span>
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

export function RegistryView({
  registry,
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
