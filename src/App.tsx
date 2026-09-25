import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Mark } from './components/Mark';
import { APP_NAME } from './constants/app';
import { getAppInfo } from './lib/ipc';

type VersionState =
  { status: 'loading' } | { status: 'ready'; version: string } | { status: 'unavailable' };

export default function App() {
  const { t } = useTranslation();
  const [version, setVersion] = useState<VersionState>({ status: 'loading' });

  useEffect(() => {
    // La versione è un'informazione accessoria: se il comando fallisce lo si registra e lo si
    // dice, senza bloccare il resto dell'interfaccia (A.7.5). (v0.1.0)
    let active = true;
    getAppInfo()
      .then((info) => {
        if (active) setVersion({ status: 'ready', version: info.version });
      })
      .catch((error: unknown) => {
        console.error('app_info non riuscito', error);
        if (active) setVersion({ status: 'unavailable' });
      });
    return () => {
      active = false;
    };
  }, []);

  return (
    <div className="shell">
      <header className="shell__header">
        <Mark size={36} />
        <span className="wordmark">{APP_NAME}</span>
      </header>

      <main className="shell__main">
        <section className="empty-state" aria-labelledby="registry-empty-title">
          <h1 id="registry-empty-title" className="empty-state__title">
            {t('registry.emptyTitle')}
          </h1>
          <p className="empty-state__body">{t('registry.emptyBody')}</p>
        </section>
      </main>

      {/* Altezza fissa: la riga compare senza spostare il resto del layout (A.8). */}
      <footer className="shell__footer">
        {version.status === 'ready' && t('footer.version', { version: version.version })}
        {version.status === 'unavailable' && t('footer.versionUnavailable')}
      </footer>
    </div>
  );
}
