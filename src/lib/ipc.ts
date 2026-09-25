import { invoke } from '@tauri-apps/api/core';

/**
 * Stessa forma di `AppInfo` in src-tauri/src/commands.rs: un test Rust ne blocca le chiavi,
 * così una modifica da una parte non rompe l'altra in silenzio. (v0.1.0)
 */
export interface AppInfo {
  name: string;
  version: string;
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>('app_info');
}
