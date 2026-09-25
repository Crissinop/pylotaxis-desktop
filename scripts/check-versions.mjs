// Controlla che tutti i manifest riportino la stessa versione (A.6 n. 16). (v0.1.0)
// Tre file allineati a mano sono un meccanismo fragile: qui l'allineamento diventa un controllo.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const read = (path) => readFileSync(join(ROOT, path), 'utf8');
const SEMVER = /^\d+\.\d+\.\d+$/;

/** Legge `version` dalla sola sezione [workspace.package] del Cargo.toml alla radice. */
function workspaceVersion(toml) {
  let inSection = false;
  for (const raw of toml.split(/\r?\n/)) {
    const line = raw.trim();
    if (line.startsWith('[')) inSection = line === '[workspace.package]';
    else if (inSection) {
      const match = /^version\s*=\s*"([^"]*)"/.exec(line);
      if (match) return match[1];
    }
  }
  return undefined;
}

const problems = [];

const versions = {
  'package.json': JSON.parse(read('package.json')).version,
  'package-lock.json': JSON.parse(read('package-lock.json')).packages?.['']?.version,
  'Cargo.toml [workspace.package]': workspaceVersion(read('Cargo.toml')),
  'src-tauri/tauri.conf.json': JSON.parse(read('src-tauri/tauri.conf.json')).version,
};

// I crate devono ereditare la versione del workspace, non dichiararne una propria.
for (const crate of ['src-tauri/Cargo.toml', 'crates/domain/Cargo.toml']) {
  if (!/^version\.workspace\s*=\s*true\s*$/m.test(read(crate))) {
    problems.push(`${crate}: manca "version.workspace = true"`);
  }
}

const distinct = new Set(Object.values(versions));
if (distinct.size !== 1) problems.push('versioni diverse tra i manifest');
for (const [file, version] of Object.entries(versions)) {
  if (!version || !SEMVER.test(version))
    problems.push(`${file}: versione mancante o non semantica`);
}

if (problems.length > 0) {
  console.error('Versioni non allineate:');
  for (const [file, version] of Object.entries(versions)) {
    console.error(`  ${file}: ${version ?? '(mancante)'}`);
  }
  for (const problem of problems) console.error(`  - ${problem}`);
  process.exit(1);
}
console.log(`Versioni allineate: ${versions['package.json']}`);
