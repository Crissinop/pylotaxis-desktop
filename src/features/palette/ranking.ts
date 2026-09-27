import type { Environment, Group, RegisteredApp, Registry, SecretInfo } from '../../lib/ipc';

/**
 * Voci della palette e loro classifica. Funzioni pure: la palette mostra, Rust decide cosa
 * succede a un avvio o a una copia (A.7.3). (v0.5.0)
 */
export type PaletteAction = 'lock' | 'open';

export type PaletteEntry =
  | { kind: 'app'; key: string; app: RegisteredApp; title: string; terms: string[] }
  | {
      kind: 'secret';
      key: string;
      secret: SecretInfo;
      app: RegisteredApp;
      title: string;
      terms: string[];
    }
  | {
      kind: 'group';
      key: string;
      group: Group;
      /** App del gruppo in ordine di avvio; `production` se almeno una lo è (v0.6.0). */
      apps: RegisteredApp[];
      production: boolean;
      title: string;
      terms: string[];
    }
  | { kind: 'action'; key: string; action: PaletteAction; title: string; terms: string[] };

/** Testi tradotti che la palette mostra e in cui si cerca (v0.6.0). */
export interface PaletteLabels {
  lock: string;
  open: string;
  /** Prefisso dei gruppi: "Gruppo · Nome". */
  group: string;
  environments: Record<Environment, string>;
}

/** Risultati mostrati al massimo: la palette è per scegliere in fretta, non per sfogliare. */
export const PALETTE_MAX_RESULTS = 50;

/** Minuscole e senza segni diacritici: "Caffè" e "caffe" si trovano a vicenda. */
export function normalize(text: string): string {
  return text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();
}

/**
 * Voci dal registro: le app (cercabili anche per ambiente), i gruppi, i segreti
 * ("App · Etichetta") e le azioni della palette.
 */
export function buildEntries(registry: Registry, labels: PaletteLabels): PaletteEntry[] {
  const apps = new Map(registry.apps.map((app) => [app.id, app]));
  const categories = new Map(registry.categories.map((category) => [category.id, category.name]));
  const entries: PaletteEntry[] = registry.apps.map((app) => ({
    kind: 'app',
    key: `app:${app.id}`,
    app,
    title: app.name,
    terms: [
      ...app.tags,
      ...(app.categoryId ? [categories.get(app.categoryId) ?? ''] : []),
      ...(app.environment ? [labels.environments[app.environment]] : []),
    ],
  }));
  for (const group of registry.groups) {
    const members = group.appIds.flatMap((id) => {
      const member = apps.get(id);
      return member ? [member] : [];
    });
    entries.push({
      kind: 'group',
      key: `group:${group.id}`,
      group,
      apps: members,
      production: members.some((member) => member.environment === 'production'),
      title: `${labels.group} · ${group.name}`,
      terms: [],
    });
  }
  for (const secret of registry.secrets) {
    const app = apps.get(secret.appId);
    if (!app) continue;
    entries.push({
      kind: 'secret',
      key: `secret:${secret.id}`,
      secret,
      app,
      title: `${app.name} · ${secret.label}`,
      terms: secret.username ? [secret.username] : [],
    });
  }
  for (const action of ['lock', 'open'] as const) {
    entries.push({
      kind: 'action',
      key: `action:${action}`,
      action,
      title: labels[action],
      terms: [],
    });
  }
  return entries;
}

const KIND_ORDER: Record<PaletteEntry['kind'], number> = { app: 0, group: 1, secret: 2, action: 3 };

/** Quanto bene una parola cercata corrisponde a una voce: più basso è meglio; null = no. */
function tokenRank(token: string, title: string, terms: string[]): number | null {
  if (title.startsWith(token)) return 0;
  if (title.split(/[\s·._-]+/).some((word) => word.startsWith(token))) return 1;
  if (title.includes(token)) return 2;
  if (terms.some((term) => term.startsWith(token))) return 3;
  if (terms.some((term) => term.includes(token))) return 4;
  return null;
}

/**
 * Voci che contengono tutte le parole cercate, dalla più pertinente. Senza ricerca: le app,
 * poi i gruppi e le azioni; i segreti compaiono solo cercandoli, per non allungare l'elenco.
 */
export function rank(entries: readonly PaletteEntry[], query: string): PaletteEntry[] {
  const tokens = normalize(query).split(/\s+/).filter(Boolean);
  const scored: { entry: PaletteEntry; score: number }[] = [];
  for (const entry of entries) {
    if (tokens.length === 0) {
      if (entry.kind !== 'secret') scored.push({ entry, score: 0 });
      continue;
    }
    const title = normalize(entry.title);
    const terms = entry.terms.map(normalize);
    let score = 0;
    for (const token of tokens) {
      const found = tokenRank(token, title, terms);
      if (found === null) {
        score = -1;
        break;
      }
      score += found;
    }
    if (score >= 0) scored.push({ entry, score });
  }
  return scored
    .sort(
      (a, b) =>
        a.score - b.score ||
        KIND_ORDER[a.entry.kind] - KIND_ORDER[b.entry.kind] ||
        a.entry.title.localeCompare(b.entry.title, undefined, { sensitivity: 'base' }),
    )
    .slice(0, PALETTE_MAX_RESULTS)
    .map(({ entry }) => entry);
}
