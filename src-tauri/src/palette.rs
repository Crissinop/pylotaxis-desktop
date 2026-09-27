//! Palette di comando, nella sua finestra (v0.5.0).
//!
//! La finestra `palette` nasce nascosta con l'app (tauri.conf.json) e non si distrugge mai:
//! mostrarla costa un `show` e un evento, per il criterio dei 100 ms (A.7.6). Ha una capability
//! sua, con i soli comandi che usa. Si nasconde con Esc e dopo un'azione (frontend), quando
//! perde il fuoco (qui) e al blocco (lock.rs). Da bloccata non si apre: la scorciatoia e la
//! tray portano davanti la schermata di blocco, perché la palette elenca il registro.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State, WindowEvent};
use uuid::Uuid;

use crate::errors::CommandError;
use crate::state::AppState;
use crate::tray::{self, quietly};

pub const PALETTE_LABEL: &str = "palette";

/// Evento a ogni apertura, con l'app da preselezionare (link diretti).
pub const PALETTE_EVENT: &str = "palette-opened";

#[derive(Debug, Clone, Serialize)]
struct PaletteOpened {
    select: Option<String>,
}

/// Aperture chieste prima che la pagina della palette ascolti. All'avvio da un link la
/// finestra esiste già, ma il suo ascoltatore non ancora: l'evento andrebbe perso, e la
/// palette resterebbe vuota. La pagina lo segnala con `palette_ready`.
#[derive(Default)]
pub struct PaletteState {
    inner: Mutex<Readiness>,
}

#[derive(Default)]
struct Readiness {
    ready: bool,
    /// `Some(select)`: un'apertura in attesa; vale l'ultima.
    pending: Option<Option<Uuid>>,
}

impl PaletteState {
    /// Vero se l'apertura si può consegnare subito; altrimenti resta in attesa di `ready`.
    fn request(&self, select: Option<Uuid>) -> bool {
        match self.inner.lock() {
            Ok(mut inner) if !inner.ready => {
                inner.pending = Some(select);
                false
            }
            _ => true,
        }
    }

    /// La pagina ascolta: da qui in poi si consegna subito. Restituisce l'apertura in attesa.
    fn ready(&self) -> Option<Option<Uuid>> {
        let mut inner = self.inner.lock().ok()?;
        inner.ready = true;
        inner.pending.take()
    }
}

fn locked<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<AppState>()
        .is_none_or(|state| state.lock.is_locked())
}

/// Apre la palette, con un'app preselezionata se arriva da un link. Da bloccata porta
/// davanti la finestra principale, cioè la schermata di blocco.
pub fn open<R: Runtime>(app: &AppHandle<R>, select: Option<Uuid>) {
    if locked(app) {
        tray::show_main(app);
        return;
    }
    let Some(window) = app.get_webview_window(PALETTE_LABEL) else {
        return;
    };
    #[cfg(debug_assertions)]
    let started = std::time::Instant::now();
    quietly(window.show());
    quietly(window.set_focus());
    let deliver = app
        .try_state::<AppState>()
        .is_none_or(|state| state.palette.request(select));
    if deliver {
        let payload = PaletteOpened {
            select: select.map(|id| id.to_string()),
        };
        quietly(app.emit_to(PALETTE_LABEL, PALETTE_EVENT, payload));
    }
    // Misura della sola parte Rust: il criterio dei 100 ms si giudica in release (A.7.6).
    #[cfg(debug_assertions)]
    eprintln!(
        "palette: parte Rust in {} µs",
        started.elapsed().as_micros()
    );
}

pub fn hide<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(PALETTE_LABEL) {
        quietly(window.hide());
    }
}

/// La scorciatoia apre la palette, e se è già aperta la chiude.
pub fn toggle<R: Runtime>(app: &AppHandle<R>) {
    let visible = app
        .get_webview_window(PALETTE_LABEL)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    if visible && !locked(app) {
        hide(app);
    } else {
        open(app, None);
    }
}

/// La palette si nasconde quando perde il fuoco: non resta sopra le altre app.
pub fn hide_on_blur<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(PALETTE_LABEL) else {
        return;
    };
    let target = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            quietly(target.hide());
        }
    });
}

/// Azione "Apri" della palette: porta davanti la finestra principale.
#[tauri::command]
pub async fn main_window_show<R: Runtime>(app: AppHandle<R>) -> Result<(), CommandError> {
    tray::show_main(&app);
    Ok(())
}

/// La pagina della palette ascolta gli eventi. Ammesso da bloccata (lock.rs): segna solo che
/// la pagina è pronta, e da bloccata l'apertura in attesa si scarta.
#[tauri::command]
pub async fn palette_ready<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    if let Some(select) = state.palette.ready()
        && !state.lock.is_locked()
    {
        open(&app, select);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openings_before_the_page_listens_wait_and_the_last_one_wins() {
        let state = PaletteState::default();
        let (first, second) = (Uuid::now_v7(), Uuid::now_v7());
        assert!(!state.request(Some(first)));
        assert!(!state.request(Some(second)));
        assert_eq!(state.ready(), Some(Some(second)));
        // Da pronta si consegna subito e non resta nulla in attesa.
        assert!(state.request(None));
        assert_eq!(state.ready(), None);
    }

    #[test]
    fn a_page_ready_without_requests_has_nothing_pending() {
        let state = PaletteState::default();
        assert_eq!(state.ready(), None);
        assert!(state.request(Some(Uuid::now_v7())));
    }
}
