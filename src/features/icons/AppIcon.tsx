import type { RegisteredApp } from '../../lib/ipc';
import { initials, monogramTone } from './monogram';

type IconSize = 'large' | 'small';

interface AppIconProps {
  name: string;
  /** Immagine salvata; senza, le iniziali. */
  src: string | undefined;
  size?: IconSize;
}

/**
 * Icona di un'app (v0.7.0): l'immagine salvata o le iniziali su un tono stabile. È decorativa:
 * il nome è sempre scritto accanto, perché due app possono avere la stessa icona (A.8).
 */
export function AppIcon({ name, src, size = 'large' }: AppIconProps) {
  if (src) {
    return <img className={`app-icon app-icon--${size}`} src={src} alt="" draggable={false} />;
  }
  return (
    <span
      className={`app-icon app-icon--${size} app-icon--mono app-icon--tone-${monogramTone(name)}`}
      aria-hidden="true"
    >
      {initials(name)}
    </span>
  );
}

interface GroupIconProps {
  name: string;
  /** App del gruppo in ordine di avvio: si mostrano le prime quattro. */
  apps: readonly RegisteredApp[];
  icons: ReadonlyMap<string, string>;
  size?: IconSize;
}

/** Icona di un gruppo: le icone delle sue prime quattro app in una griglia 2×2. */
export function GroupIcon({ name, apps, icons, size = 'large' }: GroupIconProps) {
  if (apps.length === 0) return <AppIcon name={name} src={undefined} size={size} />;
  return (
    <span className={`group-icon group-icon--${size}`} aria-hidden="true">
      {apps.slice(0, 4).map((app) => (
        <AppIcon key={app.id} name={app.name} src={icons.get(app.id)} size="small" />
      ))}
    </span>
  );
}
