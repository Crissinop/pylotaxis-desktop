// Controlla che il nome del prodotto compaia solo nei punti elencati in A.2 e che le sue
// fonti coincidano. Il nome non è scritto qui: lo script lo legge, così resta valido anche
// dopo un cambio di nome. (v0.1.0)

import { readdirSync, readFileSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const read = (path) => readFileSync(join(ROOT, path), 'utf8');

const tsName = /export const APP_NAME = '([^']+)';/.exec(read('src/constants/app.ts'))?.[1];
const rustName = /pub const APP_NAME: &str = "([^"]+)";/.exec(
  read('crates/domain/src/lib.rs'),
)?.[1];
const config = JSON.parse(read('src-tauri/tauri.conf.json'));
const windowTitles = (config.app?.windows ?? []).map((window) => window.title);

const problems = [];
const sources = {
  'src/constants/app.ts (APP_NAME)': tsName,
  'crates/domain/src/lib.rs (APP_NAME)': rustName,
  'src-tauri/tauri.conf.json (productName)': config.productName,
  ...Object.fromEntries(
    windowTitles.map((title, i) => [`src-tauri/tauri.conf.json (titolo finestra ${i})`, title]),
  ),
};
if (new Set(Object.values(sources)).size !== 1 || !tsName) {
  problems.push('le fonti del nome non coincidono:');
  for (const [where, value] of Object.entries(sources)) problems.push(`    ${where}: ${value}`);
}

// Nel codice il nome può comparire solo nelle due costanti.
const ALLOWED = new Set(['src/constants/app.ts', 'crates/domain/src/lib.rs']);
const SCANNED = ['src', 'src-tauri/src', 'crates', 'scripts', 'index.html'];
const SKIPPED_DIRS = new Set(['node_modules', 'target', 'gen', 'fonts']);
const TEXT_FILE = /\.(ts|tsx|js|mjs|rs|json|css|html|sql)$/;

function* walk(path) {
  const absolute = join(ROOT, path);
  let children;
  try {
    children = readdirSync(absolute, { withFileTypes: true });
  } catch {
    yield path; // è un file
    return;
  }
  for (const child of children) {
    if (child.isDirectory() && SKIPPED_DIRS.has(child.name)) continue;
    yield* walk(join(path, child.name));
  }
}

if (tsName) {
  const needle = tsName.toLowerCase();
  for (const path of SCANNED.flatMap((entry) => [...walk(entry)])) {
    const normalized = relative(ROOT, join(ROOT, path)).split(sep).join('/');
    if (!TEXT_FILE.test(normalized) || ALLOWED.has(normalized)) continue;
    if (read(normalized).toLowerCase().includes(needle)) {
      problems.push(`${normalized}: contiene il nome del prodotto fuori dai punti ammessi`);
    }
  }
}

if (problems.length > 0) {
  console.error('Controllo del nome non superato:');
  for (const problem of problems) console.error(`  - ${problem}`);
  process.exit(1);
}
console.log(`Nome coerente: ${tsName}`);
