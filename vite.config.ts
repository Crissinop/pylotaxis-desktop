import react from '@vitejs/plugin-react';
import process from 'node:process';
import { defineConfig } from 'vitest/config';

// Impostazioni per Tauri riprese dal template ufficiale (create-tauri-app 4.7.4). (v0.1.0)
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  // Non nasconde gli errori di compilazione di Rust nel terminale.
  clearScreen: false,
  server: {
    // Tauri si aspetta una porta fissa: se è occupata, meglio fallire che cambiarla.
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: { ignored: ['**/src-tauri/**', '**/crates/**', '**/target/**'] },
  },
  build: {
    // Nessun asset incorporato come data: URI: la CSP della build ammette solo 'self' (A.7.10).
    assetsInlineLimit: 0,
  },
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
});
