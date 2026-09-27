import { useCallback, useEffect, useId, useRef, useState, type SyntheticEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { EnvironmentBadge } from '../../components/EnvironmentBadge';
import type { Category, Group, HealthStatus, RegisteredApp, Registry } from '../../lib/ipc';
import { MENU_EXIT_MS, prefersReducedMotion } from '../../lib/motion';
import { HealthDot } from '../health/HealthBadge';
import { AppIcon, GroupIcon } from '../icons/AppIcon';
import { countByApp } from '../secrets/grouping';
import { opensMenu } from './menu';
import { toSections } from './sections';
import { TileMenu, type TileMenuItem } from './TileMenu';

interface RegistryViewProps {
  registry: Registry;
  /** Stato delle app con il controllo acceso (v0.6.0). */
  health: ReadonlyMap<string, HealthStatus>;
  /** Immagini delle icone per id di app (v0.7.0). */
  icons: ReadonlyMap<string, string>;
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
  /** Dalla tessera "+" di una sezione: la categoria arriva già scelta (v0.8.0). */
  onAddApp: (categoryId: string | null) => void;
  onAddCategory: () => void;
}

type MenuState = 'closed' | 'open' | 'closing';

/**
 * Stato del menu di una tessera. Alla chiusura resta montato per `MENU_EXIT_MS`, inerte,
 * mentre svanisce: l'uscita si anima come l'entrata (v0.8.0).
 */
function useTileMenu() {
  const [state, setState] = useState<MenuState>('closed');
  const trigger = useRef<HTMLButtonElement>(null);
  const timer = useRef<number | undefined>(undefined);
  const ids = { trigger: useId(), menu: useId() };
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const close = useCallback((restoreFocus: boolean) => {
    setState((current) => (current === 'open' ? 'closing' : current));
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(
      () => setState((current) => (current === 'closing' ? 'closed' : current)),
      prefersReducedMotion() ? 0 : MENU_EXIT_MS,
    );
    if (restoreFocus) trigger.current?.focus();
  }, []);
  const show = () => {
    window.clearTimeout(timer.current);
    setState('open');
  };
  // Tasto destro, tasto Menu e Maiusc+F10 aprono lo stesso menu del pulsante "⋯".
  const openFrom = (event: SyntheticEvent) => {
    event.preventDefault();
    show();
  };
  const toggle = () => (state === 'open' ? close(false) : show());
  return { state, open: state === 'open', trigger, ids, close, openFrom, toggle };
}

function MoreIcon() {
  return (
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="3" cy="8" r="1.4" />
      <circle cx="8" cy="8" r="1.4" />
      <circle cx="13" cy="8" r="1.4" />
    </svg>
  );
}

interface MoreProps {
  menu: ReturnType<typeof useTileMenu>;
  moreLabel: string;
  items: TileMenuItem[];
}

/** Il pulsante "⋯" e il suo menu, uguali per app e gruppi (funzione di render, A.7.3). */
function renderMore({ menu, moreLabel, items }: MoreProps) {
  return (
    <>
      <button
        ref={menu.trigger}
        id={menu.ids.trigger}
        type="button"
        className="tile__more"
        aria-haspopup="menu"
        aria-expanded={menu.open}
        aria-controls={menu.open ? menu.ids.menu : undefined}
        aria-label={moreLabel}
        onClick={menu.toggle}
      >
        <MoreIcon />
      </button>
      {menu.state !== 'closed' && (
        <TileMenu
          id={menu.ids.menu}
          labelledBy={menu.ids.trigger}
          items={items}
          closing={menu.state === 'closing'}
          onClose={menu.close}
        />
      )}
    </>
  );
}

interface AppTileProps {
  app: RegisteredApp;
  icon: string | undefined;
  secretCount: number;
  health: HealthStatus | undefined;
  onLaunch: (app: RegisteredApp) => void;
  onSecrets: (app: RegisteredApp) => void;
  onEdit: (app: RegisteredApp) => void;
  onDelete: (app: RegisteredApp) => void;
}

/**
 * Tessera di un'app (v0.7.0): icona e nome, niente percorso né etichette. Clic e Invio
 * avviano; le azioni secondarie stanno nel menu. La produzione resta riconoscibile con
 * l'etichetta e il filetto bronzo (A.8).
 */
function AppTile({
  app,
  icon,
  secretCount,
  health,
  onLaunch,
  onSecrets,
  onEdit,
  onDelete,
}: AppTileProps) {
  const { t } = useTranslation();
  const menu = useTileMenu();
  const healthId = useId();
  const items: TileMenuItem[] = [
    {
      key: 'secrets',
      label:
        secretCount > 0 ? t('registry.secretCount', { count: secretCount }) : t('registry.secrets'),
      onSelect: () => onSecrets(app),
    },
    { key: 'edit', label: t('actions.edit'), onSelect: () => onEdit(app) },
    { key: 'delete', label: t('actions.delete'), onSelect: () => onDelete(app), danger: true },
  ];
  return (
    <li className={app.environment === 'production' ? 'tile tile--production' : 'tile'}>
      <button
        type="button"
        className="tile__launch"
        title={app.name}
        aria-label={t('registry.launch', { name: app.name })}
        aria-describedby={app.healthCheck ? healthId : undefined}
        onClick={() => onLaunch(app)}
        onContextMenu={menu.openFrom}
        onKeyDown={(event) => {
          if (opensMenu(event.key, event.shiftKey)) menu.openFrom(event);
        }}
      >
        <span className="tile__icon">
          <AppIcon name={app.name} src={icon} />
          {app.healthCheck && <HealthDot id={healthId} status={health} />}
        </span>
        <span className="tile__name">{app.name}</span>
        <EnvironmentBadge environment={app.environment} />
      </button>
      {renderMore({ menu, moreLabel: t('registry.moreActions', { name: app.name }), items })}
    </li>
  );
}

interface GroupTileProps {
  group: Group;
  apps: readonly RegisteredApp[];
  icons: ReadonlyMap<string, string>;
  onLaunch: (group: Group) => void;
  onEdit: (group: Group) => void;
  onDelete: (group: Group) => void;
}

/** Tessera di un gruppo: le icone delle sue app; l'avvio apre la conferma con l'elenco (A.8). */
function GroupTile({ group, apps, icons, onLaunch, onEdit, onDelete }: GroupTileProps) {
  const { t } = useTranslation();
  const menu = useTileMenu();
  const production = apps.some((app) => app.environment === 'production');
  const items: TileMenuItem[] = [
    { key: 'edit', label: t('actions.edit'), onSelect: () => onEdit(group) },
    { key: 'delete', label: t('actions.delete'), onSelect: () => onDelete(group), danger: true },
  ];
  return (
    <li className={production ? 'tile tile--production' : 'tile'}>
      <button
        type="button"
        className="tile__launch"
        title={`${group.name} · ${t('groups.count', { count: group.appIds.length })}`}
        aria-label={t('groups.launch', { name: group.name })}
        disabled={group.appIds.length === 0}
        onClick={() => onLaunch(group)}
        onContextMenu={menu.openFrom}
        onKeyDown={(event) => {
          if (opensMenu(event.key, event.shiftKey)) menu.openFrom(event);
        }}
      >
        <span className="tile__icon">
          <GroupIcon name={group.name} apps={apps} icons={icons} />
        </span>
        <span className="tile__name">{group.name}</span>
        {production && <EnvironmentBadge environment="production" />}
      </button>
      {renderMore({ menu, moreLabel: t('groups.moreActions', { name: group.name }), items })}
    </li>
  );
}

/**
 * Tessera "+" in fondo a una sezione (v0.8.0): aggiungere si fa dove si guarda, con la stessa
 * forma delle tessere che crea, al posto dei pulsanti nella barra.
 */
function renderAddTile(label: string, hint: string, onAdd: () => void) {
  return (
    <li className="tile tile--add">
      <button type="button" className="tile__launch" title={hint} onClick={onAdd}>
        <span className="tile__icon">
          <span className="add-icon" aria-hidden="true">
            <svg viewBox="0 0 16 16">
              <path d="M8 3v10M3 8h10" />
            </svg>
          </span>
        </span>
        <span className="tile__name">{label}</span>
      </button>
    </li>
  );
}

export function RegistryView({
  registry,
  health,
  icons,
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
          <button type="button" className="button button--primary" onClick={() => onAddApp(null)}>
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
          </header>
          <ul className="tile-grid">
            {registry.groups.map((group) => (
              <GroupTile
                key={group.id}
                group={group}
                apps={group.appIds.flatMap((id) => appsById.get(id) ?? [])}
                icons={icons}
                onLaunch={onLaunchGroup}
                onEdit={onEditGroup}
                onDelete={onDeleteGroup}
              />
            ))}
            {renderAddTile(t('groups.new'), t('groups.addHint'), onCreateGroup)}
          </ul>
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
            <ul className="tile-grid">
              {apps.map((app) => (
                <AppTile
                  key={app.id}
                  app={app}
                  icon={icons.get(app.id)}
                  secretCount={secretCounts.get(app.id) ?? 0}
                  health={health.get(app.id)}
                  onLaunch={onLaunch}
                  onSecrets={onSecrets}
                  onEdit={onEditApp}
                  onDelete={onDeleteApp}
                />
              ))}
              {renderAddTile(
                t('actions.addApp'),
                category ? t('registry.addHintIn', { name: category.name }) : t('registry.addHint'),
                () => onAddApp(category?.id ?? null),
              )}
            </ul>
          </section>
        );
      })}
      <button type="button" className="add-category" onClick={onAddCategory}>
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M8 3v10M3 8h10" />
        </svg>
        {t('actions.newCategory')}
      </button>
    </div>
  );
}
