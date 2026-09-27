//! Comandi del blocco e delle impostazioni di sicurezza (v0.3.0).
//!
//! Quali comandi passano da bloccata lo decide il filtro in lock.rs, non questo file. Qui i
//! comandi della schermata di blocco leggono il database con `with_db_even_if_locked`; tutti
//! gli altri con `with_db`, che da bloccata rifiuta comunque. Nessun comando restituisce il
//! PIN o la sua impronta: solo lo stato che l'interfaccia deve mostrare (A.7.10).

use std::time::{SystemTime, UNIX_EPOCH};

use domain::Database;
use domain::lock::LockSettings;
use serde::Serialize;
use tauri::async_runtime::spawn_blocking;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::commands::clip;
use crate::errors::CommandError;
use crate::lock;
use crate::platform;
use crate::state::AppState;

/// Etichetta della finestra principale in tauri.conf.json: il prompt di Hello e gli appunti
/// si legano a lei.
pub(crate) const MAIN_WINDOW: &str = "main";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LockStatusDto {
    locked: bool,
    pin_set: bool,
    hello_enabled: bool,
    idle_minutes: Option<u32>,
    /// Millisecondi prima del prossimo tentativo di PIN ammesso; 0 = subito.
    retry_after_ms: u64,
}

impl LockStatusDto {
    fn new(locked: bool, settings: &LockSettings, now_ms: i64) -> Self {
        Self {
            locked,
            pin_set: settings.pin_set,
            hello_enabled: settings.hello_enabled,
            idle_minutes: settings.idle_minutes,
            retry_after_ms: settings.retry_after_ms(now_ms),
        }
    }
}

/// Ora in millisecondi Unix, per l'attesa dopo i tentativi falliti e le date dei segreti.
pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

fn status(state: &AppState) -> Result<LockStatusDto, CommandError> {
    let settings = state.with_db_even_if_locked(|db| Ok(db.lock_settings()?))?;
    Ok(LockStatusDto::new(
        state.lock.is_locked(),
        &settings,
        now_ms(),
    ))
}

/// Applica una modifica della configurazione e riallinea la copia in memoria del blocco.
/// Passa da `with_db`: da bloccata le impostazioni non si toccano.
fn change(
    state: &AppState,
    apply: impl FnOnce(&Database) -> Result<(), domain::Error>,
) -> Result<LockStatusDto, CommandError> {
    let settings = state.with_db(|db| {
        apply(db)?;
        Ok(db.lock_settings()?)
    })?;
    state.lock.sync(&settings);
    Ok(LockStatusDto::new(
        state.lock.is_locked(),
        &settings,
        now_ms(),
    ))
}

/// Esegue fuori dal runtime asincrono il lavoro che calcola un'impronta Argon2id (64 MiB,
/// qualche centinaio di millisecondi): non tiene occupato un thread dei comandi.
async fn off_thread<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    work: impl FnOnce(&AppState) -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    let app = app.clone();
    spawn_blocking(move || {
        let state = app
            .try_state::<AppState>()
            .ok_or(CommandError::STATE_UNAVAILABLE)?;
        work(&state)
    })
    .await
    .map_err(|_| CommandError::STATE_UNAVAILABLE)?
}

/// Verifica la presenza dell'utente con Windows Hello, legata alla finestra principale.
async fn verify_presence<R: Runtime>(
    app: &AppHandle<R>,
    message: &str,
) -> Result<(), CommandError> {
    let window = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or(CommandError::STATE_UNAVAILABLE)?;
    // Prompt di sistema solo con l'app in primo piano (A.7.5): una richiesta arrivata con la
    // finestra in secondo piano non l'ha chiesta l'utente con un gesto.
    if !window.is_focused().unwrap_or(false) {
        return Err(CommandError::NOT_FOREGROUND);
    }
    let owner = platform::window_owner(&window)?;
    let message = clip(message);
    spawn_blocking(move || platform::hello_verify(owner, &message))
        .await
        .map_err(|_| CommandError::HELLO_FAILED)?
}

#[tauri::command]
pub async fn lock_status(state: State<'_, AppState>) -> Result<LockStatusDto, CommandError> {
    status(&state)
}

#[tauri::command]
pub async fn lock_now<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<LockStatusDto, CommandError> {
    if !state.lock.is_configured() {
        return Err(domain::Error::PinNotSet.into());
    }
    lock::engage(&app, &state);
    status(&state)
}

/// Sblocca con il PIN. Se l'app è già sbloccata restituisce lo stato senza verificare nulla:
/// due richieste ravvicinate dalla schermata di blocco non producono un errore.
#[tauri::command]
pub async fn unlock_pin<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    pin: String,
) -> Result<LockStatusDto, CommandError> {
    if state.lock.is_locked() {
        off_thread(&app, move |state| {
            state.with_db_even_if_locked(|db| Ok(db.check_pin(&pin, now_ms())?))
        })
        .await?;
        lock::release(&app, &state);
    }
    status(&state)
}

/// Sblocca con Windows Hello, se l'utente l'ha attivato. Il testo del prompt arriva dal
/// frontend, già tradotto, come per la finestra di scelta dei file.
#[tauri::command]
pub async fn unlock_hello<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    message: String,
) -> Result<LockStatusDto, CommandError> {
    if state.lock.is_locked() {
        let settings = state.with_db_even_if_locked(|db| Ok(db.lock_settings()?))?;
        if !settings.hello_enabled {
            return Err(CommandError::HELLO_NOT_ENABLED);
        }
        verify_presence(&app, &message).await?;
        lock::release(&app, &state);
    }
    status(&state)
}

/// Dice se Windows Hello è pronto su questa macchina, per le impostazioni.
#[tauri::command]
pub async fn hello_availability() -> Result<(), CommandError> {
    spawn_blocking(platform::hello_availability)
        .await
        .map_err(|_| CommandError::HELLO_FAILED)?
}

/// Imposta il primo PIN, che accende il blocco, oppure lo cambia (serve quello attuale).
/// L'app resta sbloccata: il blocco scatta dal prossimo avvio, dall'inattività o a mano.
#[tauri::command]
pub async fn security_set_pin<R: Runtime>(
    app: AppHandle<R>,
    current_pin: Option<String>,
    new_pin: String,
) -> Result<LockStatusDto, CommandError> {
    off_thread(&app, move |state| {
        change(state, |db| {
            db.set_pin(current_pin.as_deref(), &new_pin, now_ms())
        })
    })
    .await
}

#[tauri::command]
pub async fn security_disable<R: Runtime>(
    app: AppHandle<R>,
    current_pin: String,
) -> Result<LockStatusDto, CommandError> {
    off_thread(&app, move |state| {
        change(state, |db| db.disable_lock(&current_pin, now_ms()))
    })
    .await
}

/// Accende o spegne Windows Hello. Si accende solo dopo una verifica riuscita: se Hello non
/// funziona lo si scopre adesso, non alla prima schermata di blocco.
#[tauri::command]
pub async fn security_set_hello<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    enabled: bool,
    message: String,
) -> Result<LockStatusDto, CommandError> {
    if enabled {
        let settings = state.with_db(|db| Ok(db.lock_settings()?))?;
        if !settings.pin_set {
            return Err(domain::Error::PinNotSet.into());
        }
        verify_presence(&app, &message).await?;
    }
    change(&state, |db| db.set_hello_enabled(enabled))
}

#[tauri::command]
pub async fn security_set_idle(
    state: State<'_, AppState>,
    minutes: Option<u32>,
) -> Result<LockStatusDto, CommandError> {
    change(&state, |db| db.set_idle_minutes(minutes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(value: &serde_json::Value) -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort_unstable();
        keys
    }

    /// Il frontend legge esattamente queste chiavi (src/lib/ipc.ts), e tra loro non c'è
    /// nulla del PIN: né il valore né l'impronta (A.6 n. 14).
    #[test]
    fn the_status_has_the_expected_keys_and_nothing_about_the_pin() {
        let settings = LockSettings {
            pin_set: true,
            hello_enabled: true,
            idle_minutes: Some(15),
            failed_attempts: 5,
            last_failure_ms: Some(1_000),
        };
        let json = serde_json::to_value(LockStatusDto::new(true, &settings, 11_000)).unwrap();
        assert_eq!(
            keys(&json),
            [
                "helloEnabled",
                "idleMinutes",
                "locked",
                "pinSet",
                "retryAfterMs"
            ]
        );
        assert_eq!(json["retryAfterMs"], 20_000);
        assert_eq!(
            keys(&serde_json::to_value(lock::LockEvent { locked: true }).unwrap()),
            ["locked"]
        );
    }
}
