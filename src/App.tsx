import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from './components/ConfirmDialog';
import { Mark } from './components/Mark';
import { WindowControls } from './components/WindowControls';
import { NameDialog } from './components/NameDialog';
import { APP_NAME } from './constants/app';
import { LockScreen } from './features/lock/LockScreen';
import { AppDialog } from './features/registry/AppDialog';
import { RegistryView } from './features/registry/RegistryView';
import { secretsOf } from './features/secrets/grouping';
import { SecretsDialog } from './features/secrets/SecretsDialog';
import { SecurityView } from './features/security/SecurityView';
import { GroupDialog } from './features/groups/GroupDialog';
import { GroupLaunchDialog } from './features/groups/GroupLaunchDialog';
import { GroupResultDialog } from './features/groups/GroupResultDialog';
import { useHealth } from './features/health/useHealth';
import { useAppIcons } from './features/icons/useAppIcons';
import { AutostartSettings } from './features/settings/AutostartSettings';
import { ShortcutSettings } from './features/settings/ShortcutSettings';
import { errorCode } from './lib/errors';
import {
  createApp,
  createCategory,
  deleteApp,
  deleteCategory,
  getAppInfo,
  getLockStatus,
  createGroup,
  deleteGroup,
  launchApp,
  launchGroup,
  onGroupResult,
  updateGroup,
  type Group,
  type GroupLaunchResult,
  listRegistry,
  lockNow,
  onLockChanged,
  renameCategory,
  setupTray,
  updateApp,
  type Category,
  type LockStatus,
  type RegisteredApp,
  type Registry,
} from './lib/ipc';

type RegistryState =
  | { status: 'loading' }
  | { status: 'ready'; registry: Registry }
  | { status: 'failed'; code: string };

/** Stato del blocco come arriva da Rust; `receivedAt` fa partire l'attesa del PIN. (v0.3.0) */
type LockView =
  | { status: 'loading' }
  | { status: 'ready'; lock: LockStatus; receivedAt: number }
  | { status: 'failed'; code: string };

type View = 'registry' | 'security';

/** Una sola finestra aperta alla volta, descritta da dati e non da flag sparsi. */
type Modal =
  | { type: 'none' }
  | { type: 'createApp' }
  | { type: 'editApp'; app: RegisteredApp }
  | { type: 'deleteApp'; app: RegisteredApp }
  | { type: 'changedApp'; app: RegisteredApp }
  /** Per id: dopo ogni modifica il registro si rilegge, e la finestra ne mostra i dati nuovi. */
  | { type: 'secrets'; appId: string }
  | { type: 'createCategory' }
  | { type: 'renameCategory'; category: Category }
  | { type: 'deleteCategory'; category: Category }
  // Gruppi di avvio (v0.6.0).
  | { type: 'createGroup' }
  | { type: 'editGroup'; group: Group }
  | { type: 'deleteGroup'; group: Group }
  | { type: 'launchGroup'; group: Group };

/**
 * Esito di un gruppo con qualcosa in sospeso. Sta fuori da `Modal`: se arriva dalla palette
 * mentre qui è aperta un'altra finestra, aspetta che si chiuda invece di sostituirla e far
 * perdere ciò che si stava scrivendo. `at` rimonta la finestra a ogni esito nuovo. (v0.6.0)
 */
interface ShownResult {
  group: Group;
  result: GroupLaunchResult;
  at: number;
}

const NO_MODAL: Modal = { type: 'none' };

/**
 * Ripiego se Rust avvisa del blocco ma lo stato completo non arriva: si mostra il PIN, che
 * esiste sempre quando il blocco è configurato. (v0.3.0)
 */
const LOCKED_FALLBACK: LockStatus = {
  locked: true,
  pinSet: true,
  helloEnabled: false,
  idleMinutes: null,
  retryAfterMs: 0,
  // Lunghezza ignota: si conferma con Invio, come prima della v0.7.0.
  pinLength: null,
};

/**
 * Legge il registro da Rust e lo traduce in uno stato dell'interfaccia. Il registro sta in
 * Rust: dopo ogni modifica si rilegge, invece di tenerne una copia locale che potrebbe
 * divergere (A.7.3). (v0.2.0)
 */
async function loadRegistry(): Promise<RegistryState> {
  try {
    return { status: 'ready', registry: await listRegistry() };
  } catch (failure: unknown) {
    return { status: 'failed', code: errorCode(failure) };
  }
}

/**
 * Avvio: prima lo stato del blocco, e il registro solo se l'app è sbloccata. Da bloccata
 * Rust lo rifiuterebbe comunque; così non lo si chiede nemmeno. (v0.3.0)
 */
async function loadStartup(): Promise<{ lock: LockView; registry: RegistryState | null }> {
  let lock: LockView;
  try {
    lock = { status: 'ready', lock: await getLockStatus(), receivedAt: Date.now() };
  } catch (failure: unknown) {
    lock = { status: 'failed', code: errorCode(failure) };
  }
  const unlocked = lock.status === 'ready' && !lock.lock.locked;
  return { lock, registry: unlocked ? await loadRegistry() : null };
}

/** Nessuna app: riferimento stabile, la firma delle icone non cambia a ogni render. */
const NO_APPS: RegisteredApp[] = [];

export default function App() {
  const { t } = useTranslation();
  const [lockView, setLockView] = useState<LockView>({ status: 'loading' });
  const [registry, setRegistry] = useState<RegistryState>({ status: 'loading' });
  const [view, setView] = useState<View>('registry');
  const [modal, setModal] = useState<Modal>(NO_MODAL);
  const [groupResult, setGroupResult] = useState<ShownResult | null>(null);
  const [message, setMessage] = useState('');
  const [version, setVersion] = useState<string | null>(null);

  const reload = useCallback(async () => setRegistry(await loadRegistry()), []);

  const applyStartup = useCallback((next: { lock: LockView; registry: RegistryState | null }) => {
    setLockView(next.lock);
    if (next.registry) setRegistry(next.registry);
  }, []);

  const applyLock = useCallback((lock: LockStatus) => {
    setLockView({ status: 'ready', lock, receivedAt: Date.now() });
  }, []);

  /**
   * L'app si è bloccata: il registro esce dalla memoria e dallo schermo, insieme a finestre
   * aperte e messaggi (che contengono nomi di app). Poi si legge lo stato completo. (v0.3.0)
   */
  const hideForLock = useCallback(() => {
    const now = Date.now();
    setRegistry({ status: 'loading' });
    setModal(NO_MODAL);
    setGroupResult(null);
    setMessage('');
    setLockView((current) => ({
      status: 'ready',
      lock: { ...(current.status === 'ready' ? current.lock : LOCKED_FALLBACK), locked: true },
      receivedAt: now,
    }));
    getLockStatus()
      .then(applyLock)
      .catch(() => undefined);
  }, [applyLock]);

  /** Un comando rifiutato con LOCKED porta alla schermata di blocco anche senza evento. */
  const report = useCallback(
    (failure: unknown) => {
      const code = errorCode(failure);
      if (code === 'LOCKED') hideForLock();
      else setMessage(t(`errors.${code}`, { defaultValue: t('errors.UNKNOWN') }));
    },
    [hideForLock, t],
  );

  const lock = useCallback(() => {
    lockNow().then(hideForLock).catch(report);
  }, [hideForLock, report]);

  useEffect(() => {
    // Lo stato si aggiorna solo quando arriva la risposta, e non dopo lo smontaggio.
    let active = true;
    void loadStartup().then((next) => {
      if (active) applyStartup(next);
    });
    // La tray è accessoria (A.7.5): senza, la finestra resta raggiungibile riavviando l'app,
    // che per l'istanza singola porta davanti quella già aperta. (v0.5.0)
    setupTray({
      tooltip: APP_NAME,
      open: t('tray.open'),
      palette: t('tray.palette'),
      lock: t('tray.lock'),
      quit: t('tray.quit'),
    }).catch(() => undefined);
    // La versione è accessoria: se non arriva, il resto funziona lo stesso (A.7.5).
    getAppInfo()
      .then((info) => active && setVersion(info.version))
      .catch(() => active && setVersion(null));
    return () => {
      active = false;
    };
  }, [applyStartup, t]);

  useEffect(() => {
    // Blocco deciso da Rust (inattività, sessione di Windows): arriva come evento. Se
    // l'ascolto non parte, resta il filtro di Rust e il primo comando rifiutato. (v0.3.0)
    let active = true;
    let stop: (() => void) | null = null;
    onLockChanged((locked) => {
      if (locked) hideForLock();
    })
      .then((unlisten) => {
        if (active) stop = unlisten;
        else unlisten();
      })
      .catch(() => undefined);
    return () => {
      active = false;
      stop?.();
    };
  }, [hideForLock]);

  // Esito di un gruppo avviato dalla palette con qualcosa da confermare: Rust mostra questa
  // finestra e lo consegna qui. (v0.6.0)
  useEffect(() => {
    let active = true;
    let stop: (() => void) | null = null;
    onGroupResult(({ group, result }) => {
      if (!active) return;
      setMessage(t('status.groupLaunched', { count: result.launched.length, name: group.name }));
      setGroupResult({ group, result, at: Date.now() });
    })
      .then((unlisten) => {
        if (active) stop = unlisten;
        else unlisten();
      })
      .catch(() => undefined);
    return () => {
      active = false;
      stop?.();
    };
  }, [t]);

  const ready = lockView.status === 'ready' ? lockView.lock : null;
  const locked = ready?.locked ?? false;
  const canLock = ready !== null && ready.pinSet && !ready.locked;

  useEffect(() => {
    if (!canLock) return;
    // Ctrl+L blocca subito, da qualunque punto dell'app (A.7.8, tastiera prima di tutto).
    const onKey = (event: KeyboardEvent) => {
      const plain = !event.altKey && !event.shiftKey && !event.metaKey;
      if (event.ctrlKey && plain && event.key.toLowerCase() === 'l') {
        event.preventDefault();
        lock();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [canLock, lock]);

  // Stato delle app web: solo con il registro in vista e sbloccato (v0.6.0).
  const health = useHealth(view === 'registry' && registry.status === 'ready' && !locked);
  const icons = useAppIcons(registry.status === 'ready' ? registry.registry.apps : NO_APPS);

  const launch = (app: RegisteredApp, acceptChanged: boolean) => {
    launchApp(app.id, acceptChanged)
      .then(() => setMessage(t('status.launched', { name: app.name })))
      .catch((failure: unknown) => {
        if (errorCode(failure) === 'HASH_MISMATCH') setModal({ type: 'changedApp', app });
        else report(failure);
      });
  };

  /** Esegue un'azione, chiude la finestra, rilegge il registro e lo dice nella barra di stato. */
  const complete = async (action: Promise<unknown>, done: string) => {
    await action;
    setModal(NO_MODAL);
    setMessage(done);
    await reload();
  };

  /** Avvia il gruppo; se qualcosa non parte, l'esito lo elenca e lo fa confermare. */
  const runGroup = (group: Group) => {
    setModal(NO_MODAL);
    launchGroup(group.id)
      .then((result) => {
        setMessage(t('status.groupLaunched', { count: result.launched.length, name: group.name }));
        if (result.changed.length > 0 || result.failed.length > 0) {
          setGroupResult({ group, result, at: Date.now() });
        }
      })
      .catch(report);
  };

  const categories = registry.status === 'ready' ? registry.registry.categories : [];
  const apps = registry.status === 'ready' ? registry.registry.apps : [];
  // Con il registro vuoto le azioni stanno solo al centro: un'azione primaria per area (A.8).
  const isEmpty =
    registry.status === 'ready' &&
    registry.registry.apps.length === 0 &&
    registry.registry.categories.length === 0;
  const startupFailure =
    lockView.status === 'failed'
      ? lockView.code
      : registry.status === 'failed' && !locked
        ? registry.code
        : null;

  return (
    <div className="shell">
      {/* Barra del titolo propria: tutta l'intestazione trascina la finestra ("deep"); i
          pulsanti, per Tauri, non trascinano. Il doppio clic ingrandisce. (v0.5.0) */}
      <header className="shell__header" data-tauri-drag-region="deep">
        <div className="brand">
          <Mark size={28} />
          <span className="wordmark">{APP_NAME}</span>
        </div>
        {ready && !locked && startupFailure === null && (
          <div className="toolbar">
            {ready.pinSet && (
              <button
                type="button"
                className="button button--ghost"
                onClick={lock}
                aria-keyshortcuts="Control+L"
                title={t('actions.lockShortcut')}
              >
                {t('actions.lock')}
              </button>
            )}
            <button
              type="button"
              className="button button--ghost"
              onClick={() => setView(view === 'security' ? 'registry' : 'security')}
            >
              {view === 'security' ? t('actions.backToRegistry') : t('actions.settings')}
            </button>
            {view === 'registry' && registry.status === 'ready' && !isEmpty && (
              <>
                <button
                  type="button"
                  className="button button--secondary"
                  onClick={() => setModal({ type: 'createCategory' })}
                >
                  {t('actions.newCategory')}
                </button>
                <button
                  type="button"
                  className="button button--primary"
                  onClick={() => setModal({ type: 'createApp' })}
                >
                  {t('actions.addApp')}
                </button>
              </>
            )}
          </div>
        )}
        <WindowControls />
      </header>

      {/* La chiave rimonta il contenuto a ogni cambio di vista: l'entrata la anima il CSS. (v0.7.0) */}
      <main key={locked ? 'locked' : view} className="shell__main">
        {startupFailure !== null && (
          <section className="empty-state" role="alert">
            <h1 className="empty-state__title">{t('startup.title')}</h1>
            <p className="empty-state__body">
              {t(`errors.${startupFailure}`, { defaultValue: t('errors.UNKNOWN') })}
            </p>
            <button
              type="button"
              className="button button--secondary"
              onClick={() => void loadStartup().then(applyStartup)}
            >
              {t('actions.retry')}
            </button>
          </section>
        )}
        {startupFailure === null && ready && locked && lockView.status === 'ready' && (
          <LockScreen
            key={lockView.receivedAt}
            status={ready}
            receivedAt={lockView.receivedAt}
            onUnlocked={(next) => {
              applyLock(next);
              void reload();
            }}
          />
        )}
        {startupFailure === null && ready && !locked && view === 'security' && (
          <div className="settings-page">
            <h1 className="settings-page__title">{t('settings.title')}</h1>
            <SecurityView
              status={ready}
              onStatus={(next, text) => {
                applyLock(next);
                setMessage(text);
              }}
            />
            <ShortcutSettings onMessage={setMessage} />
            <AutostartSettings onMessage={setMessage} />
          </div>
        )}
        {startupFailure === null &&
          !locked &&
          view === 'registry' &&
          registry.status === 'ready' && (
            <RegistryView
              registry={registry.registry}
              onLaunch={(app) => launch(app, false)}
              health={health}
              icons={icons}
              onLaunchGroup={(group) => setModal({ type: 'launchGroup', group })}
              onCreateGroup={() => setModal({ type: 'createGroup' })}
              onEditGroup={(group) => setModal({ type: 'editGroup', group })}
              onDeleteGroup={(group) => setModal({ type: 'deleteGroup', group })}
              onSecrets={(app) => setModal({ type: 'secrets', appId: app.id })}
              onEditApp={(app) => setModal({ type: 'editApp', app })}
              onDeleteApp={(app) => setModal({ type: 'deleteApp', app })}
              onRenameCategory={(category) => setModal({ type: 'renameCategory', category })}
              onDeleteCategory={(category) => setModal({ type: 'deleteCategory', category })}
              onAddApp={() => setModal({ type: 'createApp' })}
              onAddCategory={() => setModal({ type: 'createCategory' })}
            />
          )}
      </main>

      {/* Altezza fissa: i messaggi compaiono senza spostare il resto del layout (A.8). */}
      <footer className="shell__footer">
        {/* La regione resta, cambia il testo: il lettore di schermo lo annuncia (v0.7.0). */}
        <p className="status" role="status">
          <span key={message} className="status__text">
            {message}
          </span>
        </p>
        <span className="version">
          {version ? t('footer.version', { version }) : t('footer.versionUnavailable')}
        </span>
      </footer>

      {(modal.type === 'createApp' || modal.type === 'editApp') && (
        <AppDialog
          app={modal.type === 'editApp' ? modal.app : undefined}
          categories={categories}
          icon={modal.type === 'editApp' ? icons.get(modal.app.id) : undefined}
          onIconChanged={() => {
            reload().catch(() => undefined);
            setMessage(t('status.iconChanged'));
          }}
          onCancel={() => setModal(NO_MODAL)}
          onSubmit={(input) =>
            complete(
              modal.type === 'editApp' ? updateApp(modal.app.id, input) : createApp(input),
              t('status.saved', { name: input.name.trim() }),
            )
          }
        />
      )}
      {modal.type === 'deleteApp' && (
        <ConfirmDialog
          title={t('confirm.deleteAppTitle', { name: modal.app.name })}
          body={t('confirm.deleteAppBody')}
          confirmLabel={t('actions.delete')}
          destructive
          onCancel={() => setModal(NO_MODAL)}
          onConfirm={() =>
            void complete(
              deleteApp(modal.app.id),
              t('status.deleted', { name: modal.app.name }),
            ).catch(report)
          }
        />
      )}
      {modal.type === 'secrets' &&
        registry.status === 'ready' &&
        (() => {
          const app = registry.registry.apps.find((candidate) => candidate.id === modal.appId);
          return app ? (
            <SecretsDialog
              app={app}
              secrets={secretsOf(registry.registry.secrets, app.id)}
              onChanged={(text) => {
                setMessage(text);
                void reload();
              }}
              onMessage={setMessage}
              onLaunch={(target) => {
                setModal(NO_MODAL);
                launch(target, false);
              }}
              onClose={() => setModal(NO_MODAL)}
            />
          ) : null;
        })()}
      {modal.type === 'changedApp' && (
        <ConfirmDialog
          title={t('confirm.changedTitle', { name: modal.app.name })}
          body={t('confirm.changedBody')}
          confirmLabel={t('confirm.changedConfirm')}
          onCancel={() => setModal(NO_MODAL)}
          onConfirm={() => {
            setModal(NO_MODAL);
            launch(modal.app, true);
          }}
        />
      )}
      {(modal.type === 'createGroup' || modal.type === 'editGroup') && (
        <GroupDialog
          group={modal.type === 'editGroup' ? modal.group : undefined}
          apps={apps}
          onCancel={() => setModal(NO_MODAL)}
          onSubmit={(input) =>
            complete(
              modal.type === 'editGroup' ? updateGroup(modal.group.id, input) : createGroup(input),
              t('status.groupSaved', { name: input.name.trim() }),
            )
          }
        />
      )}
      {modal.type === 'deleteGroup' && (
        <ConfirmDialog
          title={t('confirm.deleteGroupTitle', { name: modal.group.name })}
          body={t('confirm.deleteGroupBody')}
          confirmLabel={t('actions.delete')}
          destructive
          onCancel={() => setModal(NO_MODAL)}
          onConfirm={() => {
            complete(
              deleteGroup(modal.group.id),
              t('status.groupDeleted', { name: modal.group.name }),
            ).catch(report);
          }}
        />
      )}
      {modal.type === 'launchGroup' && (
        <GroupLaunchDialog
          group={modal.group}
          apps={apps}
          onCancel={() => setModal(NO_MODAL)}
          onConfirm={() => runGroup(modal.group)}
        />
      )}
      {modal.type === 'none' && groupResult && (
        <GroupResultDialog
          key={groupResult.at}
          group={groupResult.group}
          result={groupResult.result}
          apps={apps}
          onClose={() => setGroupResult(null)}
          onConfirmChanged={(app) =>
            launchApp(app.id, true).then(() => setMessage(t('status.launched', { name: app.name })))
          }
        />
      )}
      {modal.type === 'createCategory' && (
        <NameDialog
          title={t('category.createTitle')}
          label={t('category.nameLabel')}
          onCancel={() => setModal(NO_MODAL)}
          onSubmit={(name) =>
            complete(createCategory(name), t('status.saved', { name: name.trim() }))
          }
        />
      )}
      {modal.type === 'renameCategory' && (
        <NameDialog
          title={t('category.renameTitle')}
          label={t('category.nameLabel')}
          initialName={modal.category.name}
          onCancel={() => setModal(NO_MODAL)}
          onSubmit={(name) =>
            complete(
              renameCategory(modal.category.id, name),
              t('status.saved', { name: name.trim() }),
            )
          }
        />
      )}
      {modal.type === 'deleteCategory' && (
        <ConfirmDialog
          title={t('confirm.deleteCategoryTitle', { name: modal.category.name })}
          body={t('confirm.deleteCategoryBody')}
          confirmLabel={t('actions.delete')}
          destructive
          onCancel={() => setModal(NO_MODAL)}
          onConfirm={() =>
            void complete(
              deleteCategory(modal.category.id),
              t('status.deleted', { name: modal.category.name }),
            ).catch(report)
          }
        />
      )}
    </div>
  );
}
