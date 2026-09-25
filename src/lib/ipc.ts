import { invoke } from '@tauri-apps/api/core';

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

export interface Registry {
  categories: Category[];
  apps: RegisteredApp[];
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
