import { getCurrentWindow } from '@tauri-apps/api/window';
import { useEffect, useId, useRef, useState } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';

import { EnvironmentBadge } from '../../components/EnvironmentBadge';
import { APP_NAME } from '../../constants/app';
import { errorCode } from '../../lib/errors';
import {
  copySecret,
  launchApp,
  launchGroup,
  listRegistry,
  lockNow,
  onLockChanged,
  onPaletteOpened,
  paletteReady,
  showMainWindow,
  type Environment,
  type HealthStatus,
  type Registry,
} from '../../lib/ipc';
import { useHealth } from '../health/useHealth';
import { buildEntries, rank, type PaletteEntry } from './ranking';

const hide = () => void getCurrentWindow().hide();

/** Riga di dettaglio di una voce: che cos'è, com'è (stato) e dove porta. */
function detail(
  entry: PaletteEntry,
  t: TFunction,
  health: ReadonlyMap<string, HealthStatus>,
): string {
  if (entry.kind === 'app') {
    const status = entry.app.healthCheck ? health.get(entry.app.id) : undefined;
    return [
      t(`registry.kind.${entry.app.kind}`),
      ...(status ? [t(`health.${status}`)] : []),
      entry.app.target,
    ].join(' · ');
  }
  if (entry.kind === 'group') return t('palette.groupDetail', { count: entry.apps.length });
  if (entry.kind === 'secret') return entry.secret.username ?? t('palette.kind.secret');
  return t('palette.kind.action');
}

/** Ambiente da mostrare accanto al titolo; per un gruppo, la produzione se ne contiene. */
function environmentOf(entry: PaletteEntry): Environment | null {
  if (entry.kind === 'app' || entry.kind === 'secret') return entry.app.environment;
  if (entry.kind === 'group') return entry.production ? 'production' : null;
  return null;
}

/**
 * Palette di comando, nella sua finestra (v0.5.0). Precaricata e nascosta: Rust la mostra con
 * la scorciatoia e manda `palette-opened`, a cui si rilegge il registro. Tastiera prima di
 * tutto: frecce per scegliere, Invio per eseguire, Esc per chiudere. Al blocco dimentica il
 * registro, e Rust la nasconde.
 */
export function Palette() {
  const { t } = useTranslation();
  const ids = { input: useId(), list: useId(), error: useId() };
  const input = useRef<HTMLInputElement>(null);
  const [registry, setRegistry] = useState<Registry | null>(null);
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);
  const [preselect, setPreselect] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const health = useHealth(registry !== null);

  useEffect(() => {
    let mounted = true;
    const stops: (() => void)[] = [];
    const keep = (stop: () => void) => (mounted ? stops.push(stop) : stop());
    onPaletteOpened((select) => {
      setQuery('');
      setActive(0);
      setError(null);
      setPreselect(select);
      input.current?.focus();
      listRegistry()
        .then((next) => mounted && setRegistry(next))
        .catch(() => mounted && setRegistry(null));
    })
      .then((stop) => {
        keep(stop);
        // Solo ora un'apertura chiesta prima (link all'avvio) può arrivare a destinazione.
        return paletteReady();
      })
      .catch(() => undefined);
    onLockChanged((locked) => {
      if (!locked) return;
      setRegistry(null);
      setQuery('');
      setPreselect(null);
    })
      .then(keep)
      .catch(() => undefined);
    return () => {
      mounted = false;
      stops.forEach((stop) => stop());
    };
  }, []);

  const entries = registry
    ? buildEntries(registry, {
        lock: t('palette.actionLock'),
        open: t('palette.actionOpen', { name: APP_NAME }),
        group: t('palette.group'),
        environments: {
          development: t('environment.development'),
          test: t('environment.test'),
          production: t('environment.production'),
        },
      })
    : [];
  const results = rank(entries, query);
  const preselected =
    preselect === null ? -1 : results.findIndex((e) => e.key === `app:${preselect}`);
  const current = preselected >= 0 ? preselected : Math.min(active, results.length - 1);

  const move = (step: number) => {
    if (results.length === 0) return;
    setPreselect(null);
    setActive((current + step + results.length) % results.length);
  };

  const run = (entry: PaletteEntry | undefined) => {
    if (!entry) return;
    setError(null);
    const fail = (failure: unknown) => {
      const code = errorCode(failure);
      if (code === 'LOCKED') hide();
      else setError(code === 'HASH_MISMATCH' ? 'palette.changed' : `errors.${code}`);
    };
    // Un gruppo si apre con Invio e la palette si chiude. Se qualcosa non parte, Rust porta
    // l'esito nella finestra principale, dove lo si conferma (v0.6.0).
    if (entry.kind === 'group') {
      launchGroup(entry.group.id).then(hide).catch(fail);
      return;
    }
    const request =
      entry.kind === 'app'
        ? launchApp(entry.app.id, false)
        : entry.kind === 'secret'
          ? copySecret(entry.secret.id)
          : entry.action === 'lock'
            ? lockNow()
            : showMainWindow();
    request.then(hide).catch(fail);
  };

  const optionId = (index: number) => `${ids.list}-${index}`;

  return (
    <div className="palette">
      <input
        ref={input}
        id={ids.input}
        className="palette__input"
        role="combobox"
        aria-label={t('palette.label')}
        aria-expanded={results.length > 0}
        aria-controls={ids.list}
        aria-activedescendant={current >= 0 ? optionId(current) : undefined}
        aria-describedby={error ? ids.error : undefined}
        placeholder={t('palette.placeholder')}
        value={query}
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => {
          setQuery(event.target.value);
          setActive(0);
          setPreselect(null);
        }}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown') move(1);
          else if (event.key === 'ArrowUp') move(-1);
          else if (event.key === 'Enter') run(results[current]);
          else if (event.key === 'Escape') hide();
          else return;
          event.preventDefault();
        }}
      />
      <ul id={ids.list} className="palette__list" role="listbox" aria-label={t('palette.label')}>
        {results.map((entry, index) => (
          <li
            key={entry.key}
            id={optionId(index)}
            role="option"
            aria-selected={index === current}
            className="palette__option"
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => run(entry)}
          >
            <span className="palette__title">
              {entry.title}
              <EnvironmentBadge environment={environmentOf(entry)} />
            </span>
            <span className="palette__detail">{detail(entry, t, health)}</span>
          </li>
        ))}
      </ul>
      {results.length === 0 && (
        <p className="palette__empty">{registry ? t('palette.empty') : t('palette.loading')}</p>
      )}
      <p id={ids.error} className="palette__footer" role="status">
        {error ? t(error, { defaultValue: t('errors.UNKNOWN') }) : t('palette.hint')}
      </p>
    </div>
  );
}
