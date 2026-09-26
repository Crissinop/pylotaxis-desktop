//! Guscio Tauri: registra i comandi e avvia la finestra.
//! Le regole vivono nel crate `domain`; qui si valida, si delega e si traduce (A.7.3). (v0.1.0)

mod commands;
mod errors;
mod keystore;
mod lock;
mod platform;
mod security;
mod state;

use tauri::ipc::Invoke;
use tauri::{Manager, Runtime};

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

/// Filtro del blocco davanti al gestore generato (v0.3.0).
///
/// Ogni comando dell'app passa di qui prima di leggere i suoi argomenti, compresi quelli
/// aggiunti in futuro. Senza lo stato dell'app il filtro la considera bloccata: nel dubbio
/// si rifiuta. Il vincolo su `F` serve anche a dare un tipo alla closure del gestore.
fn gated<R, F>(commands: F) -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static
where
    R: Runtime,
    F: Fn(Invoke<R>) -> bool + Send + Sync + 'static,
{
    move |invoke| {
        let locked = invoke
            .message
            .webview_ref()
            .try_state::<AppState>()
            .is_none_or(|state| state.lock.is_locked());
        match lock::gate(invoke.message.command(), locked) {
            Ok(()) => commands(invoke),
            Err(error) => {
                invoke.resolver.reject(error);
                true
            }
        }
    }
}

/// Registra i comandi dietro il filtro. È generico sul runtime perché i test lo provano
/// identico con il runtime simulato di Tauri. (v0.3.0)
fn with_commands<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(gated(tauri::generate_handler![
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
        security::lock_status,
        security::lock_now,
        security::unlock_pin,
        security::unlock_hello,
        security::hello_availability,
        security::security_set_pin,
        security::security_disable,
        security::security_set_hello,
        security::security_set_idle,
    ]))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Unica eccezione alla regola "niente expect" (A.7.5): se il runtime non si avvia
    // non esiste un comportamento degradato possibile, e il messaggio è l'unica diagnosi.
    #[allow(clippy::expect_used)]
    with_commands(tauri::Builder::default())
        // Plugin usati solo da Rust: la capability non concede al webview nessun loro comando.
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let registry = open_registry(app);
            app.manage(AppState::new(registry));
            lock::spawn_monitor(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("avvio del runtime Tauri non riuscito");
}

#[cfg(test)]
mod tests {
    use domain::{Database, DatabaseKey};
    use serde_json::{Value, json};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{
        INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
    };
    use tauri::webview::InvokeRequest;
    use tauri::{WebviewWindow, WebviewWindowBuilder};

    use super::*;
    use crate::lock::ALLOWED_WHILE_LOCKED;

    mod app_commands {
        include!("../app_commands.rs");
    }
    use app_commands::APP_COMMANDS;

    const PIN: &str = "482915";

    /// App con il runtime simulato, il gestore vero e un registro con il PIN impostato:
    /// parte bloccata, come dopo un riavvio.
    fn locked_app() -> (
        tempfile::TempDir,
        tauri::App<MockRuntime>,
        WebviewWindow<MockRuntime>,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(
            &dir.path().join("r.db"),
            &DatabaseKey::from_bytes([9; domain::db::KEY_LEN]),
        )
        .unwrap();
        db.set_pin(None, PIN, 0).unwrap();
        let app = with_commands(mock_builder())
            .manage(AppState::new(Ok(db)))
            .build(mock_context(noop_assets()))
            .unwrap();
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        (dir, app, webview)
    }

    /// Invoca un comando come farebbe il webview, passando da `on_message` (A.6 n. 13).
    fn invoke(
        webview: &WebviewWindow<MockRuntime>,
        command: &str,
        args: Value,
    ) -> Result<Value, Value> {
        let url = if cfg!(windows) {
            "http://tauri.localhost"
        } else {
            "tauri://localhost"
        };
        get_ipc_response(
            webview,
            InvokeRequest {
                cmd: command.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: url.parse().unwrap(),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| body.deserialize::<Value>().unwrap())
    }

    fn locked_error() -> Value {
        json!({ "code": "LOCKED" })
    }

    /// Il criterio della v0.3.0: da bloccata ogni comando, tranne sblocco e app_info, viene
    /// rifiutato con un codice. La prova passa dal gestore vero, non da una sua copia.
    #[test]
    fn while_locked_every_command_but_unlocking_is_refused() {
        let (_dir, _app, webview) = locked_app();
        for command in APP_COMMANDS {
            let result = invoke(&webview, command, json!({}));
            if ALLOWED_WHILE_LOCKED.contains(command) {
                assert_ne!(result.err(), Some(locked_error()), "{command} deve passare");
            } else {
                assert_eq!(
                    result.err(),
                    Some(locked_error()),
                    "{command} deve essere rifiutato"
                );
            }
        }
        // Rifiuto per default: un comando che nessuno ha ancora scritto nasce bloccato.
        assert_eq!(
            invoke(&webview, "comando_futuro", json!({})).err(),
            Some(locked_error())
        );
    }

    #[test]
    fn the_pin_reopens_the_commands_and_locking_closes_them_again() {
        let (_dir, _app, webview) = locked_app();
        let status = invoke(&webview, "lock_status", json!({})).unwrap();
        assert_eq!(
            (status["locked"].clone(), status["pinSet"].clone()),
            (json!(true), json!(true))
        );

        assert_eq!(
            invoke(&webview, "unlock_pin", json!({ "pin": "730268" })).err(),
            Some(json!({ "code": "PIN_WRONG" }))
        );
        assert_eq!(
            invoke(&webview, "registry_list", json!({})).err(),
            Some(locked_error())
        );

        let unlocked = invoke(&webview, "unlock_pin", json!({ "pin": PIN })).unwrap();
        assert_eq!(unlocked["locked"], json!(false));
        assert!(invoke(&webview, "registry_list", json!({})).is_ok());

        let relocked = invoke(&webview, "lock_now", json!({})).unwrap();
        assert_eq!(relocked["locked"], json!(true));
        assert_eq!(
            invoke(&webview, "registry_list", json!({})).err(),
            Some(locked_error())
        );
    }

    /// Nessuna risposta dei comandi del blocco contiene il PIN o la sua impronta (A.6 n. 14).
    #[test]
    fn no_response_carries_the_pin_or_its_hash() {
        let (_dir, _app, webview) = locked_app();
        let mut responses = vec![
            invoke(&webview, "lock_status", json!({})),
            invoke(&webview, "unlock_pin", json!({ "pin": "000001" })),
            invoke(&webview, "unlock_pin", json!({ "pin": PIN })),
            invoke(
                &webview,
                "security_set_pin",
                json!({ "currentPin": PIN, "newPin": "730268" }),
            ),
            invoke(&webview, "security_set_idle", json!({ "minutes": 30 })),
        ];
        responses.push(invoke(
            &webview,
            "security_disable",
            json!({ "currentPin": "730268" }),
        ));
        for response in responses {
            let text = match response {
                Ok(value) | Err(value) => value.to_string(),
            };
            for secret in [PIN, "730268", "argon2", "$argon2id"] {
                assert!(!text.contains(secret), "{secret} in {text}");
            }
        }
    }

    /// La capability concede esattamente i comandi dell'elenco, più i due permessi per gli
    /// eventi del blocco: nessun permesso in più (A.6 n. 10). (v0.3.0)
    #[test]
    fn the_capability_grants_exactly_the_listed_commands() {
        let capability: Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let mut granted: Vec<String> = capability["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap().to_owned())
            .collect();
        granted.sort_unstable();
        let mut expected: Vec<String> = APP_COMMANDS
            .iter()
            .map(|command| format!("allow-{}", command.replace('_', "-")))
            .chain([
                "core:event:allow-listen".into(),
                "core:event:allow-unlisten".into(),
            ])
            .collect();
        expected.sort_unstable();
        assert_eq!(granted, expected);
    }
}
