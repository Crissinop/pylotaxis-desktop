//! Comandi dei segreti delle app (v0.4.0).
//!
//! Il valore entra da qui una volta sola (creazione o sostituzione) e non esce mai: nessun
//! comando lo restituisce, nemmeno in parte. Per usarlo, `secret_copy` lo porta direttamente
//! negli appunti. Da bloccata tutti questi comandi sono rifiutati dal filtro (lock.rs) e da
//! `with_db` (A.7.10).

use domain::secrets::{SecretInfo, new_secret_id, validate_secret_fields, validate_secret_value};
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::clipboard::{self, CLIPBOARD_CLEAR_MS};
use crate::errors::CommandError;
use crate::platform;
use crate::security::{MAIN_WINDOW, now_ms};
use crate::state::{AppState, parse_id};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretDto {
    id: String,
    app_id: String,
    label: String,
    username: Option<String>,
    updated_ms: i64,
}

impl From<SecretInfo> for SecretDto {
    fn from(secret: SecretInfo) -> Self {
        Self {
            id: secret.id.to_string(),
            app_id: secret.app_id.to_string(),
            label: secret.label,
            username: secret.username,
            updated_ms: secret.updated_ms,
        }
    }
}

/// Risposta della copia: solo quando gli appunti verranno svuotati, mai il valore.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyDto {
    clear_after_ms: u64,
}

/// Crea un segreto. La credenziale nasce prima della riga: se la riga non si scrive (es.
/// etichetta già usata), la credenziale si elimina subito; se l'app si interrompe a metà,
/// la ritrova la pulizia all'avvio.
#[tauri::command]
pub async fn secret_create(
    state: State<'_, AppState>,
    app_id: String,
    label: String,
    username: Option<String>,
    value: String,
) -> Result<SecretDto, CommandError> {
    let app_id = parse_id(&app_id)?;
    let fields = validate_secret_fields(&label, username.as_deref())?;
    validate_secret_value(&value)?;
    state.with_db(|db| {
        db.app(app_id)?;
        let id = new_secret_id();
        state.vault.store(id, &value)?;
        db.create_secret(id, app_id, &fields, now_ms())
            .map(Into::into)
            .map_err(|error| {
                let _ = state.vault.remove(id);
                error.into()
            })
    })
}

/// Cambia etichetta e nome utente; il valore non si tocca.
#[tauri::command]
pub async fn secret_update(
    state: State<'_, AppState>,
    id: String,
    label: String,
    username: Option<String>,
) -> Result<SecretDto, CommandError> {
    let id = parse_id(&id)?;
    let fields = validate_secret_fields(&label, username.as_deref())?;
    state.with_db(|db| Ok(db.update_secret(id, &fields, now_ms())?.into()))
}

/// Sostituisce il valore. Modificare significa sostituire, mai leggere.
#[tauri::command]
pub async fn secret_replace(
    state: State<'_, AppState>,
    id: String,
    value: String,
) -> Result<SecretDto, CommandError> {
    let id = parse_id(&id)?;
    validate_secret_value(&value)?;
    state.with_db(|db| {
        db.secret(id)?;
        state.vault.store(id, &value)?;
        Ok(db.touch_secret(id, now_ms())?.into())
    })
}

/// Elimina un segreto: prima la riga, poi la credenziale. Se la credenziale non si elimina,
/// il segreto è comunque sparito dal registro e la pulizia all'avvio la ritrova.
#[tauri::command]
pub async fn secret_delete(state: State<'_, AppState>, id: String) -> Result<(), CommandError> {
    let id = parse_id(&id)?;
    state.with_db(|db| Ok(db.delete_secret(id)?))?;
    let _ = state.vault.remove(id);
    Ok(())
}

/// Copia il valore negli appunti, fuori dalla cronologia e dalla sincronizzazione, e ne
/// programma lo svuotamento. Il valore passa dal Credential Manager agli appunti senza
/// attraversare l'IPC.
#[tauri::command]
pub async fn secret_copy<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: String,
) -> Result<CopyDto, CommandError> {
    let id = parse_id(&id)?;
    let window = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or(CommandError::STATE_UNAVAILABLE)?;
    let owner = platform::window_owner(&window)?;
    let sequence = state.with_db(|db| {
        db.secret(id)?;
        state
            .vault
            .with_value(id, |value| platform::clipboard_copy_private(owner, value))
    })?;
    state.clipboard.remember(sequence);
    clipboard::schedule_clear(app, sequence);
    Ok(CopyDto {
        clear_after_ms: CLIPBOARD_CLEAR_MS,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(value: &serde_json::Value) -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort_unstable();
        keys
    }

    /// Chiavi lette dal frontend (src/lib/ipc.ts): nessuna porta il valore.
    #[test]
    fn secret_dtos_have_the_expected_keys_and_no_value() {
        let secret = SecretDto {
            id: String::new(),
            app_id: String::new(),
            label: String::new(),
            username: None,
            updated_ms: 0,
        };
        assert_eq!(
            keys(&serde_json::to_value(secret).unwrap()),
            ["appId", "id", "label", "updatedMs", "username"]
        );
        assert_eq!(
            keys(&serde_json::to_value(CopyDto { clear_after_ms: 1 }).unwrap()),
            ["clearAfterMs"]
        );
    }
}
