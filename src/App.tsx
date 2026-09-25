import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from './components/ConfirmDialog';
import { Mark } from './components/Mark';
import { NameDialog } from './components/NameDialog';
import { APP_NAME } from './constants/app';
import { AppDialog } from './features/registry/AppDialog';
import { RegistryView } from './features/registry/RegistryView';
import { errorCode } from './lib/errors';
import {
  createApp,
  createCategory,
  deleteApp,
  deleteCategory,
  getAppInfo,
  launchApp,
  listRegistry,
  renameCategory,
  updateApp,
  type Category,
  type RegisteredApp,
  type Registry,
} from './lib/ipc';

type RegistryState =
  | { status: 'loading' }
  | { status: 'ready'; registry: Registry }
  | { status: 'failed'; code: string };

/** Una sola finestra aperta alla volta, descritta da dati e non da flag sparsi. */
type Modal =
  | { type: 'none' }
  | { type: 'createApp' }
  | { type: 'editApp'; app: RegisteredApp }
  | { type: 'deleteApp'; app: RegisteredApp }
  | { type: 'changedApp'; app: RegisteredApp }
  | { type: 'createCategory' }
  | { type: 'renameCategory'; category: Category }
  | { type: 'deleteCategory'; category: Category };

const NO_MODAL: Modal = { type: 'none' };

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

export default function App() {
  const { t } = useTranslation();
  const [registry, setRegistry] = useState<RegistryState>({ status: 'loading' });
  const [modal, setModal] = useState<Modal>(NO_MODAL);
  const [message, setMessage] = useState('');
  const [version, setVersion] = useState<string | null>(null);

  const reload = useCallback(async () => setRegistry(await loadRegistry()), []);

  useEffect(() => {
    // Lo stato si aggiorna solo quando arriva la risposta, e non dopo lo smontaggio.
    let active = true;
    void loadRegistry().then((next) => {
      if (active) setRegistry(next);
    });
    // La versione è accessoria: se non arriva, il resto funziona lo stesso (A.7.5).
    getAppInfo()
      .then((info) => active && setVersion(info.version))
      .catch(() => active && setVersion(null));
    return () => {
      active = false;
    };
  }, []);

  const report = (failure: unknown) =>
    setMessage(t(`errors.${errorCode(failure)}`, { defaultValue: t('errors.UNKNOWN') }));

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

  const categories = registry.status === 'ready' ? registry.registry.categories : [];
  // Con il registro vuoto le azioni stanno solo al centro: un'azione primaria per area (A.8).
  const isEmpty =
    registry.status === 'ready' &&
    registry.registry.apps.length === 0 &&
    registry.registry.categories.length === 0;

  return (
    <div className="shell">
      <header className="shell__header">
        <div className="brand">
          <Mark size={36} />
          <span className="wordmark">{APP_NAME}</span>
        </div>
        {registry.status === 'ready' && !isEmpty && (
          <div className="toolbar">
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
          </div>
        )}
      </header>

      <main className="shell__main">
        {registry.status === 'ready' && (
          <RegistryView
            registry={registry.registry}
            onLaunch={(app) => launch(app, false)}
            onEditApp={(app) => setModal({ type: 'editApp', app })}
            onDeleteApp={(app) => setModal({ type: 'deleteApp', app })}
            onRenameCategory={(category) => setModal({ type: 'renameCategory', category })}
            onDeleteCategory={(category) => setModal({ type: 'deleteCategory', category })}
            onAddApp={() => setModal({ type: 'createApp' })}
            onAddCategory={() => setModal({ type: 'createCategory' })}
          />
        )}
        {registry.status === 'failed' && (
          <section className="empty-state" role="alert">
            <h1 className="empty-state__title">{t('startup.title')}</h1>
            <p className="empty-state__body">
              {t(`errors.${registry.code}`, { defaultValue: t('errors.UNKNOWN') })}
            </p>
            <button
              type="button"
              className="button button--secondary"
              onClick={() => void reload()}
            >
              {t('actions.retry')}
            </button>
          </section>
        )}
      </main>

      {/* Altezza fissa: i messaggi compaiono senza spostare il resto del layout (A.8). */}
      <footer className="shell__footer">
        <p className="status" role="status">
          {message}
        </p>
        <span className="version">
          {version ? t('footer.version', { version }) : t('footer.versionUnavailable')}
        </span>
      </footer>

      {(modal.type === 'createApp' || modal.type === 'editApp') && (
        <AppDialog
          app={modal.type === 'editApp' ? modal.app : undefined}
          categories={categories}
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
