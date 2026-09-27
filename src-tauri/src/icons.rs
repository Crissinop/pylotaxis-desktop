//! Icone delle app (v0.7.0).
//!
//! L'icona di un eseguibile si estrae con le API di Windows alla registrazione, quando cambia
//! l'impronta e, per le app registrate prima della v0.7.0, alla prima richiesta. Un'immagine
//! scelta dall'utente arriva dalla finestra di Windows aperta da Rust e il dominio la verifica.
//! Il webview riceve solo immagini, mai percorsi (A.7.10).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use domain::icons::{
    EXE_ICON_SIDE, ICON_MAX_BYTES, IconKind, encode_rgba_png, icon_rgba, png_data_url,
};
use domain::registry::Target;
use serde::Serialize;
use tauri::async_runtime::spawn_blocking;
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::commands::clip;
use crate::errors::CommandError;
use crate::platform::executable_icon_pixels;
use crate::state::{AppState, parse_id};

/// Eseguibili già provati in questa sessione, per id e impronta: un file senza icona non si
/// riapre a ogni richiesta.
#[derive(Default)]
pub struct IconState {
    attempted: Mutex<HashSet<(Uuid, String)>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconDto {
    id: String,
    data_url: String,
}

/// PNG dell'icona di un eseguibile, se il file ne ha una.
pub fn executable_icon(path: &Path) -> Option<Vec<u8>> {
    let pixels = executable_icon_pixels(path, EXE_ICON_SIDE)?;
    let rgba = icon_rgba(&pixels.bgra, pixels.mask.as_deref());
    encode_rgba_png(pixels.width, pixels.height, &rgba).ok()
}

/// Estrae e salva l'icona dell'eseguibile di un'app. È una funzione accessoria: un errore non
/// ferma chi l'ha chiamata (A.7.5), e un'icona scelta dall'utente non viene sostituita.
pub(crate) async fn refresh_executable_icon(state: &AppState, id: Uuid) {
    let Ok(app) = state.with_db(|db| Ok(db.app(id)?)) else {
        return;
    };
    let Target::Executable { path, .. } = app.target else {
        return;
    };
    store_executable_icon(state, id, path).await;
}

async fn store_executable_icon(state: &AppState, id: Uuid, path: PathBuf) {
    if let Ok(Some(png)) = spawn_blocking(move || executable_icon(&path)).await {
        let _ = state.with_db(|db| Ok(db.set_app_icon(id, &png, IconKind::Executable)?));
    }
}

/// Icone salvate di tutte le app. Gli eseguibili ancora senza icona la ricevono qui, una
/// volta per sessione e per impronta.
#[tauri::command]
pub async fn app_icons(state: State<'_, AppState>) -> Result<Vec<IconDto>, CommandError> {
    let apps = state.with_db(|db| Ok(db.registry()?.apps))?;
    let missing: Vec<(Uuid, PathBuf)> = {
        let mut attempted = state
            .icons
            .attempted
            .lock()
            .map_err(|_| CommandError::STATE_UNAVAILABLE)?;
        apps.into_iter()
            .filter(|app| app.icon_rev.is_none())
            .filter_map(|app| match app.target {
                Target::Executable { path, sha256 } => {
                    attempted.insert((app.id, sha256)).then_some((app.id, path))
                }
                _ => None,
            })
            .collect()
    };
    for (id, path) in missing {
        store_executable_icon(&state, id, path).await;
    }
    let icons = state.with_db(|db| Ok(db.app_icons()?))?;
    Ok(icons
        .into_iter()
        .map(|(id, png)| IconDto {
            id: id.to_string(),
            data_url: png_data_url(&png),
        })
        .collect())
}

/// Sceglie un'immagine PNG con la finestra di Windows e la usa come icona dell'app.
/// `false` se la finestra è stata chiusa senza scegliere.
#[tauri::command]
pub async fn app_icon_pick<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: String,
    title: String,
    filter_label: String,
) -> Result<bool, CommandError> {
    let id = parse_id(&id)?;
    state.with_db(|db| Ok(db.app(id)?))?;
    let dialog = app.dialog().clone();
    let (title, filter_label) = (clip(&title), clip(&filter_label));
    let picked = spawn_blocking(move || {
        dialog
            .file()
            .set_title(title)
            .add_filter(filter_label, &["png"])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| CommandError::DIALOG_FAILED)?;
    let Some(file) = picked else {
        return Ok(false);
    };
    let path = file.into_path().map_err(|_| domain::Error::IconInvalid)?;
    let png = spawn_blocking(move || read_bounded(&path))
        .await
        .map_err(|_| CommandError::DIALOG_FAILED)??;
    state.with_db(|db| Ok(db.set_app_icon(id, &png, IconKind::Custom)?))?;
    Ok(true)
}

/// Legge un file solo se non supera il peso ammesso: un file enorme non entra in memoria.
fn read_bounded(path: &Path) -> Result<Vec<u8>, domain::Error> {
    let size = std::fs::metadata(path).map_err(domain::Error::Io)?.len();
    if size > u64::try_from(ICON_MAX_BYTES).unwrap_or(u64::MAX) {
        return Err(domain::Error::IconTooLarge);
    }
    std::fs::read(path).map_err(domain::Error::Io)
}

/// Toglie l'immagine scelta: un eseguibile riprende la sua icona, le altre app le iniziali.
#[tauri::command]
pub async fn app_icon_reset(state: State<'_, AppState>, id: String) -> Result<(), CommandError> {
    let id = parse_id(&id)?;
    state.with_db(|db| Ok(db.clear_app_icon(id)?))?;
    refresh_executable_icon(&state, id).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_serialize_with_the_keys_the_frontend_expects() {
        let dto = IconDto {
            id: String::new(),
            data_url: String::new(),
        };
        let value = serde_json::to_value(dto).unwrap();
        let mut keys: Vec<&String> = value.as_object().unwrap().keys().collect();
        keys.sort_unstable();
        assert_eq!(keys, ["dataUrl", "id"]);
    }

    #[test]
    fn a_file_over_the_limit_is_refused_before_reading_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("grande.png");
        std::fs::write(&path, vec![0_u8; ICON_MAX_BYTES + 1]).unwrap();
        assert!(matches!(
            read_bounded(&path),
            Err(domain::Error::IconTooLarge)
        ));
    }

    #[test]
    fn outside_windows_there_is_no_icon_and_nothing_fails() {
        #[cfg(not(windows))]
        assert_eq!(executable_icon(Path::new("C:/app.exe")), None);
    }
}
