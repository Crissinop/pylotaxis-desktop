import type { Category, RegisteredApp, Registry } from '../../lib/ipc';

export interface Section {
  /** `null` per le app senza categoria. */
  category: Category | null;
  apps: RegisteredApp[];
}

/**
 * Raggruppa le app per categoria, nell'ordine che arriva da Rust. Le categorie vuote restano,
 * così si possono gestire; la sezione senza categoria va in fondo e compare solo se serve.
 * Un'app con una categoria sconosciuta finisce senza categoria invece di sparire. (v0.2.0)
 */
export function toSections(registry: Registry): Section[] {
  const known = new Set(registry.categories.map((category) => category.id));
  const sections: Section[] = registry.categories.map((category) => ({
    category,
    apps: registry.apps.filter((app) => app.categoryId === category.id),
  }));
  const uncategorized = registry.apps.filter(
    (app) => app.categoryId === null || !known.has(app.categoryId),
  );
  if (uncategorized.length > 0) sections.push({ category: null, apps: uncategorized });
  return sections;
}
