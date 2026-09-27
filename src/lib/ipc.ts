import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

/**
 * Ponte verso i comandi Rust (src-tauri/src/commands.rs). Le forme dei dati coincidono con i
 * DTO Rust, e un test Rust ne blocca le chiavi: una modifica da una parte non rompe l'altra
 * in silenzio. Il frontend invia id e gettoni, mai percorsi da eseguire (A.7.10). (v0.2.0)
 */

export interface AppInfo {
  name: string;
  version: string;
}

export type AppKind = 'executable' | 'web' | 'protocol';

export interface Category {
  id: string;
  name: string;
}

export interface RegisteredApp {
  id: string;
  name: string;
  kind: AppKind;
  /** Solo da mostrare: per avviare si usa l'id. */
  target: string;
  categoryId: string | null;
  tags: string[];
}

/**
 * Segreto di un'app: solo i dati descrittivi (src-tauri/src/secrets.rs). Il valore non arriva
 * mai al webview: si scrive una volta e poi si copia negli appunti da Rust. (v0.4.0)
 */
export interface SecretInfo {
  id: string;
  appId: string;
  label: string;
  username: string | null;
  updatedMs: number;
}

export interface Registry {
  categories: Category[];
  apps: RegisteredApp[];
  secrets: SecretInfo[];
}

export interface ExecutablePick {
  token: string;
  path: string;
  suggestedName: string;
}

export type TargetInput =
  | { kind: 'executable'; pickToken: string | null }
  | { kind: 'web'; url: string }
  | { kind: 'protocol'; uri: string };

export interface AppInput {
  name: string;
  target: TargetInput;
  categoryId: string | null;
  tags: string[];
}

export const getAppInfo = () => invoke<AppInfo>('app_info');
export const listRegistry = () => invoke<Registry>('registry_list');
export const createCategory = (name: string) => invoke<Category>('category_create', { name });
export const renameCategory = (id: string, name: string) =>
  invoke<Category>('category_rename', { id, name });
export const deleteCategory = (id: string) => invoke<void>('category_delete', { id });
export const pickExecutable = (title: string, filterLabel: string) =>
  invoke<ExecutablePick | null>('executable_pick', { title, filterLabel });
export const createApp = (input: AppInput) => invoke<RegisteredApp>('app_create', { input });
export const updateApp = (id: string, input: AppInput) =>
  invoke<RegisteredApp>('app_update', { id, input });
export const deleteApp = (id: string) => invoke<void>('app_delete', { id });
export const launchApp = (id: string, acceptChanged: boolean) =>
  invoke<void>('app_launch', { id, acceptChanged });

/**
 * Stato del blocco (src-tauri/src/security.rs). Dice se il PIN c'è, mai quale sia: nessun
 * comando restituisce il PIN o la sua impronta (A.7.10). (v0.3.0)
 */
export interface LockStatus {
  locked: boolean;
  pinSet: boolean;
  helloEnabled: boolean;
  /** `null` = nessun blocco per inattività. */
  idleMinutes: number | null;
  /** Millisecondi prima del prossimo tentativo di PIN ammesso; 0 = subito. */
  retryAfterMs: number;
}

export const getLockStatus = () => invoke<LockStatus>('lock_status');
export const lockNow = () => invoke<LockStatus>('lock_now');
export const unlockWithPin = (pin: string) => invoke<LockStatus>('unlock_pin', { pin });
export const unlockWithHello = (message: string) => invoke<LockStatus>('unlock_hello', { message });
export const checkHelloAvailability = () => invoke<void>('hello_availability');
export const setPin = (currentPin: string | null, newPin: string) =>
  invoke<LockStatus>('security_set_pin', { currentPin, newPin });
export const disableLock = (currentPin: string) =>
  invoke<LockStatus>('security_disable', { currentPin });
export const setHelloEnabled = (enabled: boolean, message: string) =>
  invoke<LockStatus>('security_set_hello', { enabled, message });
export const setIdleMinutes = (minutes: number | null) =>
  invoke<LockStatus>('security_set_idle', { minutes });

export const createSecret = (
  appId: string,
  label: string,
  username: string | null,
  value: string,
) => invoke<SecretInfo>('secret_create', { appId, label, username, value });
export const updateSecret = (id: string, label: string, username: string | null) =>
  invoke<SecretInfo>('secret_update', { id, label, username });
export const replaceSecret = (id: string, value: string) =>
  invoke<SecretInfo>('secret_replace', { id, value });
export const deleteSecret = (id: string) => invoke<void>('secret_delete', { id });
/** Rust copia il valore negli appunti e risponde solo quando li svuoterà. */
export const copySecret = (id: string) => invoke<{ clearAfterMs: number }>('secret_copy', { id });

/** Scorciatoia della palette (src-tauri/src/shortcut.rs): quella scelta, se Windows l'ha concessa, e le opzioni. (v0.5.0) */
export interface ShortcutStatus {
  shortcut: string;
  active: boolean;
  options: string[];
}

export const getShortcut = () => invoke<ShortcutStatus>('shortcut_status');
export const setShortcut = (shortcut: string) =>
  invoke<ShortcutStatus>('shortcut_set', { shortcut });

/** Testi del menu della tray, già tradotti: Rust non ha le traduzioni (src-tauri/src/tray.rs). */
export interface TrayLabels {
  tooltip: string;
  open: string;
  palette: string;
  lock: string;
  quit: string;
}

export const setupTray = (labels: TrayLabels) => invoke<void>('tray_setup', { labels });
export const showMainWindow = () => invoke<void>('main_window_show');

/** Etichetta della finestra della palette in tauri.conf.json. */
export const PALETTE_WINDOW = 'palette';

/** Evento di Rust a ogni apertura della palette, con l'app da preselezionare (link diretti). */
export const PALETTE_OPENED = 'palette-opened';

export const onPaletteOpened = (handler: (select: string | null) => void) =>
  listen<{ select: string | null }>(PALETTE_OPENED, (event) => handler(event.payload.select));

/** La palette ascolta: Rust consegna un'apertura rimasta in attesa (link all'avvio). (v0.5.0) */
export const paletteReady = () => invoke<void>('palette_ready');

/** Evento di Rust a ogni blocco e sblocco (src-tauri/src/lock.rs, `LOCK_EVENT`). */
export const LOCK_EVENT = 'lock-changed';

export const onLockChanged = (handler: (locked: boolean) => void) =>
  listen<{ locked: boolean }>(LOCK_EVENT, (event) => handler(event.payload.locked));
