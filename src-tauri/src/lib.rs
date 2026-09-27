//! Guscio Tauri: registra i comandi e avvia la finestra.
//! Le regole vivono nel crate `domain`; qui si valida, si delega e si traduce (A.7.3). (v0.1.0)

mod autostart;
mod clipboard;
mod commands;
mod errors;
mod groups;
mod health;
mod icons;
mod keystore;
mod links;
mod lock;
mod palette;
mod platform;
mod secrets;
mod security;
mod shortcut;
mod state;
mod tray;
mod vault;

use std::collections::HashSet;

use tauri::ipc::Invoke;
use tauri::{Manager, RunEvent, Runtime};

use crate::errors::CommandError;
use crate::keystore::KeyStore;
use crate::state::AppState;
use crate::vault::{KeyringBackend, Vault};

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
        secrets::secret_create,
        secrets::secret_update,
        secrets::secret_replace,
        secrets::secret_delete,
        secrets::secret_copy,
        shortcut::shortcut_status,
        shortcut::shortcut_set,
        tray::tray_setup,
        palette::main_window_show,
        palette::palette_ready,
        groups::group_create,
        groups::group_update,
        groups::group_delete,
        groups::group_launch,
        health::health_status,
        autostart::autostart_status,
        autostart::autostart_set,
        icons::app_icons,
        icons::app_icon_pick,
        icons::app_icon_reset,
    ]))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Unica eccezione alla regola "niente expect" (A.7.5): se il runtime non si avvia
    // non esiste un comportamento degradato possibile, e il messaggio è l'unica diagnosi.
    #[allow(clippy::expect_used)]
    let app = with_commands(tauri::Builder::default())
        // Istanza singola come primo plugin, come chiede la sua documentazione. Una seconda
        // istanza porta davanti la finestra; se porta un link, il plugin dei link l'ha già
        // ricevuto e apre la palette (links.rs). (v0.5.0)
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !links::carries_link(app, &args) {
                tray::show_main(app);
            }
        }))
        // Plugin usati solo da Rust: nessuna capability concede al webview un loro comando.
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // Avvio con Windows: la voce porta l'argomento che tiene nascosta la finestra (v0.6.0).
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args([autostart::AUTOSTART_ARG])
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let registry = open_registry(app);
            let vault = Vault::new(Box::new(KeyringBackend::new(&app.config().identifier)));
            let state = AppState::new(registry, vault);
            sweep_orphan_secrets(&state);
            app.manage(state);
            lock::spawn_monitor(app.handle().clone());
            // Rapidità (v0.5.0): chiusura verso la tray, palette nascosta quando perde il
            // fuoco, scorciatoia salvata e link diretti.
            let handle = app.handle();
            tray::keep_in_tray(handle);
            palette::hide_on_blur(handle);
            shortcut::register_saved(handle, &app.state::<AppState>());
            links::listen(handle);
            // La finestra nasce nascosta (tauri.conf.json) e si mostra qui, tranne quando
            // Windows avvia l'app all'accesso: allora resta nella tray. (v0.6.0)
            if !autostart::started_at_login() {
                tray::show_main(handle);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("avvio del runtime Tauri non riuscito");
    app.run(|handle, event| {
        // Alla chiusura un segreto copiato non resta negli appunti (v0.4.0).
        if let RunEvent::Exit = event
            && let Some(state) = handle.try_state::<AppState>()
        {
            state.clipboard.clear_now();
        }
    });
}

/// Elimina le credenziali dei segreti che il registro non conosce più: restano da un errore
/// a metà tra registro e Credential Manager. Si fa all'avvio, anche da bloccata, perché è
/// lavoro interno che non passa dall'IPC. Senza registro leggibile non si tocca nulla. (v0.4.0)
fn sweep_orphan_secrets(state: &AppState) {
    let known = state.with_db_even_if_locked(|db| {
        Ok(db
            .secrets()?
            .into_iter()
            .map(|secret| secret.id)
            .collect::<HashSet<_>>())
    });
    if let Ok(known) = known {
        state.vault.sweep(&known);
    }
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
    use crate::vault::MemoryBackend;

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
            .manage(AppState::new(Ok(db), memory_vault()))
            .build(mock_context(noop_assets()))
            .unwrap();
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        (dir, app, webview)
    }

    fn memory_vault() -> Vault {
        Vault::new(Box::new(MemoryBackend::default()))
    }

    /// App sbloccata (nessun PIN) con un'app web nel registro, di cui restituisce l'id.
    fn unlocked_app() -> (
        tempfile::TempDir,
        tauri::App<MockRuntime>,
        WebviewWindow<MockRuntime>,
        String,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(
            &dir.path().join("r.db"),
            &DatabaseKey::from_bytes([8; domain::db::KEY_LEN]),
        )
        .unwrap();
        let portal = db
            .create_app(&domain::registry::AppInput {
                name: "Portale".into(),
                target: domain::registry::Target::Web {
                    url: "https://example.com/".into(),
                },
                category_id: None,
                tags: Vec::new(),
                environment: None,
                health_check: false,
            })
            .unwrap();
        let app = with_commands(mock_builder())
            .manage(AppState::new(Ok(db), memory_vault()))
            .build(mock_context(noop_assets()))
            .unwrap();
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        (dir, app, webview, portal.id.to_string())
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

    const SENTINEL: &str = "valore-sentinella-9f2c";
    const SENTINEL_REPLACED: &str = "sostituto-sentinella-41aa";

    /// Il criterio della v0.4.0: nessun comando restituisce un segreto al webview. Il valore
    /// sentinella entra dal gestore vero e si verifica che stia davvero nel vault; poi si
    /// invocano tutti i comandi, con argomenti validi dove possono girare senza finestre di
    /// sistema (scelta dei file e avvio restano con argomenti vuoti), e nessuna risposta lo
    /// contiene. Una sonda che copia il valore nell'etichetta fa fallire il test (A.6 n. 14).
    #[test]
    fn no_command_ever_returns_a_secret_value() {
        let (_dir, app, webview, portal) = unlocked_app();
        let created = invoke(
            &webview,
            "secret_create",
            json!({ "appId": portal, "label": "Token", "username": "mario", "value": SENTINEL }),
        )
        .unwrap();
        let secret = created["id"].as_str().unwrap().to_owned();
        let state = app.state::<AppState>();
        let stored = state
            .vault
            .with_value(secret.parse().unwrap(), |value| Ok(value == SENTINEL))
            .unwrap();
        assert!(stored, "il valore deve stare nel vault");

        let mut texts = vec![created.to_string()];
        let with_args = |command: &str| match command {
            "secret_update" => json!({ "id": secret, "label": "Token API", "username": null }),
            "secret_replace" => json!({ "id": secret, "value": SENTINEL_REPLACED }),
            "secret_copy" => json!({ "id": secret }),
            _ => json!({}),
        };
        for command in APP_COMMANDS.iter().filter(|c| **c != "secret_delete") {
            let response = invoke(&webview, command, with_args(command));
            texts.push(match response {
                Ok(value) | Err(value) => value.to_string(),
            });
        }
        for text in &texts {
            for value in [SENTINEL, SENTINEL_REPLACED] {
                assert!(!text.contains(value), "{value} in {text}");
            }
        }
        let listed = invoke(&webview, "registry_list", json!({})).unwrap();
        assert_eq!(listed["secrets"][0]["label"], "Token API");
    }

    /// Eliminare un segreto o l'app che lo contiene elimina anche il valore dal vault.
    #[test]
    fn deleting_a_secret_or_its_app_removes_the_value_too() {
        let (_dir, app, webview, portal) = unlocked_app();
        let create = |label: &str| {
            invoke(
                &webview,
                "secret_create",
                json!({ "appId": portal, "label": label, "username": null, "value": "x" }),
            )
            .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let (first, _second) = (create("Uno"), create("Due"));
        let state = app.state::<AppState>();
        assert_eq!(state.vault.stored_ids().len(), 2);
        invoke(&webview, "secret_delete", json!({ "id": first })).unwrap();
        assert_eq!(state.vault.stored_ids().len(), 1);
        invoke(&webview, "app_delete", json!({ "id": portal })).unwrap();
        assert!(state.vault.stored_ids().is_empty());
    }

    /// Una riga che non si scrive (etichetta già usata) non lascia il valore nel vault.
    #[test]
    fn a_refused_secret_leaves_no_value_behind() {
        let (_dir, app, webview, portal) = unlocked_app();
        let args = json!({ "appId": portal, "label": "Token", "username": null, "value": "x" });
        invoke(&webview, "secret_create", args.clone()).unwrap();
        assert_eq!(
            invoke(&webview, "secret_create", args).err(),
            Some(json!({ "code": "SECRET_LABEL_DUPLICATE" }))
        );
        assert_eq!(app.state::<AppState>().vault.stored_ids().len(), 1);
    }

    /// Comandi della palette, e quelli che ha solo lei (v0.5.0).
    const PALETTE_COMMANDS: &[&str] = &[
        "registry_list",
        "app_launch",
        "secret_copy",
        "lock_now",
        "main_window_show",
        "palette_ready",
        // Un gruppo si apre anche dalla palette, che mostra lo stato delle app (v0.6.0).
        "group_launch",
        "health_status",
        // La palette mostra le icone accanto ai nomi (v0.7.0).
        "app_icons",
    ];
    const PALETTE_ONLY: &[&str] = &["main_window_show", "palette_ready"];
    /// Permessi core: eventi per tutte e due; barra del titolo propria per la principale;
    /// per la palette solo nascondersi.
    const MAIN_CORE: &[&str] = &[
        "core:event:allow-listen",
        "core:event:allow-unlisten",
        "core:window:allow-start-dragging",
        "core:window:allow-internal-toggle-maximize",
        "core:window:allow-minimize",
        "core:window:allow-toggle-maximize",
        "core:window:allow-is-maximized",
        "core:window:allow-close",
    ];
    const PALETTE_CORE: &[&str] = &[
        "core:event:allow-listen",
        "core:event:allow-unlisten",
        "core:window:allow-hide",
    ];

    /// Permessi di una capability, ordinati, dopo aver verificato a quale finestra va.
    fn granted(source: &str, window: &str) -> Vec<String> {
        let capability: Value = serde_json::from_str(source).unwrap();
        assert_eq!(capability["windows"], json!([window]));
        let mut granted: Vec<String> = capability["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap().to_owned())
            .collect();
        granted.sort_unstable();
        granted
    }

    fn expected<'a>(commands: impl Iterator<Item = &'a &'a str>, core: &[&str]) -> Vec<String> {
        let mut expected: Vec<String> = commands
            .map(|command| format!("allow-{}", command.replace('_', "-")))
            .chain(core.iter().map(|permission| (*permission).to_owned()))
            .collect();
        expected.sort_unstable();
        expected
    }

    /// Ogni finestra riceve esattamente i suoi permessi, nessuno in più (A.6 n. 10 e 13): la
    /// principale tutti i comandi tranne quelli della sola palette; la palette solo i suoi.
    /// Ogni comando dell'elenco è concesso ad almeno una finestra. (v0.3.0, per finestra dalla
    /// v0.5.0)
    #[test]
    fn each_window_gets_exactly_its_commands() {
        assert_eq!(
            granted(include_str!("../capabilities/default.json"), "main"),
            expected(
                APP_COMMANDS.iter().filter(|c| !PALETTE_ONLY.contains(c)),
                MAIN_CORE
            )
        );
        assert_eq!(
            granted(include_str!("../capabilities/palette.json"), "palette"),
            expected(PALETTE_COMMANDS.iter(), PALETTE_CORE)
        );
        for command in PALETTE_COMMANDS {
            assert!(APP_COMMANDS.contains(command), "{command} non è un comando");
        }
    }

    /// Una scorciatoia fuori elenco si rifiuta nel dominio; una che il sistema non concede
    /// (qui manca il plugin) lascia la precedente e il valore salvato com'erano. (v0.5.0)
    #[test]
    fn a_refused_shortcut_leaves_the_previous_one_in_place() {
        let (_dir, _app, webview, _portal) = unlocked_app();
        assert_eq!(
            invoke(
                &webview,
                "shortcut_set",
                json!({ "shortcut": "Ctrl+Alt+Delete" })
            )
            .err(),
            Some(json!({ "code": "SHORTCUT_INVALID" }))
        );
        assert_eq!(
            invoke(
                &webview,
                "shortcut_set",
                json!({ "shortcut": "Ctrl+Alt+P" })
            )
            .err(),
            Some(json!({ "code": "SHORTCUT_TAKEN" }))
        );
        let status = invoke(&webview, "shortcut_status", json!({})).unwrap();
        assert_eq!(status["shortcut"], "Ctrl+Alt+Space");
        assert_eq!(status["options"].as_array().unwrap().len(), 3);
    }
}
