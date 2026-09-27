//! Icona nella tray e finestra principale che vive lì (v0.5.0).
//!
//! I testi del menu arrivano dal frontend con `tray_setup`, perché le traduzioni stanno lì:
//! così la tray parla la lingua dell'interfaccia. Il comando è ammesso da bloccata, perché
//! l'app può partire bloccata, e può soltanto cambiare quelle etichette; lo concede solo la
//! capability della finestra principale. Chiudere la finestra la nasconde: si esce dalla tray.

use std::sync::Mutex;

use serde::Deserialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, WindowEvent};

use crate::errors::CommandError;
use crate::state::AppState;
use crate::{lock, palette};

pub const MAIN_LABEL: &str = "main";
const TRAY_ID: &str = "main";
const MENU_OPEN: &str = "open";
const MENU_PALETTE: &str = "palette";
const MENU_LOCK: &str = "lock";
const MENU_QUIT: &str = "quit";

/// Lunghezza massima di un'etichetta: i testi arrivano dal webview e si validano (A.7.5).
const MAX_LABEL_CHARS: usize = 64;

/// Una configurazione alla volta: due chiamate ravvicinate non creano due icone.
static SETUP: Mutex<()> = Mutex::new(());

/// Operazione accessoria su una finestra o un evento (A.7.5): un errore non rompe il flusso
/// principale; in sviluppo si stampa.
pub fn quietly(result: tauri::Result<()>) {
    if let Err(_error) = result {
        #[cfg(debug_assertions)]
        eprintln!("operazione sulla finestra non riuscita: {_error}");
    }
}

/// Testi del menu, già tradotti dal frontend.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrayLabels {
    tooltip: String,
    open: String,
    palette: String,
    lock: String,
    quit: String,
}

impl TrayLabels {
    /// Non vuote, non oltre `MAX_LABEL_CHARS` caratteri e senza caratteri di controllo.
    fn validate(&self) -> Result<(), CommandError> {
        let fine = |text: &String| {
            !text.trim().is_empty()
                && text.chars().count() <= MAX_LABEL_CHARS
                && !text.chars().any(char::is_control)
        };
        let all = [
            &self.tooltip,
            &self.open,
            &self.palette,
            &self.lock,
            &self.quit,
        ];
        if all.into_iter().all(fine) {
            Ok(())
        } else {
            Err(domain::Error::NameInvalid.into())
        }
    }
}

fn menu<R: Runtime>(app: &AppHandle<R>, labels: &TrayLabels) -> tauri::Result<Menu<R>> {
    let open = MenuItem::with_id(app, MENU_OPEN, &labels.open, true, None::<&str>)?;
    let palette = MenuItem::with_id(app, MENU_PALETTE, &labels.palette, true, None::<&str>)?;
    let lock = MenuItem::with_id(app, MENU_LOCK, &labels.lock, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, &labels.quit, true, None::<&str>)?;
    Menu::with_items(app, &[&open, &palette, &lock, &separator, &quit])
}

fn on_menu<R: Runtime>(app: &AppHandle<R>, id: &str) {
    match id {
        MENU_OPEN => show_main(app),
        MENU_PALETTE => palette::open(app, None),
        // Senza PIN non c'è blocco da applicare: `engage` non fa nulla.
        MENU_LOCK => {
            if let Some(state) = app.try_state::<AppState>() {
                lock::engage(app, &state);
            }
        }
        // All'uscita gli appunti si svuotano (RunEvent::Exit, lib.rs).
        MENU_QUIT => app.exit(0),
        _ => {}
    }
}

/// Crea l'icona al primo invio, poi ne aggiorna menu e suggerimento (cambio di lingua).
#[tauri::command]
pub async fn tray_setup<R: Runtime>(
    app: AppHandle<R>,
    labels: TrayLabels,
) -> Result<(), CommandError> {
    labels.validate()?;
    let _one_at_a_time = SETUP.lock().map_err(|_| CommandError::STATE_UNAVAILABLE)?;
    let menu = menu(&app, &labels).map_err(|_| CommandError::TRAY_UNAVAILABLE)?;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(menu))
            .and_then(|()| tray.set_tooltip(Some(&labels.tooltip)))
            .map_err(|_| CommandError::TRAY_UNAVAILABLE)?;
        return Ok(());
    }
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or(CommandError::TRAY_UNAVAILABLE)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip(&labels.tooltip)
        .menu(&menu)
        // Clic sinistro: la finestra; il menu con il destro, come nelle app di Windows.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(&app)
        .map_err(|_| CommandError::TRAY_UNAVAILABLE)?;
    Ok(())
}

/// Porta davanti la finestra principale: dalla tray, da una seconda istanza, dalla palette.
pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_LABEL) {
        quietly(window.unminimize());
        quietly(window.show());
        quietly(window.set_focus());
    }
}

/// Chiudere la finestra principale (pulsante o Alt+F4) la nasconde: il portale resta nella
/// tray (A.8). Se l'icona non si fosse creata, riavviare l'app porta davanti quella aperta
/// (istanza singola).
pub fn keep_in_tray<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(MAIN_LABEL) else {
        return;
    };
    let target = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            quietly(target.hide());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(open: &str) -> TrayLabels {
        TrayLabels {
            tooltip: "Portale".into(),
            open: open.into(),
            palette: "Palette".into(),
            lock: "Blocca".into(),
            quit: "Esci".into(),
        }
    }

    #[test]
    fn labels_are_validated_before_reaching_the_menu() {
        assert!(labels("Apri").validate().is_ok());
        assert!(labels(&"à".repeat(MAX_LABEL_CHARS)).validate().is_ok());
        for bad in [
            "",
            "   ",
            "Apri\n",
            "Apri\u{7}",
            &"a".repeat(MAX_LABEL_CHARS + 1),
        ] {
            assert_eq!(
                labels(bad).validate(),
                Err(CommandError {
                    code: "NAME_INVALID"
                }),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn unknown_fields_are_refused() {
        let extra = r#"{"tooltip":"a","open":"b","palette":"c","lock":"d","quit":"e","x":"f"}"#;
        assert!(serde_json::from_str::<TrayLabels>(extra).is_err());
    }
}
