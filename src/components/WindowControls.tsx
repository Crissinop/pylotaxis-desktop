import { getCurrentWindow } from '@tauri-apps/api/window';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

/**
 * Riduci, ingrandisci e chiudi per la barra del titolo propria (v0.5.0). Chiudere nasconde la
 * finestra: Rust intercetta la richiesta e l'app resta nella tray. La finestra è
 * ridimensionabile dai bordi anche senza decorazioni (tao, WM_NCHITTEST).
 */
export function WindowControls() {
  const { t } = useTranslation();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const window = getCurrentWindow();
    let mounted = true;
    let stop: (() => void) | null = null;
    const sync = () =>
      void window
        .isMaximized()
        .then((value) => mounted && setMaximized(value))
        .catch(() => undefined);
    sync();
    window
      .onResized(sync)
      .then((unlisten) => {
        if (mounted) stop = unlisten;
        else unlisten();
      })
      .catch(() => undefined);
    return () => {
      mounted = false;
      stop?.();
    };
  }, []);

  const window = getCurrentWindow();
  return (
    <div className="window-controls">
      <button
        type="button"
        className="window-controls__button"
        aria-label={t('titlebar.minimize')}
        title={t('titlebar.minimize')}
        onClick={() => void window.minimize()}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path d="M1 5.5h8" />
        </svg>
      </button>
      <button
        type="button"
        className="window-controls__button"
        aria-label={maximized ? t('titlebar.restore') : t('titlebar.maximize')}
        title={maximized ? t('titlebar.restore') : t('titlebar.maximize')}
        onClick={() => void window.toggleMaximize()}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true">
          {maximized ? <path d="M3 1.5h5.5V7M1.5 3h5.5v5.5H1.5z" /> : <path d="M1.5 1.5h7v7h-7z" />}
        </svg>
      </button>
      <button
        type="button"
        className="window-controls__button window-controls__button--close"
        aria-label={t('titlebar.close')}
        title={t('titlebar.close')}
        onClick={() => void window.close()}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path d="M1.5 1.5l7 7M8.5 1.5l-7 7" />
        </svg>
      </button>
    </div>
  );
}
