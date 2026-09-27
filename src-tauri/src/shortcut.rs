//! Scorciatoia globale della palette (v0.5.0).
//!
//! La combinazione si sceglie dall'elenco chiuso del dominio (`domain::shortcut`). Windows può
//! non concederla, se un'altra app l'ha già presa: resta attiva la precedente e il comando
//! risponde `SHORTCUT_TAKEN`. Da bloccata la scorciatoia porta davanti la schermata di blocco
//! (palette.rs). Il plugin esegue la registrazione sul thread principale e ne attende l'esito.

use std::sync::Mutex;

use domain::shortcut::{DEFAULT_PALETTE_SHORTCUT, PALETTE_SHORTCUTS, validate_shortcut};
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_global_shortcut::{GlobalShortcut, ShortcutState as KeyState};

use crate::errors::CommandError;
use crate::palette;
use crate::state::AppState;

/// Combinazione in uso e se Windows l'ha concessa. Il mutex resta preso per tutto un cambio:
/// due richieste ravvicinate non si intrecciano.
pub struct ShortcutState {
    current: Mutex<Current>,
}

#[derive(Debug, Clone, Copy)]
struct Current {
    shortcut: &'static str,
    active: bool,
}

impl Default for ShortcutState {
    fn default() -> Self {
        Self {
            current: Mutex::new(Current {
                shortcut: DEFAULT_PALETTE_SHORTCUT,
                active: false,
            }),
        }
    }
}

/// Ciò che mostrano le impostazioni: la scelta, se è attiva e le opzioni ammesse.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatusDto {
    shortcut: &'static str,
    active: bool,
    options: &'static [&'static str],
}

impl From<Current> for ShortcutStatusDto {
    fn from(current: Current) -> Self {
        Self {
            shortcut: current.shortcut,
            active: current.active,
            options: PALETTE_SHORTCUTS,
        }
    }
}

/// Registra la combinazione con il suo gestore. Un rifiuto del plugin (combinazione già presa,
/// gestore dei tasti non disponibile) diventa `SHORTCUT_TAKEN`; senza il plugin, come nel
/// runtime dei test, non c'è nessuno che possa concederla e vale lo stesso.
fn register<R: Runtime>(app: &AppHandle<R>, shortcut: &'static str) -> Result<(), CommandError> {
    let manager = app
        .try_state::<GlobalShortcut<R>>()
        .ok_or(CommandError::SHORTCUT_TAKEN)?;
    manager
        .on_shortcut(shortcut, |app, _shortcut, event| {
            // Una pressione produce due eventi, pressione e rilascio: conta solo la prima.
            if event.state == KeyState::Pressed {
                palette::toggle(app);
            }
        })
        .map_err(|_error| {
            #[cfg(debug_assertions)]
            eprintln!("scorciatoia {shortcut} non concessa: {_error}");
            CommandError::SHORTCUT_TAKEN
        })
}

fn unregister<R: Runtime>(app: &AppHandle<R>, shortcut: &'static str) {
    if let Some(manager) = app.try_state::<GlobalShortcut<R>>()
        && let Err(_error) = manager.unregister(shortcut)
    {
        #[cfg(debug_assertions)]
        eprintln!("scorciatoia {shortcut} non rilasciata: {_error}");
    }
}

/// All'avvio registra la scorciatoia salvata. Si legge anche da bloccata perché è lavoro
/// interno, che non passa dall'IPC: da bloccata la scorciatoia deve portare alla schermata di
/// blocco. Senza registro leggibile vale la predefinita.
pub fn register_saved<R: Runtime>(app: &AppHandle<R>, state: &AppState) {
    let shortcut = state
        .with_db_even_if_locked(|db| Ok(db.palette_shortcut()?))
        .unwrap_or(DEFAULT_PALETTE_SHORTCUT);
    let active = register(app, shortcut).is_ok();
    if let Ok(mut current) = state.shortcut.current.lock() {
        *current = Current { shortcut, active };
    }
}

#[tauri::command]
pub async fn shortcut_status(
    state: State<'_, AppState>,
) -> Result<ShortcutStatusDto, CommandError> {
    let current = state
        .shortcut
        .current
        .lock()
        .map_err(|_| CommandError::STATE_UNAVAILABLE)?;
    Ok((*current).into())
}

/// Cambia la scorciatoia. La nuova si registra prima di lasciare la vecchia: se Windows la
/// rifiuta, la precedente resta attiva e nulla viene salvato. Si salva solo una combinazione
/// concessa, e la vecchia si rilascia per ultima.
#[tauri::command]
pub async fn shortcut_set<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    shortcut: String,
) -> Result<ShortcutStatusDto, CommandError> {
    let next = validate_shortcut(&shortcut)?;
    let mut current = state
        .shortcut
        .current
        .lock()
        .map_err(|_| CommandError::STATE_UNAVAILABLE)?;
    if next == current.shortcut {
        // Stessa scelta: se all'avvio Windows l'aveva rifiutata, si riprova.
        if !current.active {
            register(&app, next)?;
            current.active = true;
        }
        return Ok((*current).into());
    }
    register(&app, next)?;
    if let Err(error) = state.with_db(|db| Ok(db.set_palette_shortcut(next)?)) {
        unregister(&app, next);
        return Err(error);
    }
    if current.active {
        unregister(&app, current.shortcut);
    }
    *current = Current {
        shortcut: next,
        active: true,
    };
    Ok((*current).into())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use tauri_plugin_global_shortcut::Shortcut;

    use super::*;

    /// Ogni voce dell'elenco del dominio si interpreta con il parser del plugin: il testo
    /// salvato è sempre una combinazione valida.
    #[test]
    fn every_listed_shortcut_parses_with_the_plugin() {
        for shortcut in PALETTE_SHORTCUTS {
            assert!(Shortcut::from_str(shortcut).is_ok(), "{shortcut}");
        }
    }
}
