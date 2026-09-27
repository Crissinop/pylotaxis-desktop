import { useEffect, useState } from 'react';

import { getAppIcons, type RegisteredApp } from '../../lib/ipc';

const NO_ICONS: ReadonlyMap<string, string> = new Map();

/**
 * Firma delle icone: cambia solo se cambia un'app o un'icona. Il registro non trasporta le
 * immagini, così la palette resta rapida all'apertura; le immagini si rileggono solo qui.
 */
export function iconKey(apps: readonly Pick<RegisteredApp, 'id' | 'iconRev'>[]): string {
  return apps
    .map((app) => `${app.id}:${app.iconRev ?? ''}`)
    .sort()
    .join('|');
}

/** Immagini delle icone per id di app; le app assenti mostrano le iniziali. (v0.7.0) */
export function useAppIcons(apps: readonly RegisteredApp[]): ReadonlyMap<string, string> {
  const key = iconKey(apps);
  const [icons, setIcons] = useState<ReadonlyMap<string, string>>(NO_ICONS);

  // Il risultato si applica nel callback, mai nel corpo dell'effetto (lezione 33). Da
  // bloccata Rust rifiuta: restano le icone già note.
  useEffect(() => {
    let alive = true;
    getAppIcons()
      .then((list) => {
        if (alive) setIcons(new Map(list.map((icon) => [icon.id, icon.dataUrl])));
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [key]);

  return icons;
}
