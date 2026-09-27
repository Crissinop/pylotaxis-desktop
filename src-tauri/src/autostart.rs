//! Avvio con Windows (v0.6.0), spento per default. Lo stato vero è quello di Windows (la
//! voce nella chiave Run dell'utente, scritta dal plugin): l'app non ne tiene una copia che potrebbe
//! divergere. Avviata così, l'app parte nella tray, senza finestra.

use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_autostart::AutoLaunchManager;

use crate::errors::CommandError;

/// Argomento con cui Windows avvia l'app all'accesso.
pub const AUTOSTART_ARG: &str = "--autostart";

fn manager<R: Runtime>(app: &AppHandle<R>) -> Result<State<'_, AutoLaunchManager>, CommandError> {
    app.try_state::<AutoLaunchManager>()
        .ok_or(CommandError::AUTOSTART_UNAVAILABLE)
}

#[tauri::command]
pub async fn autostart_status<R: Runtime>(app: AppHandle<R>) -> Result<bool, CommandError> {
    manager(&app)?
        .is_enabled()
        .map_err(|_| CommandError::AUTOSTART_UNAVAILABLE)
}

/// Accende o spegne l'avvio con Windows e restituisce lo stato riletto dal sistema.
#[tauri::command]
pub async fn autostart_set<R: Runtime>(
    app: AppHandle<R>,
    enabled: bool,
) -> Result<bool, CommandError> {
    let manager = manager(&app)?;
    let changed = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    changed.map_err(|_| CommandError::AUTOSTART_UNAVAILABLE)?;
    manager
        .is_enabled()
        .map_err(|_| CommandError::AUTOSTART_UNAVAILABLE)
}

/// Vero se Windows ha avviato l'app all'accesso: in quel caso la finestra resta nascosta.
pub fn started_at_login() -> bool {
    std::env::args().skip(1).any(|arg| arg == AUTOSTART_ARG)
}
