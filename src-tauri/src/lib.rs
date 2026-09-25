//! Guscio Tauri: registra i comandi e avvia la finestra.
//! Le regole vivono nel crate `domain`; qui si valida, si delega e si traduce (A.7.3). (v0.1.0)

mod commands;
mod errors;
mod keystore;
mod state;

use tauri::Manager;

use crate::errors::CommandError;
use crate::keystore::KeyStore;
use crate::state::AppState;

/// Apre il database all'avvio. Un errore non blocca la finestra: resta nello stato e
/// l'interfaccia lo spiega con il suo codice (A.7.5). (v0.2.0)
fn open_registry(app: &tauri::App) -> Result<domain::Database, CommandError> {
    keystore::init_store()?;
    let keys = KeyStore::open(&app.config().identifier)?;
    // Cartella locale (%LOCALAPPDATA%), non roaming: il registro contiene percorsi di
    // questa macchina e non deve seguire il profilo altrove. (v0.2.0)
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|_| CommandError::APP_DATA_UNAVAILABLE)?;
    state::open_database(&dir, &keys)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Unica eccezione alla regola "niente expect" (A.7.5): se il runtime non si avvia
    // non esiste un comportamento degradato possibile, e il messaggio è l'unica diagnosi.
    #[allow(clippy::expect_used)]
    tauri::Builder::default()
        // Plugin usati solo da Rust: la capability non concede al webview nessun loro comando.
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let registry = open_registry(app);
            app.manage(AppState::new(registry));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::registry_list,
            commands::category_create,
            commands::category_rename,
            commands::category_delete,
            commands::executable_pick,
            commands::app_create,
            commands::app_update,
            commands::app_delete,
            commands::app_launch,
        ])
        .run(tauri::generate_context!())
        .expect("avvio del runtime Tauri non riuscito");
}
