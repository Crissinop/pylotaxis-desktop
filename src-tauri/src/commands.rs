//! Comandi esposti al webview. Ogni comando va elencato anche in app_commands.rs e concesso
//! in capabilities/default.json, altrimenti viene rifiutato (A.7.10). (v0.1.0)
//!
//! I comandi sono sottili: leggono l'input, delegano al dominio, traducono l'errore (A.7.3).
//! Sono tutti `async`: così non girano sul thread dell'interfaccia, e le operazioni
//! lente (finestra di scelta, impronta di un eseguibile) passano da `spawn_blocking`.

use domain::launch::{LaunchPlan, plan_launch};
use domain::registry::{App, AppInput, Category, Registry, Target, inspect_executable};
use domain::secrets::SecretInfo;
use serde::{Deserialize, Serialize};
use tauri::async_runtime::spawn_blocking;
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

use crate::errors::CommandError;
use crate::secrets::SecretDto;
use crate::state::{AppState, PendingPick, TargetInput, parse_id, resolve_target};

/// Lunghezza massima dei testi dell'interfaccia passati alla finestra di sistema.
const DIALOG_TEXT_MAX_CHARS: usize = 120;

/// Dati dell'app mostrati dall'interfaccia. Nessun dato sensibile (A.7.10). (v0.1.0)
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDto {
    id: String,
    name: String,
}

/// App come la vede l'interfaccia. `target` serve solo a mostrarla: per avviarla il
/// frontend invia l'id, mai questo testo.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDto {
    id: String,
    name: String,
    kind: &'static str,
    target: String,
    category_id: Option<String>,
    tags: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryDto {
    categories: Vec<CategoryDto>,
    apps: Vec<AppDto>,
    /// Segreti di tutte le app, con il loro `appId`: solo i dati descrittivi (v0.4.0).
    secrets: Vec<SecretDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickDto {
    token: String,
    path: String,
    suggested_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppInputDto {
    name: String,
    target: TargetInput,
    category_id: Option<String>,
    tags: Vec<String>,
}

impl From<Category> for CategoryDto {
    fn from(category: Category) -> Self {
        Self {
            id: category.id.to_string(),
            name: category.name,
        }
    }
}

impl From<App> for AppDto {
    fn from(app: App) -> Self {
        let (kind, target) = match app.target {
            Target::Executable { path, .. } => ("executable", path.display().to_string()),
            Target::Web { url } => ("web", url),
            Target::Protocol { uri } => ("protocol", uri),
        };
        Self {
            id: app.id.to_string(),
            name: app.name,
            kind,
            target,
            category_id: app.category_id.map(|id| id.to_string()),
            tags: app.tags,
        }
    }
}

impl RegistryDto {
    fn new(registry: Registry, secrets: Vec<SecretInfo>) -> Self {
        Self {
            categories: registry.categories.into_iter().map(Into::into).collect(),
            apps: registry.apps.into_iter().map(Into::into).collect(),
            secrets: secrets.into_iter().map(Into::into).collect(),
        }
    }
}

/// Accorcia i testi dell'interfaccia passati alle finestre di sistema (scelta dei file,
/// prompt di Windows Hello).
pub(crate) fn clip(text: &str) -> String {
    text.chars().take(DIALOG_TEXT_MAX_CHARS).collect()
}

fn to_input(
    state: &AppState,
    dto: AppInputDto,
    existing: Option<&App>,
) -> Result<AppInput, CommandError> {
    let target = resolve_target(dto.target, &mut *state.pick()?, existing)?;
    let category_id = dto.category_id.as_deref().map(parse_id).transpose()?;
    Ok(AppInput {
        name: dto.name,
        target,
        category_id,
        tags: dto.tags,
    })
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: domain::APP_NAME,
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[tauri::command]
pub async fn registry_list(state: State<'_, AppState>) -> Result<RegistryDto, CommandError> {
    state.with_db(|db| Ok(RegistryDto::new(db.registry()?, db.secrets()?)))
}

#[tauri::command]
pub async fn category_create(
    state: State<'_, AppState>,
    name: String,
) -> Result<CategoryDto, CommandError> {
    state.with_db(|db| Ok(db.create_category(&name)?.into()))
}

#[tauri::command]
pub async fn category_rename(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<CategoryDto, CommandError> {
    let id = parse_id(&id)?;
    state.with_db(|db| Ok(db.rename_category(id, &name)?.into()))
}

#[tauri::command]
pub async fn category_delete(state: State<'_, AppState>, id: String) -> Result<(), CommandError> {
    let id = parse_id(&id)?;
    state.with_db(|db| Ok(db.delete_category(id)?))
}

/// Apre la finestra di sistema per scegliere un eseguibile. È l'unico modo in cui un
/// percorso entra nel registro: richiede un gesto dell'utente e non si può simulare
/// dal webview (A.7.10). I testi arrivano dal frontend, già tradotti.
#[tauri::command]
pub async fn executable_pick<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    title: String,
    filter_label: String,
) -> Result<Option<PickDto>, CommandError> {
    let dialog = app.dialog().clone();
    let (title, filter_label) = (clip(&title), clip(&filter_label));
    let picked = spawn_blocking(move || {
        dialog
            .file()
            .set_title(title)
            .add_filter(filter_label, &["exe"])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| CommandError::DIALOG_FAILED)?;

    let Some(file) = picked else {
        return Ok(None);
    };
    let path = file
        .into_path()
        .map_err(|_| domain::Error::ExecutableInvalid)?;
    let info = spawn_blocking(move || inspect_executable(&path))
        .await
        .map_err(|_| CommandError::DIALOG_FAILED)??;

    let dto = PickDto {
        token: Uuid::now_v7().to_string(),
        path: info.path.display().to_string(),
        suggested_name: info.suggested_name.clone(),
    };
    *state.pick()? = Some(PendingPick {
        token: dto.token.clone(),
        info,
    });
    Ok(Some(dto))
}

#[tauri::command]
pub async fn app_create(
    state: State<'_, AppState>,
    input: AppInputDto,
) -> Result<AppDto, CommandError> {
    let input = to_input(&state, input, None)?;
    state.with_db(|db| Ok(db.create_app(&input)?.into()))
}

#[tauri::command]
pub async fn app_update(
    state: State<'_, AppState>,
    id: String,
    input: AppInputDto,
) -> Result<AppDto, CommandError> {
    let id = parse_id(&id)?;
    let existing = state.with_db(|db| Ok(db.app(id)?))?;
    let input = to_input(&state, input, Some(&existing))?;
    state.with_db(|db| Ok(db.update_app(id, &input)?.into()))
}

#[tauri::command]
pub async fn app_delete(state: State<'_, AppState>, id: String) -> Result<(), CommandError> {
    let id = parse_id(&id)?;
    // Le righe dei segreti le elimina il database (CASCADE); le credenziali si eliminano dopo,
    // e quelle che non si eliminano le ritrova la pulizia all'avvio (v0.4.0).
    let secrets = state.with_db(|db| {
        let secrets = db.secret_ids_of_app(id)?;
        db.delete_app(id)?;
        Ok(secrets)
    })?;
    for secret in secrets {
        let _ = state.vault.remove(secret);
    }
    Ok(())
}

/// Avvia un'app registrata, per id. Un eseguibile cambiato dall'ultima conferma risponde
/// `HASH_MISMATCH`; l'interfaccia chiede conferma e riprova con `accept_changed`.
#[tauri::command]
pub async fn app_launch<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: String,
    accept_changed: bool,
) -> Result<(), CommandError> {
    let id = parse_id(&id)?;
    let entry = state.with_db(|db| Ok(db.app(id)?))?;
    let planned = spawn_blocking(move || plan_launch(&entry, accept_changed))
        .await
        .map_err(|_| CommandError::LAUNCH_FAILED)??;

    if let Some(sha256) = &planned.new_sha256 {
        state.with_db(|db| Ok(db.repin_executable(id, sha256)?))?;
    }
    match planned.plan {
        LaunchPlan::Spawn {
            program,
            working_dir,
        } => {
            let mut command = std::process::Command::new(program);
            if let Some(dir) = working_dir {
                command.current_dir(dir);
            }
            command.spawn().map_err(|_| CommandError::LAUNCH_FAILED)?;
        }
        LaunchPlan::Open { uri } => app
            .opener()
            .open_url(uri, None::<&str>)
            .map_err(|_| CommandError::LAUNCH_FAILED)?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_reports_the_domain_name_and_the_crate_version() {
        let info = app_info();
        assert_eq!(info.name, domain::APP_NAME);
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }

    fn keys(value: &serde_json::Value) -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort_unstable();
        keys
    }

    /// Il frontend legge esattamente queste chiavi (src/lib/ipc.ts): se cambiano,
    /// il test fallisce prima che l'interfaccia si rompa in silenzio.
    #[test]
    fn dtos_serialize_with_the_keys_the_frontend_expects() {
        assert_eq!(
            keys(&serde_json::to_value(app_info()).unwrap()),
            ["name", "version"]
        );
        let app = AppDto {
            id: String::new(),
            name: String::new(),
            kind: "web",
            target: String::new(),
            category_id: None,
            tags: Vec::new(),
        };
        assert_eq!(
            keys(&serde_json::to_value(app).unwrap()),
            ["categoryId", "id", "kind", "name", "tags", "target"]
        );
        let pick = PickDto {
            token: String::new(),
            path: String::new(),
            suggested_name: String::new(),
        };
        assert_eq!(
            keys(&serde_json::to_value(pick).unwrap()),
            ["path", "suggestedName", "token"]
        );
        assert_eq!(
            keys(&serde_json::to_value(CommandError::LAUNCH_FAILED).unwrap()),
            ["code"]
        );
    }

    #[test]
    fn app_input_rejects_unknown_fields() {
        let valid = r#"{"name":"A","target":{"kind":"web","url":"https://a.it"},"categoryId":null,"tags":[]}"#;
        assert!(serde_json::from_str::<AppInputDto>(valid).is_ok());
        let extra = r#"{"name":"A","target":{"kind":"web","url":"https://a.it"},"categoryId":null,"tags":[],"sha256":"x"}"#;
        assert!(serde_json::from_str::<AppInputDto>(extra).is_err());
    }
}
